// Hidden fail-to-pass tests: RegExpRouter must associate wildcard middleware
// with the routes it covers — suffix wildcards without a slash, and trailing
// wildcards after (patterned) labels across different parameter names.
//
// These exercise RegExpRouter directly: Hono's SmartRouter would fall back to
// another router and mask the defect.
import { test } from 'node:test'
import assert from 'node:assert'
import { RegExpRouter } from './src/router/reg-exp-router/index.ts'
import type { ParamIndexMap, Params } from './src/router/index.ts'

const matchAll = (router: RegExpRouter<string>, method: string, path: string) => {
  const [matchRes, stash] = router.match(method, path)
  return matchRes.map((r) =>
    stash
      ? {
          handler: r[0],
          params: Object.keys(r[1]).reduce((acc, key) => {
            acc[key] = stash[(r[1] as ParamIndexMap)[key]]
            return acc
          }, {} as Params),
        }
      : { handler: r[0], params: r[1] as Params }
  )
}

test('vb suffix wildcard without slash associates with covered route', () => {
  const router = new RegExpRouter<string>()
  router.add('POST', '/assets*', 'middleware')
  router.add('POST', '/assets/app.js', 'handler')
  const res = matchAll(router, 'POST', '/assets/app.js')
  assert.deepStrictEqual(
    res.map((r) => r.handler),
    ['middleware', 'handler']
  )
})

test('vb suffix wildcard without slash associates in reverse order', () => {
  const router = new RegExpRouter<string>()
  router.add('POST', '/assets/app.js', 'handler')
  router.add('POST', '/assets*', 'middleware')
  const res = matchAll(router, 'POST', '/assets/app.js')
  assert.deepStrictEqual(
    res.map((r) => r.handler),
    ['handler', 'middleware']
  )
})

test('vb label wildcard middleware crosses param names', () => {
  const router = new RegExpRouter<string>()
  router.add('GET', '/:name/*', 'middleware')
  router.add('GET', '/:id', 'handler')
  const res = matchAll(router, 'GET', '/abc')
  assert.strictEqual(res.length, 2)
  assert.deepStrictEqual(res[0], { handler: 'middleware', params: { name: 'abc' } })
  assert.deepStrictEqual(res[1], { handler: 'handler', params: { id: 'abc' } })
})

test('vb nested-brace pattern wildcard middleware crosses param names', () => {
  const router = new RegExpRouter<string>()
  router.add('GET', '/posts/:year{[0-9]{4}}/*', 'middleware')
  router.add('GET', '/posts/:yr{[0-9]{4}}/comments', 'handler')
  const res = matchAll(router, 'GET', '/posts/2024/comments')
  assert.deepStrictEqual(res, [
    { handler: 'middleware', params: { year: '2024' } },
    { handler: 'handler', params: { yr: '2024' } },
  ])
})

test('vb regexp-meta pattern wildcard middleware crosses param names', () => {
  const router = new RegExpRouter<string>()
  router.add('GET', '/files/:kind{(?:foo|bar)}/*', 'middleware')
  router.add('GET', '/files/:type{(?:foo|bar)}/detail', 'handler')
  const res = matchAll(router, 'GET', '/files/foo/detail')
  assert.deepStrictEqual(res, [
    { handler: 'middleware', params: { kind: 'foo' } },
    { handler: 'handler', params: { type: 'foo' } },
  ])
})

test('vb default-pattern wildcard middleware in reverse registration order', () => {
  const router = new RegExpRouter<string>()
  router.add('GET', '/user/:id/profile', 'handler')
  router.add('GET', '/user/:userId{[^/]+}/*', 'middleware')
  const res = matchAll(router, 'GET', '/user/123/profile')
  assert.deepStrictEqual(res, [
    { handler: 'handler', params: { id: '123' } },
    { handler: 'middleware', params: { userId: '123' } },
  ])
})
