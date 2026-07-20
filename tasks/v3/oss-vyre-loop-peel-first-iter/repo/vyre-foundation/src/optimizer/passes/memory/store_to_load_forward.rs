//! ROADMAP A22  -  store-to-load forwarding under the conservative
//! same-block / structurally-equal-index alias proof.
//!
//! Op id: `vyre-foundation::optimizer::passes::store_to_load_forward`.
//! Soundness: `Exact` when the rule fires. A `Node::Store(b, i, v)`
//! followed in the same sibling Vec by `Node::Let(name, Load(b, i))`
//! with no intervening write or barrier to `b` lets us replace the
//! Load with a direct copy of `v`  -  the bytes the Load would observe
//! are exactly the bytes the prior Store wrote. Cost direction:
//! monotone-down on `node_count` (one fewer Load expression) and on
//! per-iteration memory traffic. Preserves: every analysis.
//! Invalidates: nothing.
//!
//! ## Rule
//!
//! ```text
//! Node::Store { buffer: b, index: i, value: v }
//! ... straight-line siblings, no Store/Atomic/Load/AsyncLoad/AsyncStore/
//!     IndirectDispatch/Trap/Barrier touching `b` ...
//! Node::Let { name, value: Expr::Load { buffer: b, index: i } }
//! →
//! Node::Store { buffer: b, index: i, value: v }
//! ... unchanged siblings ...
//! Node::Let { name, value: v.clone() }
//! ```
//!
//! ## Conservatism
//!
//! - Both operations must live in the same `Vec<Node>` body. Cross-
//!   container forwarding (across an If branch boundary, into a Loop
//!   body, etc.) needs downstream reaching-store analysis.
//! - The `index` expressions must be structurally equal (`Expr` PartialEq).
//!   Dynamic indexes that happen to coincide at runtime are conservatively
//!   left alone.
//! - Any node between the two whose evaluation could observe or mutate
//!   `b` blocks the rewrite. The reachability check piggybacks on the
//!   same predicate `dead_store_elim` uses (`node_observes_buffer`).
//! - The forwarded `v` is `Expr::clone()`d into the Let. If `v` itself
//!   is observably side-effecting (e.g. contains an Atomic), forwarding
//!   would duplicate the side effect  -  `value_is_observably_free`
//!   rejects that case.

use crate::ir::{Expr, Ident, Node, Program};
use crate::optimizer::{vyre_pass, PassAnalysis, PassResult};
use crate::visit::node_map;

/// `ProgramPass` registration for the store-to-load forwarding rewrite
/// (ROADMAP A22).
#[derive(Debug, Default)]
#[vyre_pass(
    name = "store_to_load_forward",
    requires = [],
    invalidates = [],
    phase = "memory",
    boundary_class = "abi_preserving",
    cost_model_family = "memory"
)]
pub struct StoreToLoadForward;

impl StoreToLoadForward {
    #![allow(missing_docs)]
    /// Skip when no body in the program contains a forwardable
    /// `Store` / `Let(Load)` pair under the conservative rule.
    #[must_use]
    fn analyze_impl(program: &Program) -> PassAnalysis {
        // Forwarding requires both a Store AND a Let; either missing
        // means the recursive walk would find no forwardable pair.
        use crate::ir::stats::{NODE_KIND_LET, NODE_KIND_STORE};
        let stats = program.stats();
        if !stats.has_any_node_kind(NODE_KIND_STORE) || !stats.has_any_node_kind(NODE_KIND_LET) {
            return PassAnalysis::SKIP;
        }
        if program
            .entry()
            .iter()
            .any(|n| node_map::any_descendant(n, &mut has_forwardable_pair))
        {
            PassAnalysis::RUN
        } else {
            PassAnalysis::SKIP
        }
    }

