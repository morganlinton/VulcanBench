//! Tier 2.5 parsing primitives.
//!
//! These are the reusable optimizer kernels that Tier 3 language packs compose
//! into full parsing/AST passes.

/// Generic delimiter-depth scan for paired delimiter token streams.
pub mod core_delimiter_match;

/// SSA dominance-frontier phi discovery scan.
pub mod ssa_dominance_scan;

/// Shared AST opcode constants.
pub mod ast_ops;

/// Pack an opcode → handler dispatch table into one u32 per entry for fast
/// GPU-side bytecode interpretation. Foundational primitive for
/// warp-specialized interpreter loops where every thread executes the same
/// opcode in the same warp.
pub mod bytecode_dispatch_table_pack;

/// Word-at-a-time whitespace classification (#P-PRIM-WS-CLASSIFY).
/// Foundational primitive for structural parsers (JSON, CSV, HTTP, INI):
/// loads 4 bytes per u32, emits a 4-bit per-word "is-whitespace" mask
/// using pure arithmetic (no per-byte branches → no warp divergence).
/// Composes with `stream_compact` for the canonical simdjson-style
/// whitespace-skip pipeline.
pub mod whitespace_classify_word;

/// Per-byte kept-mask for C translation phase 2 (`\<newline>` deletion).
/// One thread per input byte; ±2-byte sliding window classifies each of
/// the five splice cases. Composes with `stream_compact` to materialise
/// the post-phase-2 byte stream and the original-offset map. Replaces
/// the CPU-only `c_translation_phase_line_splice` helper used pre-lex
/// by every C tokenization path.
pub mod line_splice_classify;

/// AST-level constant-folding wave operating on packed-AST u32 buffers.
/// NOT the vyre-IR `optimizer::passes::fusion_cse::cse` (audit cleanup A8,
/// 2026-04-30): the `ast_` prefix marks this as a parsing-domain primitive
/// that runs against a packed-AST representation, not against `Expr` /
/// `Node` of the IR.
pub mod ast_cse_constant_fold;

/// AST-level structural-hash CSE probe/insert wave operating on packed-
/// AST u32 buffers. NOT the vyre-IR CSE  -  the `ast_` prefix disambiguates.
pub mod ast_cse_structural_hash;

/// 2D / planar grammar rewrite scheduler (#11). Picks a maximal
/// non-overlapping set of `k × k` matches to apply in one wave.
/// User: scene parsing, parallel cellular automata, document layout.
pub mod planar_rewrite;
