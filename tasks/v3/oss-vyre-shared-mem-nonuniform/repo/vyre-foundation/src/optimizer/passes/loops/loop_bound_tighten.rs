//! ROADMAP A19  -  loop-bound tightening via inner predicate hoisting.
//!
//! Op id: `vyre-foundation::optimizer::passes::loop_bound_tighten`.
//! Soundness: `Exact`  -  when every observable side-effect inside a
//! `Node::Loop` body is gated by a predicate of the form
//! `Expr::lt(Expr::var(loop_var), Expr::LitU32(N))` and `N <= to`,
//! iterating from `from` to `min(to, N)` produces the same observable
//! state. Cost direction: monotone-down on dynamic execution count
//! (loop runs `min(to, N) - from` times instead of `to - from`).
//! Preserves: every analysis. Invalidates: nothing.
//!
//! ## Rule
//!
//! ```text
//! Node::Loop {
//!     var,
//!     from: LitU32(a),
//!     to: LitU32(b),
//!     body: [Node::if_then(Expr::lt(Var(var), LitU32(n)), real_body)],
//! }
//!     where n < b
//! →
//! Node::Loop {
//!     var,
//!     from: LitU32(a),
//!     to: LitU32(n),
//!     body: real_body,
//! }
//! ```
//!
//! ## Conservatism
//!
//! - Loop body must be exactly one `Node::if_then` whose otherwise arm is
//!   empty. The condition is `Lt(Var(loop_var), LitU32(n))` with `n` a
//!   compile-time constant strictly less than the upper bound `b`.
//! - Loop bounds must both be `Expr::LitU32`. Runtime bounds (e.g.
//!   `Expr::buf_len`) need range facts (ROADMAP A16) before tightening
//!   is safe; that variant lives beside the downstream range pass.
//! - When `n >= b`, the predicate is always true on every iteration  -
//!   the redundant guard is dropped by `loop_redundant_bound_check_elide`,
//!   not by this pass.

use crate::ir::{BinOp, Expr, Ident, Node, Program};
use crate::optimizer::{vyre_pass, PassAnalysis, PassResult};
use crate::visit::node_map;

/// Tighten `Node::Loop` upper bound when the body is one inner-If
/// whose predicate guards `Var(loop_var) < LitU32(n)` for some `n <
/// upper_bound`.
#[derive(Debug, Default)]
#[vyre_pass(
    name = "loop_bound_tighten",
    requires = [],
    invalidates = [],
    phase = "loop",
    boundary_class = "abi_preserving",
    cost_model_family = "loop"
)]
pub struct LoopBoundTighten;

impl LoopBoundTighten {
    /// Skip the pass when no body in the program contains a
    /// matching tighten-eligible Loop.
    #[must_use]
    fn analyze_impl(program: &Program) -> PassAnalysis {
        if !program.stats().has_node_loop() {
            return PassAnalysis::SKIP;
        }
        if program
            .entry()
            .iter()
            .any(|n| node_map::any_descendant(n, &mut is_tighten_eligible))
        {
            PassAnalysis::RUN
        } else {
            PassAnalysis::SKIP
        }
    }