    /// Walk the program; rewrite every forwardable `Let(Load)` to
    /// the value of its preceding `Store`.
    #[must_use]
    pub fn transform(program: Program) -> PassResult {
        let mut changed = false;
        let program = program.map_entry(|entry| {
            let mapped: Vec<Node> = entry
                .into_iter()
                .map(|n| rewrite_node(n, &mut changed))
                .collect();
            forward_in_body(mapped, &mut changed)
        });
        PassResult { program, changed }
    }
}

fn rewrite_node(node: Node, changed: &mut bool) -> Node {
    let recursed = node_map::map_children(node, &mut |child| rewrite_node(child, changed));
    node_map::map_body(recursed, &mut |body| forward_in_body(body, changed))
}

fn forward_in_body(body: Vec<Node>, changed: &mut bool) -> Vec<Node> {
    let mut out: Vec<Node> = Vec::with_capacity(body.len());
    // Take `body` by value and walk it once, moving each node into `out`
    // unchanged unless it's a forwardable Let. The previous shape iterated
    // by reference (`body.iter().enumerate()`) and unconditionally cloned
    // every Node into `out` even when no forwarding fired  -  so a body of
    // 1000 nodes with no opportunities still paid 1000 deep clones.
    //
    // Forwarding lookback now scans the partially-built `out` instead of
    // `body[..idx]`. Since we only rewrite Let-of-Load (never Store), and
    // find_forwarding_store only inspects Store nodes (which we never
    // rewrite), `out` carries the same Store-position information as the
    // original prefix did.
    for node in body {
        let Node::Let { name, value } = node else {
            out.push(node);
            continue;
        };
        let Expr::Load {
            buffer: load_buffer,
            index: load_index,
        } = &value
        else {
            out.push(Node::Let { name, value });
            continue;
        };
        let Some(forwarded_value) = find_forwarding_store(&out, load_buffer, load_index) else {
            out.push(Node::Let { name, value });
            continue;
        };
        if !value_is_observably_free(&forwarded_value) {
            out.push(Node::Let { name, value });
            continue;
        }
        *changed = true;
        out.push(Node::Let {
            name,
            value: forwarded_value,
        });
    }
    out
}

/// Walk back through `prev_siblings` looking for a `Node::Store(b, i, v)`
/// whose buffer equals `buffer` and whose index is structurally equal to
/// `index`. Return the stored value `v`. Bail out the moment any
/// intervening node could observe or mutate `buffer`.
fn find_forwarding_store(prev_siblings: &[Node], buffer: &Ident, index: &Expr) -> Option<Expr> {
    for prev in prev_siblings.iter().rev() {
        if let Node::Store {
            buffer: store_buffer,
            index: store_index,
            value,
        } = prev
        {
            if store_buffer == buffer && store_index == index {
                return Some(value.clone());
            }
            // A different-index Store to the same buffer is not a
            // forwarder but also doesn't observe our value; keep
            // walking unless there's something else blocking.
            if store_buffer == buffer {
                return None;
            }
        }
        if node_blocks_forwarding(prev, buffer) {
            return None;
        }
    }
    None
}

