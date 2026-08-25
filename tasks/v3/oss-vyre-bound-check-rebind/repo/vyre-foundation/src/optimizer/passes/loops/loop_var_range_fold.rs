//! ROADMAP A16  -  range facts into cast / branch / bounds-check elision.
//!
//! Loop-induction range slice shipped here. Inside `Loop(i,
//! LitU32(lo), LitU32(hi), body)`, the loop variable `i` has the
//! invariant range `[lo, hi)`. This pass uses that fact to fold
//! comparison-with-literal `If` conditions whose truth value is
//! determined by the range alone:
//!
//! ```text
//! Loop(i, lo, hi, [..., If(Lt(Var(i), LitU32(n)), then, otherwise), ...])
//!     where n >= hi  → If condition is always true  → splat `then`
//!     where n <= lo  → If condition is always false → splat `otherwise`
//! ```
//!
//! Same for `Le`, `Gt`, `Ge`, `Eq`, `Ne` with the appropriate
//! range comparisons.
//!
//! Op id: `vyre-foundation::optimizer::passes::loop_var_range_fold`.
//! Soundness: `Exact`. The range `[lo, hi)` is a structural
//! invariant of the loop construct; every iteration writes `i` to
//! a value in that range. A condition determined entirely by the
//! range gives the same value at every iteration, so replacing
//! the `If` with the constant arm changes nothing observable.
//!
//! Cost direction: monotone-down on `node_count` (one fewer If
//! wrapper per fired fold) and monotone-down on per-iteration
//! branch overhead.
//!
//! Preserves: every analysis. Invalidates: nothing  -  the surviving
//! arm executes on every iteration just as it did before.
//!
//! ## Conservatism
//!
//! - Both loop bounds must be `Expr::LitU32` literals. Symbolic
//!   bounds need the full range substrate (intervals + symbolic
//!   bounds via a downstream range analysis).
//! - The condition must be a comparison of `Var(loop_var)` against
//!   a `LitU32` or `BufLen(buffer)` whose range is proved by
//!   `ProgramShapeFacts`. Compound conditions (BinOp::And / Or chains) and
//!   non-Var operands are skipped  -  the next algebraic round will
//!   simplify them and a future pass can re-attempt.
//! - The loop variable must not be reassigned inside the body
//!   (no `Assign { name: i, .. }`, no `Let { name: i, .. }`, no
//!   nested Loop with `var: i`). Reassignment breaks the range
//!   invariant.
//! - The fold runs bottom-up: nested loops are folded before their
//!   container, so a `Loop(j, ..., [Loop(i, ..., [If(...)])])`
//!   has the inner If folded against `j`'s range first if `j`
//!   appears in the condition, then against `i`'s range.

use crate::ir::{BinOp, Expr, Ident, Node, Program};
use crate::optimizer::program_shape_facts::ProgramShapeFacts;
use crate::optimizer::{vyre_pass, PassAnalysis, PassResult};
use crate::visit::node_map;

/// Fold loop-induction-range-determined `If` conditions.
#[derive(Debug, Default)]
#[vyre_pass(
    name = "loop_var_range_fold",
    requires = ["const_fold"],
    invalidates = []
)]
pub struct LoopVarRangeFoldPass;

impl LoopVarRangeFoldPass {
    /// Skip programs without a foldable If inside a Loop.
    #[must_use]
    fn analyze_impl(program: &Program) -> PassAnalysis {
        // The fold rule requires both an If AND a Loop; if either is
        // absent the pass cannot fire. Compose the bitset check for
        // both kinds before paying the recursive walk.
        use crate::ir::stats::{NODE_KIND_IF, NODE_KIND_LOOP};
        let stats = program.stats();
        if !stats.has_any_node_kind(NODE_KIND_LOOP) || !stats.has_any_node_kind(NODE_KIND_IF) {
            return PassAnalysis::SKIP;
        }
        let shape_facts = ProgramShapeFacts::derive_cached(program);
        if program
            .entry()
            .iter()
            .any(|n| node_map::any_descendant(n, &mut |node| has_foldable_if(node, &shape_facts)))
        {
            PassAnalysis::RUN
        } else {
            PassAnalysis::SKIP
        }
    }

