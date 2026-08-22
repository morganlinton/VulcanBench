// Hidden fail-to-pass tests: cache() and deduplicate() must work when
// composed onto Client/Pool (regression for silently-inert interceptors when
// dispatch options carry no origin). Loopback servers only.
'use strict';
const { test } = require('node:test');
const assert = require('node:assert');
const { createServer } = require('node:http');
const { Client, Pool, interceptors } = require('./index.js');

const listen = (server) => new Promise((resolve) => server.listen(0, '127.0.0.1', resolve));
const close = (server) => new Promise((resolve) => server.close(resolve));

const cacheableServer = (counter) =>
  createServer((req, res) => {
    counter.hits++;
    res.writeHead(200, { 'cache-control': 'public, max-age=60', 'content-type': 'text/plain' });
    res.end('cacheable');
  });

test('vb cache() caches when composed onto a Client', async () => {
  const counter = { hits: 0 };
  const server = cacheableServer(counter);
  await listen(server);
  const { port } = server.address();
  const client = new Client(`http://127.0.0.1:${port}`).compose(interceptors.cache());
  try {
    for (let i = 0; i < 2; i++) {
      const res = await client.request({ path: '/', method: 'GET' });
      assert.equal(await res.body.text(), 'cacheable');
    }
    assert.equal(counter.hits, 1, 'second request must be served from cache');
  } finally {
    await client.close();
    await close(server);
  }
});

test('vb cache() caches when composed onto a Pool', async () => {
  const counter = { hits: 0 };
  const server = cacheableServer(counter);
  await listen(server);
  const { port } = server.address();
  const pool = new Pool(`http://127.0.0.1:${port}`).compose(interceptors.cache());
  try {
    for (let i = 0; i < 2; i++) {
      const res = await pool.request({ path: '/', method: 'GET' });
      await res.body.text();
    }
    assert.equal(counter.hits, 1, 'second request must be served from cache');
  } finally {
    await pool.close();
    await close(server);
  }
});

test('vb deduplicate() coalesces concurrent requests on a Client', async () => {
  const counter = { hits: 0 };
  const server = createServer((req, res) => {
    counter.hits++;
    setTimeout(() => {
      res.writeHead(200, { 'content-type': 'text/plain' });
      res.end('deduped');
    }, 50);
  });
  await listen(server);
  const { port } = server.address();
  const client = new Client(`http://127.0.0.1:${port}`).compose(interceptors.deduplicate());
  try {
    const [a, b] = await Promise.all([
      client.request({ path: '/', method: 'GET' }),
      client.request({ path: '/', method: 'GET' }),
    ]);
    assert.equal(await a.body.text(), 'deduped');
    assert.equal(await b.body.text(), 'deduped');
    assert.equal(counter.hits, 1, 'concurrent identical requests must coalesce');
  } finally {
    await client.close();
    await close(server);
  }
});
