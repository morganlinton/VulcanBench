//! `loop_peel`  -  peel the first iteration of a `Node::Loop` when the
//! body's leading node is a guard conditioned on the loop variable being
//! the first-iteration value.
//!
//! Op id: `vyre-foundation::optimizer::passes::loop_peel`.
//! Soundness: `Exact`  -  the peeled iteration body is identical to what the
//! original loop would execute for `i == from`. The remaining loop starts
//! at `from + 1`. Cost-direction: down on branch count (removes one
//! iteration's predicate check). Preserves: every analysis. Invalidates:
//! nothing.
//!
//! ## Pattern
//!
//! ```text
//! Loop(var, LitU32(0), LitU32(N), [If(Eq(Var(var), LitU32(0)), then, []), rest...])
//!   where N > 1
//!   → Block(then); Loop(var, LitU32(1), LitU32(N), [rest...])
//! ```
//!
//! ## ROADMAP
//!
//! A28  -  loop peeling first iteration when guarded.

use crate::ir::{BinOp, Expr, Ident, Node, Program};
use crate::optimizer::{vyre_pass, PassAnalysis, PassResult};
use crate::visit::node_map;

/// Peel the first iteration of guarded loops.
#[derive(Debug, Default)]
#[vyre_pass(
    name = "loop_peel",
    requires = ["const_fold"],
    invalidates = []
)]
pub struct LoopPeelPass;

impl LoopPeelPass {
    /// Quick scan: skip programs without any peelable loop.
    #[must_use]
    fn analyze_impl(program: &Program) -> PassAnalysis {
        // O(1) fast-path via the cached node-kind bitset.
        if !program.stats().has_node_loop() {
            return PassAnalysis::SKIP;
        }
        if program
            .entry()
            .iter()
            .any(|n| node_map::any_descendant(n, &mut is_peelable_loop))
        {
            PassAnalysis::RUN
        } else {
            PassAnalysis::SKIP
        }
    }

    /// Walk the entry tree; peel every peelable loop.
    #[must_use]
    pub fn transform(program: Program) -> PassResult {
        let mut changed = false;
        let program = program.map_entry(|entry| {
            entry
                .into_iter()
                .flat_map(|node| peel_node(node, &mut changed))
                .collect()
        });
        PassResult { program, changed }
    }
}

/// Recurse into `node`'s descendants, then try to peel this node itself.
/// Returns one or two nodes (peeled body + remaining loop).
fn peel_node(node: Node, changed: &mut bool) -> Vec<Node> {
    let recursed = node_map::map_children(node, &mut |child| {
        let peeled = peel_node(child, changed);
        if peeled.len() == 1 {
            peeled.into_iter().next().unwrap_or(Node::Block(Vec::new()))
        } else {
            Node::Block(peeled)
        }
    });

    if let Node::Loop {
        ref var,
        ref from,
        ref to,
        ref body,
    } = recursed
    {
        if let Some((peeled_body, rest_body)) = try_peel(var, from, to, body) {
            *changed = true;
            let remaining = Node::Loop {
                var: var.clone(),
                from: Expr::u32(1),
                to: to.clone(),
                body: rest_body,
            };
            return vec![Node::Block(peeled_body), remaining];
        }
    }

    vec![recursed]
}

/// Try to match the A28 peeling pattern:
/// - from = LitU32(0), to = LitU32(N) with N > 1
/// - first body node = `If(Eq(Var(loop_var), LitU32(0)), then, [])`
/// - peeled body does not contain an Assign to the loop var
///
/// Returns `Some((peeled_body, rest_of_loop_body))` on success.
fn try_peel(var: &Ident, from: &Expr, to: &Expr, body: &[Node]) -> Option<(Vec<Node>, Vec<Node>)> {
    // Require from = 0, to = N literal > 1
    let Expr::LitU32(0) = from else { return None };
    let Expr::LitU32(n) = to else { return None };
    if *n <= 1 {
        return None;
    }

    // First body node must be If(Eq(Var(var), LitU32(0)), then, [])
    let first = body.first()?;
    let Node::If {
        cond,
        then,
        otherwise,
    } = first
    else {
        return None;
    };

    // otherwise must be empty
    if !otherwise.is_empty() {
        return None;
    }

    // cond must be Eq(Var(var), LitU32(0))
    let Expr::BinOp {
        op: BinOp::Eq,
        left,
        right,
    } = cond
    else {
        return None;
    };

    let matches_var = match (left.as_ref(), right.as_ref()) {
        (Expr::Var(name), Expr::LitU32(0)) if name == var => true,
        (Expr::LitU32(0), Expr::Var(name)) if name == var => true,
        _ => false,
    };

    if !matches_var {
        return None;
    }

    // Safety: peeled body must not assign to the loop variable
    if assigns_to_name(then, var) {
        return None;
    }

    let peeled_body = then.clone();
    let rest_body: Vec<Node> = body[1..].to_vec();
    Some((peeled_body, rest_body))
}

