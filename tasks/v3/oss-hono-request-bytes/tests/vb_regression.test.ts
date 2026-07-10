// pass_to_pass regression guard: pre-existing HonoRequest body parsers are
// unchanged. Does not reference bytes(), so it passes at the base commit too.

import { test } from 'node:test'
import assert from 'node:assert'
import { HonoRequest } from './src/request'

test('vb existing body parsers unaffected', async () => {
  const req = new HonoRequest(
    new Request('http://localhost/entry', { method: 'POST', body: '{"a":1}' })
  )
  assert.deepStrictEqual(await req.json(), { a: 1 })
  // cached body: a second read through another parser still works
  assert.strictEqual(await req.text(), '{"a":1}')
  const buf = await req.arrayBuffer()
  assert.strictEqual(buf.byteLength, 7)
})
