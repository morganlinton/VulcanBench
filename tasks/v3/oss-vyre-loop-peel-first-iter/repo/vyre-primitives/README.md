# vyre-primitives

Compositional primitives for vyre: the Tier-2.5 LEGO substrate that
sits between Tier-2 hardware intrinsics (`vyre-intrinsics`) and Tier-3
domain libraries (`vyre-libs`). Every domain library reuses primitives
from this crate; consumers compose primitives without touching
`vyre-driver-*` directly.

## What this crate is

The crate is feature-gated per domain so a consumer that wants only
bitset operations does not pay for the matching DFA, the d-DNNF
compiler, or the cryptographic hash family. Each domain is a feature
flag; a crate-level marker type lives at
`vyre-primitives::<domain>::*` and registers through
`inventory::submit!(OpEntry { … })` so `vyre::registered_primitives()`
enumerates every primitive the current build links.

The crate intentionally has zero concrete backend dependencies. It
depends only on `vyre-foundation` + `vyre-spec`.
The boundary is enforced by `scripts/check_ownership_boundaries.sh`
and `OWNERSHIP.md`.

## Domain layout

Mirrors the Linux kernel `fs/` / `mm/` / `net/` shape: each domain is
its own subdirectory under `src/`, gated behind a Cargo feature flag.

| Feature              | Subsystem            | Highlights                                              |
|---------------------|----------------------|---------------------------------------------------------|
| `bitset`            | `bitset/`            | bitset_and / or / xor / not / popcount / contains       |
| `reduce`            | `reduce/`            | reduce_sum / max / min / count / scatter / segment      |
| `text`              | `text/`              | char_class / line_index / utf8_validate                |
| `matching`          | `matching/`          | dfa_compile, classifier_emit, region builder           |
| `math`              | `math/`              | linalg primitives, sparse_recovery, interval algebra   |
| `nn`                | `nn/`                | activation, attention scaffolding (composed in libs)   |
| `hash`              | `hash/`              | perfect_hash, blake3_round                             |
| `parsing`           | `parsing/`           | bracket_match, ast_walk_preorder                       |
| `graph`             | `graph/`             | csr_*, motif, reachable, union_find, exploded          |
| `bitset` `+ reduce` | derived              | scan_*, prefix_*, four_russians readiness              |
| `label`             | `label/`             | resolve_family, label_program                          |
| `predicate`         | `predicate/`         | size_argument_of and friends (predicate substrate)     |
| `fixpoint`          | `fixpoint/`          | persistent_fixpoint, level_wave                        |
| `dnnf`              | `dnnf/`              | host-side d-DNNF compiler + model counter (P-PRIM-6)    |
| `inventory-registry`| (gate)               | Enables `inventory::submit!` registration system       |

`all-lego` enables every Tier-2.5 domain; `default` enables the small
core (`bitset`, `reduce`, `inventory-registry`).

## Architecture decisions

- **Marker types only at the public surface.** Each primitive is a
  unit struct that implements the relevant trait
  (`ReferenceEvaluator`, `BackendEmitter`, etc.). The implementations
  live in `vyre-reference` (CPU oracle) and the per-backend crate.
- **No GPU code in this crate.** Every primitive's GPU lowering lives
  in the concrete driver crate that owns the target. The marker type
  does not import shader strings: the
  `check_no_string_wgsl.sh` gate is non-negotiable.
- **Promotion path.** A composition that lives in `vyre-libs` is
  promoted to a primitive here only after ≥3 distinct callers and an
  explicit architectural review. The `LEGO_PRIMITIVES.md` audit tracks
  candidates.
- **Per-domain test directories.** Every domain ships positive,
  negative, adversarial, cross-call, and proptest fixtures; see
  `tests/<domain>_*.rs`.

## Where to look

- `src/lib.rs`: feature-gate table and the public domain list.
- `src/markers.rs`: the always-on marker registry types.
- `tests/`: per-domain adversarial corpora.
- `OWNERSHIP.md` (workspace root): boundary definition.
- `audits/LEGO_PRIMITIVES.md`: promotion candidates and Tier
  assignment rationale.

## Conformance

Every primitive ships a CPU reference (in `vyre-reference`) byte
identical to the GPU output. The conformance runner
(`vyre-conform`) walks the registered primitive table and asserts
parity across backends; drift is publish-blocking.
