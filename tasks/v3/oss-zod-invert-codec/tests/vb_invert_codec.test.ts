// Hidden test for oss-zod-invert-codec (colinhacks/zod PR #5770, closes #5625).
// A net-new `invertCodec(codec)` function must be added to BOTH the classic and
// mini v4 APIs: it returns a new codec with swapped input/output schemas and
// swapped decode/encode transforms, without mutating the original codec.
// Run with `tsx --test`.

import { test } from 'node:test'
import assert from 'node:assert'
import * as z from './packages/zod/src/index'
import * as zm from './packages/zod/src/mini'

const lengthCodec = () =>
  z.codec(z.string(), z.number(), {
    decode: (s: string) => s.length,
    encode: (n: number) => 'x'.repeat(n),
  })

test('vb invert swaps decode and encode', () => {
  const inv = z.invertCodec(lengthCodec())
  // decode on the inverted codec = encode of the original (number -> string)
  assert.strictEqual(z.decode(inv, 3), 'xxx')
  // encode on the inverted codec = decode of the original (string -> number)
  assert.strictEqual(z.encode(inv, 'abcd'), 4)
})

test('vb invert swaps the schemas', () => {
  const inv = z.invertCodec(lengthCodec())
  // the inverted codec's decode input is validated against the ORIGINAL output
  // schema (number), so a string input must fail validation
  const bad = z.safeDecode(inv, 'not-a-number' as unknown as number)
  assert.strictEqual(bad.success, false)
  const good = z.safeDecode(inv, 5)
  assert.strictEqual(good.success, true)
  assert.strictEqual(good.data, 'xxxxx')
})

test('vb double inversion round-trips', () => {
  const codec = lengthCodec()
  const back = z.invertCodec(z.invertCodec(codec))
  assert.strictEqual(z.decode(back, 'hello'), 5)
  assert.strictEqual(z.encode(back, 2), 'xx')
})

test('vb invert does not mutate the original codec', () => {
  const codec = lengthCodec()
  const inv = z.invertCodec(codec)
  assert.notStrictEqual(inv, codec)
  // original still decodes string -> number after inversion
  assert.strictEqual(z.decode(codec, 'abc'), 3)
  assert.strictEqual(z.encode(codec, 4), 'xxxx')
})

test('vb mini api has invertCodec too', () => {
  const codec = zm.codec(zm.string(), zm.number(), {
    decode: (s: string) => s.length,
    encode: (n: number) => 'x'.repeat(n),
  })
  const inv = zm.invertCodec(codec)
  assert.strictEqual(zm.decode(inv, 2), 'xx')
  assert.strictEqual(zm.encode(inv, 'abc'), 3)
})
