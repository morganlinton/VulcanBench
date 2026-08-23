// Hidden fail-to-pass tests: a record's key schema inside an intersection
// governs only the keys it matches — exactly as TypeScript treats an index
// signature in `{name: string} & Record<`S_${string}`, string>`. Graded via
// parse results and structured issue codes; never message text.
import { test } from 'node:test'
import assert from 'node:assert'
import * as z from './packages/zod/src/v4/index'

const codes = (r: { success: boolean; error?: { issues: { code: string }[] } }) =>
  r.success ? 'OK' : r.error!.issues.map((i) => i.code).sort()

test('vb regex-keyed record in intersection admits non-matching object keys', () => {
  const schema = z.object({ name: z.string() }).and(z.record(z.string().regex(/^S_/), z.string()))
  assert.deepStrictEqual(schema.safeParse({ name: 'a', S_a: 's' }).data, { name: 'a', S_a: 's' })
  // A key the record schema DOES match, with the wrong value type, is still
  // an error — a value error, not a key error.
  assert.deepStrictEqual(codes(schema.safeParse({ name: 'a', S_a: 42 })), ['invalid_type'])
})

test('vb object-governed key with wrong value reports only the value error', () => {
  const schema = z.object({ name: z.string() }).and(z.record(z.string().regex(/^S_/), z.string()))
  assert.deepStrictEqual(codes(schema.safeParse({ name: 42, S_a: 's' })), ['invalid_type'])
})

test('vb template-literal-keyed record in intersection behaves the same way', () => {
  const schema = z.object({ n: z.string() }).and(z.record(z.templateLiteral(['k_', z.string()]), z.number()))
  assert.deepStrictEqual(schema.safeParse({ n: 'a', k_1: 5 }).data, { n: 'a', k_1: 5 })
  assert.deepStrictEqual(codes(schema.safeParse({ n: 'a', k_1: 'no' })), ['invalid_type'])
})

test('vb refinement on a strict operand still runs when an extra key is present', () => {
  let ran = false
  const schema = z.strictObject({ a: z.string() }).refine((v) => {
    ran = true
    return v.a.length > 0
  })
  ran = false
  schema.safeParse({ a: 'ok', extra: 1 })
  assert.strictEqual(ran, true)
})
