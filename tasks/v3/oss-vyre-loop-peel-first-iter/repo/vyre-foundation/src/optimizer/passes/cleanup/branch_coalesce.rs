//! `branch_coalesce`  -  collapse nested `Node::If` whose outer body is
//! exactly one inner `If` with no `otherwise` arm into a single `If`
//! whose condition is `And(outer_cond, inner_cond)`.
//!
//! Op id: `vyre-foundation::optimizer::passes::branch_coalesce`.
//! Soundness: `Exact`  -  both `Then` arms run only when both
//! conditions are true; both `Otherwise` arms are empty so there is no
//! else-arm semantics to preserve. Cost direction: monotone-down on
//! `node_count + control_flow_count`. Preserves: every analysis.
//! Invalidates: nothing.
//!
//! ## Rule
//!
//! ```text
//! Node::If {
//!     cond: c1,
//!     then: [Node::If { cond: c2, then: body, otherwise: [] }],
//!     otherwise: [],
//! }
//! →
//! Node::If {
//!     cond: And(c1, c2),
//!     then: body,
//!     otherwise: [],
//! }
//! ```
//!
//! Comes up frequently after region inlining and CSE: domain code
//! often writes `if (in_bounds(x)) { if (matches_pattern(x)) { ... } }`
//! and the optimizer should see one combined predicate instead of two
//! nested branches. Coalescing also unblocks downstream
//! const-fold/boolean-simplification (ROADMAP A25) since the combined
//! predicate may collapse further when one of the conditions is a
//! literal.
//!
//! Does NOT fire (deliberately):
//!   - when the outer `then` has more than one child node  -  sibling
//!     statements would otherwise be hoisted into the inner branch and
//!     change observable order.
//!   - when either `otherwise` arm is non-empty  -  would lose else-arm
//!     semantics.
//!   - when the conditions involve side-effects (Load, Atomic, Call,
//!     Opaque). Even pure-looking expression evaluation may matter when
//!     the inner cond depends on a state mutation hidden inside the
//!     outer cond's evaluation; the conservative rule keeps both
//!     conditions evaluated lexically by skipping when either touches
//!     impure constructs.

use crate::ir::{Expr, Node, Program};
use crate::optimizer::{vyre_pass, PassAnalysis, PassResult};
use crate::visit::node_map;

/// Drop the inner `Node::If` and merge its condition into the outer's
/// via logical AND.
#[derive(Debug, Default)]
#[vyre_pass(
    name = "branch_coalesce",
    requires = [],
    invalidates = [],
    phase = "cleanup",
    boundary_class = "abi_preserving",
    cost_model_family = "fusion"
)]
pub struct BranchCoalesce;

impl BranchCoalesce {
    /// Skip the pass when no body in the program contains a nested-If
    /// pair matching the rule.
    #[must_use]
    fn analyze_impl(program: &Program) -> PassAnalysis {
        // Coalescing rewrites require an If; absent any If the
        // recursive walk would find nothing to coalesce.
        if !program
            .stats()
            .has_any_node_kind(crate::ir::stats::NODE_KIND_IF)
        {
            return PassAnalysis::SKIP;
        }
        if program
            .entry()
            .iter()
            .any(|n| node_map::any_descendant(n, &mut is_coalesceable_if))
        {
            PassAnalysis::RUN
        } else {
            PassAnalysis::SKIP
        }
    }

    /// Walk the program; replace every coalesceable nested If with a
    /// single If carrying the conjoined predicate.
    #[must_use]
    pub fn transform(program: Program) -> PassResult {
        let mut changed = false;
        let program = program.map_entry(|entry| {
            entry
                .into_iter()
                .map(|n| rewrite_node(n, &mut changed))
                .collect()
        });
        PassResult { program, changed }
    }
}

/// Recurse into `node`'s descendants, then attempt to coalesce at
/// `node` itself. Children are rewritten first so deeply-nested
/// `If(c1) { If(c2) { If(c3) { ... } } }` chains coalesce bottom-up
/// in a single pass.
fn rewrite_node(node: Node, changed: &mut bool) -> Node {
    let recursed = node_map::map_children(node, &mut |child| rewrite_node(child, changed));
    let recursed = node_map::map_body(recursed, &mut |body| {
        body.into_iter().map(|n| rewrite_node(n, changed)).collect()
    });
    coalesce_if(recursed, changed)
}