    /// Walk the entry tree and fold every range-determined If.
    #[must_use]
    pub fn transform(program: Program) -> PassResult {
        let mut changed = false;
        let shape_facts = ProgramShapeFacts::derive_cached(&program);
        let program = program.map_entry(|entry| {
            entry
                .into_iter()
                .map(|n| recurse(n, None, &shape_facts, &mut changed))
                .collect()
        });
        PassResult { program, changed }
    }
}

#[derive(Clone, Copy)]
struct LoopRange<'a> {
    var: &'a Ident,
    lo: u32,
    hi: u32,
}

#[derive(Clone, Copy)]
struct BoundRange {
    min: u32,
    max: Option<u32>,
}

#[expect(
    clippy::too_many_lines,
    reason = "range-fold tree rewrite keeps loop/if/block/region reconstruction in one ownership-preserving pass"
)]
fn recurse(
    node: Node,
    range: Option<LoopRange<'_>>,
    shape_facts: &ProgramShapeFacts,
    changed: &mut bool,
) -> Node {
    match node {
        Node::Loop {
            var,
            from,
            to,
            body,
        } => {
            let body_range = match (&from, &to) {
                (Expr::LitU32(lo), Expr::LitU32(hi)) if !body_rebinds_var(&body, &var) => {
                    Some((var.clone(), *lo, *hi))
                }
                _ => None,
            };
            let new_body: Vec<Node> = if let Some((var_owned, lo, hi)) = body_range {
                let inner_range = LoopRange {
                    var: &var_owned,
                    lo,
                    hi,
                };
                body.into_iter()
                    .flat_map(|n| {
                        let folded = recurse(n, Some(inner_range), shape_facts, changed);
                        flatten_block(folded)
                    })
                    .collect()
            } else {
                body.into_iter()
                    .flat_map(|n| {
                        let folded = recurse(n, range, shape_facts, changed);
                        flatten_block(folded)
                    })
                    .collect()
            };
            Node::Loop {
                var,
                from,
                to,
                body: new_body,
            }
        }
        Node::If {
            cond,
            then,
            otherwise,
        } => {
            if let Some(range) = range {
                if let Some(verdict) = condition_verdict(&cond, &range, shape_facts) {
                    *changed = true;
                    let new_body = if verdict { then } else { otherwise };
                    let folded: Vec<Node> = new_body
                        .into_iter()
                        .map(|n| recurse(n, Some(range), shape_facts, changed))
                        .collect();
                    if folded.len() == 1 {
                        return folded
                            .into_iter()
                            .next()
                            .unwrap_or_else(|| unreachable!("folded.len() == 1 by guard above"));
                    }
                    return Node::Block(folded);
                }
            }
            Node::If {
                cond,
                then: then
                    .into_iter()
                    .map(|n| recurse(n, range, shape_facts, changed))
                    .collect(),
                otherwise: otherwise
                    .into_iter()
                    .map(|n| recurse(n, range, shape_facts, changed))
                    .collect(),
            }
        }
        Node::Block(body) => Node::Block(
            body.into_iter()
                .flat_map(|n| {
                    let folded = recurse(n, range, shape_facts, changed);
                    flatten_block(folded)
                })
                .collect(),
        ),
        Node::Region {
            generator,
            source_region,
            body,
        } => {
            let body_vec: Vec<Node> = match std::sync::Arc::try_unwrap(body) {
                Ok(v) => v,
                Err(arc) => (*arc).clone(),
            };
            Node::Region {
                generator,
                source_region,
                body: std::sync::Arc::new(
                    body_vec
                        .into_iter()
                        .flat_map(|n| {
                            let folded = recurse(n, range, shape_facts, changed);
                            flatten_block(folded)
                        })
                        .collect(),
                ),
            }
        }
        other => other,
    }
}

fn flatten_block(node: Node) -> Vec<Node> {
    match node {
        Node::Block(body) => body,
        other => vec![other],
    }
}

