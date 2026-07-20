# vyre-primitives  -  architecture

The lowest tier of compositional ops. Each primitive is a
single-file `fn(...) -> Program` that lowers via existing
`vyre::ir` constructors only  -  no hardware emission, no
self-substrate.

This crate is the substrate `vyre-libs` and downstream predicate
systems compose on top of.

## Modules (one folder per family)

### `bitset/`
Per-word bitwise ops + per-bitset higher-level (and, or, xor,
copy, count_set, popcount). Used everywhere the primitive shape
is "boolean reachability."

### `graph/`
CSR-encoded graph traversal: `csr_forward_traverse`,
`csr_backward_traverse`, `program_graph::ProgramGraphShape`,
`level_wave_program`.

### `fixpoint/`
Convergence primitives: `bitset_fixpoint`, `persistent_fixpoint`
(single-dispatch GPU loop with cross-workgroup synchronization
via the persistence contract).

### `reduce/`
Cross-lane reductions (sum, max, min, any, all). Composes the
subgroup-ops from `vyre-intrinsics` when present.

### `matching/`
Match-result building blocks. The full pattern engines live in
`vyre-libs::matching`; here are the per-state DFA-step helpers.

### `nfa/`
NFA → DFA construction primitive. CPU-side; the DFA blob is
shipped to GPU as a buffer.

### `decode/`
Per-encoding decoder bricks (base64, hex, urlencoded). Composes
into `vyre-libs::decode` pipelines.

### `hash/`
Per-block compressors: blake3, sha2, fnv1a, crc32. Each declares
its constants as buffer literals.

### `label/`
Label-bitmask primitives. Downstream `label_by_family` compositions
lower via this.

### `predicate/`
Predicate scaffolding helpers used by downstream predicate
registries.

### `geom/`
Geometric / spatial primitives  -  bounding box test, point-in-
range. Used by domains like graphics + spatial indexing
downstream.

### `math/`
Numeric primitives that don't need hardware intrinsics
(integer arithmetic, fixed-point ops).

### `nn/`
Neural-net building blocks at the primitive level (im2col,
gemv). Composes into `vyre-libs::nn`.

### `parsing/`
Generic parsing scaffolds (bracket_match, char_class) consumed
by frontend source adapters.

### `text/`
String-search primitives (utf8 decode, character classification).

### `opt/`
Mathematical optimizers (gradient step, projected gradient).

### `topology/`
Topology-aware partitioning helpers  -  the substrate the megakernel
scaling layer reaches into.

### `vfs/`
Virtual-filesystem-style buffer addressing (multi-buffer offsets +
permissions table). Used by the megakernel's IO queue.

### `markers.rs`
Algebraic-law marker registrations (Commutative / Associative /
Idempotent / OverflowWrapping).

### `range.rs`
Range-bound helpers consumed by both the matching and dataflow
stacks.

### `harness.rs`
Per-primitive CPU reference + GPU programmatic build, exported as
the canonical conform-suite probe set.

## Public types

- **`bitset::bitset_words(node_count: u32) → u32`**  -  the
  word-count helper every consumer uses for sizing.
- **`graph::program_graph::ProgramGraphShape`**  -  `(node_count,
  edge_count)` shape carried through every graph-touching
  primitive.
- **(per-primitive)** Each ships a constructor + a `cpu_ref(...)`
  pair; the harness module aggregates them.

## Integration points

- Consumed by `vyre-libs` for higher-level compositions.
- Consumed by downstream predicate lowering directly.
- Conform runner cross-checks every primitive's CPU reference
  against its GPU dispatch byte-by-byte.
