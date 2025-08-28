addEventListener('fetch', event => {
  event.respondWith(new Response('Welcome to Qanto Blockchain - Decentralized and Untraceable', { status: 200 }));
});