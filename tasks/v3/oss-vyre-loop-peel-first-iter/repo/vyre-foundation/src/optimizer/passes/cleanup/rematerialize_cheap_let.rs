//! `rematerialize_cheap_let`  -  inline `Let` bindings whose value is a
//! trivially cheap leaf expression, then drop the binding.
//!
//! Op id: `vyre-foundation::optimizer::passes::rematerialize_cheap_let`.
//! Soundness: `Exact`. The inlined expressions are leaf-shaped and pure
//! (literals, named variables, BufLen, InvocationId/WorkgroupId/LocalId,
//! SubgroupLocalId, SubgroupSize): every reference site sees the same
//! value as a single materialization, and the IR's defined order keeps
//! every observable side effect intact because none of these
//! expressions has one. Cost-direction: monotone-down on
//! `register_pressure_estimate`  -  each successful rewrite removes one
//! named live binding; the substituted cheap expressions occupy the
//! same register lifetime they would have occupied as a `Var(name)`
//! load. Preserves: ABI and observable value semantics. Invalidates:
//! region inlining, canonicalization, constant folding, CSE, and DCE
//! because substituting leaves can expose smaller regions, foldable
//! expressions, duplicate expressions, and dead bindings.
//!
//! ## Pattern
//!
//! ```text
//! Let(x, cheap)        ;; cheap ∈ {Lit*, Var, BufLen, InvocationId,
//!                       ;;          WorkgroupId, LocalId,
//!                       ;;          SubgroupLocalId, SubgroupSize}
//! ...uses of Var(x)... ;; no Assign(x, _) in scope
//! → ...uses of cheap... ;; the Let is dropped, every Var(x) inlined
//! ```
//!
//! ## Why only the leaf set?
//!
//! Inlining a `Load`, `Atomic`, `Call`, `Opaque`, `BinOp`, or any
//! compound expression recomputes work at every reference site, which
//! is the opposite of register-pressure reduction. The leaf set above
//! is the strict subset where one register holding a name and one
//! register holding the same value cost the same  -  so dropping the
//! name pays one fewer name without paying any extra arithmetic.
//!
//! ## Safety against rebinding
//!
//! `Node::Assign { name, value: _ }` rebinds an existing `Let` slot.
//! If `name` is ever reassigned in the same scope (or any scope that
//! captures it), inlining its first definition would reorder the
//! observed sequence of values. The scan below rejects any `Let`
//! whose name appears as the target of a `Node::Assign` anywhere in
//! the sibling/descendant sequence the pass is currently rewriting.
//!
//! ## ROADMAP
//!
//! A14  -  live-range and register-pressure model with rematerialization.
//! `register_pressure_estimate` already exists at the
//! `ProgramStats`/`OptimizationCost` layer; this pass is the
//! rewrite-side companion that drops the easiest contributors to that
//! estimate without needing a downstream live-range substrate.

use crate::ir::{Expr, Node, Program};
use crate::optimizer::{vyre_pass, PassAnalysis, PassResult};

/// Drop `Let` bindings whose value is a trivially cheap leaf and whose
/// name is never reassigned, inlining the value at every use site.
#[derive(Debug, Default)]
#[vyre_pass(
    name = "rematerialize_cheap_let",
    requires = [],
    invalidates = ["region_inline", "canonicalize", "const_fold", "cse", "dce"],
    phase = "cleanup",
    boundary_class = "abi_preserving",
    cost_model_family = "scalar"
)]
pub struct RematerializeCheapLetPass;

impl RematerializeCheapLetPass {
    /// Skip programs without any candidate `Let`.
    #[must_use]
    fn analyze_impl(program: &Program) -> PassAnalysis {
        if !program.stats().has_node_let() {
            return PassAnalysis::SKIP;
        }
        let mut found = false;
        scan_for_candidate(program.entry(), &mut found);
        if found {
            PassAnalysis::RUN
        } else {
            PassAnalysis::SKIP
        }
    }

    /// Walk the entry tree and rematerialize cheap single-binding Lets.
    #[must_use]
    pub fn transform(program: Program) -> PassResult {
        let mut changed = false;
        let program = program.map_entry(|entry| rewrite_sequence(entry, &mut changed));
        PassResult { program, changed }
    }
}