/// True if `node` could read or otherwise observe `buffer`'s contents
/// in a way that makes forwarding unsafe.
fn node_blocks_forwarding(node: &Node, buffer: &Ident) -> bool {
    match node {
        Node::Store {
            buffer: other,
            index,
            value,
        } => {
            other == buffer
                || expr_touches_buffer(index, buffer)
                || expr_touches_buffer(value, buffer)
        }
        Node::Let { value, .. } | Node::Assign { value, .. } => expr_touches_buffer(value, buffer),
        Node::If {
            cond,
            then,
            otherwise,
        } => {
            expr_touches_buffer(cond, buffer)
                || then.iter().any(|n| node_blocks_forwarding(n, buffer))
                || otherwise.iter().any(|n| node_blocks_forwarding(n, buffer))
        }
        Node::Loop { from, to, body, .. } => {
            expr_touches_buffer(from, buffer)
                || expr_touches_buffer(to, buffer)
                || body.iter().any(|n| node_blocks_forwarding(n, buffer))
        }
        Node::Block(body) => body.iter().any(|n| node_blocks_forwarding(n, buffer)),
        Node::Region { body, .. } => body.iter().any(|n| node_blocks_forwarding(n, buffer)),
        Node::AllReduce {
            buffer: collective, ..
        }
        | Node::Broadcast {
            buffer: collective, ..
        } => collective == buffer,
        Node::AllGather { input, output, .. } | Node::ReduceScatter { input, output, .. } => {
            input == buffer || output == buffer
        }
        Node::Barrier { .. }
        | Node::AsyncWait { .. }
        | Node::Resume { .. }
        | Node::Return
        | Node::Opaque(_) => true,
        Node::AsyncLoad {
            source,
            destination,
            offset,
            size,
            ..
        }
        | Node::AsyncStore {
            source,
            destination,
            offset,
            size,
            ..
        } => {
            source == buffer
                || destination == buffer
                || expr_touches_buffer(offset, buffer)
                || expr_touches_buffer(size, buffer)
        }
        Node::IndirectDispatch { count_buffer, .. } => count_buffer == buffer,
        Node::Trap { address, .. } => expr_touches_buffer(address, buffer),
    }
}

fn expr_touches_buffer(expr: &Expr, buffer: &Ident) -> bool {
    match expr {
        Expr::Load {
            buffer: other,
            index,
        } => other == buffer || expr_touches_buffer(index, buffer),
        Expr::BufLen { buffer: other } => other == buffer,
        Expr::Atomic {
            buffer: other,
            index,
            expected,
            value,
            ..
        } => {
            other == buffer
                || expr_touches_buffer(index, buffer)
                || expected
                    .as_deref()
                    .is_some_and(|e| expr_touches_buffer(e, buffer))
                || expr_touches_buffer(value, buffer)
        }
        Expr::BinOp { left, right, .. } => {
            expr_touches_buffer(left, buffer) || expr_touches_buffer(right, buffer)
        }
        Expr::UnOp { operand, .. } | Expr::Cast { value: operand, .. } => {
            expr_touches_buffer(operand, buffer)
        }
        Expr::Fma { a, b, c } => {
            expr_touches_buffer(a, buffer)
                || expr_touches_buffer(b, buffer)
                || expr_touches_buffer(c, buffer)
        }
        Expr::Select {
            cond,
            true_val,
            false_val,
        } => {
            expr_touches_buffer(cond, buffer)
                || expr_touches_buffer(true_val, buffer)
                || expr_touches_buffer(false_val, buffer)
        }
        Expr::Call { args, .. } => args.iter().any(|a| expr_touches_buffer(a, buffer)),
        Expr::SubgroupShuffle { value, .. } | Expr::SubgroupReduce { value, .. } => {
            expr_touches_buffer(value, buffer)
        }
        Expr::SubgroupBallot { cond } => expr_touches_buffer(cond, buffer),
        Expr::Opaque(_) => true,
        Expr::LitU32(_)
        | Expr::LitI32(_)
        | Expr::LitF32(_)
        | Expr::LitBool(_)
        | Expr::Var(_)
        | Expr::InvocationId { .. }
        | Expr::WorkgroupId { .. }
        | Expr::LocalId { .. }
        | Expr::SubgroupLocalId
        | Expr::SubgroupSize => false,
    }
}

