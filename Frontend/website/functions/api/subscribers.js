import { isAuthorizedAdmin, jsonResponse } from "../_lib/waitlist.js";

export async function onRequestGet(context) {
  const { request, env } = context;

  if (!env.DB) {
    return jsonResponse(
      {
        ok: false,
        message: "Waitlist database binding is missing.",
      },
      503,
    );
  }

  if (!isAuthorizedAdmin(request, env)) {
    return jsonResponse(
      {
        ok: false,
        message: "Unauthorized.",
      },
      401,
    );
  }

  const url = new URL(request.url);
  const limitParam = Number.parseInt(url.searchParams.get("limit") || "25", 10);
  const limit = Number.isFinite(limitParam) ? Math.min(Math.max(limitParam, 1), 100) : 25;

  try {
    const [{ total = 0 } = {}, rowsResult] = await Promise.all([
      env.DB.prepare("SELECT COUNT(*) AS total FROM waitlist_subscribers").first(),
      env.DB.prepare(
        `SELECT email, subscribed_at, source, ip_country, referrer
         FROM waitlist_subscribers
         ORDER BY subscribed_at DESC
         LIMIT ?1`,
      )
        .bind(limit)
        .all(),
    ]);

    return jsonResponse({
      ok: true,
      total,
      limit,
      subscribers: rowsResult.results || [],
    });
  } catch (error) {
    console.error(
      JSON.stringify({
        event: "waitlist.admin_query_failed",
        message: error instanceof Error ? error.message : "unknown query error",
      }),
    );

    return jsonResponse(
      {
        ok: false,
        message: "Unable to query waitlist subscribers.",
      },
      500,
    );
  }
}
