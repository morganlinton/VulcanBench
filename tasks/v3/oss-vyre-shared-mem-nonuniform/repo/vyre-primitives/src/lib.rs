// Crate policy: unsafe is DENIED by default (was `forbid`), so the crate stays
// unsafe-free everywhere except call sites that carry an explicit
// `#[allow(unsafe_code)]` plus a `// SAFETY:` proof. The sole current exception
// is `wire::fill_le_words_into`, where eliminating a redundant pre-copy
// zero-fill on the GPU-readback decode hot path is worth a single, audited
// uninitialized-write. `deny` (not `forbid`) is required so that one annotated
// exception can compile; every other `unsafe` in the crate still hard-errors.
#![deny(unsafe_code)]
//! `vyre-primitives`  -  compositional primitives for vyre.
//!
//! Shape (mirrors Linux kernel `fs/` / `mm/` / `net/`  -  subsystem
//! directories under one crate, feature-gated for consumers):
//!
//! ```text
//! vyre-primitives/
//!   src/
//!     lib.rs                  # subsystem table (this file)
//!     markers.rs              # unit-struct marker types, always on
//!     text/                   # feature = "text"
//!       mod.rs
//!       char_class.rs
//!       utf8_validate.rs
//!       line_index.rs
//!     matching/               # feature = "matching"
//!       mod.rs
//!       bracket_match.rs
//!     bitset/                 # feature = "bitset"
//!     fixpoint/               # feature = "fixpoint"
//!     graph/                  # feature = "graph"     (CSR + BFS + SCC + motif + toposort)
//!     hash/                   # feature = "hash"
//!     label/                  # feature = "label"
//!     math/                   # feature = "math"
//!     nn/                     # feature = "nn"
//!     parsing/                # feature = "parsing"
//!     predicate/              # feature = "predicate"
//!     reduce/                 # feature = "reduce"
//! ```
//!
//! Two kinds of primitive live here:
//!
//! 1. **Marker types** (`markers`, always on, zero deps)  -  unit
//!    structs the reference interpreter and backend emitters dispatch
//!    on.
//!
//! 2. **Tier 2.5 substrate** (per-domain feature flags)  -  shared
//!    `fn(...) -> Program` primitives reused by ≥ 2 Tier-3 dialects.
//!    Each domain is one folder + one feature flag. Tier 3 crates
//!    depend on `vyre-primitives` and enable only the domains they
//!    need.
//!
//! The path IS the interface. Subsystem `mod.rs` exposes sub-modules,
//! not a flat namespace  -  callers write
//! `vyre_primitives::text::char_class::char_class(...)` so the LEGO
//! chain is visible at every call site.
//!
//! See `docs/primitives-tier.md` and `docs/lego-block-rule.md` for
//! the tier rule, admission criteria, and Gate 1 enforcement.

mod markers;
pub mod wire;
#[cfg(feature = "vyre-foundation")]
use std::sync::Arc;

pub use markers::{
    ArithAdd, ArithMul, BitwiseAnd, BitwiseOr, BitwiseXor, Clz, CombineOp, CompareEq, CompareLt,
    Gather, HashBlake3, HashFnv1a, PatternMatchDfa, PatternMatchLiteral, Popcount, Reduce,
    RegionId, Scan, Scatter, ShiftLeft, ShiftRight, Shuffle,
};
#[cfg(feature = "vyre-foundation")]
use vyre_foundation::ir::model::expr::Ident;
#[cfg(feature = "vyre-foundation")]
use vyre_foundation::ir::{BufferDecl, DataType, Expr, Node, Program};

/// Build a scalar trap program for invalid primitive builder inputs.
///
/// Primitive constructors are intentionally infallible for composition with
/// registry fixtures and generated dialect code. Invalid user-controlled
/// shapes must therefore become explicit IR traps, not host panics.
#[cfg(feature = "vyre-foundation")]
pub(crate) fn invalid_output_program(
    op_id: &'static str,
    output: &str,
    data_type: DataType,
    message: String,
) -> Program {
    Program::wrapped(
        vec![BufferDecl::output(output, 0, data_type).with_count(1)],
        [1, 1, 1],
        vec![Node::Region {
            generator: Ident::from(op_id),
            source_region: None,
            body: Arc::new(vec![Node::trap(Expr::u32(0), message)]),
        }],
    )
}

/// Return `(left * right) >> 16` for unsigned 16.16 fixed-point lanes without
/// losing the high half of the product to 32-bit overflow.
#[cfg(any(feature = "graph", feature = "math", feature = "geom", feature = "opt"))]
pub(crate) fn fixed_mul_16_16_expr(left: Expr, right: Expr) -> Expr {
    let low = Expr::mul(left.clone(), right.clone());
    let high = Expr::mulhi(left, right);
    Expr::bitor(
        Expr::shr(low, Expr::u32(16)),
        Expr::shl(high, Expr::u32(16)),
    )
}

