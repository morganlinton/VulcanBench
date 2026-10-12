#![allow(clippy::expect_used)]
use crate::ir::{BinOp, Expr, Node, Program};
use smallvec::SmallVec;
use std::borrow::Cow;
use std::sync::Arc;

/// Run an expression-rewrite closure over every node in \`program\`.
pub(crate) fn rewrite_program(
    program: Program,
    mut expr: impl FnMut(&Expr) -> Option<Expr>,
) -> (Program, bool) {
    match rewrite_nodes_cow(program.entry(), &mut expr) {
        Cow::Borrowed(_) => (program, false),
        Cow::Owned(entry) => (program.with_rewritten_entry(entry), true),
    }
}

fn rewrite_nodes_cow<'a>(
    nodes: &'a [Node],
    expr: &mut impl FnMut(&Expr) -> Option<Expr>,
) -> Cow<'a, [Node]> {
    let mut rewritten: Option<Vec<Node>> = None;
    for (index, node) in nodes.iter().enumerate() {
        match rewrite_node_cow(node, expr) {
            Cow::Borrowed(_) if rewritten.is_none() => {}
            Cow::Borrowed(borrowed) => {
                if let Some(out) = rewritten.as_mut() {
                    out.push(borrowed.clone());
                }
            }
            Cow::Owned(owned) => {
                let out = rewritten.get_or_insert_with(|| {
                    // Final length is exactly nodes.len(); pre-size so the
                    // push loop never reallocates as remaining nodes
                    // (whether borrowed or owned) are appended.
                    let mut v = Vec::with_capacity(nodes.len());
                    v.extend_from_slice(&nodes[..index]);
                    v
                });
                out.push(owned);
            }
        }
    }
    rewritten.map_or(Cow::Borrowed(nodes), Cow::Owned)
}

