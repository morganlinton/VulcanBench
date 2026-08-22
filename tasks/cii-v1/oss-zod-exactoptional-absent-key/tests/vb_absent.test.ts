// Hidden fail-to-pass tests: an absent key on the middle optionality rung
// (exactOptional: absence permitted, nothing substituted) must contribute
// nothing — never a value the schema invented from `undefined`. Must hold in
// the interpreted, jitless, and compiled paths. Public API only.
import { test } from 'node:test'
import assert from 'node:assert'
import * as z from './packages/zod/src/v4/index'

test('vb absent coerced key stays absent', () => {
  const schema = z.object({ a: z.coerce.string().exactOptional() })
  assert.deepStrictEqual(schema.parse({}), {})
  assert.deepStrictEqual(schema.parse({}, { jitless: true }), {})
})

test('vb absent key with catch stays absent', () => {
  const schema = z.object({ a: z.string().catch('C').exactOptional() })
  assert.deepStrictEqual(schema.parse({}), {})
  assert.deepStrictEqual(schema.parse({}, { jitless: true }), {})
})

test('vb absent key with preprocess stays absent', () => {
  const schema = z.object({ a: z.preprocess((v) => v ?? 'X', z.string()).exactOptional() })
  assert.deepStrictEqual(schema.parse({}), {})
  assert.deepStrictEqual(schema.parse({}, { jitless: true }), {})
})

test('vb absent tuple slot on the middle rung truncates', () => {
  assert.deepStrictEqual(z.tuple([z.string(), z.coerce.string().exactOptional()]).parse(['x']), ['x'])
  assert.deepStrictEqual(
    z.tuple([z.string(), z.string().catch('C').exactOptional()]).parse(['x']),
    ['x']
  )
})

test('vb compiled path agrees on absent middle-rung keys', () => {
  const coerced = z.compile(z.object({ a: z.coerce.string().exactOptional() }))
  assert.deepStrictEqual(coerced.parse({}), {})
  const caught = z.compile(z.object({ a: z.string().catch('C').exactOptional() }))
  assert.deepStrictEqual(caught.parse({}), {})
})

test('vb compiled path agrees on absent middle-rung tuple slots', () => {
  const t = z.compile(z.tuple([z.string(), z.coerce.string().exactOptional()]))
  assert.deepStrictEqual(t.parse(['x']), ['x'])
})
