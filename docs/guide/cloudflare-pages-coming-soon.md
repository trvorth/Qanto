# QANTO Cloudflare Pages Coming Soon Runbook

This runbook defines the recommended Cloudflare setup for temporarily serving the QANTO production domain with a "Coming Soon" landing page while core application development continues.

## Target Topology

- Pages project: `qanto-org`
- Production branch: `production`
- Source directory: `Frontend/website`
- Production domain: `qanto.org`
- Secondary domain: `www.qanto.org`

## Repository Commands

From the repository root:

```bash
cd Frontend/website
npm install
npm run build
```

Manual deployment:

```bash
cd Frontend/website
CLOUDFLARE_PAGES_PROJECT=qanto-org CLOUDFLARE_PAGES_BRANCH=production npm run deploy:pages
```

Workspace shortcut:

```bash
cd Frontend
npm run deploy:frontend
```

## Cloudflare Account Setup

1. Add `qanto.org` to Cloudflare and switch the registrar nameservers to the two nameservers assigned by Cloudflare.
2. Wait for the zone status to become `Active`.
3. Enable DNSSEC after the zone is active and publish the DS record at the registrar if it is not automated.

## DNS Recommendations

Use Cloudflare-managed DNS for the public site:

- `qanto.org` -> Cloudflare Pages custom domain binding
- `www.qanto.org` -> CNAME to `qanto.org`

Do not point the root domain directly to an origin server while the Coming Soon rollout is active.

## Cloudflare Pages Configuration

Create a Pages project with these values:

- Project name: `qanto-org`
- Production branch: `main` if deploying via Git integration, or `production` if deploying via Wrangler branch naming
- Framework preset: `Vite`
- Build command: `npm run build`
- Build output directory: `dist`
- Root directory: `Frontend/website`

If you use GitHub Actions instead of Git integration, configure these repository secrets:

- `CLOUDFLARE_API_TOKEN`
- `CLOUDFLARE_ACCOUNT_ID`

The included workflow file is:

- `.github/workflows/cloudflare-pages-coming-soon.yml`

## Required Cloudflare Security Settings

Recommended baseline:

- SSL/TLS mode: `Full (strict)`
- Always Use HTTPS: `On`
- Automatic HTTPS Rewrites: `On`
- Minimum TLS version: `1.2`
- HSTS: `On`
- Brotli: `On`
- HTTP/3: `On`
- Bot Fight Mode: `On`
- WAF Managed Rules: `On`
- Security Level: `Medium`

## Caching Recommendations

The landing page build emits:

- `_redirects` with `/* /index.html 200`
- `_headers` with security headers and cache policy

Recommended cache behavior:

- `index.html`: revalidate on every request
- versioned assets under `/assets/`: immutable cache

This ensures all routes render the Coming Soon page while preserving fast CDN delivery.

## Domain Binding

In Cloudflare Pages:

1. Open the `qanto-org` project.
2. Add custom domain `qanto.org`.
3. Add custom domain `www.qanto.org`.
4. Allow Cloudflare to provision certificates automatically.
5. Confirm both domains show `Active`.

## Online Verification Checklist

Run these checks after deployment:

```bash
curl -I https://qanto.org
curl -I https://www.qanto.org
curl -I https://qanto.org/bridge
curl -I https://qanto.org/explorer
```

Expected results:

- `200 OK` for the root page
- valid HTTPS certificate
- `bridge` and `explorer` resolve to the same Coming Soon experience

## Later Cutover To Full Product

When the full frontend is production-ready:

1. Replace the temporary landing page build with the full frontend build.
2. Remove or narrow the catch-all `/* /index.html 200` redirect as required.
3. Keep the same Pages project and custom domains to avoid DNS churn.
4. Re-run smoke tests on root, app routes, API integrations, and wallet flows.
