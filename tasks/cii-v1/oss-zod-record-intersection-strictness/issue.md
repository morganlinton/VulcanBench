# Intersecting an object with a record rejects keys the record should not govern

Long-standing complaint (reported independently at least four times). In
TypeScript, `{name: string} & Record<`S_${string}`, string>` means the index
signature constrains only the keys that match it — `name` is legal because
the object half declares it. `z.infer` agrees. The parser does not:

```ts
z.object({ name: z.string() })
  .and(z.record(z.string().regex(/^S_/), z.string()))
  .parse({ name: "a", S_a: "s" });
// expected: { name: "a", S_a: "s" }
// actual:   throws — the record half reports "name" as an invalid key
```

So the runtime rejects values whose type the schema itself infers as valid.
The same happens for template-literal key schemas.

The required behavior, matching TypeScript's treatment of index signatures:

- Inside an intersection, a record's key schema governs only the keys it
  matches. Keys another operand declares are that operand's business.
- A key the record schema DOES match, carrying a wrong value type, is still
  an error — reported as a value error on that key.
- A refinement, check, or transform attached to one operand must still run
  even when the input carries a key that operand alone would reject as
  unrecognized (the overall parse can still fail — but the operand's own
  logic must have executed).
- Standalone records are unchanged: a key outside the key schema remains an
  error, like TypeScript's excess-property check on a fresh object literal.
- Exhaustiveness rules for enum-keyed records, strict-object behavior,
  intersection reconciliation of unrecognized keys, and pipe semantics must
  all keep their current behavior.
