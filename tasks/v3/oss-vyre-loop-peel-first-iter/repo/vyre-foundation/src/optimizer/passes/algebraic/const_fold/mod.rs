// Compile-time constant folding and algebraic identity elimination.
//
// This pass is organized into submodules by rule category so contributors
// can work on different rule families in parallel without merge conflicts:
//
//   binop_identities  -  binary operator algebraic identities (Add/Sub/Mul/…)
//   unary_rules       -  unary operator simplifications (involutions, idempotent)
//   select_rules      -  Select node optimizations (branch flip, cast canon.)
//   fma_rules         -  FMA synthesis and simplification
//   cast_rules        -  compile-time literal Cast folding
//
// To add a new rule:
//   1. Identify the rule category (unary, binop, select, fma, cast).
//   2. Add the rule to the appropriate submodule's function.
//   3. Add a test to the `tests` module below.
//   4. Run `./cargo_full test -p vyre-foundation --lib` to verify.

pub(crate) mod binop_identities;
mod cast_rules;
mod fma_rules;
/// ROADMAP A11  -  cross-control-flow literal Let propagation built on
/// the A2 `ProgramFacts` substrate. Propagates literal-valued Lets
/// whose name is unique program-wide to every Var read site, even
/// across sibling control-flow branches.
pub mod reaching_def_propagate;
mod select_rules;
mod unary_rules;

pub(crate) use binop_identities::is_float_expr;

use crate::ir::eval::{fold_binary_literal, fold_literal_tree, fold_unary_literal};
use crate::ir::{Expr, Program};
use crate::optimizer::rewrite::rewrite_program;
use crate::optimizer::{vyre_pass, PassAnalysis, PassResult};

/// Fold compile-time-known literal expressions.
#[derive(Debug, Default)]
#[vyre_pass(
    name = "const_fold",
    requires = [],
    invalidates = ["value_numbering"],
    phase = "scalar_algebra",
    boundary_class = "abi_preserving",
    cost_model_family = "scalar"
)]
pub struct ConstFold;

impl ConstFold {
    /// O(1) gate: const-folding only rewrites expressions, which only live
    /// inside Let / Assign / Store / If-cond / Loop bound / AsyncLoad/Store
    /// offset+size / Trap address. Programs made of pure structural nodes
    /// (Return / Barrier / `IndirectDispatch` / `AsyncWait` / Resume) have no
    /// expression tree to fold.
    #[must_use]
    fn analyze_impl(program: &Program) -> PassAnalysis {
        if !program
            .stats()
            .has_any_node_kind(crate::ir::stats::NODE_KIND_EXPRESSION_BEARING_MASK)
        {
            return PassAnalysis::SKIP;
        }
        PassAnalysis::RUN
    }

    /// Fold literal-only expressions.
    ///
    /// AUDIT_2026-04-24 F-CF-01 (closed): `rewrite_program` already
    /// preserves `non_composable_with_self` via `with_rewritten_entry`
    /// (see builder.rs line ~134). Leaving this comment so audits see
    /// the invariant is intentional and traced to the
    /// constructor, not a one-off per-pass call.
    #[must_use]
    pub fn transform(mut program: Program) -> PassResult {
        let mut overall_changed = false;

        let mut lookbehind_changed = false;
        let new_entry =
            binop_identities::fold_mod_lookbehind(program.entry(), &mut lookbehind_changed);
        if lookbehind_changed {
            overall_changed = true;
            program = program.with_rewritten_entry(new_entry);
        }

        let (program, changed) = rewrite_program(program, fold_expr);
        PassResult {
            program,
            changed: overall_changed || changed,
        }
    }
}

pub(crate) fn fold_expr(expr: &Expr) -> Option<Expr> {
    if let Some(folded) = fold_literal_tree(expr) {
        return Some(folded.into_owned());
    }
    match expr {
        Expr::BinOp { op, left, right } => {
            // Algebraic identities (don't require both operands to be literal).
            if let Some(simplified) = binop_identities::simplify_binop(*op, left, right) {
                return Some(simplified);
            }
            fold_binary_literal(op, left, right)
        }
        Expr::UnOp { op, operand } => {
            // Double-negation / idempotent elimination before literal fold.
            if let Some(simplified) = unary_rules::simplify_unop(op, operand) {
                return Some(simplified);
            }
            fold_unary_literal(op, operand)
        }
        Expr::Select {
            cond,
            true_val,
            false_val,
        } => select_rules::simplify_select(cond, true_val, false_val),
        // Fma fold and simplification.
        Expr::Fma { a, b, c } => fma_rules::simplify_fma(a, b, c),
        // Cast folding: compile-time type conversion between scalar literals.
        Expr::Cast { target, value } => cast_rules::fold_cast(target, value),
        _ => None,
    }
}

#[cfg(test)]
mod tests;
