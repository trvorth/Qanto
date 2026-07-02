# QANTO Waitlist Backend Setup

This runbook covers the Cloudflare Pages + D1 waitlist stack that powers the `Coming Soon` landing page.

## 1. Install dependencies

```bash
cd Frontend/website
npm install
```

## 2. Create the D1 database

```bash
cd Frontend/website
npx wrangler d1 create qanto-waitlist
```

Copy the returned `database_id` into `wrangler.toml` for both:

- `database_id`
- `preview_database_id`

## 3. Apply the schema locally

```bash
cd Frontend/website
cp .dev.vars.example .dev.vars
npx wrangler d1 execute qanto-waitlist --local --file ./schema.sql
```

## 4. Apply the schema remotely

```bash
cd Frontend/website
npx wrangler d1 execute qanto-waitlist --remote --file ./schema.sql
```

## 5. Run local Pages development

Build the static assets first, then launch Pages Functions and D1 together:

```bash
cd Frontend/website
npm run build
npm run dev:pages
```

Useful local endpoints:

- `POST /api/subscribe`
- `GET /api/health`
- `GET /api/subscribers` with `Authorization: Bearer <ADMIN_API_TOKEN>`

## 6. Deploy to production

```bash
cd Frontend/website
npm run build
CLOUDFLARE_PAGES_PROJECT=qanto-org CLOUDFLARE_PAGES_BRANCH=production npm run deploy:pages
```

## 7. Set production secrets

Use Wrangler interactive prompts for secrets:

```bash
cd Frontend/website
npx wrangler secret put WAITLIST_IP_HASH_SALT
npx wrangler secret put ADMIN_WEBHOOK_URL
npx wrangler secret put ADMIN_API_TOKEN
```

## 8. Tail logs after deployment

```bash
cd Frontend/website
npx wrangler tail qanto-org
```

## 9. Quick smoke tests

```bash
curl -X POST http://127.0.0.1:8788/api/subscribe \
  -H 'content-type: application/json' \
  -d '{"email":"builder@qanto.org","source":"coming-soon"}'

curl http://127.0.0.1:8788/api/health

curl http://127.0.0.1:8788/api/subscribers \
  -H 'Authorization: Bearer replace-with-long-admin-token'
```
