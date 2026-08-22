# String length checks count UTF-16 code units, not characters

`z.string().min/max/length` measure `input.length`, i.e. UTF-16 code units.
Any character outside the Basic Multilingual Plane counts double:

```ts
z.string().length(1).safeParse('😀').success // false — but it's one character
z.string().max(5).safeParse('😀😀😀😀😀').success // false — five characters
```

Length should be measured in **Unicode code points**: astral emoji count as
one; combining marks and ZWJ sequences still count as several (code points,
not graphemes — `'é'` is 2, `'🧑‍🍼'` is 3). This applies to `min`,
`max` and `length` on strings, in both the interpreted checks and the
compiled fast path.

Notes:

- Non-string lengthables (arrays, etc.) keep counting elements.
- ASCII behavior is unchanged, and error issues keep reporting the declared
  bound as today.
- Mind the fast path: a code point is one or two units, so the exact count
  only has to be computed when the unit count leaves the verdict in doubt —
  don't put a full scan on every parse.
