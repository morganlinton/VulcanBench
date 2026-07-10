// pass_to_pass regression guard: behavior that holds both before and after the
// fix. Ordinary catchall/strict handling of normal keys is unchanged, and a
// __proto__ input key is never an OWN key of the result either way (at base it
// becomes the prototype; after the fix it is skipped) while other unknown keys
// are still kept.

import { test } from 'node:test'
import assert from 'node:assert'
import * as z from './packages/zod/src/index'

const withProto = (json: string) => JSON.parse(json)

test('vb ordinary catchall and strict behavior unaffected', () => {
  const passthrough = z.object({ a: z.number() }).catchall(z.string())
  assert.deepStrictEqual(passthrough.parse({ a: 1, b: 'x', c: 'y' }), { a: 1, b: 'x', c: 'y' })
  assert.strictEqual(passthrough.safeParse({ a: 1, b: 2 }).success, false)

  const strict = z.strictObject({ a: z.number() })
  assert.strictEqual(strict.safeParse({ a: 1, unknown: 2 }).success, false)
  assert.deepStrictEqual(strict.parse({ a: 5 }), { a: 5 })

  // __proto__ is never an own key; other unknown keys are kept.
  const anyCatch = z.object({ a: z.number() }).catchall(z.any())
  const result: any = anyCatch.parse(withProto('{"a":1,"__proto__":{"p":1},"extra":"e"}'))
  assert.deepStrictEqual(Object.keys(result).sort(), ['a', 'extra'])
})