/// Decide the truth value of `cond` given a known loop-var range,
/// or return `None` if the range cannot determine the verdict.
fn condition_verdict(
    cond: &Expr,
    range: &LoopRange<'_>,
    shape_facts: &ProgramShapeFacts,
) -> Option<bool> {
    let Expr::BinOp { op, left, right } = cond else {
        return None;
    };
    let (bound, var_on_left) = match (left.as_ref(), right.as_ref()) {
        (Expr::Var(name), bound) if name == range.var => (bound_range(bound, shape_facts)?, true),
        (bound, Expr::Var(name)) if name == range.var => (bound_range(bound, shape_facts)?, false),
        _ => return None,
    };
    let lo = range.lo;
    let hi = range.hi;
    if hi <= lo {
        return None;
    }
    let max_inclusive = hi - 1;
    if matches!(op, BinOp::Eq | BinOp::Ne) {
        return if max_inclusive < bound.min || bound.max.is_some_and(|max| max < lo) {
            Some(matches!(op, BinOp::Ne))
        } else if hi == lo.saturating_add(1) && bound.max == Some(lo) && bound.min == lo {
            Some(matches!(op, BinOp::Eq))
        } else {
            None
        };
    }
    Some(match (op, var_on_left) {
        // Var(i) < lit
        // lit > Var(i)
        (BinOp::Lt, true) | (BinOp::Gt, false) => {
            if bound.min >= hi {
                true
            } else if bound.max.is_some_and(|max| max <= lo) {
                false
            } else {
                return None;
            }
        }
        // lit < Var(i)
        // Var(i) > lit
        (BinOp::Lt, false) | (BinOp::Gt, true) => {
            if bound.min >= max_inclusive {
                false
            } else if bound.max.is_some_and(|max| max < lo) {
                true
            } else {
                return None;
            }
        }
        // Var(i) <= lit
        // lit >= Var(i)
        (BinOp::Le, true) | (BinOp::Ge, false) => {
            if bound.min >= max_inclusive {
                true
            } else if bound.max.is_some_and(|max| max < lo) {
                false
            } else {
                return None;
            }
        }
        // lit <= Var(i)
        // Var(i) >= lit
        (BinOp::Le, false) | (BinOp::Ge, true) => {
            if bound.max.is_some_and(|max| max <= lo) {
                true
            } else if bound.min > max_inclusive {
                false
            } else {
                return None;
            }
        }
        _ => return None,
    })
}

fn bound_range(expr: &Expr, shape_facts: &ProgramShapeFacts) -> Option<BoundRange> {
    match expr {
        Expr::LitU32(value) => Some(BoundRange {
            min: *value,
            max: Some(*value),
        }),
        Expr::BufLen { buffer } => {
            let fact = shape_facts.get(buffer)?;
            Some(BoundRange {
                min: fact.min_count,
                max: fact.max_count,
            })
        }
        _ => None,
    }
}

fn body_rebinds_var(body: &[Node], var: &Ident) -> bool {
    body.iter().any(|n| node_rebinds_var(n, var))
}

fn node_rebinds_var(node: &Node, var: &Ident) -> bool {
    match node {
        Node::Assign { name, .. } | Node::Let { name, .. } => name == var,
        Node::Loop {
            var: inner, body, ..
        } => {
            if inner == var {
                return true;
            }
            body.iter().any(|n| node_rebinds_var(n, var))
        }
        Node::If {
            then, otherwise, ..
        } => {
            then.iter().any(|n| node_rebinds_var(n, var))
                || otherwise.iter().any(|n| node_rebinds_var(n, var))
        }
        Node::Block(body) => body.iter().any(|n| node_rebinds_var(n, var)),
        Node::Region { body, .. } => body.iter().any(|n| node_rebinds_var(n, var)),
        _ => false,
    }
}

fn has_foldable_if(node: &Node, shape_facts: &ProgramShapeFacts) -> bool {
    if let Node::Loop {
        var,
        from,
        to,
        body,
    } = node
    {
        let (lo, hi) = match (from, to) {
            (Expr::LitU32(lo), Expr::LitU32(hi)) if hi > lo => (*lo, *hi),
            _ => return false,
        };
        if body_rebinds_var(body, var) {
            return false;
        }
        let range = LoopRange { var, lo, hi };
        body.iter()
            .any(|n| body_has_foldable_if(n, &range, shape_facts))
    } else {
        false
    }
}

