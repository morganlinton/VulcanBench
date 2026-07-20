//! `tail_duplication`  -  hoist a common tail out of a divergent `Node::If`.
//!
//! Op id: `vyre-foundation::optimizer::passes::tail_duplication`.
//! Soundness: `Exact`  -  when both arms end with an identical, side-effect-free
//! tail node, that tail is observably equivalent to executing it after the If.
//! Cost-direction: monotone-down on code_size (removes one duplicated node).
//! Preserves: every analysis. Invalidates: nothing.
//!
//! ## Pattern
//!
//! ```text
//! If(c, [a, b], [a', b])
//!   where b == b' (identical tail)
//!   and b has length 1
//!   and b is observably side-effect-free
//!   → If(c, [a], [a']); b
//! ```
//!
//! ## ROADMAP
//!
//! A32  -  tail duplication for divergent branches.

use rustc_hash::FxHashSet;

use crate::ir::{Expr, Ident, Node, Program};
use crate::optimizer::{vyre_pass, PassAnalysis, PassResult};
use crate::visit::bound_names::collect_bound_names;
use crate::visit::node_map;

/// Hoist common side-effect-free tails out of `Node::If`.
#[derive(Debug, Default)]
#[vyre_pass(
    name = "tail_duplication",
    requires = [],
    invalidates = [],
    phase = "cleanup",
    boundary_class = "abi_preserving",
    cost_model_family = "fusion"
)]
pub struct TailDuplicationPass;

impl TailDuplicationPass {
    /// Skip programs without any candidate If.
    #[must_use]
    fn analyze_impl(program: &Program) -> PassAnalysis {
        if !program
            .stats()
            .has_any_node_kind(crate::ir::stats::NODE_KIND_IF)
        {
            return PassAnalysis::SKIP;
        }
        if program
            .entry()
            .iter()
            .any(|n| node_map::any_descendant(n, &mut is_tail_candidate))
        {
            PassAnalysis::RUN
        } else {
            PassAnalysis::SKIP
        }
    }

    /// Walk the entry tree; hoist common tails.
    #[must_use]
    pub fn transform(program: Program) -> PassResult {
        let mut changed = false;
        let program = program.map_entry(|entry| {
            entry
                .into_iter()
                .flat_map(|node| hoist_tail(node, &mut changed))
                .collect()
        });
        PassResult { program, changed }
    }
}

/// Recurse into descendants, then try to hoist this node's tail.
fn hoist_tail(node: Node, changed: &mut bool) -> Vec<Node> {
    // First recurse into children
    let recursed = node_map::map_children(node, &mut |child| {
        let hoisted = hoist_tail(child, changed);
        if hoisted.len() == 1 {
            hoisted
                .into_iter()
                .next()
                .unwrap_or(Node::Block(Vec::new()))
        } else {
            Node::Block(hoisted)
        }
    });

    // Then try to hoist from this node's body
    if let Node::If {
        cond,
        then,
        otherwise,
    } = recursed
    {
        if let Some((new_then, new_otherwise, tail)) = try_extract_tail(&then, &otherwise) {
            *changed = true;
            let new_if = Node::If {
                cond,
                then: new_then,
                otherwise: new_otherwise,
            };
            return vec![new_if, tail];
        }
        return vec![Node::If {
            cond,
            then,
            otherwise,
        }];
    }

    vec![recursed]
}

