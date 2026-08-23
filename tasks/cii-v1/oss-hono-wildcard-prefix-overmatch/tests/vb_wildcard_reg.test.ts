// Hidden pass-to-pass guards: legitimate wildcard matches unchanged.
import { test } from 'node:test'
import assert from 'node:assert'
import { LinearRouter } from './src/router/linear-router/index.ts'
import { PatternRouter } from './src/router/pattern-router/index.ts'

const count = (router: any, method: string, path: string): number =>
  router.match(method, path)[0].length

for (const [name, R] of [
  ['linear', LinearRouter],
  ['pattern', PatternRouter],
] as const) {
  test(`vb ${name} slash wildcard matches the bare prefix and sub paths`, () => {
    const r = new (R as any)()
    r.add('GET', '/path/*', 'h')
    assert.equal(count(r, 'GET', '/path'), 1)
    assert.equal(count(r, 'GET', '/path/to/file'), 1)
  })

  test(`vb ${name} suffix wildcard matches prefix and extensions`, () => {
    const r = new (R as any)()
    r.add('GET', '/assets*', 'h')
    assert.equal(count(r, 'GET', '/assets'), 1)
    assert.equal(count(r, 'GET', '/assets-v2'), 1)
    assert.equal(count(r, 'GET', '/assets/app.js'), 1)
  })

  test(`vb ${name} static and param routes unchanged`, () => {
    const r = new (R as any)()
    r.add('GET', '/users/:id', 'u')
    const res = r.match('GET', '/users/42')
    assert.equal(res[0].length, 1)
  })
}
