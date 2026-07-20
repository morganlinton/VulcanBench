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
//!   → Block((then ++ rest)[var := 0]); Loop(var, LitU32(1), LitU32(N), [rest...])
//! ```
//!
//! The peeled prologue materializes the WHOLE first iteration with `var` fixed
//! to `0`: the guard is statically true at `i == 0` so it collapses to `then`,
//! and the trailing `rest` also runs (the original loop executes `then` *and*
//! `rest` on iteration 0). `var := 0` is substituted into both halves because
//! `var` is no longer an induction variable in the lifted block.
//!
//! ## ROADMAP
//!
//! A28  -  loop peeling first iteration when guarded.

use super::substitution::{body_writes_loop_var, substitute_nodes};
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
/// - the first-iteration body (`then` ++ trailing `rest`) does not rebind the
///   loop var (we substitute `loop_var := 0` into the lifted copy)
///
/// Returns `Some((peeled_body, rest_of_loop_body))` on success, where
/// `peeled_body` is the entire first iteration `(then ++ rest)` with the loop
/// var fixed to `0`, and `rest_of_loop_body` is the loop body with the now-dead
/// first-iteration guard removed (used as the body of the `1..N` remainder).
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

    let rest_body: Vec<Node> = body[1..].to_vec();

    // Safety: the lifted prologue substitutes `var := 0`, so neither the
    // guard's `then` nor the trailing `rest` may rebind the loop var. A
    // `Let`/`Assign` to `var` would make a later `Var(var)` denote that new
    // binding, not 0, and the substitution would corrupt it. (This subsumes
    // the previous Assign-only guard.)
    if body_writes_loop_var(then, var) || body_writes_loop_var(&rest_body, var) {
        return None;
    }

    // Materialize the entire first iteration with the loop var fixed to 0:
    // the guard is statically true (0 == 0) so it collapses to `then`, then
    // `rest` runs. The original loop executes BOTH on iteration 0.
    let zero = Expr::u32(0);
    let mut peeled_body = substitute_nodes(then, var, &zero);
    peeled_body.extend(substitute_nodes(&rest_body, var, &zero));
    Some((peeled_body, rest_body))
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

    /// Collect every `(index, value)` Store pair in document order.
    fn store_pairs(nodes: &[Node]) -> Vec<(Expr, Expr)> {
        let mut out = Vec::new();
        for n in nodes {
            match n {
                Node::Store { index, value, .. } => out.push((index.clone(), value.clone())),
                Node::Block(b) => out.extend(store_pairs(b)),
                Node::If {
                    then, otherwise, ..
                } => {
                    out.extend(store_pairs(then));
                    out.extend(store_pairs(otherwise));
                }
                Node::Loop { body, .. } => out.extend(store_pairs(body)),
                Node::Region { body, .. } => out.extend(store_pairs(body)),
                _ => {}
            }
        }
        out
    }

    /// The peeled prologue must materialize the WHOLE first iteration -- the
    /// guard's `then` AND the trailing `rest` -- with the loop var fixed to 0.
    ///
    /// Two bugs this locks down, both latent because the only positive test
    /// asserted just `changed` + loop-count:
    ///   1. `then` was lifted verbatim, so `Var(i)` inside it referenced an
    ///      out-of-scope induction variable instead of `0`.
    ///   2. the first iteration's `rest` was dropped entirely (the remainder
    ///      loop starts at `i = 1`), losing `rest[i := 0]`.
    #[test]
    fn peel_materializes_full_first_iteration_with_var_substituted() {
        // `then` reads Var(i); `rest` reads Var(i). At i == 0 both are 0.
        let guard = Node::If {
            cond: Expr::eq(Expr::var("i"), Expr::u32(0)),
            then: vec![Node::store("buf", Expr::var("i"), Expr::u32(99))],
            otherwise: vec![],
        };
        let rest = Node::store("buf", Expr::var("i"), Expr::u32(7));
        let entry = vec![Node::Loop {
            var: Ident::from("i"),
            from: Expr::u32(0),
            to: Expr::u32(10),
            body: vec![guard, rest],
        }];
        let result = LoopPeelPass::transform(program_with_entry(entry));
        assert!(result.changed, "peeling must fire");

        let body = crate::test_util::region_body(&result.program);

        // Full store sequence across the peeled program, in document order.
        // The peeled prologue runs `then[i:=0]` (store 0,99) then `rest[i:=0]`
        // (store 0,7); the remainder loop runs `rest` (store Var(i),7).
        // Pre-fix this was [(Var(i),99),(Var(i),7)]: the prologue lifted `then`
        // verbatim (unsubstituted) and dropped `rest[i:=0]` entirely.
        assert_eq!(
            store_pairs(&body),
            vec![
                (Expr::u32(0), Expr::u32(99)),
                (Expr::u32(0), Expr::u32(7)),
                (Expr::var("i"), Expr::u32(7)),
            ],
            "peel must materialize then++rest at i = 0, then loop rest over 1..N"
        );

        // The remainder loop must start at i = 1 and keep `rest` with the
        // induction variable (search through the Block nesting the rewrite adds).
        fn find_loop(nodes: &[Node]) -> Option<(&Expr, &[Node])> {
            for n in nodes {
                match n {
                    Node::Loop { from, body, .. } => return Some((from, body)),
                    Node::Block(b) => {
                        if let Some(x) = find_loop(b) {
                            return Some(x);
                        }
                    }
                    Node::Region { body, .. } => {
                        if let Some(x) = find_loop(body) {
                            return Some(x);
                        }
                    }
                    _ => {}
                }
            }
            None
        }
        let (from, lbody) = find_loop(&body).expect("remainder loop present");
        assert_eq!(from, &Expr::u32(1), "remainder loop starts at i = 1");
        assert_eq!(
            store_pairs(lbody),
            vec![(Expr::var("i"), Expr::u32(7))],
            "remainder keeps rest with the induction variable"
        );
    }
}
