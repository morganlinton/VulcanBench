//! ROADMAP A36  -  minimize identity-op atomics under Relaxed ordering
//! to a plain `Expr::Load`, and eliminate unique-writer atomics.
//!
//! Op id: `vyre-foundation::optimizer::passes::atomic_minimize`.

use crate::ir::{AtomicOp, Expr, Node, Program, SubgroupReduceOp};
use crate::optimizer::{vyre_pass, PassAnalysis, PassResult};
use crate::runtime::memory_model::MemoryOrdering;
// Ident hashes well into the FxHash64-mixed bucket; the std SipHash-13
// default (which fights HashDoS) is overkill for an internal pass-private
// table that never sees adversarial input. FxHashMap/FxHashSet measurably
// shorten the analysis pass on programs with many distinct buffers.
use rustc_hash::{FxHashMap as HashMap, FxHashSet as HashSet};
use smallvec::SmallVec;

#[derive(Default, Debug, Clone, Copy)]
struct BufferAccesses {
    atomic_adds: u32,
    other_accesses: u32,
    /// Ordering of the first (and expected to be the sole) AtomicAdd access.
    /// Only meaningful when `atomic_adds == 1 && other_accesses == 0`.
    /// Defaults to `Relaxed` but is overwritten on first atomic-add encounter.
    sole_atomic_ordering: MemoryOrdering,
}

/// Replace identity-op Relaxed atomics with plain `Expr::Load`, and rewrite single-writer atomics.
#[derive(Debug, Default)]
#[vyre_pass(
    name = "atomic_minimize",
    requires = [],
    invalidates = [],
    phase = "sync",
    boundary_class = "abi_preserving",
    cost_model_family = "sync"
)]
pub struct AtomicMinimizePass;

impl AtomicMinimizePass {
    /// Skip programs that do not contain a candidate atomic.
    #[must_use]
    fn analyze_impl(program: &Program) -> PassAnalysis {
        // Both rewrites need at least one Atomic op anywhere; without
        // any atomic the two follow-up tree walks (identity scan +
        // buffer-access count) would always come up empty.
        if program.stats().atomic_op_count == 0 {
            return PassAnalysis::SKIP;
        }
        let mut found = false;
        scan_for_identity_candidate(program.entry(), &mut found);
        if found {
            return PassAnalysis::RUN;
        }

        let mut access_counts = HashMap::default();
        count_buffer_accesses(program.entry(), &mut access_counts);
        let has_single_writer = access_counts
            .values()
            .any(|counts| counts.atomic_adds == 1 && counts.other_accesses == 0);

        if has_single_writer {
            PassAnalysis::RUN
        } else {
            PassAnalysis::SKIP
        }
    }

    /// Walk the program and collapse atomics.
    #[must_use]
    pub fn transform(program: Program) -> PassResult {
        let mut access_counts = HashMap::default();
        count_buffer_accesses(program.entry(), &mut access_counts);
        // Only Relaxed single-writer atomics are eligible for the load+store
        // rewrite. SeqCst/AcqRel/Acquire/Release atomics provide ordering
        // guarantees to concurrent readers; replacing them with a plain store
        // would silently drop those guarantees even when there is only one
        // writer. The ordering guard is applied here, not just in
        // count_expr_accesses, so the set only contains buffers whose sole
        // atomic-add access is Relaxed.
        let eligible_buffers: HashSet<_> = access_counts
            .into_iter()
            .filter(|(_, counts)| counts.atomic_adds == 1 && counts.other_accesses == 0)
            .filter(|(_, counts)| counts.sole_atomic_ordering == MemoryOrdering::Relaxed)
            .map(|(buf, _)| buf)
            .collect();

        let mut changed = false;
        let program = program.map_entry(|entry| {
            entry
                .into_iter()
                .flat_map(|n| rewrite_node_multi(n, &eligible_buffers, &mut changed))
                .collect()
        });
        PassResult { program, changed }
    }
}

