// Hidden test for oss-zod-proto-catchall (colinhacks/zod PR #5898).
// In handleCatchall(), an unrecognized `__proto__` key from the input was
// assigned onto the fresh result object via `result[key] = value`, which goes
// through the __proto__ setter and replaces the result's prototype (prototype
// pollution). The fix skips a `__proto__` key during catchall iteration.
// Every test here FAILS at the unfixed base and passes after the fix.
// Run with `tsx --test`.

import { test } from 'node:test'
import assert from 'node:assert'
import * as z from './packages/zod/src/index'

// JSON.parse produces a real own "__proto__" data property (unlike an object
// literal, where __proto__: is the setter), which is the vector being tested.
const withProto = (json: string) => JSON.parse(json)

test('vb catchall result keeps a clean prototype', () => {
  // At base the __proto__ value replaces the result's prototype.
  const schema = z.object({ a: z.number() }).catchall(z.any())
  const result: any = schema.parse(withProto('{"a":1,"__proto__":{"polluted":true}}'))
  assert.strictEqual(Object.getPrototypeOf(result), Object.prototype)
})

test('vb catchall does not expose the attacker payload', () => {
  // At base, result inherits `polluted` from the replaced prototype.
  const schema = z.object({ a: z.number() }).catchall(z.any())
  const result: any = schema.parse(withProto('{"a":1,"__proto__":{"polluted":true},"extra":"e"}'))
  assert.strictEqual(result.polluted, undefined)
  assert.strictEqual(result.extra, 'e')
})

test('vb strictObject with a __proto__ key parses successfully', () => {
  // At base __proto__ is collected as unrecognized, so strict rejects it.
  // With the fix it is skipped, so an object of known keys + __proto__ passes.
  const schema = z.strictObject({ a: z.number() })
  const r = schema.safeParse(withProto('{"a":1,"__proto__":{"x":1}}'))
  assert.strictEqual(r.success, true)
  if (r.success) assert.deepStrictEqual(Object.keys(r.data), ['a'])
})
