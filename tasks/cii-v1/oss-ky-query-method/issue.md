# Add QUERY method support

The HTTP QUERY method (a safe, idempotent method that conveys a request
body) has no first-class support:

```ts
ky.query('https://example.com/search', {json: {q: 'select 1'}})
// TypeError: ky.query is not a function

await ky('https://example.com/search', {method: 'query'})
// sends the literal lowercase method "query" over the wire
```

Wanted:

- A `ky.query()` shortcut alongside `ky.get`/`ky.post`/etc., returning the
  usual response promise with body methods; `json:` bodies work.
- `query` joins the standard methods that are uppercased before sending
  (servers treat method tokens case-sensitively), so `{method: 'query'}`
  goes out as `QUERY`.
- QUERY is safe and idempotent, so it belongs in the default retryable
  methods.
- Types are updated accordingly (shortcut, method unions, retry defaults).
- Existing shortcuts, uppercasing of the other standard methods, and
  HTTPError behavior are unchanged.
