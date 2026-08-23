// Hidden fail-to-pass tests: onBodySent/onRequestSent must be forwarded
// through wrapped handlers (DecoratorHandler and the interceptors built on
// it). Loopback servers only.
'use strict';
const { test } = require('node:test');
const assert = require('node:assert');
const { createServer } = require('node:http');
const { once } = require('node:events');
const { Client, DecoratorHandler, interceptors } = require('./index.js');

const BODY = '{"hello":"world"}';

function dispatchAndTrack (dispatcher) {
  const seen = { bodySent: [], requestSent: 0 };
  return new Promise((resolve, reject) => {
    dispatcher.dispatch(
      { method: 'POST', path: '/', headers: { 'content-type': 'application/json' }, body: BODY },
      {
        onRequestStart () {},
        onBodySent (chunk) { seen.bodySent.push(Buffer.from(chunk).toString()); },
        onRequestSent () { seen.requestSent++; },
        onResponseStart () {},
        onResponseData () {},
        onResponseEnd () { resolve(seen); },
        onResponseError (_controller, err) { reject(err); }
      }
    );
  });
}

async function withClient (t, compose) {
  const server = createServer((req, res) => {
    req.resume();
    req.on('end', () => res.end('ok'));
  });
  server.listen(0, '127.0.0.1');
  await once(server, 'listening');
  t.after(() => server.close());
  let client = new Client(`http://127.0.0.1:${server.address().port}`);
  if (compose) client = compose(client);
  t.after(() => client.close());
  return client;
}

test('vb hooks survive DecoratorHandler', async (t) => {
  const client = await withClient(t, (c) =>
    c.compose((dispatch) => (opts, handler) => dispatch(opts, new DecoratorHandler(handler)))
  );
  const seen = await dispatchAndTrack(client);
  assert.deepStrictEqual(seen.bodySent, [BODY]);
  assert.strictEqual(seen.requestSent, 1);
});

test('vb hooks survive interceptors.retry()', async (t) => {
  const client = await withClient(t, (c) => c.compose(interceptors.retry()));
  const seen = await dispatchAndTrack(client);
  assert.deepStrictEqual(seen.bodySent, [BODY]);
  assert.strictEqual(seen.requestSent, 1);
});

test('vb hooks survive interceptors.redirect()', async (t) => {
  const client = await withClient(t, (c) => c.compose(interceptors.redirect({ maxRedirections: 1 })));
  const seen = await dispatchAndTrack(client);
  assert.deepStrictEqual(seen.bodySent, [BODY]);
  assert.strictEqual(seen.requestSent, 1);
});
