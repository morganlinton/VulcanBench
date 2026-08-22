// Hidden pass-to-pass guards: existing TrieRouter matching unchanged.
import { test } from 'node:test'
import assert from 'node:assert'
import { TrieRouter } from './src/router/trie-router/index.ts'

type Match = [string, Record<string, string>][]
const handlers = (m: unknown): Match => (m as [Match])[0]

test('vb slash wildcard still matches sub paths', () => {
  const router = new TrieRouter<string>()
  router.add('GET', '/assets/*', 'assets')
  assert.strictEqual(handlers(router.match('GET', '/assets/app.js')).length, 1)
  assert.strictEqual(handlers(router.match('GET', '/other')).length, 0)
})

test('vb params still captured', () => {
  const router = new TrieRouter<string>()
  router.add('GET', '/users/:id', 'user')
  const res = handlers(router.match('GET', '/users/42'))
  assert.strictEqual(res.length, 1)
  assert.deepStrictEqual({ ...res[0][1] }, { id: '42' })
})

test('vb static routes still exact', () => {
  const router = new TrieRouter<string>()
  router.add('GET', '/about', 'about')
  assert.strictEqual(handlers(router.match('GET', '/about')).length, 1)
  assert.strictEqual(handlers(router.match('GET', '/about/us')).length, 0)
})

// Unregistered shorter prefix was already unmatched at base: kept as a guard.
test('vb suffix wildcard does not match a shorter prefix', () => {
  const router = new TrieRouter<string>()
  router.add('GET', '/assets*', 'assets')
  assert.strictEqual(handlers(router.match('GET', '/asset')).length, 0)
})
