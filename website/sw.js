// Qanto Website - Service Worker
// Provides offline functionality, caching, and performance optimization

const CACHE_NAME = 'qanto-website-v1.0.0';
const STATIC_CACHE = 'qanto-static-v1.0.0';
const DYNAMIC_CACHE = 'qanto-dynamic-v1.0.0';
const IMAGE_CACHE = 'qanto-images-v1.0.0';

// Assets to cache immediately
const STATIC_ASSETS = [
    '/',
    '/index.html',
    '/assets/css/main.css',
    '/assets/css/components.css',
    '/assets/css/animations.css',
    '/assets/js/main.js',
    '/assets/js/animations.js',
    '/assets/js/visualizations.js',
    '/assets/images/favicon.ico',
    '/assets/images/logo.svg',
    '/manifest.json',
    // Google Fonts
    'https://fonts.googleapis.com/css2?family=Inter:wght@300;400;500;600;700;800&display=swap',
    'https://fonts.gstatic.com/s/inter/v12/UcCO3FwrK3iLTeHuS_fvQtMwCp50KnMw2boKoduKmMEVuLyfAZ9hiJ-Ek-_EeA.woff2'
];

// Network-first resources (always try network first)
const NETWORK_FIRST = [
    '/api/',
    'https://api.qanto.network/'
];

// Cache-first resources (images, fonts, etc.)
const CACHE_FIRST = [
    '/assets/images/',
    '/assets/fonts/',
    'https://fonts.googleapis.com/',
    'https://fonts.gstatic.com/'
];

// Maximum cache sizes
const MAX_CACHE_SIZE = {
    [STATIC_CACHE]: 50,
    [DYNAMIC_CACHE]: 100,
    [IMAGE_CACHE]: 200
};

// Cache expiration times (in milliseconds)
const CACHE_EXPIRATION = {
    [STATIC_CACHE]: 7 * 24 * 60 * 60 * 1000, // 7 days
    [DYNAMIC_CACHE]: 24 * 60 * 60 * 1000,    // 1 day
    [IMAGE_CACHE]: 30 * 24 * 60 * 60 * 1000  // 30 days
};

// Install event - cache static assets
self.addEventListener('install', event => {
    console.log('[SW] Installing service worker...');
    
    event.waitUntil(
        Promise.all([
            // Cache static assets
            caches.open(STATIC_CACHE).then(cache => {
                console.log('[SW] Caching static assets');
                return cache.addAll(STATIC_ASSETS.map(url => {
                    return new Request(url, { cache: 'reload' });
                }));
            }),
            // Initialize other caches
            caches.open(DYNAMIC_CACHE),
            caches.open(IMAGE_CACHE)
        ]).then(() => {
            console.log('[SW] Installation complete');
            // Force activation of new service worker
            return self.skipWaiting();
        }).catch(error => {
            console.error('[SW] Installation failed:', error);
        })
    );
});

// Activate event - clean up old caches
self.addEventListener('activate', event => {
    console.log('[SW] Activating service worker...');
    
    event.waitUntil(
        Promise.all([
            // Clean up old caches
            caches.keys().then(cacheNames => {
                return Promise.all(
                    cacheNames.map(cacheName => {
                        if (cacheName !== STATIC_CACHE && 
                            cacheName !== DYNAMIC_CACHE && 
                            cacheName !== IMAGE_CACHE) {
                            console.log('[SW] Deleting old cache:', cacheName);
                            return caches.delete(cacheName);
                        }
                    })
                );
            }),
            // Take control of all clients
            self.clients.claim()
        ]).then(() => {
            console.log('[SW] Activation complete');
        }).catch(error => {
            console.error('[SW] Activation failed:', error);
        })
    );
});

// Fetch event - handle requests with different strategies
self.addEventListener('fetch', event => {
    const { request } = event;
    const url = new URL(request.url);
    
    // Skip non-GET requests
    if (request.method !== 'GET') {
        return;
    }
    
    // Skip chrome-extension and other non-http(s) requests
    if (!url.protocol.startsWith('http')) {
        return;
    }
    
    // Handle different types of requests
    if (isNetworkFirst(request.url)) {
        event.respondWith(networkFirst(request));
    } else if (isCacheFirst(request.url)) {
        event.respondWith(cacheFirst(request));
    } else if (isImageRequest(request)) {
        event.respondWith(imageHandler(request));
    } else {
        event.respondWith(staleWhileRevalidate(request));
    }
});

