//! Compiler-oriented IR primitives.
//!
//! These primitives describe target-independent compiler data structures and
//! algorithms as vyre operation specifications. Portable kernel source assets
//! live beside these operation specs; backend crates decide how to lower them.

/// Fixed-arity u32 input/output buffer layouts used by compiler-internal ops.
pub mod buffer_layouts;
/// Monotone forward dataflow analysis to a fixed point on GPU.
pub mod dataflow_fixpoint;
/// Dominator tree construction for control-flow analysis.
pub mod dominator_tree;
/// Bounded table-driven recursive-descent parsing primitive.
pub mod recursive_descent;
/// Inventory-based target-text source provider (assets live in driver crates).
pub mod shader_provider;
/// Deterministic workgroup-local string interner (symbol table).
pub mod string_interner;
/// Bounded workgroup-local bump allocator for tree building.
pub mod typed_arena;
/// Bounded post-order tree traversal primitive.
pub mod visitor_walk;

pub(crate) use buffer_layouts::U32X4_INPUTS;
