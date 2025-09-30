// src/worker.js - Enhanced Security & Performance Implementation
import { getAssetFromKV } from '@cloudflare/kv-asset-handler';

// Security configuration constants
const SECURITY_HEADERS = {
  'Strict-Transport-Security': 'max-age=63072000; includeSubDomains; preload',
  'X-Frame-Options': 'DENY',
  'X-Content-Type-Options': 'nosniff',
  'X-XSS-Protection': '1; mode=block',
  'Referrer-Policy': 'strict-origin-when-cross-origin',
  'Permissions-Policy': 'geolocation=(), microphone=(), camera=(), payment=(), usb=(), magnetometer=(), gyroscope=(), accelerometer=()',
  'Content-Security-Policy': "default-src 'self'; script-src 'self' 'sha256-' https://www.googletagmanager.com https://www.google-analytics.com https://cdn.jsdelivr.net; style-src 'self' 'unsafe-inline' https://fonts.googleapis.com; font-src 'self' https://fonts.gstatic.com; img-src 'self' data: https:; connect-src 'self' https: wss: https://api.qanto.org; frame-ancestors 'none'; base-uri 'self'; form-action 'self'; upgrade-insecure-requests;",
  'Cross-Origin-Embedder-Policy': 'require-corp',
  'Cross-Origin-Opener-Policy': 'same-origin',
  'Cross-Origin-Resource-Policy': 'same-origin'
};

// Cache configuration for different asset types
const CACHE_CONFIG = {
  html: { browserTTL: 0, edgeTTL: 300 }, // 5 minutes edge cache, no browser cache
  css: { browserTTL: 31536000, edgeTTL: 31536000 }, // 1 year
  js: { browserTTL: 31536000, edgeTTL: 31536000 }, // 1 year
  images: { browserTTL: 86400, edgeTTL: 86400 }, // 1 day
  fonts: { browserTTL: 31536000, edgeTTL: 31536000 }, // 1 year
  default: { browserTTL: 3600, edgeTTL: 3600 } // 1 hour
};

addEventListener('fetch', event => {
  event.respondWith(handleEvent(event));
});

/**
 * Enhanced event handler with security, caching, and error handling
 */
async function handleEvent(event) {
  const request = event.request;
  const url = new URL(request.url);
  
  // Security: Block suspicious requests
  if (isBlockedRequest(request)) {
    return new Response('Forbidden', { 
      status: 403,
      headers: SECURITY_HEADERS
    });
  }

  try {
    const response = await getAssetFromKV(event, {
      mapRequestToAsset: request => mapToAsset(request),
      cacheControl: getCacheControl(url.pathname)
    });

    return enhanceResponse(response, url.pathname);
  } catch (err) {
    return handleError(event, err);
  }
}

/**
 * Enhanced error handling with custom error pages
 */
async function handleError(event, originalError) {
  const url = new URL(event.request.url);
  
  try {
    // Try to serve custom 404 page
    const notFoundReq = new Request(url.origin + '/404.html', event.request);
    const notFoundResp = await getAssetFromKV({ request: notFoundReq }, {
      mapRequestToAsset: () => notFoundReq
    });
    
    return enhanceResponse(notFoundResp, '/404.html', 404);
  } catch (e) {
    // Fallback to basic 404 with security headers
    return new Response(generateErrorPage(404, 'Page Not Found'), { 
      status: 404,
      headers: {
        ...SECURITY_HEADERS,
        'Content-Type': 'text/html; charset=utf-8'
      }
    });
  }
}

/**
 * Enhanced request mapping with SPA routing support
 */
