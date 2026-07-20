# Substrate Layout

P0 inventory item #67  -  single canonical home for the three substrate
concepts.

vyre uses the word "substrate" in three related but distinct senses.
Each one has exactly one home in the workspace. The audit flags any
new substrate-prefixed module that does not match this map.

| Substrate kind   | Home                                  | What it owns                                                                                  |
|------------------|---------------------------------------|-----------------------------------------------------------------------------------------------|
| **substrate**    | `vyre-primitives/src/<domain>/`       | Hardware-faithful primitive kernels (Tier 2.5 LEGO substrate). Domain-feature-gated.          |
| **self-substrate** | `vyre-driver/src/self_substrate/`   | The self-recursion + composition layer  -  primitives that recompose driver-side decisions through vyre IR. |
| **pass-substrate** | `vyre-foundation/src/pass_substrate/` | The optimizer / transform pass framework  -  typed passes, scheduler, invalidation metadata.    |

## Why three homes, not one

- **substrate** is the LEGO library. It has no notion of "passes" or
  "self-consumers"  -  it just compiles a primitive into a `Program`.
  Lives at the lowest tier (Tier 2.5).
- **self-substrate** binds primitives to driver-side decisions:
  fusion, scheduling, autotune, eviction, provenance closure. It has
  to live in `vyre-driver` because it consumes driver-tier types
  (Cache, Bind groups, capability metadata).
- **pass-substrate** is the optimizer's typed-pass framework. It runs
  inside `vyre-foundation` because every pass operates on
  foundation-owned IR; pulling it up would invert the layer DAG.

## Forbidden patterns

- A new module named `*_substrate` outside one of the three homes is
  banned by `scripts/check_release_signoff.sh` (see the
  `architectural-invariants.yml` workflow).
- A primitive that lives in `vyre-primitives` but pulls
  `vyre-driver::self_substrate::*` inverts the DAG and is rejected
  by `scripts/check_layering.sh` and
  `scripts/check_ownership_boundaries.sh`.
- A pass that lives in `vyre-driver` instead of `vyre-foundation`
  fails `docs/OWNERSHIP.md` review.

## How a new substrate component lands

1. Pick the home from the table above. If none fits, an audit row is
   the right answer  -  add a new `[#NN]` entry to
   `audits/VYRE_PERFORMANCE_ARCHITECTURE_INVENTORY_2026-04-28.md`
   describing why a new home is needed.
2. The module file declares `pub fn <name>_program(...) -> Program`
   so consumers compose it the same way every other vyre primitive
   composes.
3. The op id namespace is the home crate: `vyre-primitives::<domain>::<op>`,
   `vyre-libs::self_substrate::<op>`, `vyre-foundation::pass_substrate::<pass>`.
4. The recursion gate (`xtask::recursion_gate`) verifies that any
   self-substrate consumer has a paired primitive.

## Cross-references

- `docs/OWNERSHIP.md`  -  per-crate dependency boundaries.
- `docs/MIGRATION.md`  -  historical moves (the `vyre-ops → vyre-intrinsics`
  shift and the `Match → ByteRange` rename).
- `~/.claude/analysts/visions/vyre.md`  -  the long-term thesis these
  three homes serve.