// Network-first strategy (for API calls)
async function networkFirst(request) {
    try {
        const networkResponse = await fetch(request);
        
        if (networkResponse.ok) {
            // Cache successful responses
            const cache = await caches.open(DYNAMIC_CACHE);
            cache.put(request, networkResponse.clone());
            await limitCacheSize(DYNAMIC_CACHE, MAX_CACHE_SIZE[DYNAMIC_CACHE]);
        }
        
        return networkResponse;
    } catch (error) {
        console.log('[SW] Network failed, trying cache:', request.url);
        
        const cachedResponse = await caches.match(request);
        if (cachedResponse) {
            return cachedResponse;
        }
        
        // Return offline page for navigation requests
        if (request.mode === 'navigate') {
            return caches.match('/offline.html') || new Response(
                'Offline - Please check your internet connection',
                { status: 503, statusText: 'Service Unavailable' }
            );
        }
        
        throw error;
    }
}

// Cache-first strategy (for static assets)
async function cacheFirst(request) {
    const cachedResponse = await caches.match(request);
    
    if (cachedResponse && !isExpired(cachedResponse)) {
        return cachedResponse;
    }
    
    try {
        const networkResponse = await fetch(request);
        
        if (networkResponse.ok) {
            const cache = await caches.open(getCacheForRequest(request));
            cache.put(request, networkResponse.clone());
            await limitCacheSize(cache.name, MAX_CACHE_SIZE[cache.name]);
        }
        
        return networkResponse;
    } catch (error) {
        if (cachedResponse) {
            return cachedResponse;
        }
        throw error;
    }
}

// Stale-while-revalidate strategy (for general content)
async function staleWhileRevalidate(request) {
    const cache = await caches.open(DYNAMIC_CACHE);
    const cachedResponse = await cache.match(request);
    
    // Fetch from network in background
    const networkPromise = fetch(request).then(networkResponse => {
        if (networkResponse.ok) {
            cache.put(request, networkResponse.clone());
            limitCacheSize(DYNAMIC_CACHE, MAX_CACHE_SIZE[DYNAMIC_CACHE]);
        }
        return networkResponse;
    }).catch(() => null);
    
    // Return cached version immediately if available
    if (cachedResponse && !isExpired(cachedResponse)) {
        return cachedResponse;
    }
    
    // Wait for network if no cache or expired
    const networkResponse = await networkPromise;
    return networkResponse || cachedResponse || new Response(
        'Content not available offline',
        { status: 503, statusText: 'Service Unavailable' }
    );
}

// Image handler with WebP support
async function imageHandler(request) {
    const cache = await caches.open(IMAGE_CACHE);
    const cachedResponse = await cache.match(request);
    
    if (cachedResponse && !isExpired(cachedResponse)) {
        return cachedResponse;
    }
    
    try {
        const networkResponse = await fetch(request);
        
        if (networkResponse.ok) {
            cache.put(request, networkResponse.clone());
            await limitCacheSize(IMAGE_CACHE, MAX_CACHE_SIZE[IMAGE_CACHE]);
        }
        
        return networkResponse;
    } catch (error) {
        if (cachedResponse) {
            return cachedResponse;
        }
        
        // Return placeholder image for failed image requests
        return new Response(
            '<svg xmlns="http://www.w3.org/2000/svg" width="200" height="200" viewBox="0 0 200 200"><rect width="200" height="200" fill="#f3f4f6"/><text x="100" y="100" text-anchor="middle" dy=".3em" fill="#9ca3af">Image unavailable</text></svg>',
            {
                headers: {
                    'Content-Type': 'image/svg+xml',
                    'Cache-Control': 'no-cache'
                }
            }
        );
    }
}

// Helper functions
function isNetworkFirst(url) {
    return NETWORK_FIRST.some(pattern => url.includes(pattern));
}

function isCacheFirst(url) {
    return CACHE_FIRST.some(pattern => url.includes(pattern));
}

function isImageRequest(request) {
    return request.destination === 'image' || 
           /\.(jpg|jpeg|png|gif|webp|svg|ico)$/i.test(new URL(request.url).pathname);
}

function getCacheForRequest(request) {
    if (isImageRequest(request)) {
        return IMAGE_CACHE;
    }
    if (STATIC_ASSETS.includes(request.url) || request.url.includes('/assets/')) {
        return STATIC_CACHE;
    }
    return DYNAMIC_CACHE;
}

function isExpired(response) {
    const dateHeader = response.headers.get('date');
    if (!dateHeader) return false;
    
    const responseDate = new Date(dateHeader);
    const now = new Date();
    const age = now.getTime() - responseDate.getTime();
    
    // Determine expiration based on cache type
    const cacheType = response.headers.get('x-cache-type') || DYNAMIC_CACHE;
    const maxAge = CACHE_EXPIRATION[cacheType] || CACHE_EXPIRATION[DYNAMIC_CACHE];
    
    return age > maxAge;
}