fn body_has_foldable_if(
    node: &Node,
    range: &LoopRange<'_>,
    shape_facts: &ProgramShapeFacts,
) -> bool {
    match node {
        Node::If { cond, .. } => condition_verdict(cond, range, shape_facts).is_some(),
        Node::Block(body) => body
            .iter()
            .any(|n| body_has_foldable_if(n, range, shape_facts)),
        Node::Loop { body, .. } => body
            .iter()
            .any(|n| body_has_foldable_if(n, range, shape_facts)),
        Node::Region { body, .. } => body
            .iter()
            .any(|n| body_has_foldable_if(n, range, shape_facts)),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{BufferAccess, BufferDecl, DataType, Expr, Ident, Node, ShapePredicate};

    fn buf() -> BufferDecl {
        BufferDecl::storage("buf", 0, BufferAccess::ReadWrite, DataType::U32).with_count(8)
    }

    fn program(entry: Vec<Node>) -> Program {
        Program::wrapped(vec![buf()], [1, 1, 1], entry)
    }

    fn program_with_buffers(buffers: Vec<BufferDecl>, entry: Vec<Node>) -> Program {
        Program::wrapped(buffers, [1, 1, 1], entry)
    }

    fn loop_with_if(
        cond: Expr,
        then: Vec<Node>,
        otherwise: Vec<Node>,
        lo: u32,
        hi: u32,
    ) -> Vec<Node> {
        vec![Node::Loop {
            var: Ident::from("i"),
            from: Expr::u32(lo),
            to: Expr::u32(hi),
            body: vec![Node::If {
                cond,
                then,
                otherwise,
            }],
        }]
    }

    fn store(name: &str, idx: Expr, val: Expr) -> Node {
        Node::store(name, idx, val)
    }

    fn count_ifs(nodes: &[Node]) -> usize {
        let mut total = 0;

        for n in nodes {
            match n {
                Node::If {
                    then, otherwise, ..
                } => {
                    total += 1;
                    total += count_ifs(then);
                    total += count_ifs(otherwise);
                }
                Node::Loop { body, .. } => total += count_ifs(body),
                Node::Block(body) => total += count_ifs(body),
                Node::Region { body, .. } => total += count_ifs(body),
                _ => {}
            }
        }
        total
    }

    /// Positive: `Lt(Var(i), n)` with `n >= hi` is always true →
    /// then arm splatted, If gone.
    #[test]
    fn folds_lt_when_lit_at_least_hi() {
        let entry = loop_with_if(
            Expr::lt(Expr::var("i"), Expr::u32(8)),
            vec![store("buf", Expr::var("i"), Expr::u32(1))],
            vec![store("buf", Expr::var("i"), Expr::u32(99))],
            0,
            8,
        );
        let result = LoopVarRangeFoldPass::transform(program(entry));
        assert!(result.changed, "Lt(i, hi) is always true");
        assert_eq!(
            count_ifs(result.program.entry()),
            0,
            "If must be folded out"
        );
    }

    /// Positive: `Lt(Var(i), n)` with `n <= lo` is always false →
    /// otherwise arm splatted, If gone.
    #[test]
    fn folds_lt_when_lit_at_most_lo() {
        let entry = loop_with_if(
            Expr::lt(Expr::var("i"), Expr::u32(0)),
            vec![store("buf", Expr::var("i"), Expr::u32(1))],
            vec![store("buf", Expr::var("i"), Expr::u32(99))],
            0,
            8,
        );
        let result = LoopVarRangeFoldPass::transform(program(entry));
        assert!(result.changed, "Lt(i, 0) is always false for i in [0,8)");
        assert_eq!(count_ifs(result.program.entry()), 0);
    }

    /// Positive: `Eq(Var(i), n)` with `n` outside `[lo, hi)` is
    /// always false.
    #[test]
    fn folds_eq_outside_range() {
        let entry = loop_with_if(
            Expr::eq(Expr::var("i"), Expr::u32(99)),
            vec![store("buf", Expr::var("i"), Expr::u32(1))],
            vec![store("buf", Expr::var("i"), Expr::u32(2))],
            0,
            8,
        );
        let result = LoopVarRangeFoldPass::transform(program(entry));
        assert!(result.changed, "Eq(i, 99) is always false for i in [0,8)");
        assert_eq!(count_ifs(result.program.entry()), 0);
    }

    /// Positive: `Ne(Var(i), n)` with `n` outside range is always
    /// true.
    #[test]
    fn folds_ne_outside_range() {
        let entry = loop_with_if(
            Expr::ne(Expr::var("i"), Expr::u32(99)),
            vec![store("buf", Expr::var("i"), Expr::u32(1))],
            vec![store("buf", Expr::var("i"), Expr::u32(2))],
            0,
            8,
        );
        let result = LoopVarRangeFoldPass::transform(program(entry));
        assert!(result.changed, "Ne(i, 99) is always true for i in [0,8)");
        assert_eq!(count_ifs(result.program.entry()), 0);
    }

    /// Negative: `Lt(Var(i), n)` with `n` strictly inside the
    /// range gives an indeterminate verdict; the If must stay.
    #[test]
    fn keeps_lt_inside_range() {
        let entry = loop_with_if(
            Expr::lt(Expr::var("i"), Expr::u32(4)),
            vec![store("buf", Expr::var("i"), Expr::u32(1))],
            vec![store("buf", Expr::var("i"), Expr::u32(2))],
            0,
            8,
        );
        let result = LoopVarRangeFoldPass::transform(program(entry));
        assert!(!result.changed);
        assert_eq!(count_ifs(result.program.entry()), 1);
    }

    /// Negative: condition compares against another Var, not a
    /// literal  -  the range fact doesn't help.
    #[test]
    fn keeps_var_lt_var() {
        let entry = loop_with_if(
            Expr::lt(Expr::var("i"), Expr::var("k")),
            vec![store("buf", Expr::var("i"), Expr::u32(1))],
            vec![store("buf", Expr::var("i"), Expr::u32(2))],
            0,
            8,
        );
        let result = LoopVarRangeFoldPass::transform(program(entry));
        assert!(!result.changed);
    }

    /// Negative: body reassigns the loop var → range invariant
    /// broken → the If must stay.
    #[test]
    fn keeps_when_body_assigns_loop_var() {
        let entry = vec![Node::Loop {
            var: Ident::from("i"),
            from: Expr::u32(0),
            to: Expr::u32(8),
            body: vec![
                Node::Assign {
                    name: Ident::from("i"),
                    value: Expr::u32(99),
                },
                Node::If {
                    cond: Expr::lt(Expr::var("i"), Expr::u32(8)),
                    then: vec![store("buf", Expr::u32(0), Expr::u32(1))],
                    otherwise: vec![],
                },
            ],
        }];
        let result = LoopVarRangeFoldPass::transform(program(entry));
        assert!(!result.changed);
    }

    /// Negative: runtime bounds skip  -  the range substrate needs
    /// literal `from`/`to`.
    #[test]
    fn keeps_runtime_bound_loop() {
        let entry = vec![Node::Loop {
            var: Ident::from("i"),
            from: Expr::u32(0),
            to: Expr::var("n"),
            body: vec![Node::If {
                cond: Expr::lt(Expr::var("i"), Expr::u32(99)),
                then: vec![store("buf", Expr::u32(0), Expr::u32(1))],
                otherwise: vec![],
            }],
        }];
        let result = LoopVarRangeFoldPass::transform(program(entry));
        assert!(!result.changed);
    }

    /// `analyze` short-circuits when no candidate exists.
    #[test]
    fn analyze_skips_program_without_loop() {
        let entry = vec![store("buf", Expr::u32(0), Expr::u32(1))];
        match crate::optimizer::ProgramPass::analyze(&LoopVarRangeFoldPass, &program(entry)) {
            PassAnalysis::SKIP => {}
            other => panic!("expected SKIP, got {other:?}"),
        }
    }

    /// Positive: nested Loop  -  the inner If folds against the
    /// inner range.
    #[test]
    fn folds_inside_nested_loop() {
        let entry = vec![Node::Loop {
            var: Ident::from("i"),
            from: Expr::u32(0),
            to: Expr::u32(4),
            body: vec![Node::Loop {
                var: Ident::from("j"),
                from: Expr::u32(0),
                to: Expr::u32(4),
                body: vec![Node::If {
                    cond: Expr::lt(Expr::var("j"), Expr::u32(4)),
                    then: vec![store("buf", Expr::var("j"), Expr::u32(1))],
                    otherwise: vec![],
                }],
            }],
        }];
        let result = LoopVarRangeFoldPass::transform(program(entry));
        assert!(
            result.changed,
            "inner Lt(j, 4) is always true for j in [0,4)"
        );
        assert_eq!(count_ifs(result.program.entry()), 0);
    }

    #[test]
    fn folds_var_lt_buf_len_when_shape_min_covers_loop_hi() {
        let buffers = vec![
            BufferDecl::read("input", 0, DataType::U32)
                .with_shape_predicate(ShapePredicate::AtLeast(8)),
            BufferDecl::storage("buf", 1, BufferAccess::ReadWrite, DataType::U32).with_count(8),
        ];
        let entry = loop_with_if(
            Expr::lt(Expr::var("i"), Expr::buf_len("input")),
            vec![store("buf", Expr::var("i"), Expr::u32(1))],
            vec![store("buf", Expr::var("i"), Expr::u32(99))],
            0,
            8,
        );

        let program = program_with_buffers(buffers, entry);
        assert_eq!(
            crate::optimizer::ProgramPass::analyze(&LoopVarRangeFoldPass, &program),
            PassAnalysis::RUN,
            "shape-backed buf_len facts must make the branch visibly foldable during analysis"
        );
        let result = LoopVarRangeFoldPass::transform(program);
        assert!(result.changed, "i < buf_len(input) is true when len >= 8");
        assert_eq!(count_ifs(result.program.entry()), 0);
    }

    #[test]
    fn keeps_var_lt_buf_len_when_shape_min_is_too_weak() {
        let buffers = vec![
            BufferDecl::read("input", 0, DataType::U32)
                .with_shape_predicate(ShapePredicate::AtLeast(4)),
            BufferDecl::storage("buf", 1, BufferAccess::ReadWrite, DataType::U32).with_count(8),
        ];
        let entry = loop_with_if(
            Expr::lt(Expr::var("i"), Expr::buf_len("input")),
            vec![store("buf", Expr::var("i"), Expr::u32(1))],
            vec![store("buf", Expr::var("i"), Expr::u32(99))],
            0,
            8,
        );

        let result = LoopVarRangeFoldPass::transform(program_with_buffers(buffers, entry));
        assert!(
            !result.changed,
            "len >= 4 cannot prove i < len for every i in [0,8)"
        );
        assert_eq!(count_ifs(result.program.entry()), 1);
    }

    #[test]
    fn folds_var_ge_buf_len_false_when_shape_min_exceeds_loop_max() {
        let buffers = vec![
            BufferDecl::read("input", 0, DataType::U32)
                .with_shape_predicate(ShapePredicate::AtLeast(9)),
            BufferDecl::storage("buf", 1, BufferAccess::ReadWrite, DataType::U32).with_count(8),
        ];
        let entry = loop_with_if(
            Expr::ge(Expr::var("i"), Expr::buf_len("input")),
            vec![store("buf", Expr::var("i"), Expr::u32(1))],
            vec![store("buf", Expr::var("i"), Expr::u32(2))],
            0,
            8,
        );

        let result = LoopVarRangeFoldPass::transform(program_with_buffers(buffers, entry));
        assert!(
            result.changed,
            "i >= buf_len(input) is false when i <= 7 and len >= 9"
        );
        assert_eq!(count_ifs(result.program.entry()), 0);
    }

    #[test]
    fn folds_eq_buf_len_false_when_shape_range_is_disjoint() {
        let buffers = vec![
            BufferDecl::read("input", 0, DataType::U32)
                .with_shape_predicate(ShapePredicate::AtLeast(16)),
            BufferDecl::storage("buf", 1, BufferAccess::ReadWrite, DataType::U32).with_count(8),
        ];
        let entry = loop_with_if(
            Expr::eq(Expr::var("i"), Expr::buf_len("input")),
            vec![store("buf", Expr::var("i"), Expr::u32(1))],
            vec![store("buf", Expr::var("i"), Expr::u32(2))],
            0,
            8,
        );

        let result = LoopVarRangeFoldPass::transform(program_with_buffers(buffers, entry));
        assert!(
            result.changed,
            "i == buf_len(input) is false when i in [0,8) and len >= 16"
        );
        assert_eq!(count_ifs(result.program.entry()), 0);
    }
}