/// Rewrite a sibling sequence in order. For each `Let(name, value)`:
/// if `value` is a cheap leaf and `name` is never reassigned in this
/// sequence (including descendant scopes), drop the `Let` and
/// substitute every `Var(name)` in the rest of the sequence and inside
/// every descendant scope with `value.clone()`.
fn rewrite_sequence(nodes: Vec<Node>, changed: &mut bool) -> Vec<Node> {
    let mut nodes: Vec<Node> = nodes
        .into_iter()
        .map(|n| recurse_into_children(n, changed))
        .collect();

    let mut i = 0;
    while i < nodes.len() {
        let take_value = match &nodes[i] {
            Node::Let { name, value } if is_cheap_leaf(value) => {
                let name = name.clone();
                let tail = &nodes[i + 1..];
                if can_rematerialize_let(&name, value, tail) {
                    Some((name, value.clone()))
                } else {
                    None
                }
            }
            _ => None,
        };

        if let Some((name, value)) = take_value {
            nodes.remove(i);
            for node in &mut nodes[i..] {
                substitute_var_in_node(node, &name, &value);
            }
            *changed = true;
            // Do not advance  -  the next index is the node previously at
            // i+1, which may itself be a candidate.
        } else {
            i += 1;
        }
    }

    nodes
}

/// Recurse into `node`'s child sequences so deep rewrites land before
/// their parent gets considered. The top-level sequence rewrite is
/// performed by the caller in `rewrite_sequence`.
fn recurse_into_children(node: Node, changed: &mut bool) -> Node {
    match node {
        Node::If {
            cond,
            then,
            otherwise,
        } => Node::If {
            cond,
            then: rewrite_sequence(then, changed),
            otherwise: rewrite_sequence(otherwise, changed),
        },
        Node::Loop {
            var,
            from,
            to,
            body,
        } => Node::Loop {
            var,
            from,
            to,
            body: rewrite_sequence(body, changed),
        },
        Node::Block(body) => Node::Block(rewrite_sequence(body, changed)),
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
                body: std::sync::Arc::new(rewrite_sequence(body_vec, changed)),
            }
        }
        other => other,
    }
}

/// Replace every `Expr::Var(name)` in `node` with `value.clone()`.
fn substitute_var_in_node(node: &mut Node, name: &str, value: &Expr) {
    match node {
        Node::Let { value: v, .. } | Node::Assign { value: v, .. } => {
            substitute_var_in_expr(v, name, value);
        }
        Node::Store {
            index, value: v, ..
        } => {
            substitute_var_in_expr(index, name, value);
            substitute_var_in_expr(v, name, value);
        }
        Node::If {
            cond,
            then,
            otherwise,
        } => {
            substitute_var_in_expr(cond, name, value);
            for n in then {
                substitute_var_in_node(n, name, value);
            }
            for n in otherwise {
                substitute_var_in_node(n, name, value);
            }
        }
        Node::Loop { from, to, body, .. } => {
            substitute_var_in_expr(from, name, value);
            substitute_var_in_expr(to, name, value);
            for n in body {
                substitute_var_in_node(n, name, value);
            }
        }
        Node::Block(body) => {
            for n in body {
                substitute_var_in_node(n, name, value);
            }
        }
        Node::Region { body, .. } => {
            let body_vec: Vec<Node> = match std::sync::Arc::try_unwrap(std::mem::take(body)) {
                Ok(v) => v,
                Err(arc) => (*arc).clone(),
            };
            let mut owned = body_vec;
            for n in &mut owned {
                substitute_var_in_node(n, name, value);
            }
            *body = std::sync::Arc::new(owned);
        }
        Node::AsyncLoad { offset, size, .. } | Node::AsyncStore { offset, size, .. } => {
            substitute_var_in_expr(offset, name, value);
            substitute_var_in_expr(size, name, value);
        }
        Node::Trap { address, .. } => {
            substitute_var_in_expr(address, name, value);
        }
        Node::Return
        | Node::Barrier { .. }
        | Node::IndirectDispatch { .. }
        | Node::AllReduce { .. }
        | Node::AllGather { .. }
        | Node::ReduceScatter { .. }
        | Node::Broadcast { .. }
        | Node::AsyncWait { .. }
        | Node::Resume { .. }
        | Node::Opaque(_) => {}
    }
}

