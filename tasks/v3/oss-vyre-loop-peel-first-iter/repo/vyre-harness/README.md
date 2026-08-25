# vyre-harness

Universal Cat-A op harness registry: the single registration surface
every domain library publishes through. Spun out of `vyre-libs` so any
crate that emits vyre Programs can register without a circular
dependency on the wider domain surface.

## What this crate is

The contract that the conformance runner (`vyre-conform`) walks. Every
op that participates in automated parity / wire-format / convergence
tests submits one `OpEntry` through `inventory::submit!` from its
home crate; this crate then exposes the unified iterator the runner
consumes.

## Key types

- `OpEntry`: POD entry: op id, builder `fn(...) -> Program`, fixture
  inputs, expected oracle bytes. POD over `&'static str` and `fn`
  pointers, so stdlib auto-traits give `Send + Sync` for free.
- `ConvergenceContract`: declared convergence semantics for fixpoint
  ops (max iterations, oracle definition, divergence detection).
- `FixpointContract`: bound + termination contract for fixpoint
  dispatch.
- `UniversalDiffExemption`: link-time exemption reason for the
  byte-identity sweep. Adding a new variant requires a CEO decision;
  no silent exemptions.
- `DiffCandidate` / `universal_diff_candidates()`: the canonical
  iteration source the conform tests walk.

## Architecture decisions

- **Single registry, many publishers.** Any crate (in or out of the
  workspace) can register primitives by depending on `vyre-harness`
  alone. `vyre-libs` re-exports it as `harness` for backward
  compatibility with the math / nn / crypto / matching modules.
- **No backend dependency.** The harness owns no backend logic. It
  cannot import concrete driver APIs. The boundary is gated by
  `scripts/check_ownership_boundaries.sh`.
- **POD over `'static`.** Because every entry is POD over `&'static
  str`, every entry can live in a static `inventory::collect!` slot,
  which turns the registry into a zero-cost cold-start lookup.
- **No opinion on home crate.** A primitive may live in `vyre-libs`,
  `vyre-primitives`, or any future external composition crate: the
  harness is registry-only.

## Who uses it

- `vyre-libs`: math / nn / crypto / matching / decode primitives.
- `vyre-primitives`: the Tier-2.5 LEGO substrate.
- External composition crates: downstream domains that register their
  own primitives without coupling this harness to their package names.
- `vyre-conform-runner`: the consumer that walks every entry to
  produce a signed certificate per op × backend × adapter.

## Where to look

- `src/lib.rs`  -  public type surface.
- `tests/universal_harness.rs`  -  the integration test that proves the
  registry is well-formed.
- `OWNERSHIP.md` (workspace root)  -  boundary definition.
- `audits/V7_api.toml`  -  frozen public-API contract for this crate.
