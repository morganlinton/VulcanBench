// Hidden pass-to-pass guards: valid limits and parsing behavior unchanged.
'use strict';
const { test } = require('node:test');
const assert = require('node:assert');
const http = require('node:http');
const bodyParser = require('./index.js');

const request = (port, path, body) =>
  new Promise((resolve, reject) => {
    const req = http.request(
      { host: '127.0.0.1', port, path, method: 'POST', headers: { 'content-type': 'application/json' } },
      (res) => {
        let buf = '';
        res.on('data', (c) => (buf += c));
        res.on('end', () => resolve({ status: res.statusCode, body: buf }));
      }
    );
    req.on('error', reject);
    req.end(body);
  });

const withParser = async (parser, fn) => {
  const server = http.createServer((req, res) => {
    parser(req, res, (err) => {
      if (err) {
        res.statusCode = err.status || 500;
        res.end('err');
        return;
      }
      res.setHeader('content-type', 'application/json');
      res.end(JSON.stringify(req.body ?? null));
    });
  });
  await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve));
  try {
    await fn(server.address().port);
  } finally {
    server.close();
  }
};

test('vb valid string limit accepted and parses json', async () => {
  await withParser(bodyParser.json({ limit: '1mb' }), async (port) => {
    const res = await request(port, '/', JSON.stringify({ a: 1 }));
    assert.equal(res.status, 200);
    assert.deepEqual(JSON.parse(res.body), { a: 1 });
  });
});

test('vb numeric limit accepted and enforced', async () => {
  await withParser(bodyParser.json({ limit: 32 }), async (port) => {
    const big = JSON.stringify({ pad: 'x'.repeat(100) });
    const res = await request(port, '/', big);
    assert.equal(res.status, 413, 'over-limit body must be rejected');
  });
});

test('vb default limit parses ordinary bodies', async () => {
  await withParser(bodyParser.json(), async (port) => {
    const res = await request(port, '/', JSON.stringify({ ok: true }));
    assert.equal(res.status, 200);
    assert.deepEqual(JSON.parse(res.body), { ok: true });
  });
});
