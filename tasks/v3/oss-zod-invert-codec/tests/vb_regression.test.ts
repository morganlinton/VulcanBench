// pass_to_pass regression guard: pre-existing codec behavior is unchanged.
// Does not reference invertCodec, so it passes at the base commit too.

import { test } from 'node:test'
import assert from 'node:assert'
import * as z from './packages/zod/src/index'

test('vb existing codec behavior unaffected', () => {
  const codec = z.codec(z.string(), z.number(), {
    decode: (s: string) => s.length,
    encode: (n: number) => 'x'.repeat(n),
  })
  assert.strictEqual(z.decode(codec, 'abc'), 3)
  assert.strictEqual(z.encode(codec, 2), 'xx')
  assert.strictEqual(z.string().parse('ok'), 'ok')
  const bad = z.safeDecode(codec, 42 as unknown as string)
  assert.strictEqual(bad.success, false)
})