#[cfg(any(feature = "graph", feature = "math"))]
pub(crate) mod fixed_u32_matmul;

#[cfg(any(feature = "label", feature = "predicate"))]
pub(crate) mod nodeset_filter;

/// Domain-neutral byte-range primitive.
///
/// CRITIQUE_VISION_ALIGNMENT_2026-04-23 V1: the foundation tier ships a
/// matching-flavoured `Match { pattern_id, start, end }` today. This
/// module introduces `ByteRange { tag, start, end }` as the neutral
/// name so new dialects do not have to adopt matching vocabulary. The
/// bridge from foundation's legacy `Match` type is implemented in
/// [`range`], so new dialects can adopt the neutral type without
/// waiting on a foundation API break.
pub mod range;

/// Tier-2.5 primitive registry. See [`harness::OpEntry`]. Gated
/// behind the `inventory-registry` feature so default builds stay
/// dep-free; the conform harness + xtask enable the feature.
#[cfg(feature = "inventory-registry")]
pub mod harness;

/// Text primitives.
#[cfg(feature = "text")]
pub mod text;

/// Pattern-matching primitives.
#[cfg(feature = "matching")]
pub mod matching;

/// Decode primitives.
#[cfg(feature = "decode")]
pub mod decode;

/// NFA primitives  -  subgroup-cooperative simulator (G1 GPU perf).
#[cfg(feature = "nfa")]
pub mod nfa;

/// Hash primitives (FNV-1a 32/64, CRC-32).
#[cfg(feature = "hash")]
pub mod hash;

/// Math primitives (dot, scan, reduce, broadcast).
#[cfg(feature = "math")]
pub mod math;

/// Parsing primitives (optimizer and AST scan kernels).
#[cfg(feature = "parsing")]
pub mod parsing;

/// Neural-network primitives (attention and normalization sub-kernels).
#[cfg(feature = "nn")]
pub mod nn;

/// Graph primitives (topological sort, reachability, CSR traversal,
/// SCC decomposition, path reconstruction  -  the Tier 2.5 substrate
/// that a external analyzer's stdlib rules compose against).
#[cfg(feature = "graph")]
pub mod graph;

/// Geometric / Clifford-algebra primitives (#8). Multivector products
/// for equivariant NNs, physics simulation, robotics, 3D vision.
#[cfg(feature = "geom")]
pub mod geom;

/// Optimization primitives (#9, #14, #46). Homotopy continuation,
/// SOS, matroid intersection. Self: vyre's megakernel scheduler.
#[cfg(feature = "opt")]
pub mod opt;

/// Topological-data-analysis primitives (#15, #32). Vietoris-Rips
/// filtration + simplicial complex operations. User: TDA, persistent
/// landscape features, call-graph topological signatures.
#[cfg(feature = "topology")]
pub mod topology;

/// Visual pixel-map primitives. Shared packed-RGBA invocation skeletons
/// reused by higher-level image-processing compositions.
#[cfg(feature = "visual")]
pub mod visual;

/// Effects-typed pipeline primitives (P-1.0-V1.x).
/// Pure-data substrate: `EffectRow` bitmask, `Handler` over a row,
/// `handler_apply` discharges effects, `handler_compose` builds a
/// joint handler. Reference for the foundation effects-typed
/// `lower` pipeline (V1.3).
#[cfg(feature = "effects")]
pub mod effects;

/// Type-discipline primitives (P-PRIM-14, …). Substructural
/// (linear/affine/relevant/unrestricted) checks the foundation
/// validate pipeline consumes per buffer.
#[cfg(feature = "types")]
pub mod types;

/// Categorical primitives (P-PRIM-16/17/18). Yoneda embedding,
/// adjoint-pair detection, Kan extension over finite categories.
/// Consumed by the optimizer's functorial_pass_composition substrate.
#[cfg(feature = "cat")]
pub mod cat;

/// ZX-calculus rewrite primitives (P-PRIM-5). Spider fusion,
/// identity removal, color change. Pure-CPU on a Vec<ZxSpider> +
/// edge multiset; no FP, no IR-builder dep.
#[cfg(feature = "zx")]
pub mod zx;

/// d-DNNF (decomposable / deterministic NNF) compiler primitive
/// (P-PRIM-6). Host-side CNF → d-DNNF via Shannon decomposition,
/// linear-time model counting on the result. Used by
/// `knowledge_compile_pass_precondition` to turn pass-precondition
/// formulae into linear-cost evaluators.
#[cfg(feature = "dnnf")]
pub mod dnnf;

/// Bitset primitives  -  `and`/`or`/`not`/`xor`/`popcount`/`any`/
/// `contains` over packed u32 bitsets. The NodeSet / ValueSet
/// representation every graph primitive consumes.
#[cfg(feature = "bitset")]
pub mod bitset;

/// Reduction primitives  -  `count`/`min`/`max`/`sum` over bitsets and
/// fixed-width ValueSets. Backs source-query dialect aggregates.
#[cfg(feature = "reduce")]
pub mod reduce;

