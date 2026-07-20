# tests/SKILL.md  -  vyre-spec

Read `../../.internals/skills/testing/SKILL.md` first for the category contract.

## Purpose

`vyre-spec` is the **frozen data contract**. It has no dependency on
`vyre`, `vyre-foundation`, or any backend crate; any conformance
runner can use it as the stable contract for byte-identity proofs.
Every type here is versioned wire surface  -  change discipline is the
whole point.

## Critical invariants

- **Every enum is `#[non_exhaustive]` and has an `Opaque(ExtensionXxxId)`
  escape hatch.** Future variants must never break downstream matches.
- **Every tag allocation is stable.** `BinOp::Add` is `0x00` forever;
  renumbering is a wire-format break and requires a major bump.
- **Every `const fn` returns deterministic values.** `DataType::min_bytes`,
  `size_bytes`, etc. are inputs to catalog generation.
- **Zero runtime state.** No `static`, no `OnceLock`, no lazy init.

## Adversarial surface

- Unknown opaque extension id → decoder returns structured
  `UnknownDiscriminant`, never panics
- Every numeric boundary for every `u32`/`u64` field in every
  descriptor
- Malformed string / non-UTF-8 in `&'static str` positions of op ids

## Current gaps

- (fill after reading `vyre-spec/Cargo.toml` + README)  -  promising
  features not yet implemented, paths into `gap.rs`

## Cross-crate contracts

- `DataType`, `BinOp`, `UnOp`, `AtomicOp`, `RuleCondition`  - 
  consumed by `vyre-foundation`, `vyre-driver`, every `vyre-ops/*`
  dialect
- `OpSignature`, `OpMetadata`, `IntrinsicDescriptor`  -  consumed by
  conform runner + catalog generators in `conform/`
- `BackendId`, `Backend`  -  consumed by `vyre-driver` and
  `vyre-driver-wgpu`

## Bench targets

- `const fn` evaluation is compile-time; no runtime bench needed
- Hash + equality for `BackendId`, `ExtensionXxxId`  -  micro-benches
  bound at nanoseconds

## Fuzz targets

None required  -  `vyre-spec` is data-only, no byte-in / byte-out API
surface. Fuzzing happens at `vyre-foundation::serial::wire` where
the spec's tag table meets the decoder.

## What NOT to test here

- Wire encode/decode  -  that lives in `vyre-foundation` tests
- Backend dispatch semantics  -  `vyre-driver-wgpu` and `vyre-reference`
- Op lowering  -  `vyre-ops` and the backend crates

## Running

```bash
./cargo_full test -p vyre-spec
./cargo_full test -p vyre-spec --test adversarial
./cargo_full test -p vyre-spec --test property
./cargo_full test -p vyre-spec --test gap
./cargo_full test -p vyre-spec --test integration
```
