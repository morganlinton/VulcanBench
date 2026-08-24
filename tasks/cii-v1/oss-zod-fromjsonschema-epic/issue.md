# `z.fromJSONSchema` conformance audit: four failing families

We generate Zod validators from third-party JSON Schemas and run the
result against a conformance corpus. Four families currently misbehave:

**1. Tuples reject trailing items they must accept.** In draft 2020-12,
`prefixItems` without `items` leaves the array *open*: extra elements
beyond the prefix are valid. We convert
`{"type":"array","prefixItems":[{"type":"string"},{"type":"number"}]}`
and `["a", 1, "extra"]` is rejected. (`"items": false` must of course
still close the tuple.)

**2. Escaped `$ref` pointers fail to resolve.** JSON Pointer escapes `/`
as `~1` and `~` as `~0`. A schema with `$defs` key `"a/b"` referenced as
`"#/$defs/a~1b"` throws "Reference not found". Same for `~0`.

**3. `propertyNames` is ignored or mis-composed.** Standalone
`propertyNames` is not enforced at all (any key passes). Combined with
`properties` + `additionalProperties`, valid inputs are rejected while
inputs whose keys violate `propertyNames` pass. Per the spec,
`propertyNames` constrains every key, independently of which other
keyword validates the value.

**4. `format: "hostname"` is not validated** — any string passes.

Everything else in the conversion must not regress: `minItems` with
`prefixItems`, `additionalProperties: false` with `patternProperties`,
plain `$ref`s, required properties, numeric bounds, `date-time` with
RFC 3339 numeric offsets, and draft-04 boolean `exclusiveMinimum` /
`exclusiveMaximum` handling.
