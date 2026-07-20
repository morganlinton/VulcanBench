# tests/SKILL.md  -  vyre-primitives

Read `../../.internals/skills/testing/SKILL.md` first for the category contract.

## Purpose

`vyre-primitives` holds **marker types for reference-interpreter
primitive dispatch**. Very narrow surface: the crate exists to let
the reference interpreter route by primitive kind without pulling in
the full op catalog.

## Critical invariants

- **Every marker is `#[non_exhaustive]`**  -  adding a primitive
  later is non-breaking.
- **Every marker has a stable `&'static str` identity** that
  matches the op id in `vyre-ops`.

## Adversarial surface

- Minimal  -  this crate has no byte-in / byte-out surface. Tests
  focus on ensuring the marker types remain stable.

## Current gaps

- No test today confirms every `vyre-ops` primitive has a marker
  in this crate. Gap: "every `primitive.*` op in the registry has
  a matching `PrimitiveKind` variant".

## Cross-crate contracts

- Consumed by `vyre-reference` for dispatch
- Consumed by conform runners to enumerate primitives

## Bench targets

None  -  this crate has no runtime behavior.

## Fuzz targets

None.

## What NOT to test here

- Actual primitive semantics  -  `vyre-ops`, `vyre-reference`
- Registry behavior  -  `vyre-driver`

## Running

```bash
./cargo_full test -p vyre-primitives
# Integration smoke (registry + one `Program` validate) needs Tier-2.5 builders + inventory:
./cargo_full test -p vyre-primitives --features "hash,inventory-registry" --test integration
```
