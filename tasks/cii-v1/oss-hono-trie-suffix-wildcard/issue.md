# TrieRouter: suffix wildcard routes never match

A route registered with a trailing wildcard that has **no preceding slash**
(`/assets*`) never matches in `TrieRouter`:

```ts
const router = new TrieRouter<string>()
router.add('GET', '/assets*', 'assets')
router.match('GET', '/assets/app.js') // no handlers
router.match('GET', '/assets')        // no handlers
router.match('GET', '/assets-v2')     // no handlers
```

`RegExpRouter` handles this pattern, so apps only hit the bug when the smart
router falls back to `TrieRouter` (e.g. because another route forces the
fallback) — and then `/assets*` middleware/handlers silently stop applying.

Expected: `/assets*` matches the bare prefix (`/assets`), same-segment
extensions (`/assets-v2`), and deeper paths (`/assets/app.js`). It must not
match a shorter path (`/asset`). Regular-expression metacharacters in the
prefix are literal path characters (`/file.+*` matches `/file.+js`, not
`/fileZZjs`), and registration order must not matter when a node for the
prefix already exists.

Existing matching — `/assets/*` slash wildcards, `:param` captures, exact
static routes — is unchanged.
