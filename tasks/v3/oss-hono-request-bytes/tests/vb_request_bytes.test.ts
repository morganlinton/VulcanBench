// Hidden test for oss-hono-request-bytes (honojs/hono PR #4921).
// HonoRequest must gain a `bytes()` method returning Promise<Uint8Array> of the
// request body, wired through the same cached-body mechanism as text()/json()/
// arrayBuffer() so the body can be read more than once. Run with `tsx --test`.

import { test } from 'node:test'
import assert from 'node:assert'
import { HonoRequest } from './src/request'

const post = (body: BodyInit) =>
  new HonoRequest(new Request('http://localhost/entry', { method: 'POST', body }))

test('vb bytes returns Uint8Array of the body', async () => {
  const req = post('hello hono')
  const b = await req.bytes()
  assert.ok(b instanceof Uint8Array)
  assert.strictEqual(new TextDecoder().decode(b), 'hello hono')
})

test('vb bytes preserves binary content', async () => {
  const payload = new Uint8Array([0, 1, 2, 250, 251, 252, 253, 254, 255])
  const req = post(payload)
  const b = await req.bytes()
  assert.deepStrictEqual(Array.from(b), Array.from(payload))
})

test('vb bytes uses the cached body (multiple reads work)', async () => {
  // A naive `this.raw.bytes()` fails here: the raw body stream is consumed by
  // the first read. The cached-body mechanism must allow bytes() after text()
  // and repeated bytes() calls.
  const req = post('cache me')
  assert.strictEqual(await req.text(), 'cache me')
  const b1 = await req.bytes()
  const b2 = await req.bytes()
  assert.strictEqual(new TextDecoder().decode(b1), 'cache me')
  assert.strictEqual(new TextDecoder().decode(b2), 'cache me')
})

test('vb bytes empty body', async () => {
  const req = post('')
  const b = await req.bytes()
  assert.ok(b instanceof Uint8Array)
  assert.strictEqual(b.byteLength, 0)
})
