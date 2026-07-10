Fix a prototype-pollution bug in object catchall parsing.

When a Zod object schema has a catchall (`z.object({...}).catchall(z.any())`),
unrecognized input keys are copied onto a fresh result object. The copy is done
with a plain assignment, `result[key] = value`. If the unrecognized key is
`__proto__` (for example from `JSON.parse('{"__proto__":{...}}')`, which
produces a real own `__proto__` property), the assignment goes through the
`__proto__` setter instead of creating an own property. This can replace the
result object's prototype, a prototype-pollution vector.

Fix `handleCatchall` (in `packages/zod/src/v4/core/schemas.ts`) so that a
`__proto__` key in the input is skipped during catchall iteration and never
assigned onto the result.

Required behavior after the fix:

- Parsing `{"a":1,"__proto__":{...},"extra":"e"}` (via `JSON.parse`) through
  `z.object({ a: z.number() }).catchall(z.any())` returns an object whose own
  keys are exactly `a` and `extra`; `__proto__` is dropped.
- The result's prototype remains `Object.prototype`, and nothing leaks onto the
  global `Object.prototype`.
- Because `__proto__` is skipped rather than collected, a `strictObject` that is
  given only its known keys plus a `__proto__` key parses successfully.
- Ordinary catchall and strict-object behavior for normal keys is unchanged
  (typed catchall values still validate; genuine unknown keys still make a
  strict object fail).
