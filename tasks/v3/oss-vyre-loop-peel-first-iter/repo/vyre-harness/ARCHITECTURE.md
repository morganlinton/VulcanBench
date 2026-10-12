# vyre-harness  -  architecture

Test + bench harness shared across vyre crates. Owns
`region::tag_program` (the conformance-region wrapper helper) and
the canonical fixture corpora.

## Modules

### `lib.rs`
Public re-exports. The test corpora and helpers downstream crates
need.

### `region.rs`
`tag_program(op_id, program) → Program`  -  wraps a Program's entry
in a `Node::Region { generator: op_id, source_region: ..., body }`
so the conform runner knows which op produced which output.

## Public types

- **`region::tag_program`**  -  region-wrapping helper.
- **(future)** `corpora::standard`  -  the standard fixture set
  every bench/test consumes. Currently expected by per-pass
  benches; lands as the surface stabilises.

## Integration points

- Consumed by every `tests/` and `benches/` directory in vyre.
- Used by the conform runner to wrap probe outputs in the
  generator-tagged Region the certificate hash covers.