    /// Rewrite every tighten-eligible Loop in the program tree.
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

fn rewrite_node(node: Node, changed: &mut bool) -> Node {
    let recursed = node_map::map_children(node, &mut |child| rewrite_node(child, changed));
    let recursed = node_map::map_body(recursed, &mut |body| {
        body.into_iter().map(|n| rewrite_node(n, changed)).collect()
    });
    tighten_if_eligible(recursed, changed)
}

fn tighten_if_eligible(node: Node, changed: &mut bool) -> Node {
    let Node::Loop {
        var,
        from,
        to,
        body,
    } = node
    else {
        return node_unchanged_helper(node);
    };
    let Some((upper_lit, predicate_lit, real_body)) =
        match_tighten_pattern(&var, &from, &to, &body)
    else {
        return Node::Loop {
            var,
            from,
            to,
            body,
        };
    };
    if predicate_lit >= upper_lit {
        return Node::Loop {
            var,
            from,
            to,
            body,
        };
    }
    *changed = true;
    Node::Loop {
        var,
        from,
        to: Expr::u32(predicate_lit),
        body: real_body,
    }
}

fn node_unchanged_helper(node: Node) -> Node {
    node
}

fn match_tighten_pattern(
    loop_var: &Ident,
    from: &Expr,
    to: &Expr,
    body: &[Node],
) -> Option<(u32, u32, Vec<Node>)> {
    let Expr::LitU32(_) = from else { return None };
    let Expr::LitU32(upper) = to else {
        return None;
    };
    if body.len() != 1 {
        return None;
    }
    let Node::If {
        cond,
        then,
        otherwise,
    } = &body[0]
    else {
        return None;
    };
    if !otherwise.is_empty() {
        return None;
    }
    let Expr::BinOp {
        op: BinOp::Lt,
        left,
        right,
    } = cond
    else {
        return None;
    };
    let Expr::Var(name) = left.as_ref() else {
        return None;
    };
    if name != loop_var {
        return None;
    }
    let Expr::LitU32(n) = right.as_ref() else {
        return None;
    };
    Some((*upper, *n, then.clone()))
}

fn is_tighten_eligible(node: &Node) -> bool {
    let Node::Loop {
        var,
        from,
        to,
        body,
    } = node
    else {
        return false;
    };
    let Some((upper, n, _)) = match_tighten_pattern(var, from, to, body) else {
        return false;
    };
    n < upper
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{BufferAccess, BufferDecl, DataType, Expr, Node};

    fn buf() -> BufferDecl {
        BufferDecl::storage("buf", 0, BufferAccess::ReadWrite, DataType::U32).with_count(8)
    }

    fn program(entry: Vec<Node>) -> Program {
        Program::wrapped(vec![buf()], [1, 1, 1], entry)
    }

    fn loop_with_to(entry: &[Node]) -> Option<u32> {
        for n in entry {
            match n {
                Node::Loop { to, .. } => match to {
                    Expr::LitU32(v) => return Some(*v),
                    _ => return None,
                },
                Node::Region { body, .. } => {
                    if let Some(v) = loop_with_to(body) {
                        return Some(v);
                    }
                }
                _ => {}
            }
        }
        None
    }

    #[test]
    fn tightens_upper_bound_when_inner_predicate_is_smaller() {
        let entry = vec![Node::loop_for(
            "i",
            Expr::u32(0),
            Expr::u32(64),
            vec![Node::if_then(
                Expr::lt(Expr::var("i"), Expr::u32(8)),
                vec![Node::store("buf", Expr::var("i"), Expr::u32(7))],
            )],
        )];
        let result = LoopBoundTighten::transform(program(entry));
        assert!(result.changed);
        assert_eq!(
            loop_with_to(result.program.entry()),
            Some(8),
            "loop's upper bound must shrink from 64 to the predicate constant 8"
        );
    }

    #[test]
    fn does_not_tighten_when_predicate_meets_or_exceeds_upper() {
        // Predicate `i < 64` matches the loop upper bound; tightening
        // would be a no-op (and the redundant guard is the
        // loop_redundant_bound_check_elide pass's job, not ours).
        let entry = vec![Node::loop_for(
            "i",
            Expr::u32(0),
            Expr::u32(64),
            vec![Node::if_then(
                Expr::lt(Expr::var("i"), Expr::u32(64)),
                vec![Node::store("buf", Expr::var("i"), Expr::u32(7))],
            )],
        )];
        let result = LoopBoundTighten::transform(program(entry));
        assert!(
            !result.changed,
            "predicate constant equal to upper bound is not a tighten win"
        );
    }

    #[test]
    fn does_not_tighten_when_body_has_unguarded_sibling() {
        // Body has the gated If plus a separate Store  -  the second
        // Store must execute on every iteration up to `to`, so we
        // cannot tighten.
        let entry = vec![Node::loop_for(
            "i",
            Expr::u32(0),
            Expr::u32(64),
            vec![
                Node::if_then(
                    Expr::lt(Expr::var("i"), Expr::u32(8)),
                    vec![Node::store("buf", Expr::var("i"), Expr::u32(7))],
                ),
                Node::store("buf", Expr::var("i"), Expr::u32(0)),
            ],
        )];
        let result = LoopBoundTighten::transform(program(entry));
        assert!(!result.changed, "unguarded sibling Store blocks tightening");
    }

    #[test]
    fn does_not_tighten_when_inner_if_has_otherwise_arm() {
        let entry = vec![Node::loop_for(
            "i",
            Expr::u32(0),
            Expr::u32(64),
            vec![Node::if_then_else(
                Expr::lt(Expr::var("i"), Expr::u32(8)),
                vec![Node::store("buf", Expr::var("i"), Expr::u32(7))],
                vec![Node::store("buf", Expr::var("i"), Expr::u32(0))],
            )],
        )];
        let result = LoopBoundTighten::transform(program(entry));
        assert!(
            !result.changed,
            "else-arm side-effect must keep firing across full range"
        );
    }

    #[test]
    fn does_not_tighten_when_predicate_uses_different_var() {
        let entry = vec![Node::loop_for(
            "i",
            Expr::u32(0),
            Expr::u32(64),
            vec![Node::if_then(
                Expr::lt(Expr::var("j"), Expr::u32(8)),
                vec![Node::store("buf", Expr::var("i"), Expr::u32(7))],
            )],
        )];
        let result = LoopBoundTighten::transform(program(entry));
        assert!(
            !result.changed,
            "predicate on a different variable is not a tightener"
        );
    }

    #[test]
    fn does_not_tighten_when_loop_bound_is_runtime() {
        // Upper bound is `Expr::buf_len(...)` not a literal  -  without
        // range facts proving buf_len <= 8, we cannot tighten.
        let entry = vec![Node::loop_for(
            "i",
            Expr::u32(0),
            Expr::buf_len("buf"),
            vec![Node::if_then(
                Expr::lt(Expr::var("i"), Expr::u32(8)),
                vec![Node::store("buf", Expr::var("i"), Expr::u32(7))],
            )],
        )];
        let result = LoopBoundTighten::transform(program(entry));
        assert!(
            !result.changed,
            "runtime upper bound needs range facts (A16) to tighten"
        );
    }

    #[test]
    fn analyze_skips_program_with_no_eligible_loop() {
        let entry = vec![Node::loop_for(
            "i",
            Expr::u32(0),
            Expr::u32(8),
            vec![Node::store("buf", Expr::var("i"), Expr::u32(7))],
        )];
        assert_eq!(
            crate::optimizer::ProgramPass::analyze(&LoopBoundTighten, &program(entry)),
            PassAnalysis::SKIP
        );
    }

    #[test]
    fn analyze_runs_when_loop_is_tighten_eligible() {
        let entry = vec![Node::loop_for(
            "i",
            Expr::u32(0),
            Expr::u32(64),
            vec![Node::if_then(
                Expr::lt(Expr::var("i"), Expr::u32(8)),
                vec![Node::store("buf", Expr::var("i"), Expr::u32(7))],
            )],
        )];
        assert_eq!(
            crate::optimizer::ProgramPass::analyze(&LoopBoundTighten, &program(entry)),
            PassAnalysis::RUN
        );
    }
}
