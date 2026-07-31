//! Dead-code elimination  -  engine + ProgramPass registration colocated.
//!
//! Audit cleanup A7 (2026-04-30): hoisted from `transform/optimize/dce/`
//! mirroring the A6 CSE consolidation. Engine + ProgramPass registration share one home.
//!
//! ## Layout
//!
//! - `engine`  -  the `dce(program)` entry point (runs
//!   `eliminate_dead_lets` → `eliminate_unreachable` → `eliminate_dead_lets`
//!   again to catch bindings that became dead after unreachable code was
//!   stripped).
//! - `eliminate_dead_lets.rs`  -  backward liveness pass that strips dead
//!   `let` bindings. Drops a `let` when its name is not live AND its value
//!   is pure (effectful nodes always preserved).
//! - `eliminate_unreachable.rs`  -  forward pass folding constant branches +
//!   truncating after `Return`.
//! - `collect_expr_refs.rs`  -  iterative visitor accumulating every
//!   `Expr::Var` name (conservative liveness over-approximation).
//! - `const_truth.rs`  -  partial constant evaluator for boolean expressions.
//! - `const_loop_empty.rs`  -  detects statically empty loops.
//! - `live_result.rs`  -  result bundle returned by liveness pruning.
//! - `reachable_prefix.rs`  -  slice the node list up to first unconditional
//!   `Return`.
//! - `program_pass.rs`  -  the registered `DcePass` (ProgramPass impl).
//!
//! `expr_has_effect` is shared with CSE  -  single source of truth in
//! `super::cse::expr_has_effect`.

// `expr_has_effect` is shared with CSE  -  single source of truth in
// `super::cse::expr_has_effect`. Re-exported here so the DCE engine can
// say `super::expr_has_effect` without learning about the cse path.
pub(crate) use super::cse::expr_has_effect;

/// Collect variable references from an expression tree.
pub(crate) use collect_expr_refs::collect_expr_refs;
/// Evaluate whether loop bounds make a loop statically empty.
pub(crate) use const_loop_empty::const_loop_empty;
/// Evaluate an expression as a static boolean when possible.
pub(crate) use const_truth::const_truth;
/// Remove let-bindings whose values are neither live nor effectful.
pub(crate) use eliminate_dead_lets::eliminate_dead_lets;
/// Trim unreachable control-flow nodes after static terminators.
pub(crate) use eliminate_unreachable::eliminate_unreachable;
/// Result bundle returned by liveness pruning.
pub(crate) use live_result::LiveResult;
/// Return the node prefix reachable before an unconditional return.
pub(crate) use reachable_prefix::reachable_prefix;

/// Iterative `Expr::Var` collector.
pub mod collect_expr_refs;
/// Detect statically empty loops.
pub mod const_loop_empty;
/// Partial constant evaluator for boolean expressions.
pub mod const_truth;
/// Backward liveness pass that strips dead `let` bindings.
pub mod eliminate_dead_lets;
/// Forward pass folding constant branches and truncating after `Return`.
pub mod eliminate_unreachable;
/// Entry point for the dead-code elimination pass.
pub mod engine;
/// Result bundle returned by `eliminate_dead_lets`.
pub mod live_result;
/// Registered `DcePass` (ProgramPass impl) for the engine.
pub mod program_pass;
/// Slice the node list up to the first unconditional `Return`.
pub mod reachable_prefix;

pub use engine::dce;
pub use program_pass::DcePass;

/// DCE adversarial test suite.
#[cfg(test)]
mod tests;
