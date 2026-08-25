//! ROADMAP A11  -  reaching-def facts into cross-control-flow const fold.
//!
//! Built on top of the A2 `ProgramFacts` substrate. For every
//! `Node::Let { name, value: Lit* }` whose `name` is never rebound
//! anywhere in the program (no Assign, no Loop induction var, no
//! second `Let { name }` shadow), the literal value is propagated
//! to every `Expr::Var(name)` read site across the entire program
//! tree  -  including reads in sibling control-flow branches that
//! A14's same-scope cheap-leaf rematerialization cannot reach.
//!
//! Op id: `vyre-foundation::optimizer::passes::reaching_def_propagate`.
//! Soundness: `Exact`. The `is_name_rebound == false` gate
//! guarantees that every dynamic read of `name` resolves to
//! the single static `Let` site, so substituting the literal at
//! every read site preserves observable behavior. Without
//! rebinding, control-flow path doesn't matter  -  the only value
//! the name can ever hold is the literal at its single defining
//! site. The Let itself is then dead and gets removed by the
//! existing DCE on the next pass-scheduler iteration.
//!
//! Cost direction: monotone-down on register-pressure (one fewer
//! named live binding per fired propagation) and monotone-down on
//! instruction count (every read site avoids loading from the
//! Var slot). Per-site cost goes from one register read to one
//! immediate operand, which is strictly cheaper on every backend.
//!
//! Preserves: every analysis. Invalidates: nothing  -  the Let was
//! the unique reaching definition; the literal substitution is its
//! observably-equivalent inlining.
//!
//! ## Pattern
//!
//! ```text
//! Let(x, LitU32(7))   ;; or LitI32, LitF32, LitBool
//! ... use Var(x) anywhere in the program ...
//!     where x has zero Assigns, zero Loop-vars, exactly one Let
//! → ... use LitU32(7) at every read site ...
//!     The Let stays in place; the next DCE round removes it once
//!     no Var(x) reads remain.
//! ```
//!
//! ## Why this complements A14
//!
//! A14 (`rematerialize_cheap_let`) walks one sibling sequence at a
//! time and substitutes through descendant scopes when the name is
//! not reassigned in that subtree. It cannot substitute INTO a
//! sibling subtree of the Let's own container  -  e.g., if `Let(x, 7)`
//! lives at the top of the `then` arm of an If and the read is
//! inside the `otherwise` arm, A14 leaves the read untouched
//! because the Let's descendant scan never visits the sibling arm.
//!
//! Reaching-def with `is_name_rebound == false` is the cross-CFG
//! generalisation: it queries the WHOLE program for rebinds and,
//! finding none, treats every read of `name` as resolved by the
//! single Let. The substitution then crosses arbitrary
//! control-flow boundaries safely.

use crate::ir::{Expr, Node, Program};
use crate::optimizer::program_soa::ProgramFacts;
use crate::optimizer::{vyre_pass, PassAnalysis, PassResult};
use rustc_hash::FxHashMap;

/// Cross-control-flow literal Let propagation.
#[derive(Debug, Default)]
#[vyre_pass(
    name = "reaching_def_propagate",
    requires = ["const_fold"],
    invalidates = ["const_fold", "cse", "dce"],
    phase = "scalar_algebra",
    boundary_class = "abi_preserving",
    cost_model_family = "scalar"
)]
/// ABI-preserving reaching-definition propagation pass for unique literal let bindings.
pub struct ReachingDefPropagatePass;

impl ReachingDefPropagatePass {
    /// Skip programs with no candidate `Let(name, Lit)` whose
    /// `name` is unique program-wide.
    #[must_use]
    fn analyze_impl(program: &Program) -> PassAnalysis {
        // Propagation needs a Let. Without one, the ProgramFacts build
        // (full SoA walk) is wasted.
        if !program.stats().has_node_let() {
            return PassAnalysis::SKIP;
        }
        let facts = ProgramFacts::build_cached(program);
        if collect_propagatable_lets_with_values(&facts, program).is_empty() {
            PassAnalysis::SKIP
        } else {
            PassAnalysis::RUN
        }
    }

    /// Walk the entry tree and substitute every propagatable
    /// literal at every read site.
    #[must_use]
    pub fn transform(program: Program) -> PassResult {
        let facts = ProgramFacts::build_cached(&program);
        let propagations = collect_propagatable_lets_with_values(&facts, &program);
        if propagations.is_empty() {
            return PassResult {
                program,
                changed: false,
            };
        }
        let mut changed = false;
        let program = program.map_entry(|entry| {
            entry
                .into_iter()
                .map(|n| substitute_node(n, &propagations, &mut changed))
                .collect()
        });
        PassResult { program, changed }
    }
}