#[expect(
    clippy::too_many_lines,
    reason = "atomic minimization keeps hoisting/rewrite cases colocated with Node reconstruction"
)]
fn rewrite_node_multi(
    node: Node,
    eligible_buffers: &HashSet<crate::ir::Ident>,
    changed: &mut bool,
) -> Vec<Node> {
    match node {
        Node::Let { name, value } => {
            if let Expr::Atomic {
                op: AtomicOp::Add,
                buffer,
                index,
                expected: None,
                value: add_value,
                ..
            } = &value
            {
                if eligible_buffers.contains(buffer) {
                    *changed = true;
                    let new_load = Expr::Load {
                        buffer: buffer.clone(),
                        index: index.clone(),
                    };
                    let store_node = Node::Store {
                        buffer: buffer.clone(),
                        index: *index.clone(),
                        value: rewrite_expr(
                            Expr::BinOp {
                                op: crate::ir::BinOp::Add,
                                left: Box::new(Expr::Var(name.clone())),
                                right: add_value.clone(),
                            },
                            changed,
                        ),
                    };
                    return vec![
                        Node::Let {
                            name,
                            value: rewrite_expr(new_load, changed),
                        },
                        store_node,
                    ];
                }
            }
            vec![Node::Let {
                name,
                value: rewrite_expr(value, changed),
            }]
        }
        Node::Assign { name, value } => {
            if let Expr::Atomic {
                op: AtomicOp::Add,
                buffer,
                index,
                expected: None,
                value: add_value,
                ..
            } = &value
            {
                if eligible_buffers.contains(buffer) {
                    *changed = true;
                    let new_load = Expr::Load {
                        buffer: buffer.clone(),
                        index: index.clone(),
                    };
                    let store_node = Node::Store {
                        buffer: buffer.clone(),
                        index: *index.clone(),
                        value: rewrite_expr(
                            Expr::BinOp {
                                op: crate::ir::BinOp::Add,
                                left: Box::new(Expr::Var(name.clone())),
                                right: add_value.clone(),
                            },
                            changed,
                        ),
                    };
                    return vec![
                        Node::Assign {
                            name,
                            value: rewrite_expr(new_load, changed),
                        },
                        store_node,
                    ];
                }
            }
            vec![Node::Assign {
                name,
                value: rewrite_expr(value, changed),
            }]
        }
        Node::Store {
            buffer,
            index,
            value,
        } => vec![Node::Store {
            buffer,
            index: rewrite_expr(index, changed),
            value: rewrite_expr(value, changed),
        }],
        Node::If {
            cond,
            then,
            otherwise,
        } => vec![Node::If {
            cond: rewrite_expr(cond, changed),
            then: then
                .into_iter()
                .flat_map(|n| rewrite_node_multi(n, eligible_buffers, changed))
                .collect(),
            otherwise: otherwise
                .into_iter()
                .flat_map(|n| rewrite_node_multi(n, eligible_buffers, changed))
                .collect(),
        }],
        Node::Loop {
            var,
            from,
            to,
            body,
        } => vec![Node::Loop {
            var,
            from: rewrite_expr(from, changed),
            to: rewrite_expr(to, changed),
            body: body
                .into_iter()
                .flat_map(|n| rewrite_node_multi(n, eligible_buffers, changed))
                .collect(),
        }],
        Node::Block(body) => vec![Node::Block(
            body.into_iter()
                .flat_map(|n| rewrite_node_multi(n, eligible_buffers, changed))
                .collect(),
        )],
        Node::Region {
            generator,
            source_region,
            body,
        } => {
            let body_vec: Vec<Node> = match std::sync::Arc::try_unwrap(body) {
                Ok(v) => v,
                Err(arc) => (*arc).clone(),
            };
            vec![Node::Region {
                generator,
                source_region,
                body: std::sync::Arc::new(
                    body_vec
                        .into_iter()
                        .flat_map(|n| rewrite_node_multi(n, eligible_buffers, changed))
                        .collect(),
                ),
            }]
        }
        Node::AsyncLoad {
            source,
            destination,
            offset,
            size,
            tag,
        } => vec![Node::AsyncLoad {
            source,
            destination,
            tag,
            offset: Box::new(rewrite_expr(*offset, changed)),
            size: Box::new(rewrite_expr(*size, changed)),
        }],
        Node::AsyncStore {
            source,
            destination,
            offset,
            size,
            tag,
        } => vec![Node::AsyncStore {
            source,
            destination,
            tag,
            offset: Box::new(rewrite_expr(*offset, changed)),
            size: Box::new(rewrite_expr(*size, changed)),
        }],
        Node::Trap { address, tag } => vec![Node::Trap {
            address: Box::new(rewrite_expr(*address, changed)),
            tag,
        }],
        other => vec![other],
    }
}

