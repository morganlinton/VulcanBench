// Hidden pass-to-pass guards: conversion behaviors that must not move.
import { test } from 'node:test'
import assert from 'node:assert'
import * as z from './packages/zod/src/v4/index'

const fjs = (s: unknown, params?: unknown) => (z as any).fromJSONSchema(s, params)
const accepts = (schema: any, v: unknown) => schema.safeParse(v).success

test('vb minItems with prefixItems is respected', () => {
  const s = fjs({ type: 'array', prefixItems: [{ type: 'string' }], minItems: 1 })
  assert.strictEqual(accepts(s, []), false)
  assert.strictEqual(accepts(s, ['a']), true)
})

test('vb additionalProperties false respects patternProperties', () => {
  const s = fjs({ type: 'object', patternProperties: { '^s_': { type: 'string' } }, additionalProperties: false })
  assert.strictEqual(accepts(s, { s_a: 'v' }), true)
  assert.strictEqual(accepts(s, { other: 'v' }), false)
})

test('vb plain refs, objects and numeric bounds are unchanged', () => {
  assert.strictEqual(accepts(fjs({ $defs: { plain: { type: 'number' } }, $ref: '#/$defs/plain' }), 5), true)
  assert.strictEqual(accepts(fjs({ type: 'object', properties: { a: { type: 'string' } }, required: ['a'] }), { a: 'x' }), true)
  const n = fjs({ type: 'number', minimum: 5 })
  assert.strictEqual(accepts(n, 5), true)
  assert.strictEqual(accepts(n, 4), false)
})

test('vb date-time accepts RFC 3339 numeric offsets', () => {
  assert.strictEqual(accepts(fjs({ type: 'string', format: 'date-time' }), '2026-08-24T10:00:00+02:00'), true)
})

test('vb draft-04 boolean exclusive bounds behave as exclusive', () => {
  const lo = fjs({ type: 'number', minimum: 5, exclusiveMinimum: true }, { target: 'draft-4' })
  assert.strictEqual(accepts(lo, 5), false)
  assert.strictEqual(accepts(lo, 6), true)
  const hi = fjs({ type: 'number', maximum: 5, exclusiveMaximum: true }, { target: 'draft-4' })
  assert.strictEqual(accepts(hi, 5), false)
  assert.strictEqual(accepts(hi, 4), true)
})
