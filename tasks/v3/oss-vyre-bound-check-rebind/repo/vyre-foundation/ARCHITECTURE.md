# vyre-foundation  -  architecture

The IR + transforms + validation + frozen-data-contract crate.
Every other vyre crate depends on this one. Conversely, this
crate depends on no other vyre crate.

## Modules

### `ir_inner/`
The IR data model. `Program`, `Node`, `Expr`, `BufferDecl`,
`Ident`, plus the macro-generated AST registry. Re-exported via
`pub mod ir { ... }`.

### `optimizer/`
Optimizer passes: cse, dce, canonicalize, region_inline,
const_fold, autotune, fuse_cse.

### `pass_substrate/` (OFF-LIMITS  -  substrate hot-path wires in flight)
Pass-scheduler substrate (PassScheduler + transitive_dependents).
Currently mid-edit.

### `optimizer/scheduler.rs` (OFF-LIMITS)
PassScheduler core. Currently mid-edit.

### `execution_plan/`
Cross-pass plan: `fuse_programs`, `fuse_programs_vec`,
`FusionError`. Used by downstream fused-dispatch paths.

### `validate/`
Wire-format + structural validation. `validate(&Program)` returns
`Vec<ValidationError>`; an empty vec is the gate.

### `transform/`
Visitor-based transforms (inline, optimize, walk_nodes,
walk_exprs, walk_nodes_mut).

### `serial/`
Wire-format encode/decode. Stable u32-aligned little-endian
byte stream.

### `dialect_lookup.rs` + `extern_registry.rs`
The extension registry. Community dialect packs register here via
inventory.

### `algebraic_law_registry.rs`
Inventory of algebraic-law markers (Commutative, Associative,
Idempotent, OverflowWrapping). Optimizer passes consume the
registry to know which transformations are sound for which op.

### `cpu_op.rs` / `cpu_references.rs`
The CPU-reference oracle. Every op declares its CPU semantics
here; the conform suite measures every backend's output against
this.

### `composition.rs`
Region-chain composition helpers. Wraps op bodies in
`Node::Region { source_region, body }` so provenance survives
inlining.

### `memory_model.rs`
`MemoryOrdering`, `MemoryKind`, `MemoryHints`. The frozen memory
model.

### `vast/`
Visitor-based AST traversal helpers (preorder, postorder).

### `graph_view.rs`
Graph-shape view over the IR. Used by passes that reason about
the program as a CFG-like graph.

### `match_result.rs`
Frozen wire type for pattern-match results.

### `program_caps.rs`
Per-program capability probe (atomics-required, subgroup-ops-
required, max-output-bytes, etc.).

### `opaque_payload/`
Endian-fixed encode/decode helpers for `Expr::Opaque` /
`Node::Opaque` payloads.

### `error.rs`
Foundation error type  -  wrapped by every consumer.

## Public types

- **`Program`**  -  frozen IR root.
- **`Node` / `Expr`**  -  AST nodes.
- **`BufferDecl`**  -  buffer declaration.
- **`Ident`**  -  interned identifier.
- **`MemoryOrdering` / `MemoryKind`**  -  frozen memory model.
- **`ValidationError`**  -  emitted by `validate(&Program)`.
- **`FusionError`**  -  emitted by `fuse_programs_vec`.

## Integration points

- Every other vyre crate consumes this.
- Frozen names land at `vyre::ir::*` via the meta-shim.
- The wire format is stable  -  bumping major requires the
  conform-runner gate to allow forward-decode.