#[expect(
    clippy::too_many_lines,
    reason = "owned expression rewriter avoids recursive drop/clone on deep generated atomic expressions"
)]
fn rewrite_expr(expr: Expr, changed: &mut bool) -> Expr {
    enum Frame {
        Expr(Expr),
        Load {
            buffer: crate::ir::Ident,
        },
        BinOp {
            op: crate::ir::BinOp,
        },
        UnOp {
            op: crate::ir::UnOp,
        },
        Call {
            op_id: crate::ir::Ident,
            argc: usize,
        },
        Select,
        Cast {
            target: crate::ir::DataType,
        },
        Fma,
        Atomic {
            op: AtomicOp,
            buffer: crate::ir::Ident,
            ordering: MemoryOrdering,
            has_expected: bool,
        },
        SubgroupBallot,
        SubgroupShuffle,
        SubgroupReduce(SubgroupReduceOp),
    }

    let mut stack = vec![Frame::Expr(expr)];
    let mut results = Vec::new();

    while let Some(frame) = stack.pop() {
        match frame {
            Frame::Expr(expr) => match expr {
                Expr::Load { buffer, index } => {
                    stack.push(Frame::Load { buffer });
                    stack.push(Frame::Expr(*index));
                }
                Expr::BinOp { op, left, right } => {
                    stack.push(Frame::BinOp { op });
                    stack.push(Frame::Expr(*right));
                    stack.push(Frame::Expr(*left));
                }
                Expr::UnOp { op, operand } => {
                    stack.push(Frame::UnOp { op });
                    stack.push(Frame::Expr(*operand));
                }
                Expr::Call { op_id, args } => {
                    let argc = args.len();
                    stack.push(Frame::Call { op_id, argc });
                    stack.extend(args.into_iter().rev().map(Frame::Expr));
                }
                Expr::Select {
                    cond,
                    true_val,
                    false_val,
                } => {
                    stack.push(Frame::Select);
                    stack.push(Frame::Expr(*false_val));
                    stack.push(Frame::Expr(*true_val));
                    stack.push(Frame::Expr(*cond));
                }
                Expr::Cast { target, value } => {
                    stack.push(Frame::Cast { target });
                    stack.push(Frame::Expr(*value));
                }
                Expr::Fma { a, b, c } => {
                    stack.push(Frame::Fma);
                    stack.push(Frame::Expr(*c));
                    stack.push(Frame::Expr(*b));
                    stack.push(Frame::Expr(*a));
                }
                Expr::Atomic {
                    op,
                    buffer,
                    index,
                    expected,
                    value,
                    ordering,
                } => {
                    let has_expected = expected.is_some();
                    stack.push(Frame::Atomic {
                        op,
                        buffer,
                        ordering,
                        has_expected,
                    });
                    stack.push(Frame::Expr(*value));
                    if let Some(expected) = expected {
                        stack.push(Frame::Expr(*expected));
                    }
                    stack.push(Frame::Expr(*index));
                }
                Expr::SubgroupBallot { cond } => {
                    stack.push(Frame::SubgroupBallot);
                    stack.push(Frame::Expr(*cond));
                }
                Expr::SubgroupShuffle { value, lane } => {
                    stack.push(Frame::SubgroupShuffle);
                    stack.push(Frame::Expr(*lane));
                    stack.push(Frame::Expr(*value));
                }
                Expr::SubgroupReduce { op, value } => {
                    stack.push(Frame::SubgroupReduce(op));
                    stack.push(Frame::Expr(*value));
                }
                terminal => results.push(terminal),
            },
            Frame::Load { buffer } => {
                let index = pop_owned_expr_result(&mut results, "load index");
                results.push(Expr::Load {
                    buffer,
                    index: Box::new(index),
                });
            }
            Frame::BinOp { op } => {
                let right = pop_owned_expr_result(&mut results, "binary rhs");
                let left = pop_owned_expr_result(&mut results, "binary lhs");
                results.push(Expr::BinOp {
                    op,
                    left: Box::new(left),
                    right: Box::new(right),
                });
            }
            Frame::UnOp { op } => {
                let operand = pop_owned_expr_result(&mut results, "unary operand");
                results.push(Expr::UnOp {
                    op,
                    operand: Box::new(operand),
                });
            }
            Frame::Call { op_id, argc } => {
                let start = results.len().checked_sub(argc).unwrap_or_else(|| {
                    unreachable!(
                        "Fix: atomic_minimize owned rewriter lost call args; stack state is inconsistent."
                    )
                });
                let args = results.drain(start..).collect();
                results.push(Expr::Call { op_id, args });
            }
            Frame::Select => {
                let false_val = pop_owned_expr_result(&mut results, "select false value");
                let true_val = pop_owned_expr_result(&mut results, "select true value");
                let cond = pop_owned_expr_result(&mut results, "select condition");
                results.push(Expr::Select {
                    cond: Box::new(cond),
                    true_val: Box::new(true_val),
                    false_val: Box::new(false_val),
                });
            }
            Frame::Cast { target } => {
                let value = pop_owned_expr_result(&mut results, "cast value");
                results.push(Expr::Cast {
                    target,
                    value: Box::new(value),
                });
            }
            Frame::Fma => {
                let c = pop_owned_expr_result(&mut results, "fma c");
                let b = pop_owned_expr_result(&mut results, "fma b");
                let a = pop_owned_expr_result(&mut results, "fma a");
                results.push(Expr::Fma {
                    a: Box::new(a),
                    b: Box::new(b),
                    c: Box::new(c),
                });
            }
            Frame::Atomic {
                op,
                buffer,
                ordering,
                has_expected,
            } => {
                let value = pop_owned_expr_result(&mut results, "atomic value");
                let expected = if has_expected {
                    Some(pop_owned_expr_result(&mut results, "atomic expected"))
                } else {
                    None
                };
                let index = pop_owned_expr_result(&mut results, "atomic index");
                if expected.is_none()
                    && ordering == MemoryOrdering::Relaxed
                    && is_identity_atomic(op, &value)
                {
                    *changed = true;
                    results.push(Expr::Load {
                        buffer,
                        index: Box::new(index),
                    });
                } else {
                    results.push(Expr::Atomic {
                        op,
                        buffer,
                        index: Box::new(index),
                        expected: expected.map(Box::new),
                        value: Box::new(value),
                        ordering,
                    });
                }
            }
            Frame::SubgroupBallot => {
                let cond = pop_owned_expr_result(&mut results, "subgroup ballot condition");
                results.push(Expr::SubgroupBallot {
                    cond: Box::new(cond),
                });
            }
            Frame::SubgroupShuffle => {
                let lane = pop_owned_expr_result(&mut results, "subgroup shuffle lane");
                let value = pop_owned_expr_result(&mut results, "subgroup shuffle value");
                results.push(Expr::SubgroupShuffle {
                    value: Box::new(value),
                    lane: Box::new(lane),
                });
            }
            Frame::SubgroupReduce(op) => {
                let value = pop_owned_expr_result(&mut results, "subgroup reduce value");
                results.push(Expr::SubgroupReduce {
                    op,
                    value: Box::new(value),
                });
            }
        }
    }

    match results.pop() {
        Some(result) => result,
        None => unreachable!(
            "Fix: atomic_minimize owned rewriter produced no expression result; stack state is inconsistent."
        ),
    }
}