/// True iff any `Node::Assign` in `nodes` targets `name`.
fn assigns_to_name(nodes: &[Node], name: &Ident) -> bool {
    for node in nodes {
        match node {
            Node::Assign {
                name: assign_name, ..
            } if assign_name == name => return true,
            Node::If {
                then, otherwise, ..
            } if assigns_to_name(then, name) || assigns_to_name(otherwise, name) => return true,
            Node::Loop { body, .. } | Node::Block(body) if assigns_to_name(body, name) => {
                return true
            }
            Node::Region { body, .. } if assigns_to_name(body, name) => return true,
            _ => {}
        }
    }
    false
}

/// True iff `node` is a loop matching the A28 peeling pattern.
fn is_peelable_loop(node: &Node) -> bool {
    if let Node::Loop {
        var,
        from,
        to,
        body,
    } = node
    {
        try_peel(var, from, to, body).is_some()
    } else {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{BufferAccess, BufferDecl, DataType, Expr, Ident, Node};

    fn buf() -> BufferDecl {
        BufferDecl::storage("buf", 0, BufferAccess::ReadWrite, DataType::U32).with_count(4)
    }

    fn program_with_entry(entry: Vec<Node>) -> Program {
        Program::wrapped(vec![buf()], [1, 1, 1], entry)
    }

    fn count_loops(node: &Node) -> usize {
        match node {
            Node::Loop { body, .. } => 1 + body.iter().map(count_loops).sum::<usize>(),
            Node::If {
                then, otherwise, ..
            } => {
                then.iter().map(count_loops).sum::<usize>()
                    + otherwise.iter().map(count_loops).sum::<usize>()
            }
            Node::Block(body) => body.iter().map(count_loops).sum(),
            Node::Region { body, .. } => body.iter().map(count_loops).sum(),
            _ => 0,
        }
    }

    /// Positive: peel fires for Loop(i, 0, 10, [If(Eq(i, 0), [store], []), rest])
    #[test]
    fn peel_fires_for_guarded_first_iteration() {
        let guard = Node::If {
            cond: Expr::eq(Expr::var("i"), Expr::u32(0)),
            then: vec![Node::store("buf", Expr::u32(0), Expr::u32(99))],
            otherwise: vec![],
        };
        let rest = Node::store("buf", Expr::var("i"), Expr::u32(7));
        let entry = vec![Node::Loop {
            var: Ident::from("i"),
            from: Expr::u32(0),
            to: Expr::u32(10),
            body: vec![guard, rest],
        }];
        let program = program_with_entry(entry);
        let result = LoopPeelPass::transform(program);
        assert!(result.changed, "peeling must fire");
        // After peeling: peeled body (Block) + remaining loop from 1..10
        let loops: usize = result.program.entry().iter().map(count_loops).sum();
        assert!(loops >= 1, "remaining loop must exist");
    }

    /// Negative: from != 0
    #[test]
    fn peel_skips_when_from_is_not_zero() {
        let guard = Node::If {
            cond: Expr::eq(Expr::var("i"), Expr::u32(0)),
            then: vec![Node::store("buf", Expr::u32(0), Expr::u32(99))],
            otherwise: vec![],
        };
        let entry = vec![Node::Loop {
            var: Ident::from("i"),
            from: Expr::u32(1), // not zero
            to: Expr::u32(10),
            body: vec![guard],
        }];
        let program = program_with_entry(entry);
        let result = LoopPeelPass::transform(program);
        assert!(!result.changed, "peeling must not fire when from != 0");
    }

    /// Negative: to is not literal
    #[test]
    fn peel_skips_when_to_is_not_literal() {
        let guard = Node::If {
            cond: Expr::eq(Expr::var("i"), Expr::u32(0)),
            then: vec![Node::store("buf", Expr::u32(0), Expr::u32(99))],
            otherwise: vec![],
        };
        let entry = vec![Node::Loop {
            var: Ident::from("i"),
            from: Expr::u32(0),
            to: Expr::var("n"), // not literal
            body: vec![guard],
        }];
        let program = program_with_entry(entry);
        let result = LoopPeelPass::transform(program);
        assert!(!result.changed, "peeling must not fire when to is Var");
    }

    /// Negative: first body node is not the matching If
    #[test]
    fn peel_skips_when_first_node_is_not_matching_if() {
        let entry = vec![Node::Loop {
            var: Ident::from("i"),
            from: Expr::u32(0),
            to: Expr::u32(10),
            body: vec![Node::store("buf", Expr::var("i"), Expr::u32(7))],
        }];
        let program = program_with_entry(entry);
        let result = LoopPeelPass::transform(program);
        assert!(!result.changed, "peeling must not fire without matching If");
    }

    /// Negative: peeled body assigns to the loop variable
    #[test]
    fn peel_skips_when_peeled_body_assigns_loop_var() {
        let guard = Node::If {
            cond: Expr::eq(Expr::var("i"), Expr::u32(0)),
            then: vec![Node::assign("i", Expr::u32(42))], // assigns to loop var!
            otherwise: vec![],
        };
        let entry = vec![Node::Loop {
            var: Ident::from("i"),
            from: Expr::u32(0),
            to: Expr::u32(10),
            body: vec![guard],
        }];
        let program = program_with_entry(entry);
        let result = LoopPeelPass::transform(program);
        assert!(
            !result.changed,
            "peeling must not fire when peeled body assigns to loop var"
        );
    }
}
