// Hidden fail-to-pass tests: string length checks must count Unicode code
// points, not UTF-16 code units — in both the interpreted checks and the
// compiled fast path. Public API only.
import { test } from 'node:test'
import assert from 'node:assert'
import * as z from './packages/zod/src/v4/index'
import { compile } from './packages/zod/src/v4/core/index'

// Five faces: five code points, ten UTF-16 units.
const FIVE = '\u{1F600}\u{1F600}\u{1F600}\u{1F600}\u{1F600}'

test('vb max counts code points for astral characters', () => {
  assert.equal(z.string().max(5).safeParse(FIVE).success, true)
  assert.equal(z.string().max(5).safeParse(FIVE + '\u{1F600}').success, false)
})

test('vb min counts code points for astral characters', () => {
  assert.equal(z.string().min(5).safeParse(FIVE).success, true)
  assert.equal(z.string().min(5).safeParse('\u{1F600}\u{1F600}\u{1F600}\u{1F600}').success, false)
})

test('vb length counts code points for astral characters', () => {
  assert.equal(z.string().length(5).safeParse(FIVE).success, true)
  assert.equal(z.string().length(1).safeParse('\u{1F600}').success, true)
})

test('vb code points not graphemes: combining marks and ZWJ stay several', () => {
  // "é" as e + combining acute: two code points.
  assert.equal(z.string().length(2).safeParse('é').success, true)
  // person + ZWJ + baby bottle: three code points.
  assert.equal(z.string().length(3).safeParse('\u{1F9D1}‍\u{1F37C}').success, true)
})

test('vb compiled max counts code points', () => {
  const schema = compile(z.string().max(5))
  assert.equal(schema.safeParse(FIVE).success, true)
  assert.equal(schema.safeParse(FIVE + '\u{1F600}').success, false)
})

test('vb compiled length counts code points', () => {
  const schema = compile(z.string().length(5))
  assert.equal(schema.safeParse(FIVE).success, true)
  assert.equal(schema.safeParse('\u{1F600}\u{1F600}').success, false)
})