fn rewrite_node_cow<'a>(
    node: &'a Node,
    expr: &mut impl FnMut(&Expr) -> Option<Expr>,
) -> Cow<'a, Node> {
    match node {
        Node::Let { name, value } => match rewrite_expr(value, expr) {
            Cow::Borrowed(_) => Cow::Borrowed(node),
            Cow::Owned(value) => Cow::Owned(Node::let_bind(name, value)),
        },
        Node::Assign { name, value } => match rewrite_expr(value, expr) {
            Cow::Borrowed(_) => Cow::Borrowed(node),
            Cow::Owned(value) => Cow::Owned(Node::assign(name, value)),
        },
        Node::Store {
            buffer,
            index,
            value,
        } => {
            let idx = rewrite_expr(index, expr);
            let val = rewrite_expr(value, expr);
            if matches!((&idx, &val), (Cow::Borrowed(_), Cow::Borrowed(_))) {
                Cow::Borrowed(node)
            } else {
                Cow::Owned(Node::store(buffer, idx.into_owned(), val.into_owned()))
            }
        }
        Node::If {
            cond,
            then,
            otherwise,
        } => {
            let c = rewrite_expr(cond, expr);
            let t = rewrite_nodes_cow(then, expr);
            let o = rewrite_nodes_cow(otherwise, expr);
            if matches!(
                (&c, &t, &o),
                (Cow::Borrowed(_), Cow::Borrowed(_), Cow::Borrowed(_))
            ) {
                Cow::Borrowed(node)
            } else {
                Cow::Owned(Node::if_then_else(
                    c.into_owned(),
                    t.into_owned(),
                    o.into_owned(),
                ))
            }
        }
        Node::Loop {
            var,
            from,
            to,
            body,
        } => {
            let f = rewrite_expr(from, expr);
            let t = rewrite_expr(to, expr);
            let b = rewrite_nodes_cow(body, expr);
            if matches!(
                (&f, &t, &b),
                (Cow::Borrowed(_), Cow::Borrowed(_), Cow::Borrowed(_))
            ) {
                Cow::Borrowed(node)
            } else {
                Cow::Owned(Node::loop_for(
                    var,
                    f.into_owned(),
                    t.into_owned(),
                    b.into_owned(),
                ))
            }
        }
        Node::Block(body) => match rewrite_nodes_cow(body, expr) {
            Cow::Borrowed(_) => Cow::Borrowed(node),
            Cow::Owned(body) => Cow::Owned(Node::block(body)),
        },
        Node::Trap { address, tag } => match rewrite_expr(address, expr) {
            Cow::Borrowed(_) => Cow::Borrowed(node),
            Cow::Owned(address) => Cow::Owned(Node::Trap {
                address: Box::new(address),
                tag: tag.clone(),
            }),
        },
        Node::Region {
            generator,
            source_region,
            body,
        } => match rewrite_nodes_cow(body, expr) {
            Cow::Borrowed(_) => Cow::Borrowed(node),
            Cow::Owned(body) => Cow::Owned(Node::Region {
                generator: generator.clone(),
                source_region: source_region.clone(),
                body: Arc::new(body),
            }),
        },
        // Async copies carry `offset`/`size` EXPRESSIONS (Box<Expr>); the
        // canonical "rewrite every node" driver must descend into them just
        // like Trap's address above. Omitting them silently skipped two
        // expression positions for every pass routed through rewrite_program
        // (const_fold/strength_reduce left them unoptimized; a correctness
        // rewrite would have left a dangling reference there).
        Node::AsyncLoad {
            source,
            destination,
            offset,
            size,
            tag,
        } => {
            let off = rewrite_expr(offset, expr);
            let sz = rewrite_expr(size, expr);
            if matches!((&off, &sz), (Cow::Borrowed(_), Cow::Borrowed(_))) {
                Cow::Borrowed(node)
            } else {
                Cow::Owned(Node::async_load_ext(
                    source.clone(),
                    destination.clone(),
                    off.into_owned(),
                    sz.into_owned(),
                    tag.clone(),
                ))
            }
        }
        Node::AsyncStore {
            source,
            destination,
            offset,
            size,
            tag,
        } => {
            let off = rewrite_expr(offset, expr);
            let sz = rewrite_expr(size, expr);
            if matches!((&off, &sz), (Cow::Borrowed(_), Cow::Borrowed(_))) {
                Cow::Borrowed(node)
            } else {
                Cow::Owned(Node::async_store(
                    source.clone(),
                    destination.clone(),
                    off.into_owned(),
                    sz.into_owned(),
                    tag.clone(),
                ))
            }
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
        | Node::Opaque(_) => Cow::Borrowed(node),
    }
}

