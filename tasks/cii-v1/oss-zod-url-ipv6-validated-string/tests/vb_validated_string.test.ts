// Hidden fail-to-pass tests: url() must return the string the parser actually
// validated (WHATWG parsers delete tab/LF/CR instead of failing), and
// ipv6()/cidrv6() must reject anything outside the address alphabet.
import { test } from 'node:test'
import assert from 'node:assert'
import * as z from './packages/zod/src/v4/index'
import { compile } from './packages/zod/src/v4/core/index'

test('vb url returns the tab-newline-stripped string it validated', () => {
  assert.equal(z.url().parse('https://exa\nmple.com'), 'https://example.com')
  assert.equal(z.url().parse('https://exa\tmple.com'), 'https://example.com')
  assert.equal(z.url().parse('https://exa\rmple.com'), 'https://example.com')
  assert.equal(z.url().parse('https://example.com/a\nb?c=\td#e'), 'https://example.com/ab?c=d#e')
})

test('vb ipv6 rejects re-delimiting characters', () => {
  assert.equal(z.ipv6().safeParse('::@1\\').success, false)
  assert.equal(z.ipv6().safeParse('::]/1').success, false)
  assert.equal(z.ipv6().safeParse('@[::1').success, false)
  assert.equal(z.cidrv6().safeParse('::@1\\/64').success, false)
})

test('vb ipv6 rejects control characters', () => {
  for (const c of ['\t', '\n', '\r']) {
    assert.equal(z.ipv6().safeParse(`2001:db8::${c}1`).success, false)
    assert.equal(z.ipv6().safeParse(`::1${c}`).success, false)
  }
})

test('vb compiled url agrees on the validated string', () => {
  const schema = compile(z.url())
  assert.equal(schema.parse('https://exa\nmple.com'), 'https://example.com')
})