fn pop_owned_expr_result(results: &mut Vec<Expr>, context: &'static str) -> Expr {
    results.pop().unwrap_or_else(|| {
        unreachable!(
            "Fix: atomic_minimize owned rewriter lost {context}; stack state is inconsistent."
        )
    })
}

fn is_identity_atomic(op: AtomicOp, value: &Expr) -> bool {
    matches!(
        (op, value),
        (
            AtomicOp::Add | AtomicOp::Or | AtomicOp::Xor,
            Expr::LitU32(0) | Expr::LitI32(0)
        ) | (AtomicOp::And, Expr::LitU32(u32::MAX) | Expr::LitI32(-1))
    )
}

fn scan_for_identity_candidate(nodes: &[Node], found: &mut bool) {
    for node in nodes {
        if *found {
            return;
        }
        scan_node_for_identity(node, found);
    }
}

fn scan_node_for_identity(node: &Node, found: &mut bool) {
    match node {
        Node::Let { value, .. } | Node::Assign { value, .. } => {
            scan_expr_for_identity(value, found);
        }
        Node::Store { index, value, .. } => {
            scan_expr_for_identity(index, found);
            scan_expr_for_identity(value, found);
        }
        Node::If {
            cond,
            then,
            otherwise,
        } => {
            scan_expr_for_identity(cond, found);
            scan_for_identity_candidate(then, found);
            scan_for_identity_candidate(otherwise, found);
        }
        Node::Loop { from, to, body, .. } => {
            scan_expr_for_identity(from, found);
            scan_expr_for_identity(to, found);
            scan_for_identity_candidate(body, found);
        }
        Node::Block(body) => scan_for_identity_candidate(body, found),
        Node::Region { body, .. } => scan_for_identity_candidate(body, found),
        _ => {}
    }
}

