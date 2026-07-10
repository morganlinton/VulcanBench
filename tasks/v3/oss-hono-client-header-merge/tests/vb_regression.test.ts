// pass_to_pass regression guard: behavior that holds before and after the fix.
// - per-request headers alone are still sent (no client-level headers);
// - when both set the SAME key, the per-request value wins.

import { test } from 'node:test'
import assert from 'node:assert'
import { hc } from './src/client'

const capture = (out: Record<string, string | null>) => (_input: any, init: any) => {
  const h = new Headers(init?.headers)
  out.dyn = h.get('x-dynamic')
  out.key = h.get('x-key')
  return Promise.resolve(
    new Response('{}', { status: 200, headers: { 'content-type': 'application/json' } })
  )
}

test('vb per-request headers and same-key precedence unaffected', async () => {
  const a: Record<string, string | null> = {}
  const c1: any = hc('http://localhost', { fetch: capture(a) as any })
  await c1.posts.$post({}, { headers: { 'x-dynamic': 'only' } })
  assert.strictEqual(a.dyn, 'only')

  const b: Record<string, string | null> = {}
  const c2: any = hc('http://localhost', { headers: { 'x-key': 'base' }, fetch: capture(b) as any })
  await c2.posts.$post({}, { headers: { 'x-key': 'req' } })
  assert.strictEqual(b.key, 'req')
})