/// Label → NodeSet resolver  -  turn a TagFamily bitmask into a
/// NodeSet bitset. Implements the `@family` lookup that a external analyzer's
/// label surface surfaces.
#[cfg(feature = "label")]
pub mod label;

/// Frozen predicate primitives  -  the ~10 engine primitives (call_to,
/// return_value_of, arg_of, size_argument_of, edge, in_function,
/// in_file, in_package, literal_of, node_kind) that source-query dialect stdlib
/// rules compose into every higher-level query.
#[cfg(feature = "predicate")]
pub mod predicate;

/// Deterministic fixpoint primitive (ping-pong with convergence
/// flag). Composes `csr_forward_traverse` + bitset OR into the
/// transitive-closure driver every stdlib taint rule needs.
#[cfg(feature = "fixpoint")]
pub mod fixpoint;

/// Virtual File System DMA primitives. Uses `vyre_foundation::ir`
/// so it's gated behind the same set of features that pull
/// vyre-foundation in as an optional dep. Any of the domain
/// features enables vfs.
#[cfg(any(
    feature = "text",
    feature = "matching",
    feature = "decode",
    feature = "math",
    feature = "nn",
    feature = "hash",
    feature = "parsing",
    feature = "graph",
    feature = "bitset",
    feature = "reduce",
    feature = "label",
    feature = "predicate",
    feature = "fixpoint",
))]
pub mod vfs;

/// Wire-format envelope re-exported from vyre-foundation.
///
/// Every primitive that ships its own `to_bytes` / `from_bytes` (today:
/// `CompiledDfa`; future: serializable region tables, hash tables,
/// parser plans) composes this envelope. Re-exporting at the
/// vyre-primitives root keeps the import path uniform for consumers:
/// `vyre_primitives::serial_data::WireWriter` regardless of whether
/// the type lives at the primitive layer or higher up.
///
/// Available when any feature that pulls vyre-foundation is enabled
/// (every primitive domain enables it).
#[cfg(feature = "vyre-foundation")]
pub mod serial_data {
    pub use vyre_foundation::serial::envelope::{
        test_helpers, EnvelopeError, WireReader, WireWriter,
    };
}

/// Curated prelude - the byte-pack/decode primitives every consumer
/// needs for GPU buffer construction and readback, plus the shared
/// envelope types when vyre-foundation is in play.
///
/// `use vyre_primitives::prelude::*;` should be the only import a
/// caller needs for the common pack/unpack surface. Adding new wire
/// primitives must keep this list in sync.
pub mod prelude {
    pub use crate::wire::{
        append_f32_slice_le_bytes, append_packed_byte_lane, append_u32_slice_le_bytes,
        decode_f32_le_bytes_all, decode_i32_le_bytes_all, decode_u16_le_bytes_all,
        decode_u32_le_bytes_all, decode_u64_le_bytes_all, pack_bytes_as_u32_slice,
        pack_bytes_as_u32_slice_min_words, pack_f32_slice, pack_f32_slice_into,
        pack_f32_slice_into_uninit, pack_i32_slice, pack_i32_slice_into, pack_u16_slice,
        pack_u16_slice_into, pack_u32_slice, pack_u32_slice_into, pack_u32_slice_into_uninit,
        pack_u32_slice_min_words_into, pack_u64_slice, pack_u64_slice_into, unpack_f32_slice,
        unpack_f32_slice_into, unpack_u32_slice_into, read_f32_le_word, read_u32_le_word,
    };
}

#[cfg(feature = "predicate")]
pub(crate) mod program_region {
    use std::sync::Arc;

    use vyre_foundation::ir::model::expr::{GeneratorRef, Ident};
    use vyre_foundation::ir::{Node, Program};

    pub(crate) fn tag_program(parent_op_id: &str, program: Program) -> Program {
        Program::wrapped(
            program.buffers().to_vec(),
            program.workgroup_size(),
            vec![Node::Region {
                generator: Ident::from(parent_op_id),
                source_region: None,
                body: Arc::new(reparent_program_children(&program, parent_op_id)),
            }],
        )
    }

    fn reparent_program_children(program: &Program, parent_op_id: &str) -> Vec<Node> {
        let parent = GeneratorRef {
            name: parent_op_id.to_string(),
        };
        program
            .entry()
            .iter()
            .cloned()
            .map(|node| reparent_entry_node(node, &parent))
            .collect()
    }

    fn reparent_entry_node(node: Node, parent: &GeneratorRef) -> Node {
        match node {
            Node::Region {
                generator, body, ..
            } => Node::Region {
                generator,
                source_region: Some(parent.clone()),
                body,
            },
            other => Node::Region {
                generator: Ident::from(Program::ROOT_REGION_GENERATOR),
                source_region: Some(parent.clone()),
                body: Arc::new(vec![other]),
            },
        }
    }
}
