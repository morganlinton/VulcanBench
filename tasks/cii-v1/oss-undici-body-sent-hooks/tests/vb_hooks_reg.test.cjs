// Hidden pass-to-pass guards: bare dispatch already fires the hooks, and
// responses still flow through wrapped handlers.
'use strict';
const { test } = require('node:test');
const assert = require('node:assert');
const { createServer } = require('node:http');
const { once } = require('node:events');
const { Client, DecoratorHandler } = require('./index.js');

test('vb hooks fire on a bare Client', async (t) => {
  const server = createServer((req, res) => { req.resume(); req.on('end', () => res.end('ok')); });
  server.listen(0, '127.0.0.1');
  await once(server, 'listening');
  t.after(() => server.close());
  const client = new Client(`http://127.0.0.1:${server.address().port}`);
  t.after(() => client.close());
  const seen = await new Promise((resolve, reject) => {
    const acc = { bodySent: [], requestSent: 0 };
    client.dispatch(
      { method: 'POST', path: '/', headers: { 'content-type': 'text/plain' }, body: 'hi' },
      {
        onRequestStart () {},
        onBodySent (chunk) { acc.bodySent.push(Buffer.from(chunk).toString()); },
        onRequestSent () { acc.requestSent++; },
        onResponseStart () {},
        onResponseData () {},
        onResponseEnd () { resolve(acc); },
        onResponseError (_c, err) { reject(err); }
      }
    );
  });
  assert.deepStrictEqual(seen.bodySent, ['hi']);
  assert.strictEqual(seen.requestSent, 1);
});

test('vb responses still flow through DecoratorHandler', async (t) => {
  const server = createServer((req, res) => { req.resume(); req.on('end', () => res.end('payload')); });
  server.listen(0, '127.0.0.1');
  await once(server, 'listening');
  t.after(() => server.close());
  let client = new Client(`http://127.0.0.1:${server.address().port}`);
  client = client.compose((dispatch) => (opts, handler) => dispatch(opts, new DecoratorHandler(handler)));
  t.after(() => client.close());
  const res = await client.request({ path: '/', method: 'GET' });
  assert.equal(res.statusCode, 200);
  assert.equal(await res.body.text(), 'payload');
});
