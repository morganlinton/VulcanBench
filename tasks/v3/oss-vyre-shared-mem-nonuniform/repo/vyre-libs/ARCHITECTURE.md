# vyre-libs  -  architecture

Tier-3 compositions: every `fn(...) -> Program` that lowers via
existing IR ops + intrinsics. No hardware-specific arms.

## Modules (one folder per domain)

### `decode/`
GPU-resident decoders: base64, hex, urlencoded, gzip-fragment,
zstd-block. Composes `match::dfa` + `bitset` + `scatter`.

### `matching/`
Pattern-matching primitives: `aho_corasick`, `dfa_compile`,
`dfa_compile_with_budget`, `substring_search`. Downstream pattern
pre-passes use these.

### `math/`
Arithmetic primitives, atomic-style ops, fixed-point, hash-
adjacent math, transcendentals lowered to `Expr::Fma` chains.

### `nn/`
Neural-net building blocks: matmul_tiled, linear, layer_norm,
softmax, attention. CPU references in `vyre-reference`.

### `hash/`
Hash compressors: blake3_compress, sha2_block, fnv1a32, fnv1a64,
crc32, adler32. Composes with `bitset` for streaming.

### `graph/`
Graph traversals: csr_forward_traverse, csr_backward_traverse,
sanitizer_cut_csr, path_reconstruct.

### `dataflow/`
Bitset-fixpoint, frontier_advance, dataflow_join. The substrate
dataflow engines run IFDS over.

### `parsing/`
Source-language parsers (C/C++/Rust/Go/Python/JS/TS) wrapped as
`PackedAst` emitters. Stays on CPU per the parsing-stays-on-CPU
decision in `docs/parsing-and-frontends.md`.

### `compiler/`
Compiler-style passes that lower one Program shape into another
(pattern-engine compile, predicate-registry resolution).

### `intern/`
String/Ident interning helpers + the perfect-hash table for
canonical lookups.

### `logical/`
Boolean primitives  -  `and`, `or`, `xor`, `nand`, `nor`. Each
declares its `Commutative`/`Associative`/`Idempotent` markers
for the algebraic-law registry.

### `region.rs`
Public region-wrap helpers that consumers use.

### `descriptor.rs`
Buffer-shape descriptors used by every compositional op.

### `contracts.rs`
Per-op contract types (precondition/postcondition pairs the
conform suite checks).

### `harness.rs`
Discovery surface  -  `vyre-libs::harness::iter()` enumerates every
shipped op.

### `representation/`
Frozen wire-form types that downstream frontends rely on (PackedAst,
PgBuffers carrier, etc.).

### `range_ordering.rs`
Sorted-range helpers used by the matching + dataflow stacks.

## Public types

- **`security::*`**  -  downstream analyzers consume these: `flows_to`,
  `sanitized_by`, `bounded_by_comparison`, `dominator_tree`,
  `label_by_family`, `path_reconstruct`, `aliases_dataflow`.
- **`matching::CompiledDfa`**  -  DFA build result.
- Per-domain types are documented in their respective module
  rustdoc.

## Integration points

- Consumed by downstream analyzers for every predicate that lowers to a
  composition.
- Consumed by `vyre-runtime` for higher-level pipeline
  scheduling.
- Feature gates (`go-parser`, `intern`, `security`, etc.) keep
  consumers from pulling unused domains.
