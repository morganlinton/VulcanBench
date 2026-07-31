# tests/SKILL.md  -  vyre (meta-shim)

Read `../../.internals/skills/testing/SKILL.md` first for the category contract.

## Purpose

`vyre` (package name for `vyre-core`) is the **meta-shim**.
Consumers import `vyre::Program`, `vyre::VyreBackend`, `vyre::ir::*`
 -  everything. The shim re-exports from `vyre-foundation`,
`vyre-driver`, and `vyre-ops` so downstream code has one stable
crate to depend on.

## Critical invariants

- **Every public item the shim exports has a documented stable
  path.** `vyre::Program` is stable; `vyre::ir::model::program::Program`
  is an internal-structure leak.
- **No runtime behavior.** The shim adds zero code on the hot path;
  every call forwards to an upstream crate.
- **Four public facets.** `vyre::prelude`, `vyre::ir`, `vyre::runtime`,
  `vyre::ops`  -  any consumer import that reaches outside these is
  a boundary violation and the test must catch it.
- **Doctest coverage of the README quick-start.**

## Adversarial surface

Minimal  -  the shim has no decode surface. Tests focus on API
surface stability.

## Current gaps

- No `public_api` snapshot test today  -  every refactor risks
  accidentally renaming a re-exported type. Gap: add
  `cargo-public-api` integration test that fails when the surface
  changes without a `PUBLIC_API.md` diff.
- Every doctest in the README must compile and run as a test in
  `integration.rs`; gap test lists missing coverage.

## Cross-crate contracts

This crate is the integration point. Every cross-crate test here
exercises the shim's ability to compose surface from
vyre-foundation + vyre-driver + vyre-ops.

- `vyre::Program` === `vyre_foundation::ir::Program`
- `vyre::VyreBackend` === `vyre_driver::VyreBackend`
- `vyre::ir::*` flattened from `vyre_foundation::ir::*`
- `vyre::ops::*` flattened from `vyre_ops::*`

## Bench targets

Re-export cost is zero. No runtime benches needed.

## Fuzz targets

None directly  -  fuzz targets live in the upstream crates.

## What NOT to test here

- Every upstream crate's own invariants  -  test them there
- Concrete backend semantics  -  the owning concrete driver crate's tests

## Running

```bash
./cargo_full test -p vyre
./cargo_full test -p vyre --test integration    # primary test surface for the shim
```