#[expect(
    clippy::too_many_lines,
    reason = "iterative expression rewrite keeps traversal and reassembly order in one stack machine"
)]
pub(crate) fn rewrite_expr<'a>(
    expr: &'a Expr,
    transform: &mut impl FnMut(&Expr) -> Option<Expr>,
) -> Cow<'a, Expr> {
    enum Frame<'a> {
        Expr(&'a Expr),
        Assemble(&'a Expr),
    }

    let mut stack: SmallVec<[Frame<'_>; 32]> = SmallVec::new();
    stack.push(Frame::Expr(expr));
    let mut results: SmallVec<[Cow<'a, Expr>; 32]> = SmallVec::new();

    while let Some(frame) = stack.pop() {
        match frame {
            Frame::Expr(e) => {
                stack.push(Frame::Assemble(e));
                match e {
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
                    | Expr::Opaque(_) => {}
                    Expr::Load { index, .. } => stack.push(Frame::Expr(index)),
                    Expr::BinOp { left, right, .. } => {
                        stack.push(Frame::Expr(right));
                        stack.push(Frame::Expr(left));
                    }
                    Expr::UnOp { operand, .. } => stack.push(Frame::Expr(operand)),
                    Expr::Call { args, .. } => {
                        for arg in args.iter().rev() {
                            stack.push(Frame::Expr(arg));
                        }
                    }
                    Expr::Select {
                        cond,
                        true_val,
                        false_val,
                    } => {
                        stack.push(Frame::Expr(false_val));
                        stack.push(Frame::Expr(true_val));
                        stack.push(Frame::Expr(cond));
                    }
                    Expr::Cast { value, .. } | Expr::SubgroupReduce { value, .. } => {
                        stack.push(Frame::Expr(value));
                    }
                    Expr::Fma { a, b, c } => {
                        stack.push(Frame::Expr(c));
                        stack.push(Frame::Expr(b));
                        stack.push(Frame::Expr(a));
                    }
                    Expr::Atomic {
                        index,
                        expected,
                        value,
                        ..
                    } => {
                        stack.push(Frame::Expr(value));
                        if let Some(expected) = expected {
                            stack.push(Frame::Expr(expected));
                        }
                        stack.push(Frame::Expr(index));
                    }
                    Expr::SubgroupBallot { cond } => stack.push(Frame::Expr(cond)),
                    Expr::SubgroupShuffle { value, lane } => {
                        stack.push(Frame::Expr(lane));
                        stack.push(Frame::Expr(value));
                    }
                }
            }
            Frame::Assemble(e) => {
                let rewritten = match e {
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
                    | Expr::Opaque(_) => Cow::Borrowed(e),
                    Expr::Load { buffer, .. } => {
                        let index = pop_rewrite_result(&mut results, "load index");
                        match index {
                            Cow::Borrowed(_) => Cow::Borrowed(e),
                            Cow::Owned(index) => Cow::Owned(Expr::Load {
                                buffer: buffer.clone(),
                                index: Box::new(index),
                            }),
                        }
                    }
                    Expr::BinOp { op, .. } => {
                        let right = pop_rewrite_result(&mut results, "binary rhs");
                        let left = pop_rewrite_result(&mut results, "binary lhs");
                        rewrite_binary(e, *op, left, right)
                    }
                    Expr::UnOp { op, .. } => {
                        let operand = pop_rewrite_result(&mut results, "unary operand");
                        match operand {
                            Cow::Borrowed(_) => Cow::Borrowed(e),
                            Cow::Owned(operand) => Cow::Owned(Expr::UnOp {
                                op: op.clone(),
                                operand: Box::new(operand),
                            }),
                        }
                    }
                    Expr::Call { op_id, args } => {
                        let start_idx = results.len().checked_sub(args.len()).unwrap_or_else(|| {
                            unreachable!(
                                "Fix: iterative expression rewrite lost call arguments; child/result stack is internally inconsistent."
                            )
                        });
                        let arg_results: Vec<_> = results.drain(start_idx..).collect();
                        let changed = arg_results
                            .iter()
                            .any(|arg_res| matches!(arg_res, Cow::Owned(_)));
                        if changed {
                            Cow::Owned(Expr::Call {
                                op_id: op_id.clone(),
                                args: arg_results.into_iter().map(Cow::into_owned).collect(),
                            })
                        } else {
                            Cow::Borrowed(e)
                        }
                    }
                    Expr::Select { .. } => {
                        let false_val = pop_rewrite_result(&mut results, "select false value");
                        let true_val = pop_rewrite_result(&mut results, "select true value");
                        let cond = pop_rewrite_result(&mut results, "select condition");
                        rewrite_select(e, cond, true_val, false_val)
                    }
                    Expr::Cast { target, .. } => {
                        let value = pop_rewrite_result(&mut results, "cast value");
                        match value {
                            Cow::Borrowed(_) => Cow::Borrowed(e),
                            Cow::Owned(value) => Cow::Owned(Expr::Cast {
                                target: target.clone(),
                                value: Box::new(value),
                            }),
                        }
                    }
                    Expr::Fma { .. } => {
                        let c = pop_rewrite_result(&mut results, "fma c");
                        let b = pop_rewrite_result(&mut results, "fma b");
                        let a = pop_rewrite_result(&mut results, "fma a");
                        rewrite_fma(e, a, b, c)
                    }
                    Expr::Atomic {
                        op,
                        buffer,
                        ordering,
                        expected,
                        ..
                    } => {
                        let value = pop_rewrite_result(&mut results, "atomic value");
                        let new_expected = if expected.is_some() {
                            Some(pop_rewrite_result(&mut results, "atomic expected"))
                        } else {
                            None
                        };
                        let index = pop_rewrite_result(&mut results, "atomic index");
                        if matches!(index, Cow::Borrowed(_))
                            && new_expected
                                .as_ref()
                                .is_none_or(|ex| matches!(ex, Cow::Borrowed(_)))
                            && matches!(value, Cow::Borrowed(_))
                        {
                            Cow::Borrowed(e)
                        } else {
                            Cow::Owned(Expr::Atomic {
                                op: *op,
                                buffer: buffer.clone(),
                                index: Box::new(index.into_owned()),
                                expected: new_expected.map(|ex| Box::new(ex.into_owned())),
                                value: Box::new(value.into_owned()),
                                ordering: *ordering,
                            })
                        }
                    }
                    Expr::SubgroupBallot { .. } => {
                        let cond = pop_rewrite_result(&mut results, "subgroup ballot condition");
                        match cond {
                            Cow::Borrowed(_) => Cow::Borrowed(e),
                            Cow::Owned(cond) => Cow::Owned(Expr::SubgroupBallot {
                                cond: Box::new(cond),
                            }),
                        }
                    }
                    Expr::SubgroupShuffle { .. } => {
                        let lane = pop_rewrite_result(&mut results, "subgroup shuffle lane");
                        let value = pop_rewrite_result(&mut results, "subgroup shuffle value");
                        match (value, lane) {
                            (Cow::Borrowed(_), Cow::Borrowed(_)) => Cow::Borrowed(e),
                            (v, l) => Cow::Owned(Expr::SubgroupShuffle {
                                value: Box::new(v.into_owned()),
                                lane: Box::new(l.into_owned()),
                            }),
                        }
                    }
                    Expr::SubgroupReduce { op, .. } => {
                        let value = pop_rewrite_result(&mut results, "subgroup reduce value");
                        match value {
                            Cow::Borrowed(_) => Cow::Borrowed(e),
                            Cow::Owned(value) => Cow::Owned(Expr::SubgroupReduce {
                                op: *op,
                                value: Box::new(value),
                            }),
                        }
                    }
                };

                let transformed = if let Some(t) = transform(rewritten.as_ref()) {
                    Cow::Owned(t)
                } else {
                    rewritten
                };
                results.push(transformed);
            }
        }
    }
    match results.pop() {
        Some(result) => result,
        None => unreachable!(
            "Fix: iterative expression rewrite produced no result; child/result stack is internally inconsistent."
        ),
    }
}

#[inline]
fn pop_rewrite_result<'a>(
    results: &mut SmallVec<[Cow<'a, Expr>; 32]>,
    context: &'static str,
) -> Cow<'a, Expr> {
    match results.pop() {
        Some(result) => result,
        None => unreachable!(
            "Fix: iterative expression rewrite lost {context}; child/result stack is internally inconsistent."
        ),
    }
}

#[inline]
fn rewrite_binary<'a>(
    original: &'a Expr,
    op: BinOp,
    left: Cow<'a, Expr>,
    right: Cow<'a, Expr>,
) -> Cow<'a, Expr> {
    if matches!((&left, &right), (Cow::Borrowed(_), Cow::Borrowed(_))) {
        return Cow::Borrowed(original);
    }
    Cow::Owned(Expr::BinOp {
        op,
        left: Box::new(left.into_owned()),
        right: Box::new(right.into_owned()),
    })
}

