//! Graph / AST buffer compositions (`docs/ops-catalog.md` §1).
//!
//! Host-side packed layout lives in [`vyre_foundation::vast`]. The programs
//! here are minimal GPU-facing slices of that contract.

pub mod ast_walk_postorder;
pub mod ast_walk_preorder;

pub use ast_walk_postorder::ast_walk_postorder;
pub use ast_walk_postorder::ast_walk_postorder_nodes;
pub use ast_walk_preorder::ast_walk_preorder;
