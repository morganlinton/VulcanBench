// Hidden pass-to-pass guards: the optionality conventions that must NOT move.
//
// These pin 36 verified-stable behaviors adjacent to the fix. Several point in
// the opposite direction from the fail-to-pass tests — an optional wrapper
// must keep short-circuiting undefined for schemas whose undefined-acceptance
// comes from preprocess, catch, or a bare transform — so an over-broad fix
// fails here and zeroes the functional score.
import { test } from 'node:test'
import assert from 'node:assert'
import * as z from './packages/zod/src/v4/index'

test('vb key-level admissibility for every kind of key schema', () => {
  assert.deepStrictEqual(z.object({ a: z.string().default('D') }).parse({}), { a: 'D' })
  assert.deepStrictEqual(z.object({ a: z.string().prefault('P') }).parse({}), { a: 'P' })
  assert.deepStrictEqual(z.object({ a: z.string().optional() }).parse({}), {})
  assert.deepStrictEqual(z.object({ a: z.transform(() => 'T') }).parse({}), { a: 'T' })
  assert.deepStrictEqual(z.object({ a: z.string().catch('C') }).parse({}), { a: 'C' })
  assert.strictEqual(z.object({ a: z.string() }).safeParse({}).success, false)
})

test('vb tuple minimum length respects defaults and optionals', () => {
  assert.deepStrictEqual(z.tuple([z.string().default('D')]).parse([]), ['D'])
  assert.deepStrictEqual(z.tuple([z.string().prefault('P')]).parse([]), ['P'])
  assert.deepStrictEqual(z.tuple([z.string().optional()]).parse([]), [])
  assert.strictEqual(z.tuple([z.string()]).safeParse([]).success, false)
  assert.deepStrictEqual(z.tuple([z.string(), z.number().default(1)]).parse(['x']), ['x', 1])
})

test('vb optional short-circuits undefined for non-default undefined-acceptors', () => {
  assert.strictEqual(z.preprocess((v: unknown) => v ?? 'X', z.string()).optional().parse(undefined), undefined)
  assert.strictEqual(z.string().catch('X').optional().parse(undefined), undefined)
  assert.strictEqual(
    z.string().catch('X').transform((s: string) => `${s}!`).optional().parse(undefined),
    undefined
  )
  assert.strictEqual(z.transform(() => 'T').optional().parse(undefined), undefined)
  assert.strictEqual(
    z.union([z.preprocess((v: unknown) => v ?? 'X', z.string()), z.number()]).optional().parse(undefined),
    undefined
  )
  // ...while the same preprocess as an object key still fills the key.
  assert.deepStrictEqual(
    z.object({ a: z.preprocess((v: unknown) => v ?? 'X', z.string()) }).parse({}),
    { a: 'X' }
  )
})

test('vb a default survives every wrapper order', () => {
  assert.strictEqual(z.string().default('D').optional().parse(undefined), 'D')
  assert.strictEqual(z.string().default('D').optional().optional().parse(undefined), 'D')
  assert.strictEqual(z.string().default('D').nullable().optional().parse(undefined), 'D')
  assert.strictEqual(z.string().default('D').readonly().optional().parse(undefined), 'D')
  assert.strictEqual(z.string().default('D').catch('C').optional().parse(undefined), 'D')
  assert.strictEqual(z.string().default('D').pipe(z.string()).optional().parse(undefined), 'D')
  assert.strictEqual(z.union([z.string().default('D'), z.number()]).optional().parse(undefined), 'D')
  assert.strictEqual(z.lazy(() => z.string().default('D')).optional().parse(undefined), 'D')
})

test('vb record and catchall values honor defaults for undefined', () => {
  assert.deepStrictEqual(
    z.record(z.string(), z.string().default('D')).parse({ k: undefined }),
    { k: 'D' }
  )
  assert.deepStrictEqual(
    z.object({}).catchall(z.string().default('D')).parse({ x: undefined }),
    { x: 'D' }
  )
})

test('vb exactOptional, standalone parses and partial/required round trips', () => {
  assert.deepStrictEqual(z.object({ a: z.string().exactOptional() }).parse({}), {})
  assert.strictEqual(z.object({ a: z.string().exactOptional() }).safeParse({ a: undefined }).success, false)
  assert.strictEqual(z.string().default('D').parse(undefined), 'D')
  assert.strictEqual(z.string().optional().parse(undefined), undefined)
  assert.strictEqual(
    z.string().default('D').pipe(z.string().transform((s: string) => s.length)).parse(undefined),
    1
  )
  assert.deepStrictEqual(z.object({ a: z.string() }).partial().parse({}), {})
  assert.strictEqual(z.object({ a: z.string().optional() }).required().safeParse({}).success, false)
})

test('vb async parity for the short-circuit invariants', async () => {
  const res = await z
    .preprocess(async (v: unknown) => v ?? 'X', z.string())
    .optional()
    .safeParseAsync(undefined)
  assert.strictEqual(res.data, undefined)
})
