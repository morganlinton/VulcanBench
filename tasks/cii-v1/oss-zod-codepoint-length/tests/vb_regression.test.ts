// Hidden pass-to-pass guards: ASCII strings and non-string lengthables are
// unchanged.
import { test } from 'node:test'
import assert from 'node:assert'
import * as z from './packages/zod/src/v4/index'
import { compile } from './packages/zod/src/v4/core/index'

const FIVE = '\u{1F600}\u{1F600}\u{1F600}\u{1F600}\u{1F600}'

test('vb reported bound stays the declared one', () => {
  // Overflowing in units and in code points reports the declared bound alike;
  // this must hold before and after the counting change.
  const res = z.string().max(5).safeParse(FIVE + '\u{1F600}')
  assert.equal(res.success, false)
  const issue = res.error!.issues[0] as { code: string; maximum: number; origin: string }
  assert.equal(issue.code, 'too_big')
  assert.equal(issue.maximum, 5)
  assert.equal(issue.origin, 'string')
})

test('vb ascii bounds unchanged', () => {
  assert.equal(z.string().max(5).safeParse('hello').success, true)
  assert.equal(z.string().max(5).safeParse('hello!').success, false)
  assert.equal(z.string().min(2).safeParse('hi').success, true)
  assert.equal(z.string().min(2).safeParse('h').success, false)
  assert.equal(z.string().length(3).safeParse('abc').success, true)
  assert.equal(z.string().length(3).safeParse('abcd').success, false)
})

test('vb arrays still count elements', () => {
  assert.equal(z.array(z.string()).max(2).safeParse(['a', 'b', 'c']).success, false)
  assert.equal(z.array(z.string()).length(2).safeParse(['\u{1F600}', '\u{1F600}']).success, true)
})

test('vb nonempty on plain strings unchanged', () => {
  assert.equal(z.string().nonempty().safeParse('x').success, true)
  assert.equal(z.string().nonempty().safeParse('').success, false)
})

test('vb compiled ascii bounds unchanged', () => {
  const schema = compile(z.string().min(2).max(5))
  assert.equal(schema.safeParse('abc').success, true)
  assert.equal(schema.safeParse('a').success, false)
  assert.equal(schema.safeParse('abcdef').success, false)
})