/// Try to extract a common tail from `then` and `otherwise` arms.
///
/// Returns `Some((new_then, new_otherwise, tail))` when:
/// - Both arms are non-empty
/// - Last node of each arm is identical
/// - The common tail is a single node that is observably free
fn try_extract_tail(then: &[Node], otherwise: &[Node]) -> Option<(Vec<Node>, Vec<Node>, Node)> {
    if then.is_empty() || otherwise.is_empty() {
        return None;
    }

    let then_tail = then.last()?;
    let otherwise_tail = otherwise.last()?;

    if then_tail != otherwise_tail {
        return None;
    }

    if !node_is_observably_free(then_tail) {
        return None;
    }

    let new_then = then[..then.len() - 1].to_vec();
    let new_otherwise = otherwise[..otherwise.len() - 1].to_vec();

    // The tail is sunk PAST the If. A variable bound inside an arm (by a
    // `let` or loop var, before the tail) is out of scope once the tail is
    // hoisted out — sinking a read of it produces scope-invalid IR (the
    // reference interpreter and IR validator reject "reference to
    // undeclared variable"). Refuse when the tail reads any name bound in
    // either arm body. Names the tail reads that are NOT bound in an arm
    // must come from an enclosing scope (shadowing is disallowed), so they
    // remain in scope after the If and are safe to sink past.
    let mut arm_bound: FxHashSet<Ident> = FxHashSet::default();
    collect_bound_names(&new_then, &mut arm_bound);
    collect_bound_names(&new_otherwise, &mut arm_bound);
    if !arm_bound.is_empty() && node_reads_any(then_tail, &arm_bound) {
        return None;
    }

    let tail = then_tail.clone();
    Some((new_then, new_otherwise, tail))
}


/// True iff `node` reads (via an `Expr::Var`) any name in `names`. `node` is
/// an observably-free tail (a pure `Let`, or a `Block` of such), so only the
/// pure expression forms can appear; effectful forms are handled defensively.
fn node_reads_any(node: &Node, names: &FxHashSet<Ident>) -> bool {
    match node {
        Node::Let { value, .. } => expr_reads_any(value, names),
        Node::Block(body) => body.iter().any(|n| node_reads_any(n, names)),
        _ => false,
    }
}

/// True iff `expr` references any name in `names`.
fn expr_reads_any(expr: &Expr, names: &FxHashSet<Ident>) -> bool {
    match expr {
        Expr::Var(name) => names.contains(name),
        Expr::BinOp { left, right, .. } => {
            expr_reads_any(left, names) || expr_reads_any(right, names)
        }
        Expr::UnOp { operand, .. } => expr_reads_any(operand, names),
        Expr::Select {
            cond,
            true_val,
            false_val,
        } => {
            expr_reads_any(cond, names)
                || expr_reads_any(true_val, names)
                || expr_reads_any(false_val, names)
        }
        Expr::Cast { value, .. } => expr_reads_any(value, names),
        Expr::Fma { a, b, c } => {
            expr_reads_any(a, names) || expr_reads_any(b, names) || expr_reads_any(c, names)
        }
        Expr::Load { index, .. } => expr_reads_any(index, names),
        _ => false,
    }
}

/// True iff `node` has no observable side effects (no Store, Atomic,
/// Loop, Barrier, AsyncLoad/AsyncStore, etc.).
///
/// `Let` is observably-free only when the bound expression is itself
/// pure: hoisting `let x = atomic_add(...)` out of an If would change
/// the atomic count, and hoisting subgroup collectives would break
/// uniform-control-flow requirements. Loads are excluded because the
/// guarded If may exist precisely to avoid an out-of-bounds access.
fn node_is_observably_free(node: &Node) -> bool {
    match node {
        Node::Let { value, .. } => expr_is_pure(value),
        Node::Block(body) => body.iter().all(node_is_observably_free),
        // Everything else has or may have side effects.
        Node::Store { .. }
        | Node::Assign { .. }
        | Node::If { .. }
        | Node::Loop { .. }
        | Node::Region { .. }
        | Node::Return
        | Node::Barrier { .. }
        | Node::IndirectDispatch { .. }
        | Node::AsyncLoad { .. }
        | Node::AsyncStore { .. }
        | Node::AllReduce { .. }
        | Node::AllGather { .. }
        | Node::ReduceScatter { .. }
        | Node::Broadcast { .. }
        | Node::AsyncWait { .. }
        | Node::Trap { .. }
        | Node::Resume { .. }
        | Node::Opaque(_) => false,
    }
}

