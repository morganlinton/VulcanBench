//! `dead_store_elim`  -  drop `Node::Store` whose value is overwritten
//! by a subsequent sibling `Node::Store` to the same `(buffer, index)`
//! before any intervening side-effect could observe the first write.
//!
//! Op id: `vyre-foundation::optimizer::passes::dead_store_elim`.
//! Soundness: `Exact`  -  when no `Load` against the same buffer, no
//! `Atomic` against the same buffer, no `Store` to a different lane of
//! the same buffer, no `AsyncLoad`/`AsyncStore` referencing the same
//! buffer, no `IndirectDispatch`, no `Trap`/`Resume`, no nested
//! `If`/`Loop`/`Region`/`Block`/`Opaque`, and no `Barrier` separates the
//! two sibling stores, the earlier store cannot be observed and is
//! deleted. Cost direction: monotone-down on `node_count`.
//! Preserves: every analysis. Invalidates: nothing.
//!
//! ## Rule
//!
//! ```text
//! Node::Store { buffer: b, index: i, value: _ }
//! ... straight-line siblings, no nested control flow, no
//!     Load/Atomic/Store/Async*/Indirect/Trap/Barrier touching b ...
//! Node::Store { buffer: b, index: i, value: _ }
//! ```
//!
//! When the matcher fires, the FIRST `Store` is dropped. The second
//! survives and contributes the observable write. Indices are matched
//! by structural equality (literal-aware via `expr_eq`), so dynamic-
//! index stores are kept conservatively. The pass walks recursively
//! through `If`/`Loop`/`Block`/`Region` containers but only fires on
//! sibling sequences inside one container; cross-container DSE is left
//! to a stronger reaching-store analysis (ROADMAP A22 store-to-load
//! forwarding will produce the alias proof needed for that).
//!
//! Catches:
//!   - generated `Store(buf, 0, x); Store(buf, 0, y);` patterns from
//!     unfused arms or const-fold residue;
//!   - duplicate clears that the host would emit twice if a previous
//!     pass left a redundant initialisation in place.
//!
//! Does not catch (deliberately):
//!   - stores separated by an `If` whose branches don't touch the
//!     buffer (would need branch-aware reaching-store analysis);
//!   - stores to overlapping but not equal indices (no alias model
//!     yet);
//!   - stores where the value of the first one is later read via
//!     `Load(buffer, *)`  -  `expr_touches_buffer` keeps the first
//!     store alive when any node between the two reads from `buffer`.

use crate::ir::{AtomicOp, Expr, Ident, Node, Program};
use crate::optimizer::{vyre_pass, PassAnalysis, PassResult};
use crate::visit::node_map;

/// Drop straight-line `Node::Store` values that are overwritten by the
/// next sibling `Node::Store` to the same `(buffer, index)` with no
/// observable read in between.
#[derive(Debug, Default)]
#[vyre_pass(
    name = "dead_store_elim",
    requires = [],
    invalidates = [],
    phase = "memory",
    boundary_class = "abi_preserving",
    cost_model_family = "memory"
)]
pub struct DeadStoreElim;

impl DeadStoreElim {
    /// Skip the pass when no body in the program contains two stores
    /// to the same buffer that *could* alias each other.
    #[must_use]
    fn analyze_impl(program: &Program) -> PassAnalysis {
        if !program
            .stats()
            .has_any_node_kind(crate::ir::stats::NODE_KIND_STORE)
        {
            return PassAnalysis::SKIP;
        }
        if program
            .entry()
            .iter()
            .any(|n| node_map::any_descendant(n, &mut has_redundant_store_pair))
        {
            PassAnalysis::RUN
        } else {
            PassAnalysis::SKIP
        }
    }

    /// Walk the program tree; remove dead sibling stores in every
    /// sequence body that has them.
    #[must_use]
    pub fn transform(program: Program) -> PassResult {
        let mut changed = false;
        let program = program.map_entry(|entry| {
            drop_dead_stores(
                entry
                    .into_iter()
                    .map(|n| rewrite_node(n, &mut changed))
                    .collect(),
                &mut changed,
            )
        });
        PassResult { program, changed }
    }
}

fn rewrite_node(node: Node, changed: &mut bool) -> Node {
    let recursed = node_map::map_children(node, &mut |child| rewrite_node(child, changed));
    node_map::map_body(recursed, &mut |body| drop_dead_stores(body, changed))
}

