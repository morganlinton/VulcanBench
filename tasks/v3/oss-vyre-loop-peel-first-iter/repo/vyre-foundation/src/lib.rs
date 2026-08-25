//! vyre-foundation  -  substrate-neutral compiler foundation.
//!
//! Defines the vyre IR (`Expr`, `Node`, `Program`), the type system, the
//! memory model, the wire format, visitor traits, and extension resolvers.
//! Every other vyre crate depends on this one; this crate depends only on
//! `vyre-spec`, `vyre-macros`, and lightweight third-party data crates.
//! It never knows about concrete driver APIs, a dialect, or a backend.

// Foundation owns the IR arena (`ir_inner::model::arena`), which uses two
// `unsafe` blocks to extend bumpalo lifetimes inside a single arena owner.
// Every other unsafe usage is forbidden by `check_lib_rs_headers.sh`.
#![allow(unsafe_code)]
#![allow(
    clippy::duplicate_mod,
    clippy::too_many_arguments,
    clippy::double_must_use,
    clippy::module_inception,
    clippy::should_implement_trait,
    clippy::type_complexity
)]

extern crate self as vyre;

/// Structured optimizer diagnostics surfaced to IDEs and CI annotators.
///
/// Lightweight diagnostic type used by foundation optimizer passes.
///
/// Drivers embed these into their richer diagnostic surface; foundation
/// only needs a human-readable message plus an optional pass/op location
/// so that pass-scheduling errors can be rendered without pulling in
/// driver-tier dependencies.
pub mod diagnostics {

    /// Error-level diagnostic with an optional location hint.
    #[derive(Debug, Clone)]
    pub struct Diagnostic {
        /// Human-readable diagnostic message.
        pub message: String,
        /// Optional op/pass location the diagnostic refers to.
        pub location: Option<OpLocation>,
    }

    impl Diagnostic {
        /// Build an error-level diagnostic with no location.
        #[must_use]
        pub fn error(msg: impl Into<String>) -> Self {
            Self {
                message: msg.into(),
                location: None,
            }
        }

        /// Attach an op/pass location to this diagnostic.
        #[must_use]
        pub fn with_location(mut self, loc: OpLocation) -> Self {
            self.location = Some(loc);
            self
        }
    }

    /// Location handle pointing at a specific pass or op id.
    #[derive(Debug, Clone)]
    pub struct OpLocation {
        /// Stable pass or op identifier.
        pub op_id: String,
    }

    impl OpLocation {
        /// Construct a location hint from an op id.
        #[must_use]
        pub fn op(op_id: impl Into<String>) -> Self {
            Self {
                op_id: op_id.into(),
            }
        }
    }
}

pub mod ir {
    //! The vyre intermediate representation.
    /// Backend-neutral literal evaluation for optimizer passes and lowerings.
    pub mod eval {
        // Audit cleanup A16 (2026-04-30): replaced `pub use crate::ir_eval::*`
        // wildcard with explicit named re-exports per
        // organization_contracts::foundation_wildcard_pub_reexports_are_baselined.
        pub use crate::runtime::ir_eval::{
            fold_binary_literal, fold_cast_literal, fold_fma_literal, fold_literal_tree,
            fold_unary_literal,
        };
    }
    pub use crate::ir_inner::model;
    pub use crate::ir_inner::model::arena::{ArenaProgram, ExprArena, ExprRef};
    pub use crate::ir_inner::model::expr::{Expr, ExprNode, Ident};
    pub use crate::ir_inner::model::node::{Node, NodeExtension};
    pub use crate::ir_inner::model::node_kind::{
        EvalError, InterpCtx, NodeId, NodeStorage, OpId, RegionId, Value, VarId,
    };
    pub use crate::ir_inner::model::program::{
        BufferDecl, CacheLocality, LinearType, MemoryHints, MemoryKind, Program, ShapePredicate,
    };
    /// Per-Node-variant bit-position constants for `ProgramStats::node_kinds_present`.
    /// Compose with `ProgramStats::has_any_node_kind` for O(1) `analyze_impl` gates.
    pub mod stats {
        pub use crate::ir_inner::model::program::{
            NODE_KIND_ALL_GATHER, NODE_KIND_ALL_REDUCE, NODE_KIND_ASSIGN, NODE_KIND_ASYNC_LOAD,
            NODE_KIND_ASYNC_STORE, NODE_KIND_ASYNC_WAIT, NODE_KIND_BARRIER, NODE_KIND_BLOCK,
            NODE_KIND_BROADCAST, NODE_KIND_EXPRESSION_BEARING_MASK, NODE_KIND_IF,
            NODE_KIND_INDIRECT_DISPATCH, NODE_KIND_LET, NODE_KIND_LOOP, NODE_KIND_OPAQUE,
            NODE_KIND_REDUCE_SCATTER, NODE_KIND_REGION, NODE_KIND_RESUME, NODE_KIND_RETURN,
            NODE_KIND_STORE, NODE_KIND_TRAP,
        };
    }
    pub use crate::ir_inner::model::program::ProgramStats;
    pub use crate::ir_inner::model::types::{
        AtomicOp, BinOp, BufferAccess, CollectiveOp, CommGroup, Convention, DataType, OpSignature,
        SubgroupReduceOp, UnOp,
    };
    pub use crate::memory_model;
    pub use crate::memory_model::MemoryOrdering;
    pub use crate::optimizer::passes::fusion_cse::{cse, dce};
    pub use crate::optimizer::pre_lowering::optimize;
    pub use crate::serial::text;
    pub use crate::transform::inline::{inline_calls, inline_calls_with_resolver, OpResolver};
    pub use crate::validate::depth::{
        LimitState, DEFAULT_MAX_CALL_DEPTH, DEFAULT_MAX_NESTING_DEPTH, DEFAULT_MAX_NODE_COUNT,
    };
    pub use crate::validate::validate::validate;
    pub use crate::validate::validation_error::ValidationError;
}

