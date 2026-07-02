import test from "node:test";
import assert from "node:assert/strict";

import {
  isAuthorizedAdmin,
  normalizeEmail,
  validateSubscriberPayload,
} from "../functions/_lib/waitlist.js";

test("normalizeEmail trims and lowercases", () => {
  assert.equal(normalizeEmail("  User@Example.COM  "), "user@example.com");
});

test("validateSubscriberPayload rejects invalid input", () => {
  const result = validateSubscriberPayload({ email: "invalid-email" });
  assert.equal(result.ok, false);
  assert.match(result.message, /valid email/i);
});

test("validateSubscriberPayload accepts a valid email", () => {
  const result = validateSubscriberPayload({ email: "builder@qanto.org", source: "coming-soon" });
  assert.equal(result.ok, true);
  assert.deepEqual(result.value, {
    email: "builder@qanto.org",
    source: "coming-soon",
  });
});

test("isAuthorizedAdmin validates bearer token", () => {
  const request = new Request("https://qanto.org/api/subscribers", {
    headers: {
      Authorization: "Bearer super-secret-token",
    },
  });

  assert.equal(
    isAuthorizedAdmin(request, { ADMIN_API_TOKEN: "super-secret-token" }),
    true,
  );
  assert.equal(
    isAuthorizedAdmin(request, { ADMIN_API_TOKEN: "different-token" }),
    false,
  );
});
