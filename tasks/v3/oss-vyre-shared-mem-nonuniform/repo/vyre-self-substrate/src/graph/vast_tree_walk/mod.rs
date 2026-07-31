//! VAST tree-walk self-consumer.
//!
//! Compiler and parser passes need deterministic AST orderings before they can
//! batch semantic checks, lowering, and region rewrites on the GPU. This module
//! consumes the primitive VAST first-child / next-sibling traversal programs
//! directly instead of hand-rolling preorder/postorder walks in a higher tier.

mod builders;
#[cfg(test)]
mod tests;

pub use builders::{
    build_checked_postorder_walk, build_checked_preorder_walk, build_trusted_postorder_walk,
    build_trusted_preorder_walk, build_vast_tree_walk_plan, primitive_op_ids,
};
pub use vyre_primitives::graph::vast_tree_walk::VastTreeWalkProgramPlan as VastTreeWalkPlan;