#[inline]
fn rewrite_fma<'a>(
    original: &'a Expr,
    a: Cow<'a, Expr>,
    b: Cow<'a, Expr>,
    c: Cow<'a, Expr>,
) -> Cow<'a, Expr> {
    if matches!(
        (&a, &b, &c),
        (Cow::Borrowed(_), Cow::Borrowed(_), Cow::Borrowed(_))
    ) {
        return Cow::Borrowed(original);
    }
    Cow::Owned(Expr::Fma {
        a: Box::new(a.into_owned()),
        b: Box::new(b.into_owned()),
        c: Box::new(c.into_owned()),
    })
}

#[inline]
fn rewrite_select<'a>(
    original: &'a Expr,
    cond: Cow<'a, Expr>,
    true_val: Cow<'a, Expr>,
    false_val: Cow<'a, Expr>,
) -> Cow<'a, Expr> {
    if matches!(
        (&cond, &true_val, &false_val),
        (Cow::Borrowed(_), Cow::Borrowed(_), Cow::Borrowed(_))
    ) {
        return Cow::Borrowed(original);
    }
    Cow::Owned(Expr::Select {
        cond: Box::new(cond.into_owned()),
        true_val: Box::new(true_val.into_owned()),
        false_val: Box::new(false_val.into_owned()),
    })
}