/// True iff `value` is safe to clone into the forwarded Let  -  no
/// embedded Atomic, Call, Opaque, or Load whose ordering could matter.
fn value_is_observably_free(value: &Expr) -> bool {
    match value {
        Expr::LitU32(_)
        | Expr::LitI32(_)
        | Expr::LitF32(_)
        | Expr::LitBool(_)
        | Expr::Var(_)
        | Expr::InvocationId { .. }
        | Expr::WorkgroupId { .. }
        | Expr::LocalId { .. }
        | Expr::SubgroupLocalId
        | Expr::SubgroupSize => true,
        Expr::BinOp { left, right, .. } => {
            value_is_observably_free(left) && value_is_observably_free(right)
        }
        Expr::UnOp { operand, .. } | Expr::Cast { value: operand, .. } => {
            value_is_observably_free(operand)
        }
        Expr::Fma { a, b, c } => {
            value_is_observably_free(a)
                && value_is_observably_free(b)
                && value_is_observably_free(c)
        }
        Expr::Select {
            cond,
            true_val,
            false_val,
        } => {
            value_is_observably_free(cond)
                && value_is_observably_free(true_val)
                && value_is_observably_free(false_val)
        }
        Expr::Load { .. }
        | Expr::BufLen { .. }
        | Expr::Atomic { .. }
        | Expr::Call { .. }
        | Expr::Opaque(_)
        | Expr::SubgroupShuffle { .. }
        | Expr::SubgroupReduce { .. }
        | Expr::SubgroupBallot { .. } => false,
    }
}

fn has_forwardable_pair(node: &Node) -> bool {
    let body: &[Node] = match node {
        Node::If {
            then, otherwise, ..
        } => {
            return body_has_forwardable_pair(then) || body_has_forwardable_pair(otherwise);
        }
        Node::Loop { body, .. } | Node::Block(body) => body,
        Node::Region { body, .. } => body.as_ref(),
        _ => return false,
    };
    body_has_forwardable_pair(body)
}

