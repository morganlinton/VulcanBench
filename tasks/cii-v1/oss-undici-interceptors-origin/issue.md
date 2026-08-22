# cache() and deduplicate() interceptors are silently inert on Client/Pool

Composing the `cache()` or `deduplicate()` interceptor onto a `Client` or
`Pool` looks like it works — requests succeed — but neither interceptor
does anything:

```js
const client = new Client(origin).compose(interceptors.cache())
await client.request({ path: '/', method: 'GET' })
await client.request({ path: '/', method: 'GET' })
// the server is hit twice; nothing was cached
```

(Upstream report: nodejs/undici#5613.) `Client` and `Pool` dispatch with no
`opts.origin` — they already know their origin — and both interceptors bail
out immediately when `opts.origin` is absent, so they pass every request
straight through. The same absence makes the cache-key helper throw if it
ever gets that far.

Expected: when composed onto a `Client` or `Pool`, `cache()` serves
repeated cacheable GETs from cache (one server hit) and `deduplicate()`
coalesces concurrent identical requests (one server hit, all callers get
the response). Cache keys must tolerate a missing origin. Behavior
elsewhere is unchanged: plain dispatch untouched, non-cacheable methods
(e.g. POST) still hit the server every time, and origin-carrying dispatch
paths keep working as today.
