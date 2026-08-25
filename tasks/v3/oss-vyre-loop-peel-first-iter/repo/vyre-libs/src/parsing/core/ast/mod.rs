//! Data-parallel generic AST building blocks.

/// Shared AST node layout definitions.
pub mod node;
/// Shunting-yard AST extraction, gated with the C parser surface it depends on.
#[cfg(feature = "c-parser")]
pub mod shunting;

/// Parallel Prefix-Scan binding map.
#[cfg(feature = "c-parser")]
pub mod binding;
/// Parallel basic-block metadata for structured control flow.
#[cfg(feature = "c-parser")]
pub mod blocks;
