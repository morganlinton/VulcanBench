//! Visitor for IR traversal.
//!
//! Optimization passes, lowering, and analysis use these utilities to walk
//! the IR tree without manually matching every variant. All traversals are
//! implemented with an explicit stack rather than recursion. This is a
//! critical design choice: it prevents stack overflows when processing deep
//! ASTs (e.g., highly nested `If` or `Block` nodes) during adversarial or
//! extreme workloads.

use crate::ir_inner::model::expr::Expr;
use crate::ir_inner::model::expr::Ident;
use crate::ir_inner::model::node::Node;
use crate::ir_inner::model::program::Program;
use smallvec::SmallVec;
use std::collections::HashSet;
use std::sync::Arc;

/// Visitor called for each [`Node`] during [`walk_nodes_and_exprs`].
pub trait NodeVisitor {
    /// Invoked once for every node in the program, in the same order as
    /// [`walk_nodes`].
    fn visit_node(&mut self, node: &Node);
}

/// Visitor called for each [`Expr`] during [`walk_nodes_and_exprs`].
pub trait ExprVisitor {
    /// Invoked once for every expression in the program, in the same order
    /// as [`walk_exprs`].
    fn visit_expr(&mut self, expr: &Expr);
}

/// Walk all nodes in a program, calling `f` on each.
///
/// The traversal is depth-first and visits every statement node in the
/// program's entry block, including nested `If`, `Loop`, and `Block`
/// bodies. Because the walk is iterative, it can handle arbitrarily deep
/// nesting without growing the native call stack.
///
/// # Examples
///
/// ```
/// use vyre::ir::Program;
/// use vyre_foundation::transform::visit::walk_nodes;
///
/// let program = Program::empty();
/// walk_nodes(&program, |_node| {
///     // process node
/// });
/// ```
#[inline]
pub fn walk_nodes(program: &Program, mut f: impl FnMut(&Node)) {
    let mut stack: SmallVec<[&Node; 128]> = SmallVec::new();
    stack.reserve(program.entry().len());
    for node in program.entry().iter().rev() {
        stack.push(node);
    }

    while let Some(node) = stack.pop() {
        f(node);
        match node {
            Node::If {
                then, otherwise, ..
            } => {
                for n in otherwise.iter().rev() {
                    stack.push(n);
                }
                for n in then.iter().rev() {
                    stack.push(n);
                }
            }
            Node::Loop { body, .. } => {
                for n in body.iter().rev() {
                    stack.push(n);
                }
            }
            Node::Block(inner) => {
                for n in inner.iter().rev() {
                    stack.push(n);
                }
            }
            Node::Region { body, .. } => {
                for n in body.iter().rev() {
                    stack.push(n);
                }
            }
            Node::Let { .. }
            | Node::Assign { .. }
            | Node::Store { .. }
            | Node::Return
            | Node::Barrier { .. }
            | Node::IndirectDispatch { .. }
            | Node::AllReduce { .. }
            | Node::AllGather { .. }
            | Node::ReduceScatter { .. }
            | Node::Broadcast { .. }
            | Node::AsyncLoad { .. }
            | Node::AsyncStore { .. }
            | Node::AsyncWait { .. }
            | Node::Trap { .. }
            | Node::Resume { .. }
            | Node::Opaque(_) => {}
        }
    }
}

