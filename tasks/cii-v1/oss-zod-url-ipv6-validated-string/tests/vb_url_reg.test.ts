// Hidden pass-to-pass guards: clean inputs unchanged.
import { test } from 'node:test'
import assert from 'node:assert'
import * as z from './packages/zod/src/v4/index'
import { compile } from './packages/zod/src/v4/core/index'

test('vb clean urls still parse verbatim', () => {
  assert.equal(z.url().parse('https://example.com/path'), 'https://example.com/path')
  assert.equal(z.url().safeParse('not a url').success, false)
})

test('vb normalize still wins where it applies', () => {
  assert.equal(z.url({ normalize: true }).parse('https://example.com'), 'https://example.com/')
})

test('vb valid ipv6 addresses still accepted', () => {
  assert.equal(z.ipv6().safeParse('::1').success, true)
  assert.equal(z.ipv6().safeParse('2001:db8::1').success, true)
  assert.equal(z.cidrv6().safeParse('2001:db8::/32').success, true)
})

test('vb compiled clean url unchanged', () => {
  const schema = compile(z.url())
  assert.equal(schema.parse('https://example.com/x'), 'https://example.com/x')
})