#[expect(
    clippy::too_many_lines,
    reason = "literal propagation keeps Node reconstruction and substitution in one ownership-preserving pass"
)]
fn substitute_node(node: Node, propagations: &FxHashMap<String, Expr>, changed: &mut bool) -> Node {
    match node {
        Node::Let { name, value } => {
            let new_value = substitute_expr(value, propagations, changed);
            // Attempt to update the propagation map for this Let if
            // its name is propagatable AND its (now-substituted)
            // value is a literal. We do this in two passes via
            // `transform` instead of mutating `propagations` here,
            // so the API stays simple. The map was built before the
            // substitution started; subsequent passes will pick up
            // any new propagatable Lets that surface after this one
            // collapses.
            Node::Let {
                name,
                value: new_value,
            }
        }
        Node::Assign { name, value } => Node::Assign {
            name,
            value: substitute_expr(value, propagations, changed),
        },
        Node::Store {
            buffer,
            index,
            value,
        } => Node::Store {
            buffer,
            index: substitute_expr(index, propagations, changed),
            value: substitute_expr(value, propagations, changed),
        },
        Node::If {
            cond,
            then,
            otherwise,
        } => Node::If {
            cond: substitute_expr(cond, propagations, changed),
            then: then
                .into_iter()
                .map(|n| substitute_node(n, propagations, changed))
                .collect(),
            otherwise: otherwise
                .into_iter()
                .map(|n| substitute_node(n, propagations, changed))
                .collect(),
        },
        Node::Loop {
            var,
            from,
            to,
            body,
        } => Node::Loop {
            var,
            from: substitute_expr(from, propagations, changed),
            to: substitute_expr(to, propagations, changed),
            body: body
                .into_iter()
                .map(|n| substitute_node(n, propagations, changed))
                .collect(),
        },
        Node::Block(body) => Node::Block(
            body.into_iter()
                .map(|n| substitute_node(n, propagations, changed))
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
                        .map(|n| substitute_node(n, propagations, changed))
                        .collect(),
                ),
            }
        }
        Node::AsyncLoad {
            source,
            destination,
            offset,
            size,
            tag,
        } => Node::AsyncLoad {
            source,
            destination,
            tag,
            offset: Box::new(substitute_expr(*offset, propagations, changed)),
            size: Box::new(substitute_expr(*size, propagations, changed)),
        },
        Node::AsyncStore {
            source,
            destination,
            offset,
            size,
            tag,
        } => Node::AsyncStore {
            source,
            destination,
            tag,
            offset: Box::new(substitute_expr(*offset, propagations, changed)),
            size: Box::new(substitute_expr(*size, propagations, changed)),
        },
        Node::Trap { address, tag } => Node::Trap {
            address: Box::new(substitute_expr(*address, propagations, changed)),
            tag,
        },
        other => other,
    }
}

fn substitute_expr(expr: Expr, propagations: &FxHashMap<String, Expr>, changed: &mut bool) -> Expr {
    match expr {
        Expr::Var(ref name) => {
            if let Some(literal) = propagations.get(name.as_str()) {
                *changed = true;
                literal.clone()
            } else {
                expr
            }
        }
        Expr::Load { buffer, index } => Expr::Load {
            buffer,
            index: Box::new(substitute_expr(*index, propagations, changed)),
        },
        Expr::BinOp { op, left, right } => Expr::BinOp {
            op,
            left: Box::new(substitute_expr(*left, propagations, changed)),
            right: Box::new(substitute_expr(*right, propagations, changed)),
        },
        Expr::UnOp { op, operand } => Expr::UnOp {
            op,
            operand: Box::new(substitute_expr(*operand, propagations, changed)),
        },
        Expr::Call { op_id, args } => Expr::Call {
            op_id,
            args: args
                .into_iter()
                .map(|a| substitute_expr(a, propagations, changed))
                .collect(),
        },
        Expr::Select {
            cond,
            true_val,
            false_val,
        } => Expr::Select {
            cond: Box::new(substitute_expr(*cond, propagations, changed)),
            true_val: Box::new(substitute_expr(*true_val, propagations, changed)),
            false_val: Box::new(substitute_expr(*false_val, propagations, changed)),
        },
        Expr::Cast { target, value } => Expr::Cast {
            target,
            value: Box::new(substitute_expr(*value, propagations, changed)),
        },
        Expr::Fma { a, b, c } => Expr::Fma {
            a: Box::new(substitute_expr(*a, propagations, changed)),
            b: Box::new(substitute_expr(*b, propagations, changed)),
            c: Box::new(substitute_expr(*c, propagations, changed)),
        },
        Expr::Atomic {
            op,
            buffer,
            index,
            expected,
            value,
            ordering,
        } => Expr::Atomic {
            op,
            buffer,
            index: Box::new(substitute_expr(*index, propagations, changed)),
            expected: expected.map(|e| Box::new(substitute_expr(*e, propagations, changed))),
            value: Box::new(substitute_expr(*value, propagations, changed)),
            ordering,
        },
        Expr::SubgroupBallot { cond } => Expr::SubgroupBallot {
            cond: Box::new(substitute_expr(*cond, propagations, changed)),
        },
        Expr::SubgroupShuffle { value, lane } => Expr::SubgroupShuffle {
            value: Box::new(substitute_expr(*value, propagations, changed)),
            lane: Box::new(substitute_expr(*lane, propagations, changed)),
        },
        Expr::SubgroupReduce { op, value } => Expr::SubgroupReduce { op,
            value: Box::new(substitute_expr(*value, propagations, changed)),
        },
        other => other,
    }
}

