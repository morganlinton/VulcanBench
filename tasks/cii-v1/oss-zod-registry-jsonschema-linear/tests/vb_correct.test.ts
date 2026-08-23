// Hidden pass-to-pass guards: the conversion's OUTPUT must be unchanged.
//
// These are the tail mechanism for a performance task. The base commit already
// produces correct output, so every one of these passes before the change; a
// speed-up that drops schemas, breaks cross-references, loses metadata or
// changes single-schema conversion fails a guard and zeroes the score.
import { test } from 'node:test'
import assert from 'node:assert'
import * as z from './packages/zod/src/v4/index'

const convert = (reg: unknown, opts: Record<string, unknown> = {}) =>
  z.toJSONSchema(reg as never, { uri: (id: string) => `#/defs/${id}`, ...opts } as never) as any

test('vb every registered schema appears with its id', () => {
  const reg = z.registry<{ id: string }>()
  for (const id of ['Alpha', 'Beta', 'Gamma']) {
    reg.add(z.object({ v: z.string() }), { id })
  }
  const out = convert(reg)
  assert.deepStrictEqual(Object.keys(out.schemas).sort(), ['Alpha', 'Beta', 'Gamma'])
  assert.equal(out.schemas.Alpha.$id, '#/defs/Alpha')
  assert.equal(out.schemas.Beta.type, 'object')
})

test('vb shared schemas become cross-references, not copies', () => {
  const reg = z.registry<{ id: string }>()
  const inner = z.object({ q: z.string() })
  reg.add(z.object({ x: z.string(), nested: inner }), { id: 'A' })
  reg.add(z.object({ y: z.number(), also: inner }), { id: 'B' })
  reg.add(inner, { id: 'Inner' })

  const out = convert(reg)
  assert.equal(out.schemas.A.properties.nested.$ref, '#/defs/Inner')
  assert.equal(out.schemas.B.properties.also.$ref, '#/defs/Inner')
  assert.equal(out.schemas.Inner.properties.q.type, 'string')
})

test('vb field details and required lists survive conversion', () => {
  const reg = z.registry<{ id: string }>()
  reg.add(z.object({ a: z.string(), b: z.number(), c: z.boolean().optional() }), { id: 'S' })
  const s = convert(reg).schemas.S
  assert.equal(s.properties.a.type, 'string')
  assert.equal(s.properties.b.type, 'number')
  assert.equal(s.properties.c.type, 'boolean')
  assert.deepStrictEqual(s.required.sort(), ['a', 'b'])
  assert.equal(s.additionalProperties, false)
})

test('vb the uri callback is honored for ids and refs', () => {
  const reg = z.registry<{ id: string }>()
  const inner = z.object({ q: z.string() })
  reg.add(z.object({ nested: inner }), { id: 'Outer' })
  reg.add(inner, { id: 'Inner' })
  const out = z.toJSONSchema(reg as never, {
    uri: (id: string) => `https://example.com/schemas/${id}.json`,
  } as never) as any
  assert.equal(out.schemas.Outer.$id, 'https://example.com/schemas/Outer.json')
  assert.equal(out.schemas.Outer.properties.nested.$ref, 'https://example.com/schemas/Inner.json')
})

test('vb single-schema conversion is unaffected', () => {
  const single = z.toJSONSchema(z.object({ a: z.string(), b: z.number().optional() })) as any
  assert.equal(single.type, 'object')
  assert.equal(single.properties.a.type, 'string')
  assert.deepStrictEqual(single.required, ['a'])
})

test('vb nested and recursive structures still convert', () => {
  const reg = z.registry<{ id: string }>()
  const leaf = z.object({ name: z.string() })
  const branch = z.object({ leaf, tags: z.array(z.string()) })
  reg.add(branch, { id: 'Branch' })
  reg.add(leaf, { id: 'Leaf' })
  const out = convert(reg)
  assert.equal(out.schemas.Branch.properties.leaf.$ref, '#/defs/Leaf')
  assert.equal(out.schemas.Branch.properties.tags.type, 'array')
  assert.equal(out.schemas.Branch.properties.tags.items.type, 'string')
})

test('vb a large registry still converts correctly, not just quickly', () => {
  const reg = z.registry<{ id: string }>()
  for (let i = 0; i < 400; i++) {
    reg.add(z.object({ idx: z.number(), label: z.string() }), { id: `S${i}` })
  }
  const out = convert(reg)
  assert.equal(Object.keys(out.schemas).length, 400)
  assert.equal(out.schemas.S0.properties.idx.type, 'number')
  assert.equal(out.schemas.S399.properties.label.type, 'string')
  assert.equal(out.schemas.S399.$id, '#/defs/S399')
})
