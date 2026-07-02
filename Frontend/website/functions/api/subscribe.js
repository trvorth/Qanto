import {
  buildSubscriberRecord,
  jsonResponse,
  readJsonBody,
  validateSubscriberPayload,
} from "../_lib/waitlist.js";

async function postAdminWebhook(env, payload) {
  const webhookUrl = env.ADMIN_WEBHOOK_URL;
  if (!webhookUrl) {
    return;
  }

  const controller = new AbortController();
  const timeoutId = setTimeout(() => controller.abort(), 2500);

  try {
    await fetch(webhookUrl, {
      method: "POST",
      headers: {
        "content-type": "application/json",
      },
      body: JSON.stringify(payload),
      signal: controller.signal,
    });
  } catch {
  } finally {
    clearTimeout(timeoutId);
  }
}

export async function onRequestPost(context) {
  const { request, env } = context;

  if (!env.DB) {
    console.error(JSON.stringify({ event: "waitlist.db_missing" }));
    return jsonResponse(
      {
        ok: false,
        message: "Waitlist service is not configured yet.",
      },
      503,
    );
  }

  const payload = await readJsonBody(request);
  if (!payload) {
    return jsonResponse(
      {
        ok: false,
        message: "Request body must be valid JSON.",
      },
      400,
    );
  }

  const validation = validateSubscriberPayload(payload);
  if (!validation.ok) {
    return jsonResponse(
      {
        ok: false,
        message: validation.message,
      },
      400,
    );
  }

  const record = await buildSubscriberRecord(request, env, validation.value);

  try {
    const existingSubscriber = await env.DB.prepare(
      `SELECT id, email, subscribed_at AS subscribedAt
       FROM waitlist_subscribers
       WHERE email = ?1
       LIMIT 1`,
    )
      .bind(record.email)
      .first();

    if (existingSubscriber) {
      await env.DB.prepare(
        `UPDATE waitlist_subscribers
         SET source = ?2,
             referrer = ?3,
             user_agent = ?4,
             ip_country = ?5,
             ip_hash = COALESCE(ip_hash, ?6),
             metadata_json = ?7,
             updated_at = ?8
         WHERE email = ?1`,
      )
        .bind(
          record.email,
          record.source,
          record.referrer,
          record.userAgent,
          record.ipCountry,
          record.ipHash,
          record.metadataJson,
          record.subscribedAt,
        )
        .run();

      console.log(
        JSON.stringify({
          event: "waitlist.duplicate",
          email: record.email,
          source: record.source,
        }),
      );

      return jsonResponse({
        ok: true,
        status: "duplicate",
        message: "This email is already on the QANTO waitlist.",
      });
    }

    await env.DB.prepare(
      `INSERT INTO waitlist_subscribers (
        email,
        subscribed_at,
        source,
        referrer,
        user_agent,
        ip_country,
        ip_hash,
        metadata_json,
        created_at,
        updated_at
      ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?9)`,
    )
      .bind(
        record.email,
        record.subscribedAt,
        record.source,
        record.referrer,
        record.userAgent,
        record.ipCountry,
        record.ipHash,
        record.metadataJson,
        record.subscribedAt,
      )
      .run();

    context.waitUntil(
      postAdminWebhook(env, {
        email: record.email,
        timestamp_utc: record.subscribedAt,
      }),
    );

    console.log(
      JSON.stringify({
        event: "waitlist.subscribed",
        email: record.email,
        source: record.source,
      }),
    );

    return jsonResponse({
      ok: true,
      status: "created",
      message: "You are on the QANTO launch waitlist. We will notify you when rollout milestones open.",
    });
  } catch (error) {
    console.error(
      JSON.stringify({
        event: "waitlist.insert_failed",
        message: error instanceof Error ? error.message : "unknown database error",
      }),
    );

    return jsonResponse(
      {
        ok: false,
        message: "Unable to store your waitlist request right now.",
      },
      500,
    );
  }
}

export function onRequestOptions() {
  return new Response(null, {
    status: 204,
    headers: {
      allow: "POST, OPTIONS",
    },
  });
}
