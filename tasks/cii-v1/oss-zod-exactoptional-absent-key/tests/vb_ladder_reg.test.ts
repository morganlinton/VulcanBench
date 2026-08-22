// Hidden pass-to-pass guards: the other rungs of the optionality ladder keep
// their meaning.
import { test } from 'node:test'
import assert from 'node:assert'
import * as z from './packages/zod/src/v4/index'

test('vb top rung still substitutes defaults', () => {
  assert.deepStrictEqual(
    z.object({ a: z.string().default('D').exactOptional() }).parse({}),
    { a: 'D' }
  )
  assert.deepStrictEqual(
    z.object({ a: z.string().prefault('P').exactOptional() }).parse({}),
    { a: 'P' }
  )
  assert.deepStrictEqual(
    z.tuple([z.string(), z.string().default('D').exactOptional()]).parse(['x']),
    ['x', 'D']
  )
})

test('vb explicitly present undefined still runs the inner schema', () => {
  assert.deepStrictEqual(
    z.object({ a: z.coerce.string() }).parse({ a: undefined }),
    { a: 'undefined' }
  )
})

test('vb catch on a required key still substitutes', () => {
  const schema = z.object({ a: z.string().catch('C') })
  assert.deepStrictEqual(schema.parse({}), { a: 'C' })
  assert.deepStrictEqual(z.compile(z.object({ a: z.string().catch('C') })).parse({}), { a: 'C' })
})

test('vb plain optional absent key stays absent', () => {
  assert.deepStrictEqual(z.object({ a: z.string().optional() }).parse({}), {})
})

test('vb exactOptional still rejects explicit undefined for plain strings', () => {
  assert.equal(
    z.object({ a: z.string().exactOptional() }).safeParse({ a: undefined }).success,
    false
  )
})