/// True iff `expr` evaluates without side effects, observable I/O, or
/// uniform-control-flow obligations. The whitelist is intentional:
/// any new Expr variant defaults to `false` until classified.
fn expr_is_pure(expr: &Expr) -> bool {
    match expr {
        Expr::LitU32(_)
        | Expr::LitI32(_)
        | Expr::LitF32(_)
        | Expr::LitBool(_)
        | Expr::Var(_)
        | Expr::BufLen { .. }
        | Expr::InvocationId { .. }
        | Expr::WorkgroupId { .. }
        | Expr::LocalId { .. } => true,
        Expr::BinOp { left, right, .. } => expr_is_pure(left) && expr_is_pure(right),
        Expr::UnOp { operand, .. } => expr_is_pure(operand),
        Expr::Select {
            cond,
            true_val,
            false_val,
        } => expr_is_pure(cond) && expr_is_pure(true_val) && expr_is_pure(false_val),
        Expr::Cast { value, .. } => expr_is_pure(value),
        Expr::Fma { a, b, c } => expr_is_pure(a) && expr_is_pure(b) && expr_is_pure(c),
        // Loads are reads from buffers  -  value-pure when in-bounds, but
        // a guarded If may exist to avoid an OOB index, so hoisting
        // changes observable behavior on GPUs.
        Expr::Load { .. }
        // Atomics RMW or fence; hoisting changes count or ordering.
        | Expr::Atomic { .. }
        // Calls are opaque to this pass.
        | Expr::Call { .. }
        // Subgroup ops require uniform control flow; hoisting out of a
        // divergent If is the *opposite* of safe.
        | Expr::SubgroupBallot { .. }
        | Expr::SubgroupShuffle { .. }
        | Expr::SubgroupReduce { .. }
        // Lane-correlated builtins  -  value is always equal across
        // hoisted/unhoisted positions, but downstream lane-uniform
        // analyses treat hoisted Lets as uniform-by-construction. Keep
        // these gated for parity with branch_value_hoist::expr_is_observably_free.
        | Expr::SubgroupLocalId
        | Expr::SubgroupSize
        // Opaque extensions  -  unknown semantics, refuse.
        | Expr::Opaque(_) => false,
    }
}

