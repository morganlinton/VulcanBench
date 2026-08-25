# tests/SKILL.md  -  vyre-foundation

Read `../../.internals/skills/testing/SKILL.md` first for the category contract.

## Purpose

`vyre-foundation` is the **IR + wire format + visitors + validator +
optimizer framework**. Every program lives here; every decoder
starts here; every optimizer pass runs here. Zero dependency on
naga / wgpu / toml  -  foundation tier.

## Critical invariants

- **Wire round-trip.** `from_wire(to_wire(p)) == p` for every
  program `p` that passes `validate`. Encoder + decoder must stay
  in lockstep through every `#[non_exhaustive]` variant addition.
- **Unknown tags never panic.** Every byte in every position of a
  decoder input must either decode cleanly or return a structured
  `UnknownDiscriminant`. Random bytes through the decoder must
  never abort the process.
- **Opaque round-trip preservation.** Unknown extension bytes
  (`0x80` opaque path) are preserved byte-identically even when
  the decoder does not link the extension crate.
- **Optimizer preserves semantics.** `eval(p) == eval(optimize(p))`
  for every valid `p`.
- **Visitor default methods surface errors, never silently skip.**
  See `../../.internals/skills/testing/property.md` for the parity contract.

## Adversarial surface

- Truncated wire payload (magic but truncated body)
- Oversized length prefix pointing past EOF
- Unknown tag in reserved-unallocated range vs allocated opaque
- Null bytes / invalid UTF-8 in `Ident`
- Program with a cycle in `Loop` / `Block` references
- Extreme nesting (10 000 nested `Block`s)  -  stack-safety of the
  explicit walker
- Opaque extension with arbitrary random payload_len

## Current gaps

- `tests/opaque_wire_round_trip.rs` already exists  -  audit which
  invariants it covers vs which are missing; move missing ones to
  `gap.rs` with a citation.
- Optimizer passes: check whether `pass(pass(p)) == pass(p)` is
  proven for every pass. If not, each unproven pass gets a failing
  gap test.
- Validator: every error variant the spec implies should be
  reachable by at least one test input; gaps land in `gap.rs`.

## Cross-crate contracts

- `Program`, `Expr`, `Node`, `BufferDecl`, `MemoryKind`,
  `BufferAccess`  -  consumed by every backend + conform
- `ExprVisitor` / `NodeVisitor`  -  intended for lowering crates and
  the reference interpreter; see integration tier
- `LoweringTable` (moved from vyre-driver in 0.6)  -  consumed by
  `vyre-driver::registry`, `vyre-ops`, every backend
- `DialectLookup` trait  -  implemented by `vyre-driver::DialectRegistry`,
  consumed by `vyre-reference`

## Bench targets

- `to_wire` / `from_wire`  -  throughput across small / medium / large
  programs
- CSE `intern_expr`  -  per-call cost, measured against the v0.6
  smallvec baseline
- `walk_nodes` / `walk_exprs`  -  per-node cost on deep programs
- `DialectRegistry::lookup`  -  target < 10 ns per lookup
- `validate`  -  amortized cost per program (should be constant-time
  repeat via `is_validated` fast path)

## Fuzz targets

- `decode`  -  arbitrary bytes → `Program::from_wire` → no panic
- `round_trip`  -  `from_wire` → `to_wire` → `from_wire` → asserts
  byte-equal on re-encode
- `validate_fuzz`  -  arbitrary program shape via proptest strategy
  → `validate` never panics regardless of input

## What NOT to test here

- Concrete backend dispatch  -  the owning backend crate's tests
- Backend-specific lowering  -  the owning backend crate
- Op-specific semantics beyond `cpu_op::structured_intrinsic_cpu`
   -  `vyre-ops`

## Running

```bash
./cargo_full test -p vyre-foundation
./cargo_full test -p vyre-foundation --test adversarial
./cargo_full test -p vyre-foundation --test property
./cargo_full test -p vyre-foundation --test gap
./cargo_full test -p vyre-foundation --test integration
./cargo_full bench -p vyre-foundation
cd vyre-foundation/fuzz && ../../cargo_full fuzz run decode
```
