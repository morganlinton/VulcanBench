# exactOptional: an absent key materializes a value invented from `undefined`

`exactOptional()` is documented as "absence permitted, nothing supplied in
its place". But when the key is absent and the inner schema does something
with `undefined`, the parsed output contains a value that was never in the
input:

```ts
z.object({ a: z.coerce.string().exactOptional() }).parse({})
// -> { a: "undefined" }   (should be {})

z.object({ a: z.string().catch('C').exactOptional() }).parse({})
// -> { a: "C" }           (should be {})

z.tuple([z.string(), z.coerce.string().exactOptional()]).parse(['x'])
// -> ['x', 'undefined']   (should be ['x'])
```

An absent key on this middle rung must contribute nothing, however the
inner schema answers `undefined` — coerce, catch, preprocess, unions alike.
Absent trailing tuple slots on the middle rung truncate the tuple. The fix
must hold in every execution path: interpreted, `{ jitless: true }`, and
`z.compile()` (which assembles its own output and will otherwise silently
keep the invented value).

Everything else on the ladder keeps its meaning: `default`/`prefault`
still substitute on absence; an explicitly present `undefined` still runs
the inner schema; `catch` on a required key still substitutes; plain
`optional()` behavior is unchanged; and `exactOptional` still rejects an
explicitly present `undefined` for schemas that don't accept it.