/// Remove every `Store(b, i, _)` that has a later sibling `Store(b, i, _)`
/// with no intervening reader of `b` between them.
fn drop_dead_stores(body: Vec<Node>, changed: &mut bool) -> Vec<Node> {
    let mut keep = vec![true; body.len()];
    for first_idx in 0..body.len() {
        if !keep[first_idx] {
            continue;
        }
        let Node::Store {
            buffer: first_buf,
            index: first_idx_expr,
            ..
        } = &body[first_idx]
        else {
            continue;
        };
        for second_idx in (first_idx + 1)..body.len() {
            let between = &body[(first_idx + 1)..second_idx];
            if any_node_observes_buffer(between, first_buf) {
                break;
            }
            match &body[second_idx] {
                Node::Store {
                    buffer: second_buf,
                    index: second_idx_expr,
                    ..
                } if second_buf == first_buf
                    && expr_structurally_eq(first_idx_expr, second_idx_expr) =>
                {
                    keep[first_idx] = false;
                    *changed = true;
                    break;
                }
                node if node_observes_buffer(node, first_buf) => {
                    break;
                }
                _ => {
                    // Keep scanning forward; this sibling is not a
                    // store to (first_buf, first_idx_expr) and does
                    // not touch first_buf either.
                }
            }
        }
    }
    body.into_iter()
        .zip(keep)
        .filter_map(|(node, alive)| alive.then_some(node))
        .collect()
}

/// True iff any node in `nodes` could read or otherwise observe the
/// pre-store contents of `buffer`. Conservative: any nested control
/// flow, barrier, async transfer, atomic, indirect dispatch, trap, or
/// region is treated as observing the buffer.
fn any_node_observes_buffer(nodes: &[Node], buffer: &Ident) -> bool {
    nodes.iter().any(|n| node_observes_buffer(n, buffer))
}