#[cfg(test)]

mod tests {
    use super::*;
    use crate::ir::{BufferDecl, DataType};

    fn simple_program() -> Program {
        Program::wrapped(
            vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
            [1, 1, 1],
            vec![Node::store("out", Expr::u32(0), Expr::u32(42))],
        )
    }

    #[test]
    fn identity_rewrite_unchanged() {
        let p = simple_program();
        let (result, changed) = rewrite_program(p, |_| None);
        assert!(!changed);
        assert_eq!(result.entry().len(), 1);
    }

    #[test]
    fn rewrite_replaces_constant() {
        let p = simple_program();
        let (result, changed) = rewrite_program(p, |expr| match expr {
            Expr::LitU32(42) => Some(Expr::u32(99)),
            _ => None,
        });
        assert!(changed);
        let entry = result.entry();
        fn find_99(nodes: &[Node]) -> bool {
            nodes.iter().any(|n| match n {
                Node::Store { value, .. } => matches!(value, Expr::LitU32(99)),
                Node::Region { body, .. } => find_99(body),
                _ => false,
            })
        }
        assert!(find_99(entry));
    }

    #[test]
    fn rewrite_into_if_branch() {
        let p = Program::wrapped(
            vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
            [1, 1, 1],
            vec![Node::if_then(
                Expr::bool(true),
                vec![Node::store("out", Expr::u32(0), Expr::u32(7))],
            )],
        );
        let (_result, changed) = rewrite_program(p, |expr| match expr {
            Expr::LitU32(7) => Some(Expr::u32(8)),
            _ => None,
        });
        assert!(changed);
    }

    #[test]
    fn rewrite_rebuild_preserves_child_order() {
        let expr = Expr::sub(Expr::u32(10), Expr::u32(3));
        let rewritten = rewrite_expr(&expr, &mut |expr| match expr {
            Expr::LitU32(3) => Some(Expr::u32(4)),
            _ => None,
        });
        assert_eq!(
            rewritten.into_owned(),
            Expr::sub(Expr::u32(10), Expr::u32(4))
        );

        let expr = Expr::Select {
            cond: Box::new(Expr::bool(false)),
            true_val: Box::new(Expr::u32(1)),
            false_val: Box::new(Expr::u32(2)),
        };
        let rewritten = rewrite_expr(&expr, &mut |expr| match expr {
            Expr::LitU32(2) => Some(Expr::u32(3)),
            _ => None,
        });
        assert_eq!(
            rewritten.into_owned(),
            Expr::Select {
                cond: Box::new(Expr::bool(false)),
                true_val: Box::new(Expr::u32(1)),
                false_val: Box::new(Expr::u32(3)),
            }
        );
    }
}
