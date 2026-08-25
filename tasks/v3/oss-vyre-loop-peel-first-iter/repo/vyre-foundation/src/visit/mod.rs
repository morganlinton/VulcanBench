//! Visitor traits for traversing and lowering vyre IR.
//!
//! # Why this module exists
//!
//! Vyre's IR is `#[non_exhaustive]`. Silent default visitor bodies are
//! therefore a soundness bug: a new `Expr` or `Node` variant can compile
//! while downstream analyses quietly skip it. The visitor contracts in
//! this module are intentionally abstract-by-default so rustc forces an
//! explicit decision at every implementation site.
//!
//! Traversal order is explicit:
//! - `*_preorder` visits the current node before its children.
//! - `*_postorder` visits children before the current node.
//!
//! Visitors may short-circuit traversal by returning `ControlFlow::Break`.

/// Expr visitor contract + recursive traversal entry points.
pub mod expr;
/// Owning child-recursive `Node` map + descendant-search helpers shared
/// by the cleanup catalog (`empty_block_collapse`,
/// `region_promote_singleton_block`, `if_constant_branch_eliminate`,
/// `noop_assign_eliminate`, `loop_trip_zero_eliminate`,
/// `loops::loop_redundant_bound_check_elide`).
pub mod node_map;
/// Cross-cutting visitor contracts: `NodeVisitor`, `Lowerable`, `Evaluatable`.
pub mod traits;

/// Recursive traversal order for visitor entry points and default child walking.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VisitOrder {
    /// Visit the current node before its children.
    Preorder,
    /// Visit the current node after its children.
    Postorder,
}

pub use expr::{
    visit_expr, visit_postorder, visit_preorder, walk_expr_children_default, ExprVisitor,
};
pub use traits::{
    visit_node, visit_node_postorder, visit_node_preorder, walk_node_children_default, Evaluatable,
    Lowerable, NodeVisitor,
};

#[cfg(test)]
mod tests;