/// True iff `node` could observe the pre-store contents of `buffer`
/// before the next sibling store overwrites it.
fn node_observes_buffer(node: &Node, buffer: &Ident) -> bool {
    match node {
        Node::Store {
            buffer: _target,
            index,
            value,
        } => {
            // The store's WRITE does not read `buffer`'s pre-store contents,
            // but its index and value subexpressions are evaluated first and
            // may Load from `buffer` (e.g. `A[A[j]] = A[k]`). This holds
            // whether or not the store targets `buffer` itself: a same-buffer
            // store that addresses or sources from `buffer` still observes the
            // pre-store value, so an earlier store to it cannot be dropped.
            // (Previously a same-buffer store short-circuited to `false`,
            // skipping these subexpression reads — a dead-store miscompile.)
            expr_touches_buffer(index, buffer) || expr_touches_buffer(value, buffer)
        }
        Node::Let { value, .. } | Node::Assign { value, .. } => expr_touches_buffer(value, buffer),
        Node::If {
            cond,
            then,
            otherwise,
        } => {
            expr_touches_buffer(cond, buffer)
                || any_node_observes_buffer(then, buffer)
                || any_node_observes_buffer(otherwise, buffer)
        }
        Node::Loop { from, to, body, .. } => {
            expr_touches_buffer(from, buffer)
                || expr_touches_buffer(to, buffer)
                || any_node_observes_buffer(body, buffer)
        }
        Node::Block(body) => any_node_observes_buffer(body, buffer),
        Node::Region { body, .. } => any_node_observes_buffer(body.as_ref(), buffer),
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

/// True iff `expr` reads from `buffer` or invokes a side-effect that
/// could observe its pre-store contents.
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
            op,
            ..
        } => {
            other == buffer
                || expr_touches_buffer(index, buffer)
                || matches!(
                    op,
                    AtomicOp::CompareExchange | AtomicOp::CompareExchangeWeak
                )
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

/// Cheap structural equality between two index expressions. Conservative:
/// returns `true` only when the two expressions are syntactically
/// identical (same variant + same children). Equal under this relation
/// implies equal at runtime; the converse does not hold, so we keep
/// stores conservatively when the matcher cannot prove equality.
fn expr_structurally_eq(left: &Expr, right: &Expr) -> bool {
    left == right
}

/// Whether the program has any sibling pair of stores to the same
/// buffer  -  cheap analysis used by the pass scheduler to skip programs
/// where DSE has nothing to do.
fn has_redundant_store_pair(node: &Node) -> bool {
    let body: &[Node] = match node {
        Node::If {
            then, otherwise, ..
        } => {
            return contains_buffer_pair(then) || contains_buffer_pair(otherwise);
        }
        Node::Loop { body, .. } | Node::Block(body) => body,
        Node::Region { body, .. } => body.as_ref(),
        _ => return false,
    };
    contains_buffer_pair(body)
}

fn contains_buffer_pair(body: &[Node]) -> bool {
    let mut seen: rustc_hash::FxHashSet<&Ident> = rustc_hash::FxHashSet::default();
    for n in body {
        if let Node::Store { buffer, .. } = n {
            if !seen.insert(buffer) {
                return true;
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{BufferAccess, BufferDecl, DataType, Expr, Ident, Node};

    fn buf(name: &str) -> BufferDecl {
        BufferDecl::storage(name, 0, BufferAccess::ReadWrite, DataType::U32).with_count(4)
    }

    fn program_with_entry(entry: Vec<Node>) -> Program {
        Program::wrapped(vec![buf("buf"), buf("other")], [1, 1, 1], entry)
    }

    fn count_stores(node: &Node) -> usize {
        match node {
            Node::Store { .. } => 1,
            Node::If {
                then, otherwise, ..
            } => {
                then.iter().map(count_stores).sum::<usize>()
                    + otherwise.iter().map(count_stores).sum::<usize>()
            }
            Node::Loop { body, .. } | Node::Block(body) => body.iter().map(count_stores).sum(),
            Node::Region { body, .. } => body.iter().map(count_stores).sum(),
            _ => 0,
        }
    }

    #[test]
    fn drops_first_of_two_back_to_back_stores_to_same_index() {
        let entry = vec![
            Node::store("buf", Expr::u32(0), Expr::u32(1)),
            Node::store("buf", Expr::u32(0), Expr::u32(2)),
        ];
        let program = program_with_entry(entry);
        let result = DeadStoreElim::transform(program);
        assert!(result.changed);
        let total: usize = result.program.entry().iter().map(count_stores).sum();
        assert_eq!(total, 1, "first dead store must be dropped");
    }

    #[test]
    fn keeps_both_stores_to_different_indices() {
        let entry = vec![
            Node::store("buf", Expr::u32(0), Expr::u32(1)),
            Node::store("buf", Expr::u32(1), Expr::u32(2)),
        ];
        let program = program_with_entry(entry);
        let result = DeadStoreElim::transform(program);
        assert!(!result.changed);
        let total: usize = result.program.entry().iter().map(count_stores).sum();
        assert_eq!(total, 2);
    }

    #[test]
    fn keeps_both_stores_to_different_buffers() {
        let entry = vec![
            Node::store("buf", Expr::u32(0), Expr::u32(1)),
            Node::store("other", Expr::u32(0), Expr::u32(2)),
        ];
        let program = program_with_entry(entry);
        let result = DeadStoreElim::transform(program);
        assert!(!result.changed);
    }

    #[test]
    fn keeps_first_store_when_intervening_load_observes_it() {
        // Store(buf, 0, 1); Let(x, Load(buf, 0)); Store(buf, 0, 2)
        // The Load reads the first store; cannot drop it.
        let entry = vec![
            Node::store("buf", Expr::u32(0), Expr::u32(1)),
            Node::let_bind("x", Expr::load("buf", Expr::u32(0))),
            Node::store("buf", Expr::u32(0), Expr::u32(2)),
        ];
        let program = program_with_entry(entry);
        let result = DeadStoreElim::transform(program);
        assert!(
            !result.changed,
            "must not drop a store whose value is observably read before the overwrite"
        );
    }

    #[test]
    fn drops_first_when_intervening_let_reads_a_different_buffer() {
        // The reader touches `other`, not `buf`. Safe to drop the first
        // store to buf.
        let entry = vec![
            Node::store("buf", Expr::u32(0), Expr::u32(1)),
            Node::let_bind("x", Expr::load("other", Expr::u32(0))),
            Node::store("buf", Expr::u32(0), Expr::u32(2)),
        ];
        let program = program_with_entry(entry);
        let result = DeadStoreElim::transform(program);
        assert!(result.changed);
        let total: usize = result.program.entry().iter().map(count_stores).sum();
        assert_eq!(total, 1, "only the overwritten store is removed");
    }

    #[test]
    fn keeps_first_store_when_later_same_buffer_store_index_reads_it() {
        // Store(buf,0,1); Store(buf, Load(buf,0), 2); Store(buf,0,3)
        // The middle store writes to the SAME buffer, but its INDEX reads
        // buf[0] — the first store's value — to compute where it writes.
        // Dropping the first store would change that index: a miscompile.
        let entry = vec![
            Node::store("buf", Expr::u32(0), Expr::u32(1)),
            Node::store("buf", Expr::load("buf", Expr::u32(0)), Expr::u32(2)),
            Node::store("buf", Expr::u32(0), Expr::u32(3)),
        ];
        let program = program_with_entry(entry);
        let result = DeadStoreElim::transform(program);
        assert!(
            !result.changed,
            "a same-buffer store whose index reads buf observes the first store"
        );
        let total: usize = result.program.entry().iter().map(count_stores).sum();
        assert_eq!(total, 3, "all three stores must survive");
    }

    #[test]
    fn keeps_first_store_when_later_same_buffer_store_value_reads_it() {
        // Store(buf,0,1); Store(buf, 9, Load(buf,0)); Store(buf,0,3)
        // The middle store targets a different index of the SAME buffer, but
        // its VALUE reads buf[0] (the first store's value). Dropping the first
        // store would change that value: a miscompile.
        let entry = vec![
            Node::store("buf", Expr::u32(0), Expr::u32(1)),
            Node::store("buf", Expr::u32(9), Expr::load("buf", Expr::u32(0))),
            Node::store("buf", Expr::u32(0), Expr::u32(3)),
        ];
        let program = program_with_entry(entry);
        let result = DeadStoreElim::transform(program);
        assert!(
            !result.changed,
            "a same-buffer store whose value reads buf observes the first store"
        );
        let total: usize = result.program.entry().iter().map(count_stores).sum();
        assert_eq!(total, 3, "all three stores must survive");
    }

    #[test]
    fn keeps_first_store_when_barrier_separates_it() {
        // Barriers are observation points (other invocations may read
        // the buffer post-barrier). Conservative: keep the first store.
        let entry = vec![
            Node::store("buf", Expr::u32(0), Expr::u32(1)),
            Node::barrier(),
            Node::store("buf", Expr::u32(0), Expr::u32(2)),
        ];
        let program = program_with_entry(entry);
        let result = DeadStoreElim::transform(program);
        assert!(
            !result.changed,
            "Barrier between stores must keep the first one alive"
        );
    }

    #[test]
    fn keeps_first_store_when_atomic_read_intervenes() {
        // Atomic reads on the same buffer count as observations.
        let entry = vec![
            Node::store("buf", Expr::u32(0), Expr::u32(1)),
            Node::let_bind(
                "x",
                Expr::Atomic {
                    op: AtomicOp::Exchange,
                    buffer: Ident::from("buf"),
                    index: Box::new(Expr::u32(0)),
                    expected: None,
                    value: Box::new(Expr::u32(0)),
                    ordering: crate::ir::MemoryOrdering::Relaxed,
                },
            ),
            Node::store("buf", Expr::u32(0), Expr::u32(2)),
        ];
        let program = program_with_entry(entry);
        let result = DeadStoreElim::transform(program);
        assert!(!result.changed);
    }

    #[test]
    fn drops_dead_store_inside_if_branch() {
        let entry = vec![Node::if_then(
            Expr::var("c"),
            vec![
                Node::store("buf", Expr::u32(0), Expr::u32(1)),
                Node::store("buf", Expr::u32(0), Expr::u32(2)),
            ],
        )];
        let program = program_with_entry(entry);
        let result = DeadStoreElim::transform(program);
        assert!(result.changed);
        let total: usize = result.program.entry().iter().map(count_stores).sum();
        assert_eq!(total, 1);
    }

    #[test]
    fn keeps_stores_separated_by_nested_if() {
        // The intervening `If` could read from `buf` in either branch
        //  -  we conservatively keep the first store.
        let entry = vec![
            Node::store("buf", Expr::u32(0), Expr::u32(1)),
            Node::if_then(Expr::var("c"), vec![Node::Return]),
            Node::store("buf", Expr::u32(0), Expr::u32(2)),
        ];
        let program = program_with_entry(entry);
        let result = DeadStoreElim::transform(program);
        assert!(
            !result.changed,
            "nested If between stores is opaque under conservative DSE  -  keep the first"
        );
    }

    #[test]
    fn drops_chain_of_three_redundant_stores() {
        // s1, s2, s3 to (buf, 0): only s3 survives.
        let entry = vec![
            Node::store("buf", Expr::u32(0), Expr::u32(1)),
            Node::store("buf", Expr::u32(0), Expr::u32(2)),
            Node::store("buf", Expr::u32(0), Expr::u32(3)),
        ];
        let program = program_with_entry(entry);
        let result = DeadStoreElim::transform(program);
        assert!(result.changed);
        let total: usize = result.program.entry().iter().map(count_stores).sum();
        assert_eq!(total, 1, "only the last store survives");
    }

    #[test]
    fn analyze_skips_program_with_no_redundant_pair() {
        let entry = vec![
            Node::store("buf", Expr::u32(0), Expr::u32(1)),
            Node::store("other", Expr::u32(0), Expr::u32(2)),
        ];
        let program = program_with_entry(entry);
        assert_eq!(
            crate::optimizer::ProgramPass::analyze(&DeadStoreElim, &program),
            PassAnalysis::SKIP
        );
    }

    #[test]
    fn analyze_runs_when_redundant_pair_present() {
        let entry = vec![
            Node::store("buf", Expr::u32(0), Expr::u32(1)),
            Node::store("buf", Expr::u32(0), Expr::u32(2)),
        ];
        let program = program_with_entry(entry);
        assert_eq!(
            crate::optimizer::ProgramPass::analyze(&DeadStoreElim, &program),
            PassAnalysis::RUN
        );
    }
}