/// Replace every `Expr::Var(name)` inside `expr` (recursively) with
/// `value.clone()`. Cheap-leaf values do not embed `Var`s themselves
/// (a `Var` value's substitution chains, but the original `Let` was
/// already cheap, so the chain terminates at a leaf).
fn substitute_var_in_expr(expr: &mut Expr, name: &str, value: &Expr) {
    match expr {
        Expr::Var(ident) if ident.as_str() == name => {
            *expr = value.clone();
        }
        Expr::Var(_)
        | Expr::LitU32(_)
        | Expr::LitI32(_)
        | Expr::LitF32(_)
        | Expr::LitBool(_)
        | Expr::BufLen { .. }
        | Expr::InvocationId { .. }
        | Expr::WorkgroupId { .. }
        | Expr::LocalId { .. }
        | Expr::SubgroupLocalId
        | Expr::SubgroupSize
        | Expr::Opaque(_) => {}
        Expr::Load { index, .. } => substitute_var_in_expr(index, name, value),
        Expr::BinOp { left, right, .. } => {
            substitute_var_in_expr(left, name, value);
            substitute_var_in_expr(right, name, value);
        }
        Expr::UnOp { operand, .. } => substitute_var_in_expr(operand, name, value),
        Expr::Select {
            cond,
            true_val,
            false_val,
        } => {
            substitute_var_in_expr(cond, name, value);
            substitute_var_in_expr(true_val, name, value);
            substitute_var_in_expr(false_val, name, value);
        }
        Expr::Cast { value: v, .. } | Expr::SubgroupReduce { value: v, .. } => {
            substitute_var_in_expr(v, name, value);
        }
        Expr::Fma { a, b, c } => {
            substitute_var_in_expr(a, name, value);
            substitute_var_in_expr(b, name, value);
            substitute_var_in_expr(c, name, value);
        }
        Expr::Call { args, .. } => {
            for arg in args {
                substitute_var_in_expr(arg, name, value);
            }
        }
        Expr::Atomic {
            index,
            expected,
            value: v,
            ..
        } => {
            substitute_var_in_expr(index, name, value);
            if let Some(e) = expected.as_deref_mut() {
                substitute_var_in_expr(e, name, value);
            }
            substitute_var_in_expr(v, name, value);
        }
        Expr::SubgroupBallot { cond } => substitute_var_in_expr(cond, name, value),
        Expr::SubgroupShuffle { value: v, lane } => {
            substitute_var_in_expr(v, name, value);
            substitute_var_in_expr(lane, name, value);
        }
    }
}

/// True iff `expr` is a leaf-cheap expression safe to rematerialize.
fn is_cheap_leaf(expr: &Expr) -> bool {
    matches!(
        expr,
        Expr::LitU32(_)
            | Expr::LitI32(_)
            | Expr::LitF32(_)
            | Expr::LitBool(_)
            | Expr::Var(_)
            | Expr::BufLen { .. }
            | Expr::InvocationId { .. }
            | Expr::WorkgroupId { .. }
            | Expr::LocalId { .. }
            | Expr::SubgroupLocalId
            | Expr::SubgroupSize
    )
}

/// True iff a cheap `Let` can be replaced by its value in the remaining
/// sibling tail without changing snapshot semantics.
fn can_rematerialize_let(name: &str, value: &Expr, tail: &[Node]) -> bool {
    if tail.iter().any(|n| node_reassigns(n, name)) {
        return false;
    }
    if let Expr::Var(source) = value {
        if tail.iter().any(|n| node_reassigns(n, source.as_str())) {
            return false;
        }
    }
    true
}

