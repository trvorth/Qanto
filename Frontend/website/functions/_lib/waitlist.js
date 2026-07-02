const JSON_HEADERS = {
  "content-type": "application/json; charset=utf-8",
  "cache-control": "no-store",
};

const EMAIL_PATTERN = /^[^\s@]+@[^\s@]+\.[^\s@]+$/;

export function jsonResponse(payload, status = 200, headers = {}) {
  return new Response(JSON.stringify(payload), {
    status,
    headers: {
      ...JSON_HEADERS,
      ...headers,
    },
  });
}

export function normalizeEmail(value) {
  if (typeof value !== "string") {
    return "";
  }

  return value.trim().toLowerCase();
}

export function validateSubscriberPayload(payload) {
  const email = normalizeEmail(payload?.email);
  const source = typeof payload?.source === "string" ? payload.source.trim() : "coming-soon";

  if (!email) {
    return {
      ok: false,
      message: "Email is required.",
    };
  }

  if (email.length > 320 || !EMAIL_PATTERN.test(email)) {
    return {
      ok: false,
      message: "Please provide a valid email address.",
    };
  }

  return {
    ok: true,
    value: {
      email,
      source: source.slice(0, 64) || "coming-soon",
    },
  };
}

export async function readJsonBody(request) {
  try {
    return await request.json();
  } catch {
    return null;
  }
}

export function getRequestMetadata(request) {
  const headers = request.headers;

  return {
    ip: headers.get("CF-Connecting-IP"),
    country: headers.get("CF-IPCountry"),
    rayId: headers.get("CF-Ray"),
    referrer: headers.get("Referer"),
    origin: headers.get("Origin"),
    userAgent: headers.get("User-Agent"),
  };
}

export async function hashIdentifier(value, salt) {
  if (!value || !salt) {
    return null;
  }

  const content = new TextEncoder().encode(`${salt}:${value}`);
  const digest = await crypto.subtle.digest("SHA-256", content);
  const bytes = Array.from(new Uint8Array(digest));
  return bytes.map((byte) => byte.toString(16).padStart(2, "0")).join("");
}

export async function buildSubscriberRecord(request, env, payload) {
  const requestMetadata = getRequestMetadata(request);
  const timestamp = new Date().toISOString();

  return {
    email: payload.email,
    source: payload.source || env.QANTO_WAITLIST_SOURCE || "coming-soon",
    subscribedAt: timestamp,
    referrer: requestMetadata.referrer?.slice(0, 512) ?? null,
    userAgent: requestMetadata.userAgent?.slice(0, 512) ?? null,
    ipCountry: requestMetadata.country?.slice(0, 8) ?? null,
    ipHash: await hashIdentifier(requestMetadata.ip, env.WAITLIST_IP_HASH_SALT || ""),
    metadataJson: JSON.stringify({
      origin: requestMetadata.origin ?? null,
      rayId: requestMetadata.rayId ?? null,
      siteMode: env.QANTO_SITE_MODE ?? "coming-soon",
    }),
  };
}

export function isAuthorizedAdmin(request, env) {
  const token = env.ADMIN_API_TOKEN;
  const authHeader = request.headers.get("Authorization") || "";

  if (!token || !authHeader.startsWith("Bearer ")) {
    return false;
  }

  return authHeader.slice("Bearer ".length) === token;
}

export async function sendAdminNotification(record, env) {
  const webhookUrl = env.ADMIN_WEBHOOK_URL;
  if (!webhookUrl) {
    return { delivered: false, reason: "webhook-not-configured" };
  }

  const format = (env.ADMIN_WEBHOOK_FORMAT || "auto").toLowerCase();
  const target = webhookUrl.toLowerCase();

  let payload;
  if (format === "discord" || (format === "auto" && target.includes("discord.com/api/webhooks"))) {
    payload = {
      content: `New QANTO waitlist subscriber: ${record.email}`,
      embeds: [
        {
          title: "QANTO Waitlist Event",
          color: 53759,
          fields: [
            { name: "Email", value: record.email, inline: false },
            { name: "Source", value: record.source || "coming-soon", inline: true },
            { name: "Country", value: record.ipCountry || "unknown", inline: true },
          ],
          timestamp: record.subscribedAt,
        },
      ],
    };
  } else if (format === "telegram" || (format === "auto" && target.includes("/sendmessage"))) {
    payload = {
      text: [
        "QANTO waitlist subscriber",
        `Email: ${record.email}`,
        `Source: ${record.source || "coming-soon"}`,
        `Country: ${record.ipCountry || "unknown"}`,
        `At: ${record.subscribedAt}`,
      ].join("\n"),
    };
  } else {
    payload = {
      event: "waitlist.subscribed",
      subscriber: record,
    };
  }

  const response = await fetch(webhookUrl, {
    method: "POST",
    headers: {
      "content-type": "application/json",
    },
    body: JSON.stringify(payload),
  });

  return {
    delivered: response.ok,
    status: response.status,
  };
}
