# A `.default()` is lost when a `.transform()` follows it under an optional wrapper

Regression report. This schema used to supply its default; now the key
silently disappears:

```ts
const arrayFromString = z
  .string()
  .default("")
  .transform((v) => (v ? v.split(",") : []));

z.object({ array: arrayFromString }).partial().parse({});
// expected: { array: [] }
// actual:   {}
```

The same schema under `.optional()` shows it too:

```ts
arrayFromString.optional().parse(undefined);
// expected: []
// actual:   undefined
```

`.prefault()` followed by a transform behaves the same way, and the async
variants (`safeParseAsync`, async transforms) mirror the sync behavior.

The default must flow through the transform chain when the wrapper receives
`undefined` or the object key is absent. At the same time, none of the
established optionality semantics may change. Two examples of behavior that
must stay exactly as it is today:

```ts
// undefined-acceptance that comes from preprocess/catch/transform does NOT
// make an optional wrapper delegate — it still short-circuits:
z.preprocess((v) => v ?? "X", z.string()).optional().parse(undefined); // undefined

// a default keeps winning regardless of wrapper order:
z.string().default("D").catch("C").optional().parse(undefined); // "D"
```

Object and tuple key admissibility, `.exactOptional()`, record/catchall
value handling, unions, lazy schemas, pipes, and `z.input`/`z.output`
inference must all keep their current behavior.
