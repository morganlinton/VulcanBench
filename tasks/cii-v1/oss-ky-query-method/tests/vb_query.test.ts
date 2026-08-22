// Hidden fail-to-pass tests: ky QUERY method support. Public API via a
// loopback node:http server.
import { test } from 'node:test'
import assert from 'node:assert'
import http from 'node:http'
import ky from './source/index.ts'

const withServer = async (fn: (base: string, seen: { method?: string; body?: string }) => Promise<void>) => {
  const seen: { method?: string; body?: string } = {}
  const server = http.createServer((req, res) => {
    seen.method = req.method
    let buf = ''
    req.on('data', (c) => (buf += c))
    req.on('end', () => {
      seen.body = buf
      res.setHeader('content-type', 'application/json')
      res.end(JSON.stringify({ ok: true }))
    })
  })
  await new Promise<void>((resolve) => server.listen(0, '127.0.0.1', () => resolve()))
  const address = server.address() as { port: number }
  try {
    await fn(`http://127.0.0.1:${address.port}`, seen)
  } finally {
    server.close()
  }
}

test('vb ky.query sends a QUERY request', async () => {
  await withServer(async (base, seen) => {
    const res = await ky.query(base + '/')
    assert.equal((await res.json() as { ok: boolean }).ok, true)
    assert.equal(seen.method, 'QUERY')
  })
})

test('vb query method option is uppercased', async () => {
  await withServer(async (base, seen) => {
    await ky(base + '/', { method: 'query' })
    assert.equal(seen.method, 'QUERY')
  })
})

test('vb ky.query conveys a json body', async () => {
  await withServer(async (base, seen) => {
    await ky.query(base + '/', { json: { q: 'select 1' } })
    assert.equal(seen.method, 'QUERY')
    assert.deepEqual(JSON.parse(seen.body || '{}'), { q: 'select 1' })
  })
})
