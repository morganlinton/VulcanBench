// Hidden test for oss-hono-client-header-merge (honojs/hono PR #5092, fixes #5089).
// When a client is created with client-level `headers` AND a per-request
// `headers` is also given, BOTH sets must be sent. At base, deepMerge lets one
// replace the other so only a single set survives. The fix merges them (per-key,
// with per-request taking precedence). Every test here fails at base.
// Headers are captured via a custom `fetch` (no network / no msw). Run with `tsx --test`.

import { test } from 'node:test'
import assert from 'node:assert'
import { hc } from './src/client'

const capture = (out: Record<string, string | null>) => (_input: any, init: any) => {
  const h = new Headers(init?.headers)
  out.hono = h.get('x-hono')
  out.dyn = h.get('x-dynamic')
  return Promise.resolve(
    new Response('{}', { status: 200, headers: { 'content-type': 'application/json' } })
  )
}

test('vb merges function client headers with object request headers', async () => {
  const seen: Record<string, string | null> = {}
  const client: any = hc('http://localhost', {
    headers: async () => ({ 'x-hono': 'hono' }),
    fetch: capture(seen) as any,
  })
  await client.posts.$post({}, { headers: { 'x-dynamic': 'request' } })
  assert.strictEqual(seen.hono, 'hono')
  assert.strictEqual(seen.dyn, 'request')
})

test('vb merges object client headers with function request headers', async () => {
  const seen: Record<string, string | null> = {}
  const client: any = hc('http://localhost', {
    headers: { 'x-hono': 'hono' },
    fetch: capture(seen) as any,
  })
  await client.posts.$post({}, { headers: async () => ({ 'x-dynamic': 'request' }) })
  assert.strictEqual(seen.hono, 'hono')
  assert.strictEqual(seen.dyn, 'request')
})

test('vb merges function client headers with function request headers', async () => {
  const seen: Record<string, string | null> = {}
  const client: any = hc('http://localhost', {
    headers: () => ({ 'x-hono': 'hono' }),
    fetch: capture(seen) as any,
  })
  await client.posts.$post({}, { headers: () => ({ 'x-dynamic': 'request' }) })
  assert.strictEqual(seen.hono, 'hono')
  assert.strictEqual(seen.dyn, 'request')
})
