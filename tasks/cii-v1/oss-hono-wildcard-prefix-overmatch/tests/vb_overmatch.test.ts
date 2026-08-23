// Hidden fail-to-pass tests: LinearRouter and PatternRouter must not let
// wildcard routes overmatch bare prefixes. Exercised directly (SmartRouter
// would pick RegExpRouter and mask the defect).
import { test } from 'node:test'
import assert from 'node:assert'
import { LinearRouter } from './src/router/linear-router/index.ts'
import { PatternRouter } from './src/router/pattern-router/index.ts'

const count = (router: any, method: string, path: string): number =>
  router.match(method, path)[0].length

test('vb linear slash wildcard requires a segment boundary', () => {
  const r = new LinearRouter<string>()
  r.add('GET', '/path/*', 'h')
  assert.equal(count(r, 'GET', '/pathfoo'), 0, '/pathfoo must not match /path/*')
})

test('vb pattern slash wildcard requires a segment boundary', () => {
  const r = new PatternRouter<string>()
  r.add('GET', '/path/*', 'h')
  assert.equal(count(r, 'GET', '/pathfoo'), 0, '/pathfoo must not match /path/*')
})

test('vb linear suffix wildcard does not match a shorter prefix', () => {
  const r = new LinearRouter<string>()
  r.add('GET', '/assets*', 'h')
  assert.equal(count(r, 'GET', '/asset'), 0, '/asset must not match /assets*')
})

test('vb pattern suffix wildcard does not match a shorter prefix', () => {
  const r = new PatternRouter<string>()
  r.add('GET', '/assets*', 'h')
  assert.equal(count(r, 'GET', '/asset'), 0, '/asset must not match /assets*')
})
