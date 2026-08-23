// Hidden fail-to-pass tests: RFC 9111 cache behaviors — revalidation-only
// responses must be stored so conditional requests engage, and unsafe methods
// must invalidate the URIs named by Location / Content-Location. Loopback
// servers only; graded via origin hit counts and response bodies.
'use strict';
const { test } = require('node:test');
const assert = require('node:assert');
const http = require('node:http');
const { Client, interceptors } = require('./index.js');

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
  return { status: res.statusCode, body: await res.body.text() };
}

for (const cc of ['no-cache', 'max-age=0']) {
  test(`vb etag response with ${cc} is revalidated, not refetched`, async () => {
    let hits = 0;
    let sawConditional = false;
    const srv = http.createServer((req, res) => {
      hits++;
      if (req.headers['if-none-match'] === '"tag1"') {
        sawConditional = true;
        res.writeHead(304, { etag: '"tag1"' });
        res.end();
        return;
      }
      res.writeHead(200, { 'cache-control': cc, etag: '"tag1"' });
      res.end(hits === 1 ? 'v1' : 'v2');
    });
    await listen(srv);
    const { client, request } = cachingClient(`http://127.0.0.1:${srv.address().port}`);
    try {
      const first = await text(request({ path: '/x', method: 'GET' }));
      assert.strictEqual(first.body, 'v1');
      await settle();
      const second = await text(request({ path: '/x', method: 'GET' }));
      assert.strictEqual(sawConditional, true, 'second request must carry If-None-Match');
      assert.strictEqual(second.body, 'v1', '304 must be answered from the stored body');
    } finally {
      await client.close();
      await close(srv);
    }
  });
}

test('vb POST Location invalidates the cached target entry', async () => {
  let getHits = 0;
  const srv = http.createServer((req, res) => {
    if (req.method === 'POST') {
      res.writeHead(201, { location: '/collection/123' });
      res.end('created');
      return;
    }
    getHits++;
    res.writeHead(200, { 'cache-control': 'public, max-age=100' });
    res.end(`item-v${getHits}`);
  });
  await listen(srv);
  const { client, request } = cachingClient(`http://127.0.0.1:${srv.address().port}`);
  try {
    await text(request({ path: '/collection/123', method: 'GET' }));
    await settle();
    await text(request({ path: '/collection', method: 'POST' }));
    await settle();
    const after = await text(request({ path: '/collection/123', method: 'GET' }));
    assert.strictEqual(after.body, 'item-v2', 'entry must be refetched after the POST');
    assert.strictEqual(getHits, 2);
  } finally {
    await client.close();
    await close(srv);
  }
});

test('vb Content-Location on an unsafe method invalidates its target', async () => {
  let getHits = 0;
  const srv = http.createServer((req, res) => {
    if (req.method === 'PUT') {
      res.writeHead(200, { 'content-location': '/doc/9' });
      res.end('ok');
      return;
    }
    getHits++;
    res.writeHead(200, { 'cache-control': 'public, max-age=100' });
    res.end(`doc-v${getHits}`);
  });
  await listen(srv);
  const { client, request } = cachingClient(`http://127.0.0.1:${srv.address().port}`);
  try {
    await text(request({ path: '/doc/9', method: 'GET' }));
    await settle();
    await text(request({ path: '/docs', method: 'PUT' }));
    await settle();
    const after = await text(request({ path: '/doc/9', method: 'GET' }));
    assert.strictEqual(after.body, 'doc-v2');
  } finally {
    await client.close();
    await close(srv);
  }
});

test('vb relative Location references are resolved before invalidating', async () => {
  let getHits = 0;
  const srv = http.createServer((req, res) => {
    if (req.method === 'POST') {
      res.writeHead(201, { location: '456' });
      res.end();
      return;
    }
    getHits++;
    res.writeHead(200, { 'cache-control': 'public, max-age=100' });
    res.end(`rel-v${getHits}`);
  });
  await listen(srv);
  const { client, request } = cachingClient(`http://127.0.0.1:${srv.address().port}`);
  try {
    await text(request({ path: '/items/456', method: 'GET' }));
    await settle();
    await text(request({ path: '/items/', method: 'POST' }));
    await settle();
    const after = await text(request({ path: '/items/456', method: 'GET' }));
    assert.strictEqual(after.body, 'rel-v2');
  } finally {
    await client.close();
    await close(srv);
  }
});
