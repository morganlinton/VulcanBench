// Hidden pass-to-pass guards: RegExpRouter behavior that already worked must
// not regress.
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

test('vb slash wildcard middleware associates with covered route', () => {
  const router = new RegExpRouter<string>()
  router.add('GET', '/assets/*', 'middleware')
  router.add('GET', '/assets/app.js', 'handler')
  const res = matchAll(router, 'GET', '/assets/app.js')
  assert.deepStrictEqual(
    res.map((r) => r.handler),
    ['middleware', 'handler']
  )
})

test('vb label wildcard matches a sub path with its own param', () => {
  const router = new RegExpRouter<string>()
  router.add('GET', '/:name/*', 'middleware')
  const res = matchAll(router, 'GET', '/abc/sub')
  assert.deepStrictEqual(res, [{ handler: 'middleware', params: { name: 'abc' } }])
})

test('vb static and param routes match with own params', () => {
  const router = new RegExpRouter<string>()
  router.add('GET', '/about', 'static')
  router.add('GET', '/book/:id', 'param')
  assert.deepStrictEqual(matchAll(router, 'GET', '/about'), [
    { handler: 'static', params: {} },
  ])
  assert.deepStrictEqual(matchAll(router, 'GET', '/book/42'), [
    { handler: 'param', params: { id: '42' } },
  ])
})

test('vb unmatched path yields no handlers', () => {
  const router = new RegExpRouter<string>()
  router.add('GET', '/only/here', 'h')
  assert.strictEqual(matchAll(router, 'GET', '/nowhere').length, 0)
})