// Audit cleanup A12 (2026-04-30): grouped 13 loose `pub mod` decls into
// 4 logical subdirs. Back-compat `pub use` aliases below preserve the
// historical `vyre_foundation::<file>::*` paths so external callers
// don't break during the transition.

/// Runtime / evaluation surface (cpu_op, cpu_references, ir_eval,
/// match_result, memory_model, perf, program_caps).
pub mod runtime;

/// Dispatch surface (dialect_lookup, extension, extern_registry).
pub mod dispatch;

/// Algebraic-laws surface (algebraic_law_registry, composition).
pub mod algebra;

/// Static-analysis surface (graph_view).
pub mod analysis;

/// Substrate-neutral allocation reservation arithmetic shared by hot paths.
pub mod allocation;

// ---- Back-compat re-exports (old `vyre_foundation::<file>` paths) -----
pub use algebra::algebraic_law_registry;
pub use algebra::algebraic_law_registry::{
    has_law, is_associative, is_commutative, laws_for_op, AlgebraicLaw, AlgebraicLawRegistration,
};
pub use analysis::graph_view;
pub use dispatch::dialect_lookup;
pub use dispatch::extern_registry;
pub use runtime::memory_model;
pub use runtime::memory_model::MemoryOrdering;

/// Endian-fixed encode/decode helpers for `Expr::Opaque` / `Node::Opaque` payloads.
pub mod opaque_payload;

/// Packed AST (VAST) wire layout + host-side tree walks (`docs/parsing-and-frontends.md`).
pub mod vast;

pub use analysis::graph_view::{
    from_graph, to_graph, DataEdge, DataflowKind, EdgeKind, GraphNode, GraphValidateError,
    NodeGraph,
};
pub use dispatch::dialect_lookup::{
    dialect_lookup, install_dialect_lookup, intern_string, AttrSchema, AttrType, Category,
    DialectLookup, InternedOpId, LoweringCtx, LoweringTable, NativeModule, NativeModuleBuilder,
    OpDef, PrimaryBinaryBuilder, PrimaryTextBuilder, ReferenceKind, SecondaryTextBuilder,
    Signature, TextModule, TypedParam,
};
pub use dispatch::extern_registry::{
    all_ops as all_extern_ops, dialects as extern_dialects,
    ops_in_dialect as extern_ops_in_dialect, verify as verify_extern_registry, ExternDialect,
    ExternOp, ExternVerifyError,
};

// V7-API-017: `ir_inner` is intentionally private  -  the public surface
// re-exports through `pub mod ir` above. The internal name is pinned by
// the `vyre_macros::vyre_ast_registry!` proc-macro, which emits literal
// `crate::ir_inner::model::*` paths for the generated decoder cascades.
// Renaming `ir_inner` to `ir` requires a coordinated proc-macro rewrite
// + every dialect that uses `vyre_ast_registry!` recompiling against the
// new path. Tracked for the next semver-major.
mod ir_inner {
    pub mod model;
}
// composition / cpu_op / cpu_references / extension / ir_eval / match_result
// / perf / program_caps relocated in audit cleanup A12 (2026-04-30)  -  they
// now live under runtime/, dispatch/, algebra/, analysis/. Back-compat
// re-exports for external `vyre_foundation::<file>::*` paths land further
// up via `pub use runtime::memory_model;` etc.
pub use algebra::composition;
pub use dispatch::extension;
pub use runtime::cpu_op;
pub use runtime::cpu_references;
pub(crate) use runtime::ir_eval;
pub use runtime::match_result;
pub use runtime::match_result::ByteRange;
pub use runtime::perf;

/// Host-side IR engine helpers (prefix arrays, token filters).
pub mod engine;
/// Legacy lower helpers (transition surface pending driver-tier extraction).
pub mod lower;
/// Pass-orchestration optimizer framework.
pub mod optimizer;
/// Binary wire format + canonical text serialization.
pub mod serial;
/// IR → IR passes: inline, cse, dce, parallelism, compiler primitives.
pub mod transform;
/// Structural + semantic validation of vyre `Program`s.
pub mod validate;
/// Visitor traits + blanket adapters routing Expr/Node variants.
pub mod visit;

/// Self-substrate primitives that the optimizer + scheduler call into.
/// Moved in-tree from vyre-libs to break a cross-workspace dep cycle.
pub mod pass_substrate;

/// Program → substrate-neutral execution planning for fusion, readback,
/// provenance, autotune, and accuracy guard decisions.
pub mod execution_plan;
/// Program → required-capability analysis (used by backends and conform
/// harnesses to skip ops whose lowering needs a capability the backend
/// does not advertise, without maintaining hardcoded exempt lists).
/// Relocated to `runtime/` in audit cleanup A12 (2026-04-30).
pub use runtime::program_caps;

/// Unified error type for validation, wire format, lowering, and execution.
pub mod error;
pub use error::{Error, Result};

/// Soundness lattice for dataflow primitives. Canonical home  -  dataflow
/// engines and composition crates consume from here per the LEGO discipline
/// (consumers always
/// calls vyre, vyre never calls anything else).
pub mod soundness;

/// Test utilities shared across optimizer and transform test suites.
/// `pub(crate)` because they are an internal contract  -  no consumer
/// outside vyre-foundation should depend on these helpers.
#[cfg(test)]
pub(crate) mod test_util;
