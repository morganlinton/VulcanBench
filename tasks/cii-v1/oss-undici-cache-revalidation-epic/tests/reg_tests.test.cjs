// Hidden pass-to-pass guards: cache behaviors that must NOT change — fresh
// responses served from cache, the request URI's own invalidation, no-store,
// and the same-origin restriction on Location-based invalidation.
'use strict';
const { test } = require('node:test');
const assert = require('node:assert');
const http = require('node:http');
const { Client, interceptors } = require('./index.js');
const MemoryCacheStore = require('./lib/cache/memory-cache-store.js');

const listen = (srv) => new Promise((r) => srv.listen(0, '127.0.0.1', r));
const close = (srv) => new Promise((r) => srv.close(r));
const settle = () => new Promise((r) => setTimeout(r, 150));

function cachingClient(origin, opts) {
  const client = new Client(origin).compose(interceptors.cache(opts));
  const request = (o) => client.request({ origin, ...o });
  return { client, request };
}
async function text(resPromise) {
  const res = await resPromise;
  return await res.body.text();
}

test('vb fresh responses are served from cache', async () => {
  let hits = 0;
  const srv = http.createServer((_r, res) => {
    hits++;
    res.writeHead(200, { 'cache-control': 'public, max-age=100' });
    res.end('fresh');
  });
  await listen(srv);
  const { client, request } = cachingClient(`http://127.0.0.1:${srv.address().port}`);
  try {
    await text(request({ path: '/f', method: 'GET' }));
    await settle();
    assert.strictEqual(await text(request({ path: '/f', method: 'GET' })), 'fresh');
    assert.strictEqual(hits, 1);
  } finally {
    await client.close();
    await close(srv);
  }
});

test('vb an unsafe method still invalidates the request URI itself', async () => {
  let getHits = 0;
  const srv = http.createServer((req, res) => {
    if (req.method === 'POST') {
      res.writeHead(200);
      res.end('posted');
      return;
    }
    getHits++;
    res.writeHead(200, { 'cache-control': 'public, max-age=100' });
    res.end(`self-v${getHits}`);
  });
  await listen(srv);
  const { client, request } = cachingClient(`http://127.0.0.1:${srv.address().port}`);
  try {
    await text(request({ path: '/self', method: 'GET' }));
    await settle();
    await text(request({ path: '/self', method: 'POST' }));
    await settle();
    assert.strictEqual(await text(request({ path: '/self', method: 'GET' })), 'self-v2');
    assert.strictEqual(getHits, 2);
  } finally {
    await client.close();
    await close(srv);
  }
});

test('vb no-store responses are never cached', async () => {
  let hits = 0;
  const srv = http.createServer((_r, res) => {
    hits++;
    res.writeHead(200, { 'cache-control': 'no-store' });
    res.end('ns');
  });
  await listen(srv);
  const { client, request } = cachingClient(`http://127.0.0.1:${srv.address().port}`);
  try {
    await text(request({ path: '/n', method: 'GET' }));
    await settle();
    await text(request({ path: '/n', method: 'GET' }));
    assert.strictEqual(hits, 2);
  } finally {
    await client.close();
    await close(srv);
  }
});

test('vb a cross-origin Location does not invalidate the other origin', async () => {
  let bHits = 0;
  const srvB = http.createServer((_r, res) => {
    bHits++;
    res.writeHead(200, { 'cache-control': 'public, max-age=100' });
    res.end(`b-v${bHits}`);
  });
  await listen(srvB);
  const bOrigin = `http://127.0.0.1:${srvB.address().port}`;
  const srvA = http.createServer((_r, res) => {
    res.writeHead(201, { location: `${bOrigin}/z` });
    res.end();
  });
  await listen(srvA);
  const aOrigin = `http://127.0.0.1:${srvA.address().port}`;
  const store = new MemoryCacheStore();
  const cb = cachingClient(bOrigin, { store });
  const ca = cachingClient(aOrigin, { store });
  try {
    await text(cb.request({ path: '/z', method: 'GET' }));
    await settle();
    await text(ca.request({ path: '/w', method: 'POST' }));
    await settle();
    assert.strictEqual(await text(cb.request({ path: '/z', method: 'GET' })), 'b-v1');
    assert.strictEqual(bHits, 1);
  } finally {
    await ca.client.close();
    await cb.client.close();
    await close(srvA);
    await close(srvB);
  }
});