/// True iff `node` is an `If` whose arms have an extractable common tail.
fn is_tail_candidate(node: &Node) -> bool {
    if let Node::If {
        then, otherwise, ..
    } = node
    {
        try_extract_tail(then, otherwise).is_some()
    } else {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{BufferAccess, BufferDecl, DataType, Expr, Node};

    fn buf() -> BufferDecl {
        BufferDecl::storage("buf", 0, BufferAccess::ReadWrite, DataType::U32).with_count(4)
    }

    fn program_with_entry(entry: Vec<Node>) -> Program {
        Program::wrapped(vec![buf()], [1, 1, 1], entry)
    }

    /// Positive: common Let tail is hoisted out.
    #[test]
    fn hoists_common_let_tail() {
        let common = Node::let_bind("x", Expr::u32(42));
        let entry = vec![Node::If {
            cond: Expr::var("c"),
            then: vec![
                Node::store("buf", Expr::u32(0), Expr::u32(1)),
                common.clone(),
            ],
            otherwise: vec![Node::store("buf", Expr::u32(0), Expr::u32(2)), common],
        }];
        let program = program_with_entry(entry);
        let result = TailDuplicationPass::transform(program);
        assert!(result.changed, "common tail must be hoisted");
    }

    /// Negative: tails differ.
    #[test]
    fn keeps_when_tails_differ() {
        let entry = vec![Node::If {
            cond: Expr::var("c"),
            then: vec![Node::let_bind("x", Expr::u32(1))],
            otherwise: vec![Node::let_bind("x", Expr::u32(2))],
        }];
        let program = program_with_entry(entry);
        let result = TailDuplicationPass::transform(program);
        assert!(!result.changed, "must not hoist when tails are different");
    }

    /// Negative: tail has side effects (Store).
    #[test]
    fn keeps_when_tail_has_side_effects() {
        let common = Node::store("buf", Expr::u32(0), Expr::u32(7));
        let entry = vec![Node::If {
            cond: Expr::var("c"),
            then: vec![Node::let_bind("x", Expr::u32(1)), common.clone()],
            otherwise: vec![Node::let_bind("x", Expr::u32(2)), common],
        }];
        let program = program_with_entry(entry);
        let result = TailDuplicationPass::transform(program);
        assert!(
            !result.changed,
            "must not hoist tail with Store (side effects)"
        );
    }

    #[test]
    fn keeps_when_tail_is_loop() {
        let common = Node::Loop {
            var: crate::ir::Ident::from("i"),
            from: Expr::u32(0),
            to: Expr::u32(5),
            body: vec![],
        };
        let entry = vec![Node::If {
            cond: Expr::var("c"),
            then: vec![Node::let_bind("x", Expr::u32(1)), common.clone()],
            otherwise: vec![Node::let_bind("x", Expr::u32(2)), common],
        }];
        let program = program_with_entry(entry);
        let result = TailDuplicationPass::transform(program);
        assert!(!result.changed, "must not hoist Loop as tail");
    }

    #[test]
    fn analyze_skips_program_with_no_tail_candidates() {
        let entry = vec![Node::store("buf", Expr::u32(0), Expr::u32(7))];
        let program = program_with_entry(entry);
        assert_eq!(
            crate::optimizer::ProgramPass::analyze(&TailDuplicationPass, &program),
            PassAnalysis::SKIP
        );
    }

    #[test]
    fn analyze_runs_when_tail_candidate_present() {
        let common = Node::let_bind("x", Expr::u32(42));
        let entry = vec![Node::If {
            cond: Expr::var("c"),
            then: vec![
                Node::store("buf", Expr::u32(0), Expr::u32(1)),
                common.clone(),
            ],
            otherwise: vec![Node::store("buf", Expr::u32(0), Expr::u32(2)), common],
        }];
        let program = program_with_entry(entry);
        assert_eq!(
            crate::optimizer::ProgramPass::analyze(&TailDuplicationPass, &program),
            PassAnalysis::RUN
        );
    }

    /// Negative: an identical `Let { value: Atomic }` tail in both arms
    /// would change the atomic count if hoisted out of the If  -  hoisting
    /// it executes one RMW where the original program executed exactly
    /// one in either arm of a divergent dispatch, but the *aggregated*
    /// effect across lanes is observably different. Refuse.
    #[test]
    fn keeps_let_with_atomic_value_unhoisted() {
        let atomic = Expr::Atomic {
            op: crate::ir::AtomicOp::Add,
            buffer: crate::ir::Ident::from("buf"),
            index: Box::new(Expr::u32(0)),
            expected: None,
            value: Box::new(Expr::u32(1)),
            ordering: crate::memory_model::MemoryOrdering::Relaxed,
        };
        let common = Node::let_bind("x", atomic);
        let entry = vec![Node::If {
            cond: Expr::var("c"),
            then: vec![
                Node::store("buf", Expr::u32(1), Expr::u32(7)),
                common.clone(),
            ],
            otherwise: vec![Node::store("buf", Expr::u32(2), Expr::u32(7)), common],
        }];
        let program = program_with_entry(entry);
        let result = TailDuplicationPass::transform(program);
        assert!(
            !result.changed,
            "must not hoist Let{{Atomic}}  -  atomic count is observable"
        );
    }

    /// Negative: `Let { value: SubgroupShuffle }` requires uniform
    /// control flow. Hoisting out of a divergent If is the opposite of
    /// safe.
    #[test]
    fn keeps_let_with_subgroup_shuffle_unhoisted() {
        let shuffle = Expr::SubgroupShuffle {
            value: Box::new(Expr::var("v")),
            lane: Box::new(Expr::u32(0)),
        };
        let common = Node::let_bind("x", shuffle);
        let entry = vec![Node::If {
            cond: Expr::var("c"),
            then: vec![
                Node::store("buf", Expr::u32(0), Expr::u32(1)),
                common.clone(),
            ],
            otherwise: vec![Node::store("buf", Expr::u32(0), Expr::u32(2)), common],
        }];
        let program = program_with_entry(entry);
        let result = TailDuplicationPass::transform(program);
        assert!(
            !result.changed,
            "must not hoist Let{{SubgroupShuffle}}  -  requires uniform control flow"
        );
    }

    /// Negative: the guarded If may exist precisely to gate an OOB
    /// load. Hoisting `let x = buf[i]` out of `if (i < buf_len)` would
    /// re-introduce the OOB read.
    #[test]
    fn keeps_let_with_load_value_unhoisted() {
        let load = Expr::Load {
            buffer: crate::ir::Ident::from("buf"),
            index: Box::new(Expr::var("i")),
        };
        let common = Node::let_bind("x", load);
        let entry = vec![Node::If {
            cond: Expr::var("c"),
            then: vec![
                Node::store("buf", Expr::u32(0), Expr::u32(1)),
                common.clone(),
            ],
            otherwise: vec![Node::store("buf", Expr::u32(0), Expr::u32(2)), common],
        }];
        let program = program_with_entry(entry);
        let result = TailDuplicationPass::transform(program);
        assert!(
            !result.changed,
            "must not hoist Let{{Load}}  -  guarded If may be the OOB sanitizer"
        );
    }

    /// Positive (unchanged behavior): a pure Let{Lit} tail still hoists.
    #[test]
    fn still_hoists_pure_let_lit_after_filter() {
        let common = Node::let_bind("x", Expr::u32(123));
        let entry = vec![Node::If {
            cond: Expr::var("c"),
            then: vec![
                Node::store("buf", Expr::u32(0), Expr::u32(1)),
                common.clone(),
            ],
            otherwise: vec![Node::store("buf", Expr::u32(0), Expr::u32(2)), common],
        }];
        let program = program_with_entry(entry);
        let result = TailDuplicationPass::transform(program);
        assert!(result.changed);
    }

    /// Negative (scope): a tail `let y = t + 1` where `t` is bound INSIDE the
    /// arm must NOT be hoisted — sinking it past the If would read `t` out of
    /// scope, producing scope-invalid IR (the reference interpreter rejects
    /// "reference to undeclared variable `t`"). The oracle-differential proof
    /// lives in `tests/tail_duplication_scope.rs`.
    #[test]
    fn keeps_tail_reading_arm_local_binding() {
        let tail = Node::let_bind("y", Expr::add(Expr::var("t"), Expr::u32(1)));
        let entry = vec![Node::If {
            cond: Expr::var("c"),
            then: vec![Node::let_bind("t", Expr::u32(5)), tail.clone()],
            otherwise: vec![Node::let_bind("t", Expr::u32(9)), tail],
        }];
        let program = program_with_entry(entry);
        let result = TailDuplicationPass::transform(program);
        assert!(
            !result.changed,
            "tail reading arm-local `t` must not be sunk past the If (would read t out of scope)"
        );
    }

    /// Positive (scope): a tail reading a variable bound in an ENCLOSING
    /// scope (still in scope after the If) hoists normally — the guard is
    /// precise, not a blanket disable of every Var-reading tail.
    #[test]
    fn hoists_tail_reading_enclosing_binding() {
        let tail = Node::let_bind("y", Expr::add(Expr::var("outer"), Expr::u32(1)));
        let entry = vec![
            Node::let_bind("outer", Expr::u32(3)),
            Node::If {
                cond: Expr::var("c"),
                then: vec![Node::store("buf", Expr::u32(0), Expr::u32(1)), tail.clone()],
                otherwise: vec![Node::store("buf", Expr::u32(0), Expr::u32(2)), tail],
            },
        ];
        let program = program_with_entry(entry);
        let result = TailDuplicationPass::transform(program);
        assert!(
            result.changed,
            "tail reading enclosing-scope `outer` must still hoist (it stays in scope after the If)"
        );
    }
}