/// True iff `node` (or any descendant) reassigns `name`.
fn node_reassigns(node: &Node, name: &str) -> bool {
    match node {
        Node::Assign { name: n, .. } if n.as_str() == name => true,
        Node::Let { name: n, .. } if n.as_str() == name => true,
        Node::Assign { .. }
        | Node::Let { .. }
        | Node::Store { .. }
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
        Node::If {
            then, otherwise, ..
        } => {
            then.iter().any(|n| node_reassigns(n, name))
                || otherwise.iter().any(|n| node_reassigns(n, name))
        }
        Node::Loop { var, body, .. } => {
            if var.as_str() == name {
                return true;
            }
            body.iter().any(|n| node_reassigns(n, name))
        }
        Node::Block(body) => body.iter().any(|n| node_reassigns(n, name)),
        Node::Region { body, .. } => body.iter().any(|n| node_reassigns(n, name)),
    }
}

/// Recursive analyze helper: true iff any `Let` in the tree has a
/// cheap-leaf value.
fn scan_for_candidate(nodes: &[Node], found: &mut bool) {
    for node in nodes {
        if *found {
            return;
        }
        match node {
            Node::Let { value, .. } if is_cheap_leaf(value) => *found = true,
            Node::If {
                then, otherwise, ..
            } => {
                scan_for_candidate(then, found);
                scan_for_candidate(otherwise, found);
            }
            Node::Loop { body, .. } | Node::Block(body) => scan_for_candidate(body, found),
            Node::Region { body, .. } => scan_for_candidate(body, found),
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{BufferAccess, BufferDecl, DataType, Expr, Node};

    fn buf() -> BufferDecl {
        BufferDecl::storage("buf", 0, BufferAccess::ReadWrite, DataType::U32).with_count(4)
    }

    fn program(entry: Vec<Node>) -> Program {
        Program::wrapped(vec![buf()], [1, 1, 1], entry)
    }

    /// Walk the program's entry tree and find the first Region/Block-
    /// unwrapped sibling sequence that contains a Store. Used by tests
    /// that need to reason about the post-pass shape of the user-visible
    /// program body without caring about the Region wrapper.
    fn find_user_siblings(nodes: &[Node]) -> Option<&[Node]> {
        if nodes.iter().any(|n| {
            matches!(
                n,
                Node::Store { .. } | Node::Let { .. } | Node::If { .. } | Node::Loop { .. }
            )
        }) {
            return Some(nodes);
        }
        for node in nodes {
            let body = match node {
                Node::Block(body) => body.as_slice(),
                Node::Region { body, .. } => body.as_ref().as_slice(),
                _ => continue,
            };
            if let Some(found) = find_user_siblings(body) {
                return Some(found);
            }
        }
        None
    }

    /// Positive: `let z = 0u; store(buf, 0, z)` rematerializes to
    /// `store(buf, 0, 0u)`.
    #[test]
    fn inlines_literal_into_single_use() {
        let entry = vec![
            Node::let_bind("z", Expr::u32(0)),
            Node::store("buf", Expr::u32(0), Expr::var("z")),
        ];
        let result = RematerializeCheapLetPass::transform(program(entry));
        assert!(result.changed, "single-use literal Let must inline");
        let siblings = find_user_siblings(result.program.entry()).expect("Fix: user body present");
        assert_eq!(siblings.len(), 1, "Let dropped, only Store remains");
        match &siblings[0] {
            Node::Store { value, .. } => {
                assert_eq!(*value, Expr::LitU32(0), "literal substituted at use site");
            }
            other => panic!("expected Store, got {other:?}"),
        }
    }

    /// Positive: a literal `Let` referenced N times still drops the
    /// binding and inlines every reference. Cheap leaves cost the same
    /// to recompute as to read from a register, so dropping the name
    /// is monotone-down on register pressure regardless of use count.
    #[test]
    fn inlines_literal_into_many_uses() {
        let entry = vec![
            Node::let_bind("z", Expr::u32(7)),
            Node::store("buf", Expr::u32(0), Expr::var("z")),
            Node::store("buf", Expr::u32(1), Expr::var("z")),
            Node::store("buf", Expr::u32(2), Expr::var("z")),
        ];
        let result = RematerializeCheapLetPass::transform(program(entry));
        assert!(
            result.changed,
            "literal Let must inline regardless of use count"
        );
        let siblings = find_user_siblings(result.program.entry()).expect("Fix: user body present");
        assert_eq!(siblings.len(), 3, "Let dropped, three Stores remain");
        for n in siblings {
            match n {
                Node::Store { value, .. } => assert_eq!(*value, Expr::LitU32(7)),
                other => panic!("expected Store, got {other:?}"),
            }
        }
    }

    /// Positive: `Let(z, InvocationId(0))` inlines as well  -  the
    /// builtin reads the same register at every site.
    #[test]
    fn inlines_invocation_id() {
        let entry = vec![
            Node::let_bind("gid", Expr::InvocationId { axis: 0 }),
            Node::store("buf", Expr::var("gid"), Expr::u32(1)),
        ];
        let result = RematerializeCheapLetPass::transform(program(entry));
        assert!(result.changed, "InvocationId Let must inline");
        let siblings = find_user_siblings(result.program.entry()).expect("Fix: user body present");
        assert_eq!(siblings.len(), 1, "Let dropped");
        match &siblings[0] {
            Node::Store { index, .. } => {
                assert_eq!(*index, Expr::InvocationId { axis: 0 });
            }
            other => panic!("expected Store, got {other:?}"),
        }
    }

    /// Negative: a `Let` whose value is a `Load` must not inline  -
    /// recomputing the load at every use site re-reads memory and
    /// increases work, the opposite of the intent of this pass.
    #[test]
    fn keeps_load_let() {
        let entry = vec![
            Node::let_bind(
                "v",
                Expr::Load {
                    buffer: crate::ir::Ident::from("buf"),
                    index: Box::new(Expr::u32(0)),
                },
            ),
            Node::store("buf", Expr::u32(1), Expr::var("v")),
            Node::store("buf", Expr::u32(2), Expr::var("v")),
        ];
        let result = RematerializeCheapLetPass::transform(program(entry));
        assert!(!result.changed, "Load value must not inline");
    }

    /// Negative: a `Let` whose value is a `BinOp` must not inline  -
    /// the recomputation is not free.
    #[test]
    fn keeps_binop_let() {
        let entry = vec![
            Node::let_bind(
                "v",
                Expr::BinOp {
                    op: crate::ir::BinOp::Add,
                    left: Box::new(Expr::u32(1)),
                    right: Box::new(Expr::u32(2)),
                },
            ),
            Node::store("buf", Expr::u32(0), Expr::var("v")),
        ];
        let result = RematerializeCheapLetPass::transform(program(entry));
        assert!(!result.changed, "BinOp value must not inline");
    }

    /// Negative: a `Let` followed by an `Assign` to the same name must
    /// not inline  -  the original value is no longer in effect after
    /// the reassignment, and inlining would substitute the original
    /// value at sites that should see the reassigned one.
    #[test]
    fn keeps_let_when_name_is_reassigned() {
        let entry = vec![
            Node::let_bind("z", Expr::u32(0)),
            Node::Assign {
                name: crate::ir::Ident::from("z"),
                value: Expr::u32(99),
            },
            Node::store("buf", Expr::u32(0), Expr::var("z")),
        ];
        let result = RematerializeCheapLetPass::transform(program(entry));
        assert!(!result.changed, "reassigned Let must not be rematerialized");
    }

    /// Negative: `let tmp = x; x = y; y = tmp` is a snapshot, not an
    /// alias. Rematerializing `tmp` into `x` would turn the final
    /// assignment into `y = x` after `x` has already changed, breaking
    /// stack-machine `SWAP` and every other carrier snapshot pattern.
    #[test]
    fn keeps_var_let_when_source_is_reassigned_later() {
        let entry = vec![
            Node::let_bind("x", Expr::u32(1)),
            Node::let_bind("y", Expr::u32(2)),
            Node::let_bind("tmp", Expr::var("x")),
            Node::Assign {
                name: crate::ir::Ident::from("x"),
                value: Expr::var("y"),
            },
            Node::Assign {
                name: crate::ir::Ident::from("y"),
                value: Expr::var("tmp"),
            },
            Node::store("buf", Expr::u32(0), Expr::var("y")),
        ];

        let result = RematerializeCheapLetPass::transform(program(entry));
        let siblings = find_user_siblings(result.program.entry()).expect("Fix: user body present");

        assert!(
            siblings.iter().any(|node| matches!(
                node,
                Node::Let { name, value: Expr::Var(source) }
                    if name.as_str() == "tmp" && source.as_str() == "x"
            )),
            "source-reassigned Var Let must remain as a snapshot boundary"
        );
    }

    #[test]
    fn keeps_var_let_when_source_is_reassigned_later_inside_if() {
        let entry = vec![Node::If {
            cond: Expr::var("cond"),
            then: vec![
                Node::let_bind("tmp", Expr::var("x")),
                Node::Assign {
                    name: crate::ir::Ident::from("x"),
                    value: Expr::var("y"),
                },
                Node::Assign {
                    name: crate::ir::Ident::from("y"),
                    value: Expr::var("tmp"),
                },
            ],
            otherwise: Vec::new(),
        }];

        let result = RematerializeCheapLetPass::transform(program(entry));
        let siblings = find_user_siblings(result.program.entry()).expect("Fix: user body present");
        let Node::If { then, .. } = &siblings[0] else {
            panic!("expected If");
        };

        assert!(
            then.iter().any(|node| matches!(
                node,
                Node::Let { name, value: Expr::Var(source) }
                    if name.as_str() == "tmp" && source.as_str() == "x"
            )),
            "nested source-reassigned Var Let must remain as a snapshot boundary"
        );
    }

    /// Negative: a `Let` whose name is used as a loop induction
    /// variable in a descendant scope must not inline  -  the loop
    /// `var` rebinds the name on each iteration, and inlining the
    /// original value would lose loop-correlated semantics.
    #[test]
    fn keeps_let_when_loop_rebinds_name() {
        let entry = vec![
            Node::let_bind("i", Expr::u32(99)),
            Node::Loop {
                var: crate::ir::Ident::from("i"),
                from: Expr::u32(0),
                to: Expr::u32(4),
                body: vec![Node::store("buf", Expr::var("i"), Expr::u32(1))],
            },
        ];
        let result = RematerializeCheapLetPass::transform(program(entry));
        assert!(
            !result.changed,
            "loop-shadowed Let must not be rematerialized"
        );
    }

    /// Positive: a `Let` whose use sits inside a nested If still
    /// inlines  -  substitution recurses through all descendant scopes.
    #[test]
    fn inlines_into_nested_if() {
        let entry = vec![
            Node::let_bind("z", Expr::u32(5)),
            Node::If {
                cond: Expr::var("c"),
                then: vec![Node::store("buf", Expr::u32(0), Expr::var("z"))],
                otherwise: vec![Node::store("buf", Expr::u32(1), Expr::var("z"))],
            },
        ];
        let result = RematerializeCheapLetPass::transform(program(entry));
        assert!(result.changed, "nested-If use must be inlined");
        let siblings = find_user_siblings(result.program.entry()).expect("Fix: user body present");
        match &siblings[0] {
            Node::If {
                then, otherwise, ..
            } => {
                match &then[0] {
                    Node::Store { value, .. } => {
                        assert_eq!(*value, Expr::LitU32(5));
                    }
                    other => panic!("expected Store, got {other:?}"),
                }
                match &otherwise[0] {
                    Node::Store { value, .. } => {
                        assert_eq!(*value, Expr::LitU32(5));
                    }
                    other => panic!("expected Store, got {other:?}"),
                }
            }
            other => panic!("expected If, got {other:?}"),
        }
    }

    /// `analyze` short-circuits when there is no candidate `Let`.
    #[test]
    fn analyze_skips_program_with_no_cheap_let() {
        let entry = vec![Node::store("buf", Expr::u32(0), Expr::u32(1))];
        match crate::optimizer::ProgramPass::analyze(&RematerializeCheapLetPass, &program(entry)) {
            PassAnalysis::SKIP => {}
            other => panic!("expected SKIP, got {other:?}"),
        }
    }
}