fn scan_expr_for_identity(expr: &Expr, found: &mut bool) {
    if *found {
        return;
    }
    let mut stack: SmallVec<[&Expr; 32]> = SmallVec::new();
    stack.push(expr);
    while let Some(expr) = stack.pop() {
        if *found {
            return;
        }
        match expr {
            Expr::Atomic {
                op,
                value,
                expected,
                ordering,
                index,
                ..
            } => {
                if expected.is_none()
                    && *ordering == MemoryOrdering::Relaxed
                    && is_identity_atomic(*op, value)
                {
                    *found = true;
                    return;
                }
                push_expr_child(&mut stack, value);
                if let Some(expected) = expected.as_deref() {
                    push_expr_child(&mut stack, expected);
                }
                push_expr_child(&mut stack, index);
            }
            _ => push_expr_children(&mut stack, expr),
        }
    }
}

fn count_buffer_accesses(nodes: &[Node], counts: &mut HashMap<crate::ir::Ident, BufferAccesses>) {
    for node in nodes {
        match node {
            Node::Let { value, .. } | Node::Assign { value, .. } => {
                count_expr_accesses(value, counts);
            }
            Node::Store {
                buffer,
                index,
                value,
            } => {
                counts.entry(buffer.clone()).or_default().other_accesses += 1;
                count_expr_accesses(index, counts);
                count_expr_accesses(value, counts);
            }
            Node::If {
                cond,
                then,
                otherwise,
            } => {
                count_expr_accesses(cond, counts);
                count_buffer_accesses(then, counts);
                count_buffer_accesses(otherwise, counts);
            }
            Node::Loop { from, to, body, .. } => {
                count_expr_accesses(from, counts);
                count_expr_accesses(to, counts);
                count_buffer_accesses(body, counts);
            }
            Node::Block(body) => count_buffer_accesses(body, counts),
            Node::Region { body, .. } => count_buffer_accesses(body, counts),
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
                counts.entry(source.clone()).or_default().other_accesses += 1;
                counts
                    .entry(destination.clone())
                    .or_default()
                    .other_accesses += 1;
                count_expr_accesses(offset, counts);
                count_expr_accesses(size, counts);
            }
            Node::AllReduce { buffer, .. } | Node::Broadcast { buffer, .. } => {
                counts.entry(buffer.clone()).or_default().other_accesses += 1;
            }
            Node::AllGather { input, output, .. } | Node::ReduceScatter { input, output, .. } => {
                counts.entry(input.clone()).or_default().other_accesses += 1;
                counts.entry(output.clone()).or_default().other_accesses += 1;
            }
            Node::Trap { address, .. } => count_expr_accesses(address, counts),
            Node::Barrier { .. }
            | Node::Return
            | Node::Resume { .. }
            | Node::IndirectDispatch { .. }
            | Node::AsyncWait { .. }
            | Node::Opaque(_) => {}
        }
    }
}

fn count_expr_accesses(expr: &Expr, counts: &mut HashMap<crate::ir::Ident, BufferAccesses>) {
    let mut stack: SmallVec<[&Expr; 32]> = SmallVec::new();
    stack.push(expr);
    while let Some(expr) = stack.pop() {
        match expr {
            Expr::Atomic {
                op,
                buffer,
                index,
                expected,
                value,
                ordering,
            } => {
                if *op == AtomicOp::Add && expected.is_none() {
                    let entry = counts.entry(buffer.clone()).or_default();
                    entry.atomic_adds += 1;
                    // Record the ordering on the first encounter. If
                    // atomic_adds later exceeds 1 the buffer is ineligible
                    // regardless, so the recorded value is never misused.
                    if entry.atomic_adds == 1 {
                        entry.sole_atomic_ordering = *ordering;
                    }
                } else {
                    counts.entry(buffer.clone()).or_default().other_accesses += 1;
                }
                push_expr_child(&mut stack, value);
                if let Some(expected) = expected.as_deref() {
                    push_expr_child(&mut stack, expected);
                }
                push_expr_child(&mut stack, index);
            }
            Expr::Load { buffer, index } => {
                counts.entry(buffer.clone()).or_default().other_accesses += 1;
                push_expr_child(&mut stack, index);
            }
            _ => push_expr_children(&mut stack, expr),
        }
    }
}

fn push_expr_child<'a>(stack: &mut SmallVec<[&'a Expr; 32]>, child: &'a Expr) {
    stack.push(child);
}

