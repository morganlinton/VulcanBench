HonoRequest is missing a `bytes()` body parser.

`HonoRequest` exposes body parsers mirroring the fetch `Request` API:
`text()`, `json()`, `arrayBuffer()`, `blob()`, `formData()`. The standard
`Request` interface also has `bytes()`, which resolves the body as a
`Uint8Array`, but `HonoRequest` does not implement it, so handlers have to
write `new Uint8Array(await c.req.arrayBuffer())` by hand.

Add `bytes(): Promise<Uint8Array>` to `HonoRequest` (in `src/request.ts`):

```ts
app.post('/entry', async (c) => {
  const body = await c.req.bytes() // Uint8Array
})
```

Requirements:

- Resolves the request body as a `Uint8Array` (byte-exact for binary bodies;
  an empty body resolves to an empty `Uint8Array`).
- Must go through the same cached-body mechanism as the other parsers, so the
  body can be read multiple times: calling `bytes()` after `text()`, or
  `bytes()` twice, must work and return the same content (a plain
  `this.raw.bytes()` would fail once the stream is consumed).
- Existing parsers must be unchanged.