function mapToAsset(request) {
  const url = new URL(request.url);
  const hostname = url.hostname;
  const pathname = url.pathname;

  // Explorer subdomain routing
  if (hostname === 'explorer.qanto.org') {
    // Handle explorer routes
    if (pathname === '/' || pathname === '/explorer') {
      return new Request(url.origin + '/explorer/qanto-explorer.html', request);
    }
    // SPA routing for explorer - serve main explorer page for all routes
    if (pathname.startsWith('/explorer/') || pathname.startsWith('/block/') || pathname.startsWith('/tx/')) {
      return new Request(url.origin + '/explorer/qanto-explorer.html', request);
    }
  }

  // Main site routing
  if (pathname === '/' || pathname === '/index.html') {
    return new Request(url.origin + '/index.html', request);
  }

  // SPA routing for main site
  const spaRoutes = ['/about', '/technology', '/roadmap', '/team', '/whitepaper', '/docs'];
  if (spaRoutes.some(route => pathname.startsWith(route))) {
    return new Request(url.origin + '/index.html', request);
  }

  // Default asset mapping
  return request;
}

/**
 * Security check for blocking malicious requests
 */
function isBlockedRequest(request) {
  const url = new URL(request.url);
  const userAgent = request.headers.get('User-Agent') || '';
  
  // Block common attack patterns
  const suspiciousPatterns = [
    /\.\./, // Path traversal
    /<script/i, // XSS attempts
    /union.*select/i, // SQL injection
    /javascript:/i, // JavaScript protocol
    /data:.*base64/i // Data URI attacks
  ];
  
  // Check URL path and query
  const fullUrl = url.href;
  if (suspiciousPatterns.some(pattern => pattern.test(fullUrl))) {
    return true;
  }
  
  // Block suspicious user agents
  const blockedAgents = ['sqlmap', 'nikto', 'nmap', 'masscan'];
  if (blockedAgents.some(agent => userAgent.toLowerCase().includes(agent))) {
    return true;
  }
  
  return false;
}

/**
 * Get cache control settings based on file type
 */
function getCacheControl(pathname) {
  const ext = pathname.split('.').pop()?.toLowerCase();
  
  switch (ext) {
    case 'html':
      return CACHE_CONFIG.html;
    case 'css':
      return CACHE_CONFIG.css;
    case 'js':
    case 'mjs':
      return CACHE_CONFIG.js;
    case 'png':
    case 'jpg':
    case 'jpeg':
    case 'gif':
    case 'webp':
    case 'svg':
      return CACHE_CONFIG.images;
    case 'woff':
    case 'woff2':
    case 'ttf':
    case 'eot':
      return CACHE_CONFIG.fonts;
    default:
      return CACHE_CONFIG.default;
  }
}

/**
 * Enhance response with security headers and optimizations
 */
function enhanceResponse(response, pathname, statusOverride = null) {
  const newResponse = new Response(response.body, {
    status: statusOverride || response.status,
    statusText: response.statusText,
    headers: response.headers
  });

  // Add security headers
  Object.entries(SECURITY_HEADERS).forEach(([key, value]) => {
    newResponse.headers.set(key, value);
  });

  // Set proper cache headers
  const cacheConfig = getCacheControl(pathname);
  if (cacheConfig.browserTTL > 0) {
    newResponse.headers.set('Cache-Control', `public, max-age=${cacheConfig.browserTTL}`);
  } else {
    newResponse.headers.set('Cache-Control', 'no-cache, no-store, must-revalidate');
  }

  // Normalize HTML content type
  if (pathname.endsWith('.html') || pathname === '/') {
    newResponse.headers.set('Content-Type', 'text/html; charset=utf-8');
  }

  return newResponse;
}

/**
 * Generate error page HTML
 */
function generateErrorPage(status, message) {
  return `<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Error ${status} - Qanto</title>
    <style>
        body { 
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
            background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
            color: white;
            margin: 0;
            padding: 0;
            min-height: 100vh;
            display: flex;
            align-items: center;
            justify-content: center;
        }
        .error-container {
            text-align: center;
            padding: 2rem;
            background: rgba(255, 255, 255, 0.1);
            border-radius: 10px;
            backdrop-filter: blur(10px);
        }
        h1 { font-size: 4rem; margin: 0; }
        p { font-size: 1.2rem; margin: 1rem 0; }
        a { color: #fff; text-decoration: underline; }
    </style>
</head>
<body>
    <div class="error-container">
        <h1>${status}</h1>
        <p>${message}</p>
        <p><a href="/">Return to Qanto Home</a></p>
    </div>
</body>
</html>`;
}
