// Hidden fail-to-pass tests: four JSON Schema conformance families in
// z.fromJSONSchema. Graded through runtime parse behavior only.
import { test } from 'node:test'
import assert from 'node:assert'
import * as z from './packages/zod/src/v4/index'

const fjs = (s: unknown, params?: unknown) => (z as any).fromJSONSchema(s, params)
const accepts = (schema: any, v: unknown) => schema.safeParse(v).success

test('vb prefixItems tuples are open-ended unless items forbids extras', () => {
  const open = fjs({ type: 'array', prefixItems: [{ type: 'string' }, { type: 'number' }] })
  assert.strictEqual(accepts(open, ['a', 1, 'extra']), true, 'extras must be allowed by default')
  assert.strictEqual(accepts(open, ['a', 1]), true)
  const closed = fjs({ type: 'array', prefixItems: [{ type: 'string' }], items: false })
  assert.strictEqual(accepts(closed, ['a', 'b']), false, 'items:false still closes the tuple')
})

test('vb $ref resolves JSON Pointer escaped tokens', () => {
  assert.strictEqual(accepts(fjs({ $defs: { 'a/b': { type: 'number' } }, $ref: '#/$defs/a~1b' }), 5), true)
  assert.strictEqual(accepts(fjs({ $defs: { 'a~b': { type: 'number' } }, $ref: '#/$defs/a~0b' }), 5), true)
})

test('vb propertyNames is enforced and composes with other object keywords', () => {
  const composed = fjs({
    type: 'object',
    propertyNames: { pattern: '^[a-z]+$' },
    properties: { abc: { type: 'number' } },
    additionalProperties: { type: 'string' },
  })
  assert.deepStrictEqual(composed.safeParse({ abc: 1, xyz: 'v' }).data, { abc: 1, xyz: 'v' })
  assert.strictEqual(accepts(composed, { Bad: 'v' }), false, 'a key failing propertyNames must be rejected')
  const alone = fjs({ type: 'object', propertyNames: { pattern: '^[a-z]+$' } })
  assert.strictEqual(accepts(alone, { good: 1 }), true)
  assert.strictEqual(accepts(alone, { BAD: 1 }), false)
})

test('vb hostname format is validated', () => {
  const s = fjs({ type: 'string', format: 'hostname' })
  assert.strictEqual(accepts(s, 'example.com'), true)
  assert.strictEqual(accepts(s, 'not a host!'), false)
})
