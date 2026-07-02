import { jsonResponse } from "../_lib/waitlist.js";

export function onRequestGet(context) {
  return jsonResponse({
    ok: true,
    service: "qanto-waitlist",
    siteMode: context.env.QANTO_SITE_MODE || "coming-soon",
    timestamp: new Date().toISOString(),
  });
}
