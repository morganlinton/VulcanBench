// Hidden pass-to-pass guards: existing shortcuts unchanged.
import { test } from 'node:test'
import assert from 'node:assert'
import http from 'node:http'
import ky from './source/index.ts'

const withServer = async (fn: (base: string, seen: { method?: string }) => Promise<void>) => {
  const seen: { method?: string } = {}
  const server = http.createServer((req, res) => {
    seen.method = req.method
    req.resume()
    req.on('end', () => {
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

test('vb ky.get works', async () => {
  await withServer(async (base, seen) => {
    const res = await ky.get(base + '/')
    assert.equal((await res.json() as { ok: boolean }).ok, true)
    assert.equal(seen.method, 'GET')
  })
})

test('vb ky.post uppercases and sends', async () => {
  await withServer(async (base, seen) => {
    await ky(base + '/', { method: 'post' })
    assert.equal(seen.method, 'POST')
  })
})

test('vb error status throws HTTPError', async () => {
  const server = http.createServer((req, res) => { res.statusCode = 500; res.end('x') })
  await new Promise<void>((resolve) => server.listen(0, '127.0.0.1', () => resolve()))
  const address = server.address() as { port: number }
  try {
    await assert.rejects(ky.get(`http://127.0.0.1:${address.port}/`, { retry: 0 }))
  } finally {
    server.close()
  }
})