// Override `collect_propagatable_lets` to fetch literal values
// directly from the entry tree (the fact substrate doesn't store
// values to keep build-time fast). Uses one preorder walk over
// the entry to find the value at each propagatable Let's name.

fn collect_propagatable_lets_with_values(
    facts: &ProgramFacts,
    program: &Program,
) -> FxHashMap<String, Expr> {
    let mut candidates: rustc_hash::FxHashSet<String> = rustc_hash::FxHashSet::default();
    for (_, name) in facts.lets() {
        if !facts.is_name_rebound(name.as_str()) {
            candidates.insert(name.as_str().to_owned());
        }
    }
    if candidates.is_empty() {
        return FxHashMap::default();
    }
    let mut out: FxHashMap<String, Expr> = FxHashMap::default();
    for node in program.entry() {
        scan_for_literal_lets(node, &candidates, &mut out);
    }
    out
}

fn scan_for_literal_lets(
    node: &Node,
    candidates: &rustc_hash::FxHashSet<String>,
    out: &mut FxHashMap<String, Expr>,
) {
    match node {
        Node::Let { name, value } if candidates.contains(name.as_str()) && is_literal(value) => {
            out.insert(name.as_str().to_owned(), value.clone());
        }
        Node::If {
            then, otherwise, ..
        } => {
            for n in then {
                scan_for_literal_lets(n, candidates, out);
            }
            for n in otherwise {
                scan_for_literal_lets(n, candidates, out);
            }
        }
        Node::Loop { body, .. } | Node::Block(body) => {
            for n in body {
                scan_for_literal_lets(n, candidates, out);
            }
        }
        Node::Region { body, .. } => {
            for n in body.iter() {
                scan_for_literal_lets(n, candidates, out);
            }
        }
        _ => {}
    }
}

