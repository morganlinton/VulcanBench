// Hidden pass-to-pass guards: the strictness conventions that must NOT move.
// Standalone records keep their excess-key check; strict objects still fail on
// unrecognized keys; intersection reconciliation and pipe semantics stay put.
import { test } from 'node:test'
import assert from 'node:assert'
import * as z from './packages/zod/src/v4/index'

const codes = (r: { success: boolean; error?: { issues: { code: string }[] } }) =>
  r.success ? 'OK' : r.error!.issues.map((i) => i.code).sort()

test('vb standalone records keep their excess-key check', () => {
  const rec = z.record(z.string().regex(/^S_/), z.string())
  assert.strictEqual(rec.safeParse({ other: 'x' }).success, false)
  assert.deepStrictEqual(rec.safeParse({ S_a: 'x' }).data, { S_a: 'x' })
  // enum-keyed records still require exhaustive keys
  assert.strictEqual(z.record(z.enum(['x', 'y']), z.string()).safeParse({ x: '1' }).success, false)
})

test('vb enum and literal keyed intersections keep their behavior', () => {
  const objAndEnum = z.object({ id: z.number() }).and(z.record(z.enum(['x', 'y']), z.boolean()))
  // enum-keyed records stay exhaustive even inside an intersection
  assert.strictEqual(objAndEnum.safeParse({ id: 1, x: true }).success, false)
  assert.deepStrictEqual(objAndEnum.safeParse({ id: 1, x: true, y: false }).data, { id: 1, x: true, y: false })
  assert.strictEqual(objAndEnum.safeParse({ id: 1, x: true, y: 1 }).success, false)
  const objAndLit = z.object({ q: z.string() }).and(z.record(z.literal(['m', 'p']), z.string()))
  assert.deepStrictEqual(objAndLit.safeParse({ q: 'a', m: '1', p: '2' }).data, { q: 'a', m: '1', p: '2' })
})

test('vb strict objects still fail on unrecognized keys', () => {
  const strict = z.strictObject({ a: z.string() })
  assert.strictEqual(strict.safeParse({ a: 'ok', extra: 2 }).success, false)
  assert.ok((codes(strict.safeParse({ a: 'ok', extra: 2 })) as string[]).includes('unrecognized_keys'))
  const bothBad = codes(strict.safeParse({ a: 1, extra: 2 })) as string[]
  assert.ok(bothBad.includes('unrecognized_keys'))
})

test('vb intersection reconciliation of unrecognized keys', () => {
  // a stripping or loose operand never objects, so the key is not reported
  assert.deepStrictEqual(
    z.object({ a: z.string() }).and(z.looseObject({})).safeParse({ a: 'x', extra: 1 }).data,
    { a: 'x', extra: 1 }
  )
  // every operand objecting -> reported
  assert.strictEqual(
    z.strictObject({ a: z.string() }).and(z.strictObject({ a: z.string() })).safeParse({ a: 'x', extra: 1 }).success,
    false
  )
  // one operand covering the key -> admitted
  assert.deepStrictEqual(
    z.strictObject({ a: z.string() }).and(z.object({ extra: z.number() })).safeParse({ a: 'x', extra: 1 }).data,
    { a: 'x', extra: 1 }
  )
  // a default inside a strict intersection still applies
  const strictInt = z.strictObject({ a: z.string().default('D') }).and(z.object({ b: z.number() }))
  assert.deepStrictEqual(strictInt.safeParse({ b: 1 }).data, { a: 'D', b: 1 })
})

test('vb pipe semantics are unchanged', () => {
  const jsonPipe = z
    .string()
    .refine((s: string) => {
      try {
        JSON.parse(s)
        return true
      } catch {
        return false
      }
    })
    .transform((s: string) => JSON.parse(s))
  // a failing refinement stops the pipe rather than throwing in the transform
  assert.strictEqual(jsonPipe.safeParse('not json').success, false)
  const strictPipe = z.strictObject({ a: z.string() }).pipe(z.transform((v: object) => Object.keys(v).length))
  assert.strictEqual(strictPipe.safeParse({ a: 'x' }).data, 1)
  assert.strictEqual(strictPipe.safeParse({ a: 'x', extra: 1 }).success, false)
})

test('vb plain object, loose object and record basics', () => {
  assert.deepStrictEqual(z.object({ a: z.string() }).safeParse({ a: 'x', extra: 1 }).data, { a: 'x' })
  assert.deepStrictEqual(z.looseObject({ a: z.string() }).safeParse({ a: 'x', extra: 1 }).data, { a: 'x', extra: 1 })
  assert.deepStrictEqual(z.record(z.string(), z.number()).safeParse({ k: 1 }).data, { k: 1 })
  assert.deepStrictEqual(
    z.object({ a: z.string() }).and(z.object({ b: z.number() })).safeParse({ a: 'x', b: 1 }).data,
    { a: 'x', b: 1 }
  )
  assert.strictEqual(z.object({ a: z.string() }).and(z.object({ a: z.number() })).safeParse({ a: 'x' }).success, false)
})
