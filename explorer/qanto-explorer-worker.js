addEventListener('fetch', event => {
  event.respondWith(handleRequest(event.request));
});

async function handleRequest(request) {
  const url = new URL(request.url);
  const path = url.pathname;

  if (path === '/' || path === '/blocks') {
    return fetchBlocks();
  } else if (path.startsWith('/block/')) {
    const blockId = path.split('/')[2];
    return fetchBlock(blockId);
  } else if (path.startsWith('/tx/')) {
    const txId = path.split('/')[2];
    return fetchTransaction(txId);
  } else {
    return new Response('Not Found', { status: 404 });
  }
}

async function fetchBlocks() {
  const apiUrl = 'https://qanto-api.example.com/blocks';
  const response = await fetch(apiUrl);
  const data = await response.json();
  return new Response(JSON.stringify(data), { headers: { 'Content-Type': 'application/json' } });
}

async function fetchBlock(blockId) {
  const apiUrl = `https://qanto-api.example.com/block/${blockId}`;
  const response = await fetch(apiUrl);
  const data = await response.json();
  return new Response(JSON.stringify(data), { headers: { 'Content-Type': 'application/json' } });
}

async function fetchTransaction(txId) {
  const apiUrl = `https://qanto-api.example.com/tx/${txId}`;
  const response = await fetch(apiUrl);
  const data = await response.json();
  return new Response(JSON.stringify(data), { headers: { 'Content-Type': 'application/json' } });
}