async function limitCacheSize(cacheName, maxSize) {
    const cache = await caches.open(cacheName);
    const keys = await cache.keys();
    
    if (keys.length > maxSize) {
        // Remove oldest entries
        const keysToDelete = keys.slice(0, keys.length - maxSize);
        await Promise.all(keysToDelete.map(key => cache.delete(key)));
        console.log(`[SW] Cleaned ${keysToDelete.length} entries from ${cacheName}`);
    }
}

// Background sync for offline actions
self.addEventListener('sync', event => {
    console.log('[SW] Background sync:', event.tag);
    
    if (event.tag === 'background-sync') {
        event.waitUntil(doBackgroundSync());
    }
});

async function doBackgroundSync() {
    try {
        // Perform any background tasks here
        console.log('[SW] Performing background sync');
        
        // Example: sync offline form submissions
        const offlineActions = await getOfflineActions();
        for (const action of offlineActions) {
            await syncAction(action);
        }
        
        await clearOfflineActions();
    } catch (error) {
        console.error('[SW] Background sync failed:', error);
    }
}

async function getOfflineActions() {
    // Retrieve offline actions from IndexedDB or localStorage
    return [];
}

async function syncAction(action) {
    // Sync individual action with server
    console.log('[SW] Syncing action:', action);
}

async function clearOfflineActions() {
    // Clear synced actions from storage
    console.log('[SW] Cleared offline actions');
}

// Push notification handler
self.addEventListener('push', event => {
    console.log('[SW] Push notification received');
    
    const options = {
        body: 'New update available for Qanto',
        icon: '/assets/images/icon-192.png',
        badge: '/assets/images/badge-72.png',
        vibrate: [100, 50, 100],
        data: {
            dateOfArrival: Date.now(),
            primaryKey: 1
        },
        actions: [
            {
                action: 'explore',
                title: 'Explore',
                icon: '/assets/images/checkmark.png'
            },
            {
                action: 'close',
                title: 'Close',
                icon: '/assets/images/xmark.png'
            }
        ]
    };
    
    if (event.data) {
        const data = event.data.json();
        options.body = data.body || options.body;
        options.title = data.title || 'Qanto';
    }
    
    event.waitUntil(
        self.registration.showNotification('Qanto', options)
    );
});

// Notification click handler
self.addEventListener('notificationclick', event => {
    console.log('[SW] Notification clicked:', event.notification.tag);
    
    event.notification.close();
    
    if (event.action === 'explore') {
        event.waitUntil(
            clients.openWindow('/')
        );
    } else if (event.action === 'close') {
        // Just close the notification
        return;
    } else {
        // Default action - open the app
        event.waitUntil(
            clients.matchAll().then(clientList => {
                if (clientList.length > 0) {
                    return clientList[0].focus();
                }
                return clients.openWindow('/');
            })
        );
    }
});

// Message handler for communication with main thread
self.addEventListener('message', event => {
    console.log('[SW] Message received:', event.data);
    
    if (event.data && event.data.type) {
        switch (event.data.type) {
            case 'SKIP_WAITING':
                self.skipWaiting();
                break;
            case 'GET_VERSION':
                event.ports[0].postMessage({ version: CACHE_NAME });
                break;
            case 'CLEAR_CACHE':
                clearAllCaches().then(() => {
                    event.ports[0].postMessage({ success: true });
                });
                break;
            case 'CACHE_URLS':
                cacheUrls(event.data.urls).then(() => {
                    event.ports[0].postMessage({ success: true });
                });
                break;
            default:
                console.log('[SW] Unknown message type:', event.data.type);
        }
    }
});

async function clearAllCaches() {
    const cacheNames = await caches.keys();
    await Promise.all(cacheNames.map(name => caches.delete(name)));
    console.log('[SW] All caches cleared');
}

async function cacheUrls(urls) {
    const cache = await caches.open(DYNAMIC_CACHE);
    await cache.addAll(urls);
    console.log('[SW] URLs cached:', urls.length);
}

// Error handler
self.addEventListener('error', event => {
    console.error('[SW] Service worker error:', event.error);
});

self.addEventListener('unhandledrejection', event => {
    console.error('[SW] Unhandled promise rejection:', event.reason);
});

// Periodic background sync (if supported)
if ('periodicSync' in self.registration) {
    self.addEventListener('periodicsync', event => {
        if (event.tag === 'content-sync') {
            event.waitUntil(syncContent());
        }
    });
}

async function syncContent() {
    try {
        console.log('[SW] Syncing content in background');
        // Perform periodic content updates
    } catch (error) {
        console.error('[SW] Content sync failed:', error);
    }
}

console.log('[SW] Service worker loaded successfully');