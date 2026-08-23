// Hidden fail-to-pass tests: a .default() must keep supplying its value when
// the schema continues into a .transform() and the whole thing sits under an
// optional wrapper (.optional() or .partial()). All graded through the public
// parse surface.
import { test } from 'node:test'
import assert from 'node:assert'
import * as z from './packages/zod/src/v4/index'

const arrayFromString = z
  .string()
  .default('')
  .transform((v: string) => (v ? v.split(',') : []))

test('vb default-transform under partial supplies the default', () => {
  const schema = z.object({ a: arrayFromString }).partial()
  assert.deepStrictEqual(schema.parse({}), { a: [] })
  assert.deepStrictEqual(schema.parse({ a: undefined }), { a: [] })
  assert.deepStrictEqual(schema.parse({ a: 'x,y' }), { a: ['x', 'y'] })
})

test('vb default-transform under optional supplies the default', () => {
  assert.deepStrictEqual(arrayFromString.optional().parse(undefined), [])
  assert.deepStrictEqual(arrayFromString.optional().parse('x,y'), ['x', 'y'])
})

test('vb prefault-transform under optional supplies the prefault', () => {
  const schema = z
    .string()
    .prefault('hi')
    .transform((v: string) => v.length)
    .optional()
  assert.strictEqual(schema.parse(undefined), 2)
  assert.strictEqual(schema.parse('abcd'), 4)
})

test('vb async parity for default-transform under optional wrappers', async () => {
  const asyncArray = z
    .string()
    .default('')
    .transform(async (v: string) => (v ? v.split(',') : []))
  const viaOptional = await asyncArray.optional().safeParseAsync(undefined)
  assert.deepStrictEqual(viaOptional.data, [])
  const viaPartial = await z.object({ a: asyncArray }).partial().safeParseAsync({})
  assert.deepStrictEqual(viaPartial.data, { a: [] })
})
