// Hidden fail-to-pass tests: TrieRouter must match suffix wildcard routes
// (a trailing * with no preceding slash, e.g. /assets*).
//
// These exercise TrieRouter directly: SmartRouter would pick RegExpRouter,
// which already handles the case, and mask the defect.
import { test } from 'node:test'
import assert from 'node:assert'
import { TrieRouter } from './src/router/trie-router/index.ts'

type Match = [string, Record<string, string>][]
const handlers = (m: unknown): Match => (m as [Match])[0]

test('vb suffix wildcard matches the bare prefix and extensions', () => {
  const router = new TrieRouter<string>()
  router.add('GET', '/assets*', 'assets')
  for (const path of ['/assets', '/assets-v2', '/assets/app.js']) {
    const res = handlers(router.match('GET', path))
    assert.strictEqual(res.length, 1, `expected a match for ${path}`)
    assert.strictEqual(res[0][0], 'assets')
  }
})

test('vb regex characters in the prefix stay literal', () => {
  const router = new TrieRouter<string>()
  router.add('GET', '/file.+*', 'file')
  assert.strictEqual(handlers(router.match('GET', '/file.+js')).length, 1)
  assert.strictEqual(handlers(router.match('GET', '/fileZZjs')).length, 0)
})

test('vb suffix wildcard registers when its child already exists', () => {
  const router = new TrieRouter<string>()
  router.add('GET', '/assets*/x', 'literal')
  router.add('GET', '/assets*', 'assets')
  const res = handlers(router.match('GET', '/assets/app.js'))
  assert.strictEqual(res.length, 1)
  assert.strictEqual(res[0][0], 'assets')
})
