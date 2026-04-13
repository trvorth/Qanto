const CACHE_NAME = 'qanto-os-v2';
const urlsToCache = [
  '/',
  '/index.html',
  '/assets/css/main.css',
  '/assets/css/components.css',
  '/assets/css/animations.css',
  '/assets/js/os.js',
  '/splash-1024.svg'
];

self.addEventListener('install', event => {
  event.waitUntil(
    caches.open(CACHE_NAME)
      .then(cache => {
        console.log('Opened cache');
        return cache.addAll(urlsToCache);
      })
  );
});

self.addEventListener('fetch', event => {
  event.respondWith(
    caches.match(event.request)
      .then(response => {
        if (response) {
          return response;
        }
        return fetch(event.request).then(fetchRes => {
          if (fetchRes.status === 404 && event.request.mode === 'navigate') {
            return caches.match('/index.html');
          }
          return fetchRes;
        }).catch(() => {
            if (event.request.mode === 'navigate' || event.request.headers.get('accept').includes('text/html')) {
                return caches.match('/index.html');
            }
        });
      })
  );
});