/// Apply the coalesce rule to `node` if it matches; otherwise return
/// it unchanged.
fn coalesce_if(node: Node, changed: &mut bool) -> Node {
    let Node::If {
        cond: outer_cond,
        then,
        otherwise,
    } = node
    else {
        return node_unchanged_helper(node);
    };
    if !otherwise.is_empty() || then.len() != 1 {
        return Node::If {
            cond: outer_cond,
            then,
            otherwise,
        };
    }
    let mut then_iter = then.into_iter();
    let inner = then_iter
        .next()
        .unwrap_or_else(|| unreachable!("then.len() == 1 by guard above"));
    let Node::If {
        cond: inner_cond,
        then: inner_then,
        otherwise: inner_otherwise,
    } = inner
    else {
        return Node::If {
            cond: outer_cond,
            then: vec![inner],
            otherwise,
        };
    };
    if !inner_otherwise.is_empty() {
        return Node::If {
            cond: outer_cond,
            then: vec![Node::If {
                cond: inner_cond,
                then: inner_then,
                otherwise: inner_otherwise,
            }],
            otherwise,
        };
    }
    if !is_pure_bool_expr(&outer_cond) || !is_pure_bool_expr(&inner_cond) {
        return Node::If {
            cond: outer_cond,
            then: vec![Node::If {
                cond: inner_cond,
                then: inner_then,
                otherwise: inner_otherwise,
            }],
            otherwise,
        };
    }
    *changed = true;
    Node::If {
        cond: Expr::and(outer_cond, inner_cond),
        then: inner_then,
        otherwise,
    }
}

fn node_unchanged_helper(node: Node) -> Node {
    node
}

/// Cheap matcher used by `analyze`: true iff `node` is an outer-If
/// whose body is a single inner-If with empty otherwise. Keeps the
/// scheduler from running `transform` on programs that have no work.
fn is_coalesceable_if(node: &Node) -> bool {
    let Node::If {
        cond: outer_cond,
        then,
        otherwise,
    } = node
    else {
        return false;
    };
    if !otherwise.is_empty() || then.len() != 1 {
        return false;
    }
    let Node::If {
        cond: inner_cond,
        otherwise: inner_otherwise,
        ..
    } = &then[0]
    else {
        return false;
    };
    if !inner_otherwise.is_empty() {
        return false;
    }
    is_pure_bool_expr(outer_cond) && is_pure_bool_expr(inner_cond)
}

/// True iff `expr` produces a boolean value via pure operations only.
/// Loads, atomics, calls, and opaque extensions are rejected  -  their
/// repeated or reordered evaluation could change observable behavior.
fn is_pure_bool_expr(expr: &Expr) -> bool {
    match expr {
        Expr::BinOp { left, right, .. } => is_pure_bool_expr(left) && is_pure_bool_expr(right),
        Expr::UnOp { operand, .. } => is_pure_bool_expr(operand),
        Expr::Select {
            cond,
            true_val,
            false_val,
        } => is_pure_bool_expr(cond) && is_pure_bool_expr(true_val) && is_pure_bool_expr(false_val),
        Expr::Cast { value, .. } => is_pure_bool_expr(value),
        // Builtins are pure and observably free.
        Expr::LitBool(_)
        | Expr::Var(_)
        | Expr::InvocationId { .. }
        | Expr::WorkgroupId { .. }
        | Expr::LocalId { .. }
        | Expr::SubgroupLocalId
        | Expr::SubgroupSize
        // Literals other than bool are fine as operands of pure binops
        // (e.g. `i < n` where `n` is a u32 literal).
        | Expr::LitU32(_)
        | Expr::LitI32(_)
        | Expr::LitF32(_)
        // BufLen returns the bound buffer's length  -  a dispatch-time
        // constant, no observable side effect; conjoining `i < buf_len`
        // with a sibling predicate is safe.
        | Expr::BufLen { .. } => true,
        // Fma is fused-multiply-add  -  pure arithmetic when its operands
        // are pure. Reject when any operand is impure.
        Expr::Fma { a, b, c } => is_pure_bool_expr(a) && is_pure_bool_expr(b) && is_pure_bool_expr(c),
        // Anything that reads memory or invokes side effects is
        // rejected to keep ordering observable.
        Expr::Load { .. }
        | Expr::Atomic { .. }
        | Expr::Call { .. }
        | Expr::Opaque(_)
        | Expr::SubgroupBallot { .. }
        | Expr::SubgroupShuffle { .. }
        | Expr::SubgroupReduce { .. } => false,
    }
}

#[cfg(test)]
mod tests;