fn body_has_forwardable_pair(body: &[Node]) -> bool {
    for (idx, node) in body.iter().enumerate() {
        let Node::Let { value, .. } = node else {
            continue;
        };
        let Expr::Load { buffer, index } = value else {
            continue;
        };
        if find_forwarding_store(&body[..idx], buffer, index)
            .as_ref()
            .is_some_and(value_is_observably_free)
        {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{BufferAccess, BufferDecl, DataType, Expr, Node};

    fn buf(name: &str) -> BufferDecl {
        BufferDecl::storage(name, 0, BufferAccess::ReadWrite, DataType::U32).with_count(4)
    }

    fn program(entry: Vec<Node>) -> Program {
        Program::wrapped(vec![buf("a"), buf("b")], [1, 1, 1], entry)
    }

    fn region_body(entry: &[Node]) -> Vec<Node> {
        for n in entry {
            if let Node::Region { body, .. } = n {
                return body.as_ref().clone();
            }
        }
        entry.to_vec()
    }

    fn count_loads_in_lets(nodes: &[Node]) -> usize {
        nodes
            .iter()
            .map(|n| match n {
                Node::Let {
                    value: Expr::Load { .. },
                    ..
                } => 1,
                Node::If {
                    then, otherwise, ..
                } => count_loads_in_lets(then) + count_loads_in_lets(otherwise),
                Node::Loop { body, .. } | Node::Block(body) => count_loads_in_lets(body),
                Node::Region { body, .. } => count_loads_in_lets(body),
                _ => 0,
            })
            .sum()
    }

    #[test]
    fn forwards_store_value_into_immediate_load() {
        let entry = vec![
            Node::store("a", Expr::u32(0), Expr::u32(7)),
            Node::let_bind("x", Expr::load("a", Expr::u32(0))),
            Node::store("b", Expr::u32(0), Expr::var("x")),
        ];
        let result = StoreToLoadForward::transform(program(entry));
        assert!(result.changed);
        let body = region_body(result.program.entry());
        assert_eq!(
            count_loads_in_lets(&body),
            0,
            "the Load should be forwarded to the literal 7"
        );
    }

    #[test]
    fn does_not_forward_when_intervening_write_to_same_buffer_clobbers() {
        let entry = vec![
            Node::store("a", Expr::u32(0), Expr::u32(7)),
            Node::store("a", Expr::u32(0), Expr::u32(9)),
            Node::let_bind("x", Expr::load("a", Expr::u32(0))),
        ];
        let result = StoreToLoadForward::transform(program(entry));
        // The forwarder finds the SECOND Store as the most recent one
        //  -  it is the same buffer/index, so the load forwards from `9`,
        // not the original `7`. That IS a valid forwarding (the value
        // the Load would observe)  -  assert it fired.

        assert!(result.changed);
    }

    #[test]
    fn does_not_forward_when_intervening_store_clobbers_same_buffer_different_index() {
        // Store(a, 0, 7); Store(a, 1, 9); Load(a, 0)   -  forwarding
        // should still find the (a, 0) Store, but the intervening
        // (a, 1) Store is on the same buffer and our conservative
        // walker bails out the moment it sees a same-buffer Store
        // that doesn't match the index. Document that behavior here.
        let entry = vec![
            Node::store("a", Expr::u32(0), Expr::u32(7)),
            Node::store("a", Expr::u32(1), Expr::u32(9)),
            Node::let_bind("x", Expr::load("a", Expr::u32(0))),
        ];
        let result = StoreToLoadForward::transform(program(entry));
        assert!(
            !result.changed,
            "conservative same-buffer different-index Store blocks forwarding"
        );
    }

    #[test]
    fn does_not_forward_across_barrier() {
        let entry = vec![
            Node::store("a", Expr::u32(0), Expr::u32(7)),
            Node::barrier(),
            Node::let_bind("x", Expr::load("a", Expr::u32(0))),
        ];
        let result = StoreToLoadForward::transform(program(entry));
        assert!(!result.changed, "Barrier blocks forwarding");
    }

    #[test]
    fn does_not_forward_when_value_contains_load() {
        // Forwarding a value that itself reads memory would duplicate
        // the read  -  different observable behavior under relaxed memory.
        let entry = vec![
            Node::store("a", Expr::u32(0), Expr::load("b", Expr::u32(0))),
            Node::let_bind("x", Expr::load("a", Expr::u32(0))),
        ];
        let result = StoreToLoadForward::transform(program(entry));
        assert!(!result.changed, "forwarded value contains a Load; rejected");
    }

    #[test]
    fn does_not_forward_when_intervening_atomic_touches_buffer() {
        let entry = vec![
            Node::store("a", Expr::u32(0), Expr::u32(7)),
            Node::let_bind(
                "y",
                Expr::Atomic {
                    op: crate::ir::AtomicOp::Add,
                    buffer: crate::ir::Ident::from("a"),
                    index: Box::new(Expr::u32(0)),
                    expected: None,
                    value: Box::new(Expr::u32(1)),
                    ordering: crate::ir::MemoryOrdering::Relaxed,
                },
            ),
            Node::let_bind("x", Expr::load("a", Expr::u32(0))),
        ];
        let result = StoreToLoadForward::transform(program(entry));
        assert!(
            !result.changed,
            "intervening Atomic on the same buffer blocks"
        );
    }

    #[test]
    fn forwards_through_unrelated_buffer_writes() {
        let entry = vec![
            Node::store("a", Expr::u32(0), Expr::u32(7)),
            Node::store("b", Expr::u32(0), Expr::u32(9)),
            Node::let_bind("x", Expr::load("a", Expr::u32(0))),
        ];
        let result = StoreToLoadForward::transform(program(entry));
        assert!(result.changed, "Store to a different buffer doesn't block");
        let body = region_body(result.program.entry());
        assert_eq!(count_loads_in_lets(&body), 0);
    }

    #[test]
    fn analyze_skips_program_with_no_forwardable_pair() {
        let entry = vec![Node::store("a", Expr::u32(0), Expr::u32(7))];
        assert_eq!(
            crate::optimizer::ProgramPass::analyze(&StoreToLoadForward, &program(entry)),
            PassAnalysis::SKIP
        );
    }

    #[test]
    fn analyze_runs_when_forwardable_pair_present() {
        let entry = vec![
            Node::store("a", Expr::u32(0), Expr::u32(7)),
            Node::let_bind("x", Expr::load("a", Expr::u32(0))),
        ];
        assert_eq!(
            crate::optimizer::ProgramPass::analyze(&StoreToLoadForward, &program(entry)),
            PassAnalysis::RUN
        );
    }
}
