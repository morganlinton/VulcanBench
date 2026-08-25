//! IR transformation passes.
//!
//! Before a `Program` is lowered to backend code, it runs through a series
//! of target-independent optimizations and transformations: call inlining,
//! common-subexpression elimination, dead-code elimination, and visitor
//! utilities. These passes are the vyre analogue of LLVM's mid-level IR
//! passes.

/// Call inlining transforms.
///
/// This pass expands `Expr::Call` nodes into the callee's IR body,
/// eliminating kernel-dispatch overhead for small compositional ops.
pub mod inline;

/// Compiler-oriented IR primitives.
pub mod compiler;

/// Shared-nothing parallel dispatch analysis.
pub mod parallelism;

/// IR visitor utilities.
///
/// Provides iterative traversal functions that walk nodes and expressions
/// without recursion, preventing stack overflow on deeply nested programs.
pub mod visit;

/// Induction-variable substitution shared by the optimizer loop passes and the
/// autodiff loop arm. One complete `var -> expr` rewrite over the whole IR.
pub(crate) mod subst;

/// Reverse-mode automatic differentiation via IR transform (RFC 0002).
///
/// Given a forward `Program` + output/input buffer names, emits a backward
/// `Program` computing gradients via the chain rule.
pub mod autodiff;

/// Collective communication rewrites shared by reference and GPU backends.
pub mod collectives;
