# LinearRouter/PatternRouter: wildcard routes overmatch bare prefixes

Both `LinearRouter` and `PatternRouter` let wildcard routes match paths
that merely share a string prefix, with no segment boundary:

```ts
const r = new LinearRouter<string>()
r.add('GET', '/path/*', 'h')
r.match('GET', '/pathfoo') // matches — but /pathfoo is not under /path/

r.add('GET', '/assets*', 'a')
r.match('GET', '/asset')   // matches — /asset is shorter than the prefix
```

`RegExpRouter` gets both cases right, so apps only see the bug when the
smart router falls back — and then middleware/handlers fire on unrelated
routes.

Expected, in both routers:

- A slash wildcard `/path/*` matches `/path` and `/path/to/file`, but never
  `/pathfoo` — after the prefix there must be a segment boundary (end of
  path or `/`).
- A suffix wildcard `/assets*` matches `/assets`, `/assets-v2` and
  `/assets/app.js`, but never the shorter `/asset`.
- Static and `:param` routes are unchanged.
