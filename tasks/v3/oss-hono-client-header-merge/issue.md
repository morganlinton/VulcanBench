Client-level headers are lost when a per-request `headers` is also given.

The Hono RPC client (`hc`) accepts client-level options, including `headers`
(which may be an object or a `() => headers` function, sync or async). Each
request can also pass its own `headers`. When BOTH are present, only one set
currently reaches the outgoing request: the per-request `headers` replaces the
client-level `headers` entirely instead of merging with them (see issue #5089).

```ts
const client = hc<AppType>('http://localhost', {
  headers: async () => ({ 'x-hono': 'hono' }),  // client-level
})
await client.posts.$post({}, { headers: { 'x-dynamic': 'request' } }) // per-request
// the outgoing request should carry BOTH x-hono AND x-dynamic
```

Fix the client (`src/client/client.ts`) so that client-level and per-request
headers are merged onto the outgoing request:

- When both client-level and per-request `headers` are present, the outgoing
  request includes the union of both.
- This must work for every combination of object and function (sync or async)
  headers on either side.
- When the same header key is set on both sides, the per-request value wins.
- When only one side sets headers (e.g. per-request only, with no client-level
  headers), that side's headers are sent unchanged.