fn push_node_children_and_exprs<'a>(
    node: &'a Node,
    node_stack: &mut SmallVec<[&'a Node; 128]>,
    expr_stack: &mut SmallVec<[&'a Expr; 128]>,
) {
    match node {
        Node::Let { value, .. } | Node::Assign { value, .. } => {
            expr_stack.push(value);
        }
        Node::Store { index, value, .. } => {
            expr_stack.push(value);
            expr_stack.push(index);
        }
        Node::If {
            cond,
            then,
            otherwise,
        } => {
            for n in otherwise.iter().rev() {
                node_stack.push(n);
            }
            for n in then.iter().rev() {
                node_stack.push(n);
            }
            expr_stack.push(cond);
        }
        Node::Loop { from, to, body, .. } => {
            for n in body.iter().rev() {
                node_stack.push(n);
            }
            expr_stack.push(to);
            expr_stack.push(from);
        }
        Node::Block(nodes) => {
            for n in nodes.iter().rev() {
                node_stack.push(n);
            }
        }
        Node::Region { body, .. } => {
            for n in body.iter().rev() {
                node_stack.push(n);
            }
        }
        Node::Return
        | Node::Barrier { .. }
        | Node::IndirectDispatch { .. }
        | Node::AllReduce { .. }
        | Node::AllGather { .. }
        | Node::ReduceScatter { .. }
        | Node::Broadcast { .. }
        | Node::AsyncLoad { .. }
        | Node::AsyncStore { .. }
        | Node::AsyncWait { .. }
        | Node::Trap { .. }
        | Node::Resume { .. }
        | Node::Opaque(_) => {}
    }
}

fn drain_expr_stack<'a>(
    expr_stack: &mut SmallVec<[&'a Expr; 128]>,
    mut visit: impl FnMut(&'a Expr),
) {
    while let Some(expr) = expr_stack.pop() {
        visit(expr);
        match expr {
            Expr::Load { index, .. } => expr_stack.push(index),
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
            | Expr::SubgroupBallot { .. }
            | Expr::SubgroupShuffle { .. }
            | Expr::SubgroupReduce { .. }
            | Expr::Opaque(_) => {}
            Expr::BinOp { left, right, .. } => {
                expr_stack.push(right);
                expr_stack.push(left);
            }
            Expr::Fma { a, b, c, .. } => {
                expr_stack.push(c);
                expr_stack.push(b);
                expr_stack.push(a);
            }
            Expr::UnOp { operand, .. } => expr_stack.push(operand),
            Expr::Call { args, .. } => {
                for arg in args.iter().rev() {
                    expr_stack.push(arg);
                }
            }
            Expr::Select {
                cond,
                true_val,
                false_val,
            } => {
                expr_stack.push(false_val);
                expr_stack.push(true_val);
                expr_stack.push(cond);
            }
            Expr::Cast { value, .. } => expr_stack.push(value),
            Expr::Atomic {
                index,
                expected,
                value,
                ..
            } => {
                expr_stack.push(value);
                if let Some(expected) = expected {
                    expr_stack.push(expected);
                }
                expr_stack.push(index);
            }
        }
    }
}

/// Walk all expressions in a program, calling `f` on each.
///
/// The traversal visits every `Expr` nested inside every node, again using
/// an explicit stack. This is the primary way to inspect or transform the
/// value-producing parts of a program.
///
/// # Examples
///
/// ```
/// use vyre::ir::Program;
/// use vyre_foundation::transform::visit::walk_exprs;
///
/// let program = Program::empty();
/// walk_exprs(&program, |_expr| {
///     // process expression
/// });
/// ```
#[inline]
pub fn walk_exprs(program: &Program, mut f: impl FnMut(&Expr)) {
    let mut node_stack: SmallVec<[&Node; 128]> = SmallVec::new();
    node_stack.reserve(program.entry().len());
    for node in program.entry().iter().rev() {
        node_stack.push(node);
    }

    let mut expr_stack: SmallVec<[&Expr; 128]> = SmallVec::new();
    expr_stack.reserve(program.entry().len().saturating_mul(2));

    while let Some(node) = node_stack.pop() {
        push_node_children_and_exprs(node, &mut node_stack, &mut expr_stack);
        drain_expr_stack(&mut expr_stack, &mut f);
    }
}

/// Mutably walk all nodes, allowing in-place transformation.
///
/// This is the mutable counterpart to [`walk_nodes`]. Callers can rewrite
/// nodes in place, for example to specialize control flow or inject
/// instrumentation. The explicit-stack invariant is preserved.
///
/// # Examples
///
/// ```
/// use vyre::ir::Program;
/// use vyre_foundation::transform::visit::walk_nodes_mut;
///
/// let mut program = Program::empty();
/// walk_nodes_mut(&mut program, |_node| {
///     // modify node
/// });
/// ```
#[inline]
pub fn walk_nodes_mut(program: &mut Program, mut f: impl FnMut(&mut Node)) {
    let mut stack: SmallVec<[&mut Node; 128]> = SmallVec::new();
    stack.reserve(program.entry().len());
    for node in program.entry_mut().iter_mut().rev() {
        stack.push(node);
    }

    while let Some(node) = stack.pop() {
        f(&mut *node);
        match node {
            Node::If {
                then, otherwise, ..
            } => {
                for n in otherwise.iter_mut().rev() {
                    stack.push(n);
                }
                for n in then.iter_mut().rev() {
                    stack.push(n);
                }
            }
            Node::Loop { body, .. } => {
                for n in body.iter_mut().rev() {
                    stack.push(n);
                }
            }
            Node::Block(inner) => {
                for n in inner.iter_mut().rev() {
                    stack.push(n);
                }
            }
            Node::Region { body, .. } => {
                for n in std::sync::Arc::make_mut(body).iter_mut().rev() {
                    stack.push(n);
                }
            }
            Node::Let { .. }
            | Node::Assign { .. }
            | Node::Store { .. }
            | Node::Return
            | Node::Barrier { .. }
            | Node::IndirectDispatch { .. }
            | Node::AllReduce { .. }
            | Node::AllGather { .. }
            | Node::ReduceScatter { .. }
            | Node::Broadcast { .. }
            | Node::AsyncLoad { .. }
            | Node::AsyncStore { .. }
            | Node::AsyncWait { .. }
            | Node::Trap { .. }
            | Node::Resume { .. }
            | Node::Opaque(_) => {}
        }
    }
}

/// Walk all nodes and expressions in a program in a single traversal.
///
/// For each node, `visitor.visit_node(node)` is called, then all
/// expressions owned by that node (and their sub-expressions) are
/// visited via `visitor.visit_expr(expr)`.  Child nodes are pushed
/// onto the same explicit stack so the walk is iterative and safe
/// for arbitrarily deep ASTs.
///
/// The relative order of node visits matches [`walk_nodes`] and the
/// relative order of expression visits matches [`walk_exprs`].
///
/// # Examples
///
/// ```
/// use vyre::ir::Program;
/// use vyre_foundation::transform::visit::{walk_nodes_and_exprs, NodeVisitor, ExprVisitor};
///
/// struct CountAll;
///
/// impl NodeVisitor for CountAll {
///     fn visit_node(&mut self, _node: &vyre::ir::Node) {}
/// }
///
/// impl ExprVisitor for CountAll {
///     fn visit_expr(&mut self, _expr: &vyre::ir::Expr) {}
/// }
///
/// let program = Program::empty();
/// walk_nodes_and_exprs(&program, &mut CountAll);
/// ```
#[inline]
pub fn walk_nodes_and_exprs<V: NodeVisitor + ExprVisitor>(program: &Program, visitor: &mut V) {
    let mut node_stack: SmallVec<[&Node; 128]> = SmallVec::new();
    node_stack.reserve(program.entry().len());
    for node in program.entry().iter().rev() {
        node_stack.push(node);
    }

    let mut expr_stack: SmallVec<[&Expr; 128]> = SmallVec::new();
    expr_stack.reserve(program.entry().len().saturating_mul(2));

    while let Some(node) = node_stack.pop() {
        visitor.visit_node(node);
        push_node_children_and_exprs(node, &mut node_stack, &mut expr_stack);
        drain_expr_stack(&mut expr_stack, |expr| visitor.visit_expr(expr));
    }
}

/// This is a convenience wrapper around the visitor that extracts the set
/// of buffer identifiers actually used by the program. It is used by
/// validation and lowering to check that every declared buffer is
/// referenced and that no undeclared buffer is accessed.
///
/// The implementation uses a single combined traversal ([`walk_nodes_and_exprs`])
/// instead of the previous two-pass approach.
///
/// # Examples
///
/// ```
/// use vyre::ir::Program;
/// use vyre_foundation::transform::visit::referenced_buffers;
///
/// let program = Program::empty();
/// let buffers = referenced_buffers(&program);
/// assert!(buffers.is_empty());
/// ```
#[must_use]
#[inline]
pub fn referenced_buffers(program: &Program) -> HashSet<Ident> {
    // ProgramFacts::buffer_refs already enumerates every buffer-touching
    // node and expression in the program (Store/IndirectDispatch/AsyncLoad/
    // AsyncStore plus Load/BufLen/Atomic via the same SoA walk). Reuse the
    // OnceLock-cached facts instead of re-walking the entire tree with a
    // dedicated NodeVisitor + ExprVisitor pair.
    let facts = crate::optimizer::program_soa::ProgramFacts::build_cached(program);
    let mut names = HashSet::with_capacity(program.buffers().len());
    for (_, name, _) in facts.buffer_refs() {
        names.insert(name.clone());
    }
    names
}

/// Collect operation IDs from every [`Expr::Call`] in traversal order.
///
/// This helper is used by the inliner and the conform gate to discover
/// which operations a program depends on. The returned vector preserves
/// the order of first appearance.
///
/// # Examples
///
/// ```
/// use vyre::ir::{Expr, Node, Program};
/// use vyre_foundation::transform::visit::collect_call_op_ids;
///
/// let program = Program::wrapped(
///     Vec::new(),
///     [1, 1, 1],
///     vec![Node::let_bind("x", Expr::call("primitive.math.add", vec![Expr::u32(1)]))],
/// );
/// assert_eq!(
///     collect_call_op_ids(&program)
///         .into_iter()
///         .map(|id| id.to_string())
///         .collect::<Vec<_>>(),
///     vec!["primitive.math.add".to_string()]
/// );
/// ```
#[must_use]
#[inline]
pub fn collect_call_op_ids(program: &Program) -> Vec<Arc<str>> {
    // Cached call_count is the exact number of Expr::Call sites in
    // the program. When it is zero, skip the entire expression walk.
    // When non-zero, pre-size the output to the exact count so we
    // never resize during the walk.
    let stats = program.stats();
    let call_count = stats.call_count as usize;
    if call_count == 0 {
        return Vec::new();
    }
    let mut op_ids = Vec::with_capacity(call_count);
    walk_exprs(program, |expr| {
        if let Expr::Call { op_id, .. } = expr {
            op_ids.push(op_id.shared_text());
        }
    });
    op_ids
}

#[cfg(test)]

mod tests {
    use super::*;
    use crate::ir::{AtomicOp, BinOp, BufferDecl, DataType, Expr, Node, Program, UnOp};
    use proptest::prelude::*;

    /// Legacy double-walk implementation for equivalence verification.
    /// Mirrors every buffer-touching site that ProgramFacts::buffer_refs
    /// records so the equivalence proptest stays sound even when arb_node
    /// is extended with Async / IndirectDispatch variants.
    fn referenced_buffers_legacy(program: &Program) -> HashSet<Ident> {
        let mut names = HashSet::new();
        walk_exprs(program, |expr| match expr {
            Expr::Load { buffer, .. } | Expr::BufLen { buffer } | Expr::Atomic { buffer, .. } => {
                names.insert(buffer.clone());
            }
            _ => {}
        });
        walk_nodes(program, |node| match node {
            Node::Store { buffer, .. } => {
                names.insert(buffer.clone());
            }
            Node::IndirectDispatch { count_buffer, .. } => {
                names.insert(count_buffer.clone());
            }
            Node::AsyncLoad {
                source,
                destination,
                ..
            }
            | Node::AsyncStore {
                source,
                destination,
                ..
            } => {
                names.insert(source.clone());
                names.insert(destination.clone());
            }
            _ => {}
        });
        names
    }

    fn arb_ident() -> BoxedStrategy<String> {
        prop::sample::select(&["x", "y", "idx", "i", "acc"][..])
            .prop_map(str::to_string)
            .boxed()
    }

    fn arb_buffer_name() -> BoxedStrategy<String> {
        prop::sample::select(&["out", "input", "rw", "counts", "scratch"][..])
            .prop_map(str::to_string)
            .boxed()
    }

    fn arb_expr() -> BoxedStrategy<Expr> {
        let leaf = prop_oneof![
            any::<u32>().prop_map(Expr::LitU32),
            any::<i32>().prop_map(Expr::LitI32),
            any::<bool>().prop_map(Expr::LitBool),
            arb_ident().prop_map(Expr::var),
            arb_buffer_name().prop_map(Expr::buf_len),
        ];

        leaf.prop_recursive(3, 48, 3, |inner| {
            prop_oneof![
                (arb_buffer_name(), inner.clone()).prop_map(|(buffer, index)| Expr::Load {
                    buffer: buffer.into(),
                    index: Box::new(index),
                }),
                (inner.clone(), inner.clone()).prop_map(|(left, right)| Expr::BinOp {
                    op: BinOp::Add,
                    left: Box::new(left),
                    right: Box::new(right),
                }),
                (inner.clone(), inner.clone()).prop_map(|(left, right)| Expr::BinOp {
                    op: BinOp::Sub,
                    left: Box::new(left),
                    right: Box::new(right),
                }),
                inner.clone().prop_map(|operand| Expr::UnOp {
                    op: UnOp::Negate,
                    operand: Box::new(operand),
                }),
                (inner.clone(), inner.clone(), inner.clone()).prop_map(
                    |(cond, true_val, false_val)| Expr::Select {
                        cond: Box::new(cond),
                        true_val: Box::new(true_val),
                        false_val: Box::new(false_val),
                    }
                ),
                inner.clone().prop_map(|value| Expr::Cast {
                    target: DataType::U32,
                    value: Box::new(value),
                }),
                (
                    arb_buffer_name(),
                    inner.clone(),
                    proptest::option::of(inner.clone()),
                    inner.clone(),
                )
                    .prop_map(|(buffer, index, expected, value)| Expr::Atomic {
                        op: AtomicOp::Add,
                        buffer: buffer.into(),
                        index: Box::new(index),
                        expected: expected.map(Box::new),
                        value: Box::new(value),
                        ordering: crate::MemoryOrdering::SeqCst,
                    }),
            ]
        })
        .boxed()
    }

    fn arb_node() -> BoxedStrategy<Node> {
        arb_node_with_depth(3)
    }

    fn arb_node_with_depth(depth: u32) -> BoxedStrategy<Node> {
        let leaf = prop_oneof![
            (arb_ident(), arb_expr()).prop_map(|(name, value)| Node::Let {
                name: name.into(),
                value,
            }),
            (arb_ident(), arb_expr()).prop_map(|(name, value)| Node::Assign {
                name: name.into(),
                value,
            }),
            (arb_buffer_name(), arb_expr(), arb_expr()).prop_map(|(buffer, index, value)| {
                Node::Store {
                    buffer: buffer.into(),
                    index,
                    value,
                }
            }),
            Just(Node::Return),
            Just(Node::barrier()),
        ];

        if depth == 0 {
            return leaf.boxed();
        }

        leaf.prop_recursive(2, 32, 2, move |inner| {
            prop_oneof![
                (
                    arb_expr(),
                    prop::collection::vec(inner.clone(), 0..=3),
                    prop::collection::vec(inner.clone(), 0..=3),
                )
                    .prop_map(|(cond, then, otherwise)| Node::If {
                        cond,
                        then,
                        otherwise,
                    }),
                (
                    arb_ident(),
                    arb_expr(),
                    arb_expr(),
                    prop::collection::vec(inner.clone(), 0..=3),
                )
                    .prop_map(|(var, from, to, body)| Node::Loop {
                        var: var.into(),
                        from,
                        to,
                        body,
                    }),
                prop::collection::vec(inner, 0..=3).prop_map(Node::Block),
            ]
        })
        .boxed()
    }

    fn arb_program() -> BoxedStrategy<Program> {
        prop::collection::vec(arb_node(), 0..=8)
            .prop_map(|entry| {
                Program::wrapped(
                    vec![
                        BufferDecl::output("out", 0, DataType::U32)
                            .with_count(8)
                            .with_output_byte_range(0..16),
                        BufferDecl::read("input", 1, DataType::U32).with_count(8),
                        BufferDecl::read_write("rw", 2, DataType::U32).with_count(8),
                        BufferDecl::read("counts", 3, DataType::U32).with_count(8),
                        BufferDecl::workgroup("scratch", 4, DataType::U32),
                    ],
                    [1, 1, 1],
                    entry,
                )
            })
            .boxed()
    }

    proptest! {
        #![proptest_config(ProptestConfig {
            cases: 256,
            ..ProptestConfig::default()
        })]

        #[test]
        fn combined_walker_referenced_buffers_eq_legacy(program in arb_program()) {
            let combined = referenced_buffers(&program);
            let legacy = referenced_buffers_legacy(&program);
            prop_assert_eq!(combined, legacy);
        }
    }

    #[test]
    fn referenced_buffers_collects_from_store_and_load() {
        let program = Program::wrapped(
            vec![
                BufferDecl::read("input", 0, DataType::U32).with_count(8),
                BufferDecl::output("out", 1, DataType::U32).with_count(8),
            ],
            [1, 1, 1],
            vec![
                Node::let_bind("x", Expr::load("input", Expr::u32(0))),
                Node::store("out", Expr::u32(0), Expr::var("x")),
                Node::Return,
            ],
        );

        let buffers = referenced_buffers(&program);
        assert!(buffers.contains(&Ident::from("input")));
        assert!(buffers.contains(&Ident::from("out")));
        assert_eq!(buffers.len(), 2);
    }

    #[test]
    fn referenced_buffers_collects_from_atomic_and_indirect_dispatch() {
        let program = Program::wrapped(
            vec![
                BufferDecl::read_write("rw", 0, DataType::U32).with_count(8),
                BufferDecl::read("counts", 1, DataType::U32).with_count(8),
            ],
            [1, 1, 1],
            vec![
                Node::let_bind(
                    "x",
                    Expr::Atomic {
                        op: AtomicOp::Add,
                        buffer: "rw".into(),
                        index: Box::new(Expr::u32(0)),
                        expected: None,
                        value: Box::new(Expr::u32(1)),
                        ordering: crate::MemoryOrdering::SeqCst,
                    },
                ),
                Node::IndirectDispatch {
                    count_buffer: "counts".into(),
                    count_offset: 0,
                },
                Node::Return,
            ],
        );

        let buffers = referenced_buffers(&program);
        assert!(buffers.contains(&Ident::from("rw")));
        assert!(buffers.contains(&Ident::from("counts")));
        assert_eq!(buffers.len(), 2);
    }
}
