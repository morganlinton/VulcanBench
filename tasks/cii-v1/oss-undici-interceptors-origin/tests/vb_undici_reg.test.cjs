// Hidden pass-to-pass guards: plain dispatch and cache-exempt methods.
'use strict';
const { test } = require('node:test');
const assert = require('node:assert');
const { createServer } = require('node:http');
const { Client, interceptors } = require('./index.js');

const listen = (server) => new Promise((resolve) => server.listen(0, '127.0.0.1', resolve));
const close = (server) => new Promise((resolve) => server.close(resolve));

test('vb plain Client request works', async () => {
  const server = createServer((req, res) => {
    res.writeHead(200, { 'content-type': 'application/json' });
    res.end(JSON.stringify({ path: req.url }));
  });
  await listen(server);
  const { port } = server.address();
  const client = new Client(`http://127.0.0.1:${port}`);
  try {
    const res = await client.request({ path: '/x', method: 'GET' });
    assert.equal(res.statusCode, 200);
    assert.deepEqual(JSON.parse(await res.body.text()), { path: '/x' });
  } finally {
    await client.close();
    await close(server);
  }
});

test('vb cache() does not cache POST', async () => {
  let hits = 0;
  const server = createServer((req, res) => {
    hits++;
    res.writeHead(200, { 'cache-control': 'public, max-age=60' });
    res.end('post');
  });
  await listen(server);
  const { port } = server.address();
  const client = new Client(`http://127.0.0.1:${port}`).compose(interceptors.cache());
  try {
    for (let i = 0; i < 2; i++) {
      const res = await client.request({ path: '/', method: 'POST' });
      await res.body.text();
    }
    assert.equal(hits, 2, 'POST must not be cached');
  } finally {
    await client.close();
    await close(server);
  }
});
