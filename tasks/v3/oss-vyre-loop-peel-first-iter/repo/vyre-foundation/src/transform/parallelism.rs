//! Shared-nothing parallelism detection for IR dispatch planning.
//!
//! The analysis is conservative: a statement may enter a parallel dispatch
//! group only when its writable buffer set is disjoint from every other
//! statement in the group. Any write-after-write conflict forms a serial
//! boundary.

use crate::ir::{Expr, Ident, Node};
use rustc_hash::FxHashSet;
use smallvec::SmallVec;

/// Dispatch grouping selected by shared-nothing analysis.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum DispatchGroup {
    /// One statement must run alone because it conflicts with adjacent work.
    Serial {
        /// Original top-level node index that must dispatch alone.
        node_index: usize,
    },
    /// Several statements can be emitted as concurrent dispatches.
    Parallel {
        /// Original top-level node indices that can dispatch concurrently.
        node_indices: Vec<usize>,
    },
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct AccessSet {
    reads: FxHashSet<Ident>,
    writes: FxHashSet<Ident>,
    serial_boundary: bool,
}

/// Analyze top-level IR nodes for writable-state independence.
#[must_use]
pub fn detect_parallelism(nodes: &[Node]) -> Vec<DispatchGroup> {
    let mut groups = Vec::with_capacity(nodes.len());
    let mut current = Vec::with_capacity(nodes.len());
    let mut current_access = AccessSet::default();

    for (index, node) in nodes.iter().enumerate() {
        let access = access_set(node);
        if access.serial_boundary {
            push_group(&mut groups, &mut current);
            groups.push(DispatchGroup::Serial { node_index: index });
            current_access = AccessSet::default();
            continue;
        }
        if conflicts(&current_access, &access) {
            push_group(&mut groups, &mut current);
            current_access = AccessSet::default();
        }
        current_access.reads.extend(access.reads);
        current_access.writes.extend(access.writes);
        current.push(index);
    }
    push_group(&mut groups, &mut current);
    groups
}

fn push_group(groups: &mut Vec<DispatchGroup>, current: &mut Vec<usize>) {
    match current.len() {
        0 => {}
        1 => groups.push(DispatchGroup::Serial {
            node_index: current[0],
        }),
        _ => groups.push(DispatchGroup::Parallel {
            node_indices: std::mem::take(current),
        }),
    }
    current.clear();
}

fn conflicts(left: &AccessSet, right: &AccessSet) -> bool {
    right
        .writes
        .iter()
        .any(|buffer| left.writes.contains(buffer) || left.reads.contains(buffer))
        || right
            .reads
            .iter()
            .any(|buffer| left.writes.contains(buffer))
}

fn access_set(node: &Node) -> AccessSet {
    let mut access = AccessSet::default();
    collect_node_access(node, &mut access);
    access
}

fn collect_node_access(root: &Node, access: &mut AccessSet) {
    let mut stack = SmallVec::<[&Node; 16]>::new();
    stack.push(root);
    while let Some(node) = stack.pop() {
        match node {
            Node::Let { value, .. } | Node::Assign { value, .. } => {
                collect_expr_reads(value, access);
            }
            Node::Store {
                buffer,
                index,
                value,
            } => {
                collect_expr_reads(index, access);
                collect_expr_reads(value, access);
                access.writes.insert(buffer.clone());
            }
            Node::If {
                cond,
                then,
                otherwise,
            } => {
                collect_expr_reads(cond, access);
                stack.extend(then);
                stack.extend(otherwise);
            }
            Node::Loop { from, to, body, .. } => {
                collect_expr_reads(from, access);
                collect_expr_reads(to, access);
                stack.extend(body);
            }
            Node::Block(body) => {
                stack.extend(body);
            }
            Node::IndirectDispatch { count_buffer, .. } => {
                access.reads.insert(count_buffer.clone());
                access.serial_boundary = true;
            }
            Node::Trap { address, .. } => {
                collect_expr_reads(address, access);
                access.serial_boundary = true;
            }
            Node::Resume { .. }
            | Node::AllReduce { .. }
            | Node::AllGather { .. }
            | Node::ReduceScatter { .. }
            | Node::Broadcast { .. }
            | Node::Return
            | Node::Barrier { .. }
            | Node::AsyncLoad { .. }
            | Node::AsyncStore { .. }
            | Node::AsyncWait { .. }
            | Node::Region { .. }
            | Node::Opaque(_) => {
                access.serial_boundary = true;
            }
        }
    }
}

