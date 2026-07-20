# tests/SKILL.md  -  vyre-driver

Read `../../.internals/skills/testing/SKILL.md` first for the category contract.

## Purpose

`vyre-driver` is the **substrate-agnostic backend machinery**:
`VyreBackend` trait, `DialectRegistry`, routing, pipeline caching,
diagnostics. Every concrete backend crate depends on it. This crate
has no direct GPU / shader knowledge.

## Critical invariants

- **`VyreBackend` trait is frozen for 0.6.** Every existing backend
  must compile unchanged; every capability query + lifecycle hook
  has a conservative default.
- **Sealed trait via `__vyre_backend_sealed`.** External crates
  cannot implement `VyreBackend` without explicit opt-in.
- **`PipelineCacheKey` forms a total order over pipeline identity.**
  Two non-equivalent pipelines cannot collide; two equivalent
  pipelines must collide.
- **`DialectRegistry::global()` supports hot-reload with snapshot
  semantics.** Readers always see a complete snapshot, never a
  partial state.
- **`BackendError` variants are stable.** Renaming / removing a
  variant is a wire-ish break (downstream matches).

## Adversarial surface

- `VyreBackend` impl that returns `true` from every capability
  query but `dispatch` always errors  -  trait default paths should
  still compose
- `DialectRegistry::install` concurrent swap while readers are
  active  -  arc-swap semantics must hold
- `PipelineCacheKey` with `version != CURRENT`  -  lookup must miss,
  never falsely hit
- `DispatchConfig` with an insane `timeout` (near-zero, `u64::MAX`)
   -  must not panic

## Current gaps

- A true LRU for the pipeline cache (currently uses deterministic
  bounded eviction in concrete caches). Gap test: "cache
  keeps the hottest N pipelines after N+k insertions".
- Per-backend stats trait. Gap:
  add to VyreBackend as a defaulted method.

## Cross-crate contracts

- `VyreBackend` trait  -  implemented by concrete backend crates
- `DialectLookup` trait  -  implemented by `DialectRegistry`,
  consumed by `vyre-reference`
- `OpDef`, `LoweringTable`  -  re-exported from `vyre-foundation`;
  every `vyre-ops` dialect submits `OpDefRegistration`
- `CompiledPipeline`, `PendingDispatch`  -  implemented per-backend
- `BackendError`, `ErrorCode`  -  surfaced to every consumer

## Bench targets

- `DialectRegistry::lookup`  -  sub-10 ns per lookup
- `PipelineCacheKey` hash + eq  -  sub-100 ns
- `VyreBackend` virtual call overhead through `Arc<dyn VyreBackend>`

## Fuzz targets

Minimal  -  this crate has no direct byte-level decode surface.
Fuzzing happens at `vyre-foundation::serial::wire` and concrete
pipeline-cache modules where trust boundaries live.

## What NOT to test here

- Concrete backend lowering or dispatch  -  the owning backend crate's tests
- Wire format  -  `vyre-foundation/tests`
- Op semantics  -  `vyre-ops/tests`, `vyre-reference/tests`

## Running

```bash
./cargo_full test -p vyre-driver
./cargo_full test -p vyre-driver --test adversarial
./cargo_full test -p vyre-driver --test property
./cargo_full test -p vyre-driver --test gap
./cargo_full test -p vyre-driver --test integration      # backend_contract lives here
./cargo_full bench -p vyre-driver
```
