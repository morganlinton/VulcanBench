# Invalid limit options are silently treated as "no limit"

Every parser accepts a `limit` option bounding the request body size. An
invalid value — a string `bytes` can't parse, `NaN`, a boolean, an object —
is silently converted to `null`, which disables the limit entirely:

```js
bodyParser.json({ limit: 'foo' })   // typo — bodies are now UNLIMITED
bodyParser.raw({ limit: NaN })      // same
```

A misconfigured limit failing open is the worst outcome: the option looks
set, but nothing is enforced.

Expected: middleware creation throws a `TypeError` for a `limit` that does
not parse as a size — for all four parsers (`json`, `raw`, `text`,
`urlencoded`). Valid values are unchanged: size strings (`'1mb'`) and
numbers are accepted and enforced (over-limit bodies still get 413), and
omitting the option keeps the `100kb` default.