fn push_expr_children<'a>(stack: &mut SmallVec<[&'a Expr; 32]>, expr: &'a Expr) {
    match expr {
        Expr::BinOp { left, right, .. } => {
            push_expr_child(stack, right);
            push_expr_child(stack, left);
        }
        Expr::UnOp { operand, .. } => push_expr_child(stack, operand),
        Expr::Call { args, .. } => {
            stack.extend(args.iter().rev());
        }
        Expr::Select {
            cond,
            true_val,
            false_val,
        } => {
            push_expr_child(stack, false_val);
            push_expr_child(stack, true_val);
            push_expr_child(stack, cond);
        }
        Expr::Cast { value, .. } | Expr::SubgroupReduce { value, .. } => {
            push_expr_child(stack, value);
        }
        Expr::Fma { a, b, c } => {
            push_expr_child(stack, c);
            push_expr_child(stack, b);
            push_expr_child(stack, a);
        }
        Expr::SubgroupBallot { cond } => push_expr_child(stack, cond),
        Expr::SubgroupShuffle { value, lane } => {
            push_expr_child(stack, lane);
            push_expr_child(stack, value);
        }
        Expr::Load { index, .. } => push_expr_child(stack, index),
        Expr::Atomic {
            index,
            expected,
            value,
            ..
        } => {
            push_expr_child(stack, value);
            if let Some(expected) = expected.as_deref() {
                push_expr_child(stack, expected);
            }
            push_expr_child(stack, index);
        }
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
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{BufferAccess, BufferDecl, DataType, Expr, Ident};

    fn buf() -> BufferDecl {
        BufferDecl::storage("buf", 0, BufferAccess::ReadWrite, DataType::U32).with_count(4)
    }

    fn program(entry: Vec<Node>) -> Program {
        Program::wrapped(vec![buf()], [1, 1, 1], entry)
    }

    fn relaxed_atomic(op: AtomicOp, value: Expr) -> Expr {
        Expr::Atomic {
            op,
            buffer: Ident::from("buf"),
            index: Box::new(Expr::u32(0)),
            expected: None,
            value: Box::new(value),
            ordering: MemoryOrdering::Relaxed,
        }
    }

    fn extract_let_value(p: &Program, name: &str) -> Expr {
        fn walk<'a>(nodes: &'a [Node], target: &str) -> Option<&'a Expr> {
            for n in nodes {
                match n {
                    Node::Let { name, value } if name.as_str() == target => return Some(value),
                    Node::Block(body) => {
                        if let Some(found) = walk(body, target) {
                            return Some(found);
                        }
                    }
                    Node::Region { body, .. } => {
                        if let Some(found) = walk(body.as_ref(), target) {
                            return Some(found);
                        }
                    }
                    Node::If {
                        then, otherwise, ..
                    } => {
                        if let Some(found) = walk(then, target) {
                            return Some(found);
                        }
                        if let Some(found) = walk(otherwise, target) {
                            return Some(found);
                        }
                    }
                    Node::Loop { body, .. } => {
                        if let Some(found) = walk(body, target) {
                            return Some(found);
                        }
                    }
                    _ => {}
                }
            }
            None
        }
        walk(p.entry(), name)
            .cloned()
            .unwrap_or_else(|| panic!("expected Let `{name}` in entry tree"))
    }

    #[test]
    fn add_zero_relaxed_collapses_to_load() {
        let entry = vec![Node::let_bind(
            "x",
            relaxed_atomic(AtomicOp::Add, Expr::u32(0)),
        )];
        let result = AtomicMinimizePass::transform(program(entry));
        assert!(result.changed);
        assert_eq!(
            extract_let_value(&result.program, "x"),
            Expr::Load {
                buffer: Ident::from("buf"),
                index: Box::new(Expr::u32(0)),
            }
        );
    }

    #[test]
    fn or_zero_relaxed_collapses_to_load() {
        let entry = vec![Node::let_bind(
            "x",
            relaxed_atomic(AtomicOp::Or, Expr::u32(0)),
        )];
        let result = AtomicMinimizePass::transform(program(entry));
        assert!(result.changed);
        // Must collapse to a Load on exactly the same buffer and index, not
        // just any Expr::Load (guards against wrong-buffer/index regressions).
        assert_eq!(
            extract_let_value(&result.program, "x"),
            Expr::Load {
                buffer: Ident::from("buf"),
                index: Box::new(Expr::u32(0)),
            },
            "Or(x, 0) Relaxed must collapse to Load {{ buffer: \"buf\", index: 0 }}"
        );
    }

    #[test]
    fn xor_zero_relaxed_collapses_to_load() {
        let entry = vec![Node::let_bind(
            "x",
            relaxed_atomic(AtomicOp::Xor, Expr::u32(0)),
        )];
        let result = AtomicMinimizePass::transform(program(entry));
        assert!(result.changed);
        assert_eq!(
            extract_let_value(&result.program, "x"),
            Expr::Load {
                buffer: Ident::from("buf"),
                index: Box::new(Expr::u32(0)),
            },
            "Xor(x, 0) Relaxed must collapse to Load {{ buffer: \"buf\", index: 0 }}"
        );
    }

    #[test]
    fn and_max_relaxed_collapses_to_load() {
        let entry = vec![Node::let_bind(
            "x",
            relaxed_atomic(AtomicOp::And, Expr::u32(u32::MAX)),
        )];
        let result = AtomicMinimizePass::transform(program(entry));
        assert!(result.changed);
        assert_eq!(
            extract_let_value(&result.program, "x"),
            Expr::Load {
                buffer: Ident::from("buf"),
                index: Box::new(Expr::u32(0)),
            },
            "And(x, u32::MAX) Relaxed must collapse to Load {{ buffer: \"buf\", index: 0 }}"
        );
    }

    #[test]
    fn single_writer_atomic_add_rewritten() {
        let entry = vec![Node::let_bind(
            "x",
            relaxed_atomic(AtomicOp::Add, Expr::u32(42)),
        )];
        let result = AtomicMinimizePass::transform(program(entry));
        assert!(result.changed);

        let mut found_store = false;
        fn walk_store(nodes: &[Node], found: &mut bool) {
            for n in nodes {
                match n {
                    Node::Store { buffer, .. } if buffer.as_str() == "buf" => *found = true,
                    Node::Region { body, .. } => walk_store(body, found),
                    Node::Block(body) => walk_store(body, found),
                    Node::If {
                        then, otherwise, ..
                    } => {
                        walk_store(then, found);
                        walk_store(otherwise, found);
                    }
                    Node::Loop { body, .. } => walk_store(body, found),
                    _ => {}
                }
            }
        }
        walk_store(result.program.entry(), &mut found_store);

        assert!(found_store, "Store should have been generated");
        assert!(matches!(
            extract_let_value(&result.program, "x"),
            Expr::Load { .. }
        ));
    }

    #[test]
    fn two_atomic_adds_keep_atomic() {
        let entry = vec![
            Node::let_bind("x", relaxed_atomic(AtomicOp::Add, Expr::u32(42))),
            Node::let_bind("y", relaxed_atomic(AtomicOp::Add, Expr::u32(43))),
        ];
        let result = AtomicMinimizePass::transform(program(entry));
        assert!(!result.changed);
        assert!(matches!(
            extract_let_value(&result.program, "x"),
            Expr::Atomic { .. }
        ));
    }

    #[test]
    fn atomic_with_load_keeps_atomic() {
        let entry = vec![
            Node::let_bind("x", relaxed_atomic(AtomicOp::Add, Expr::u32(42))),
            Node::let_bind(
                "y",
                Expr::Load {
                    buffer: Ident::from("buf"),
                    index: Box::new(Expr::u32(0)),
                },
            ),
        ];
        let result = AtomicMinimizePass::transform(program(entry));
        assert!(!result.changed);
        assert!(matches!(
            extract_let_value(&result.program, "x"),
            Expr::Atomic { .. }
        ));
    }

    #[test]
    fn atomic_with_store_keeps_atomic() {
        let entry = vec![
            Node::let_bind("x", relaxed_atomic(AtomicOp::Add, Expr::u32(42))),
            Node::store("buf", Expr::u32(1), Expr::u32(99)),
        ];
        let result = AtomicMinimizePass::transform(program(entry));
        assert!(!result.changed);
        assert!(matches!(
            extract_let_value(&result.program, "x"),
            Expr::Atomic { .. }
        ));
    }

    #[test]
    fn compare_exchange_not_eligible() {
        let entry = vec![Node::let_bind(
            "x",
            Expr::Atomic {
                op: AtomicOp::CompareExchange,
                buffer: Ident::from("buf"),
                index: Box::new(Expr::u32(0)),
                expected: Some(Box::new(Expr::u32(1))),
                value: Box::new(Expr::u32(42)),
                ordering: MemoryOrdering::Relaxed,
            },
        )];
        let result = AtomicMinimizePass::transform(program(entry));
        assert!(!result.changed);
    }

    #[test]
    fn generated_deep_identity_atomic_expression_rewrites_without_recursive_expr_walk() {
        for depth in [1usize, 8, 64, 512, 4096] {
            let mut value = relaxed_atomic(AtomicOp::Add, Expr::u32(0));
            for _ in 0..depth {
                value = Expr::add(value, Expr::u32(0));
            }

            let PassResult {
                program: rewritten_program,
                changed,
            } = AtomicMinimizePass::transform(program(vec![Node::let_bind("x", value)]));
            assert!(
                changed,
                "Fix: atomic_minimize must rewrite nested identity atomic at generated depth {depth}."
            );
            let rewritten_program = Box::leak(Box::new(rewritten_program));
            let rewritten = find_let_value_ref(rewritten_program, "x")
                .expect("Fix: replace expect with fallible API or document caller precondition; panic only on programmer error - generated deep atomic program must still contain let x");
            assert!(
                !expr_contains_atomic(rewritten),
                "Fix: atomic_minimize left an atomic inside generated depth {depth}: {rewritten:?}"
            );
            assert!(
                expr_contains_load(rewritten),
                "Fix: atomic_minimize must replace the identity atomic with a load at generated depth {depth}."
            );
        }
    }

    fn find_let_value_ref<'a>(program: &'a Program, target: &str) -> Option<&'a Expr> {
        let mut stack = Vec::new();
        stack.extend(program.entry().iter().rev());
        while let Some(node) = stack.pop() {
            match node {
                Node::Let { name, value } if name.as_str() == target => return Some(value),
                Node::If {
                    then, otherwise, ..
                } => {
                    stack.extend(otherwise.iter().rev());
                    stack.extend(then.iter().rev());
                }
                Node::Loop { body, .. } | Node::Block(body) => {
                    stack.extend(body.iter().rev());
                }
                Node::Region { body, .. } => {
                    stack.extend(body.iter().rev());
                }
                _ => {}
            }
        }
        None
    }

    #[test]
    fn seq_cst_single_writer_ordering_preserved() {
        // A SeqCst single-writer atomic-add must NOT be rewritten to a plain
        // Load+Store. SeqCst provides sequential-consistency guarantees to
        // concurrent readers; silently replacing it with a non-atomic store
        // drops those guarantees. Only Relaxed single-writer atomics are
        // eligible for the rewrite.
        let entry = vec![Node::let_bind(
            "x",
            Expr::Atomic {
                op: AtomicOp::Add,
                buffer: Ident::from("buf"),
                index: Box::new(Expr::u32(0)),
                expected: None,
                value: Box::new(Expr::u32(42)),
                ordering: MemoryOrdering::SeqCst,
            },
        )];
        let result = AtomicMinimizePass::transform(program(entry));
        assert!(
            !result.changed,
            "SeqCst single-writer atomic must NOT be rewritten to plain Load+Store: \
             the rewrite would silently drop SeqCst ordering guarantees visible to concurrent readers"
        );
        // The atomic must still be present in the output, unchanged.
        assert!(
            matches!(
                extract_let_value(&result.program, "x"),
                Expr::Atomic {
                    ordering: MemoryOrdering::SeqCst,
                    ..
                }
            ),
            "SeqCst atomic must remain in the program after transform"
        );
    }

    #[test]
    fn analyze_skips_program_with_no_candidate() {
        let entry = vec![Node::let_bind("x", Expr::u32(7))];
        match crate::optimizer::ProgramPass::analyze(&AtomicMinimizePass, &program(entry)) {
            PassAnalysis::SKIP => {}
            other => panic!("expected SKIP, got {other:?}"),
        }
    }

    fn expr_contains_atomic(expr: &Expr) -> bool {
        expr_contains(expr, |expr| matches!(expr, Expr::Atomic { .. }))
    }

    fn expr_contains_load(expr: &Expr) -> bool {
        expr_contains(expr, |expr| matches!(expr, Expr::Load { .. }))
    }

    fn expr_contains(expr: &Expr, mut predicate: impl FnMut(&Expr) -> bool) -> bool {
        let mut stack: SmallVec<[&Expr; 32]> = SmallVec::new();
        stack.push(expr);
        while let Some(expr) = stack.pop() {
            if predicate(expr) {
                return true;
            }
            push_expr_children(&mut stack, expr);
        }
        false
    }
}