fn collect_expr_reads(expr: &Expr, access: &mut AccessSet) {
    let mut stack = SmallVec::<[&Expr; 32]>::new();
    stack.push(expr);
    while let Some(expr) = stack.pop() {
        match expr {
            Expr::Load { buffer, index } => {
                access.reads.insert(buffer.clone());
                stack.push(index);
            }
            Expr::BufLen { buffer } => {
                access.reads.insert(buffer.clone());
            }
            Expr::BinOp { left, right, .. } => {
                stack.push(left);
                stack.push(right);
            }
            Expr::UnOp { operand, .. } => stack.push(operand),
            Expr::Call { args, .. } => {
                stack.extend(args);
            }
            Expr::Select {
                cond,
                true_val,
                false_val,
            } => {
                stack.push(cond);
                stack.push(true_val);
                stack.push(false_val);
            }
            Expr::Cast { value, .. } => stack.push(value),
            Expr::Fma { a, b, c } => {
                stack.push(a);
                stack.push(b);
                stack.push(c);
            }
            Expr::Atomic {
                buffer,
                index,
                expected,
                value,
                ..
            } => {
                access.reads.insert(buffer.clone());
                access.writes.insert(buffer.clone());
                stack.push(index);
                if let Some(expected) = expected {
                    stack.push(expected);
                }
                stack.push(value);
            }
            Expr::LitU32(_)
            | Expr::LitI32(_)
            | Expr::LitF32(_)
            | Expr::LitBool(_)
            | Expr::Var(_)
            | Expr::InvocationId { .. }
            | Expr::WorkgroupId { .. }
            | Expr::LocalId { .. }
            | Expr::SubgroupLocalId
            | Expr::SubgroupSize
            | Expr::SubgroupBallot { .. }
            | Expr::SubgroupShuffle { .. }
            | Expr::SubgroupReduce { .. } => {}
            Expr::Opaque(_) => {
                access.serial_boundary = true;
            }
        }
    }
}

/// Parallelism analysis test suite.
#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::ir::Expr;

    /// Write-after-write on the same buffer must serialise.
    #[test]
    pub fn write_after_write_serialised() {
        let nodes = vec![
            Node::store("out", Expr::u32(0), Expr::u32(1)),
            Node::store("out", Expr::u32(1), Expr::u32(2)),
        ];

        assert_eq!(
            detect_parallelism(&nodes),
            vec![
                DispatchGroup::Serial { node_index: 0 },
                DispatchGroup::Serial { node_index: 1 },
            ]
        );
    }

    /// Independent writes to different buffers may run in parallel.
    #[test]
    pub fn independent_writes_parallelised() {
        let nodes = vec![
            Node::store("a", Expr::u32(0), Expr::u32(1)),
            Node::store("b", Expr::u32(0), Expr::u32(2)),
        ];

        assert_eq!(
            detect_parallelism(&nodes),
            vec![DispatchGroup::Parallel {
                node_indices: vec![0, 1]
            }]
        );
    }

    /// Read-after-write on the same buffer must serialise.
    #[test]
    pub fn read_after_write_serialised() {
        let nodes = vec![
            Node::store("out", Expr::u32(0), Expr::u32(1)),
            Node::let_bind("x", Expr::load("out", Expr::u32(0))),
        ];

        assert_eq!(
            detect_parallelism(&nodes),
            vec![
                DispatchGroup::Serial { node_index: 0 },
                DispatchGroup::Serial { node_index: 1 },
            ]
        );
    }

    /// A conflict only closes the current group; it does not make the next
    /// independent run permanently serial.
    #[test]
    pub fn conflict_starts_next_parallel_run() {
        let nodes = vec![
            Node::store("out", Expr::u32(0), Expr::u32(1)),
            Node::store("out", Expr::u32(1), Expr::u32(2)),
            Node::store("other", Expr::u32(0), Expr::u32(3)),
        ];

        assert_eq!(
            detect_parallelism(&nodes),
            vec![
                DispatchGroup::Serial { node_index: 0 },
                DispatchGroup::Parallel {
                    node_indices: vec![1, 2]
                },
            ]
        );
    }
}