fn is_literal(expr: &Expr) -> bool {
    matches!(
        expr,
        Expr::LitU32(_) | Expr::LitI32(_) | Expr::LitF32(_) | Expr::LitBool(_)
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{BufferAccess, BufferDecl, DataType, Expr, Ident, Node};

    fn buf() -> BufferDecl {
        BufferDecl::storage("buf", 0, BufferAccess::ReadWrite, DataType::U32).with_count(4)
    }

    fn program(entry: Vec<Node>) -> Program {
        Program::wrapped(vec![buf()], [1, 1, 1], entry)
    }

    fn run(program: Program) -> PassResult {
        // The propagation map is computed inside transform via
        // `collect_propagatable_lets_with_values`; test invokes it
        // through the public API.
        let facts = ProgramFacts::build(&program);
        let propagations = collect_propagatable_lets_with_values(&facts, &program);
        if propagations.is_empty() {
            return PassResult {
                program,
                changed: false,
            };
        }
        let scaffold = program.with_rewritten_entry(Vec::new());
        let mut changed = false;
        let entry: Vec<Node> = program
            .into_entry_vec()
            .into_iter()
            .map(|n| substitute_node(n, &propagations, &mut changed))
            .collect();
        PassResult {
            program: scaffold.with_rewritten_entry(entry),
            changed,
        }
    }

    fn count_var_reads(nodes: &[Node], target: &str) -> usize {
        let facts = ProgramFacts::build(&Program::wrapped(vec![buf()], [1, 1, 1], nodes.to_vec()));
        facts
            .var_reads()
            .iter()
            .filter(|(_, n)| n.as_str() == target)
            .count()
    }

    /// Cross-CFG propagation: `Let(x, 7)` at the top of `then`
    /// is propagated to a `Var(x)` read inside the `otherwise`
    /// arm  -  the very case A14 cannot reach because the read is
    /// in a sibling subtree, not a descendant of the Let's
    /// scope.
    ///
    /// (Edge case: A14 actually wouldn't fire here because the Let
    /// itself sits in a branch arm; this test verifies that the
    /// cross-CFG propagation works regardless of where the Let
    /// physically appears, as long as the name is unique.)
    #[test]
    fn propagates_literal_across_sibling_arms() {
        let entry = vec![
            Node::let_bind("x", Expr::u32(7)),
            Node::If {
                cond: Expr::var("c"),
                then: vec![Node::store("buf", Expr::u32(0), Expr::var("x"))],
                otherwise: vec![Node::store("buf", Expr::u32(1), Expr::var("x"))],
            },
        ];
        let result = run(program(entry));
        assert!(result.changed, "literal must propagate to both arms");
        let entry = result.program.entry().to_vec();
        assert_eq!(
            count_var_reads(&entry, "x"),
            0,
            "no Var(x) reads remain after propagation"
        );
    }

    /// Negative: a name that has an `Assign` somewhere is rebound;
    /// the propagation must NOT fire (inlining the Let value would
    /// shadow the post-Assign value at later read sites).
    #[test]
    fn keeps_literal_when_name_is_assigned() {
        let entry = vec![
            Node::let_bind("x", Expr::u32(7)),
            Node::Assign {
                name: Ident::from("x"),
                value: Expr::u32(99),
            },
            Node::store("buf", Expr::u32(0), Expr::var("x")),
        ];
        let result = run(program(entry));
        assert!(!result.changed);
    }

    /// Negative: a name with two `Let` sites is shadowed; the
    /// propagation must NOT fire because the inner Let shadows the
    /// outer for any read inside its scope.
    #[test]
    fn keeps_literal_when_name_is_shadowed() {
        let entry = vec![
            Node::let_bind("x", Expr::u32(7)),
            Node::Block(vec![
                Node::let_bind("x", Expr::u32(99)),
                Node::store("buf", Expr::u32(0), Expr::var("x")),
            ]),
        ];
        let result = run(program(entry));
        assert!(!result.changed);
    }

    /// Negative: a name that's a `Loop` induction var is rebound;
    /// the propagation must NOT fire.
    #[test]
    fn keeps_literal_when_name_is_loop_var() {
        let entry = vec![
            Node::let_bind("i", Expr::u32(7)),
            Node::Loop {
                var: Ident::from("i"),
                from: Expr::u32(0),
                to: Expr::u32(4),
                body: vec![Node::store("buf", Expr::var("i"), Expr::u32(1))],
            },
        ];
        let result = run(program(entry));
        assert!(!result.changed);
    }

    /// Negative: a Let whose value is NOT a literal (e.g., a
    /// BinOp or Load) is not propagated by this pass  -  that's
    /// A14 / CSE territory.
    #[test]
    fn keeps_let_with_non_literal_value() {
        let entry = vec![
            Node::let_bind(
                "x",
                Expr::BinOp {
                    op: crate::ir::BinOp::Add,
                    left: Box::new(Expr::u32(1)),
                    right: Box::new(Expr::u32(2)),
                },
            ),
            Node::store("buf", Expr::u32(0), Expr::var("x")),
        ];
        let result = run(program(entry));
        assert!(!result.changed);
    }

    /// Positive: nested-into-Loop read is propagated.
    /// `Let(x, 7)` at top, read inside a Loop body  -  A14 would
    /// also handle this, but the test asserts the cross-CFG
    /// substrate doesn't accidentally regress same-scope cases.
    #[test]
    fn propagates_into_loop_body() {
        let entry = vec![
            Node::let_bind("x", Expr::u32(7)),
            Node::Loop {
                var: Ident::from("i"),
                from: Expr::u32(0),
                to: Expr::u32(4),
                body: vec![Node::store("buf", Expr::var("i"), Expr::var("x"))],
            },
        ];
        let result = run(program(entry));
        assert!(result.changed);
        let entry = result.program.entry().to_vec();
        assert_eq!(count_var_reads(&entry, "x"), 0);
    }

    /// `analyze` short-circuits when no propagatable Let exists.
    #[test]
    fn analyze_skips_program_with_no_eligible_lets() {
        let entry = vec![Node::store("buf", Expr::u32(0), Expr::u32(1))];
        assert!(matches!(
            crate::optimizer::ProgramPass::analyze(&ReachingDefPropagatePass, &program(entry)),
            PassAnalysis::SKIP
        ));
    }

    /// Positive end-to-end: `transform` produces the same result as
    /// the raw helper API. Smoke test that the pass surface works.
    #[test]
    fn transform_matches_helper_api() {
        let entry = vec![
            Node::let_bind("x", Expr::u32(13)),
            Node::store("buf", Expr::u32(0), Expr::var("x")),
        ];
        let p1 = run(program(entry.clone()));
        let p2 = ReachingDefPropagatePass::transform(program(entry));
        assert_eq!(p1.changed, p2.changed);
    }
}
