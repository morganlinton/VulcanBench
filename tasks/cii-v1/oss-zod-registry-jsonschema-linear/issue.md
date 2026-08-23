# Converting a large registry to JSON Schema is unusably slow

`z.toJSONSchema(registry)` becomes unusable as the registry grows. A user
with ~3000 registered types reports the call taking around 30 seconds;
smaller registries are fine, and the time grows far faster than the number
of schemas — roughly quadrupling each time the registry doubles.

```ts
const reg = z.registry<{ id: string }>()
for (let i = 0; i < 3000; i++) {
  reg.add(z.object({ a: z.string(), b: z.number() }), { id: `S${i}` })
}
z.toJSONSchema(reg, { uri: (id) => `#/defs/${id}` })  // ~30 seconds
```

Make the conversion scale linearly in the number of registered schemas.

The emitted document must not change in any way: every registered schema
still appears under its id, schemas shared between entries are still
emitted once and referenced with `$ref` rather than copied, the `uri`
callback still determines `$id` and reference targets, field types,
`required` lists and `additionalProperties` are unchanged, and
single-schema `toJSONSchema(schema)` conversion is untouched.
