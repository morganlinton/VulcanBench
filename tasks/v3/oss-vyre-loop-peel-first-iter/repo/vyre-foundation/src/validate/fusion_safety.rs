//! Fusion-aware buffer hazard checks.
//!
//! Single-node validation knows whether one atomic expression is well-typed,
//! but it cannot see hazards introduced when independently valid nodes are
//! fused into the same kernel. This pass walks node sequences and rejects
//! mixed atomic / non-atomic access to the same buffer unless an explicit
//! `Node::Barrier { ordering: vyre::memory_model::MemoryOrdering::SeqCst }` separates them.

use crate::ir::Expr;
use crate::ir::Ident;
#[cfg(test)]
use crate::ir::Node;
#[cfg(test)]
use crate::validate::{err, ValidationError};
use rustc_hash::FxHashSet;
use smallvec::SmallVec;

#[derive(Debug, Default)]
pub(crate) struct NodeAccesses {
    // PERF: uses `Ident` (Arc<str> + pre-hashed) instead of `String`.
    // Cloning an Ident is a single atomic refcount bump (~1ns) versus
    // a heap allocation + memcpy (~30-80ns per String). On programs
    // with 1000+ buffer accesses this is a measurable win.
    pub(crate) read_buffers: FxHashSet<Ident>,
    pub(crate) atomic_buffers: FxHashSet<Ident>,
}

/// Validate fusion hazards caused by mixing non-atomic reads and atomic writes.
#[cfg(test)]
pub(crate) fn validate_fusion_alias_hazards(nodes: &[Node], errors: &mut Vec<ValidationError>) {
    validate_sequence(nodes, errors);
}

#[cfg(test)]
fn validate_sequence(nodes: &[Node], errors: &mut Vec<ValidationError>) {
    let mut reads_since_barrier = FxHashSet::<Ident>::default();
    let mut atomics_since_barrier = FxHashSet::<Ident>::default();

    for node in nodes {
        match node {
            Node::Barrier { .. } => {
                reads_since_barrier.clear();
                atomics_since_barrier.clear();
            }
            Node::If {
                cond,
                then,
                otherwise,
            } => {
                let mut accesses = NodeAccesses::default();
                collect_expr_accesses(cond, &mut accesses);
                report_alias_hazards(
                    &accesses,
                    &reads_since_barrier,
                    &atomics_since_barrier,
                    errors,
                );
                validate_sequence(then, errors);
                validate_sequence(otherwise, errors);
                reads_since_barrier.extend(accesses.read_buffers);
                atomics_since_barrier.extend(accesses.atomic_buffers);
            }
            Node::Loop { from, to, body, .. } => {
                let mut accesses = NodeAccesses::default();
                collect_expr_accesses(from, &mut accesses);
                collect_expr_accesses(to, &mut accesses);
                report_alias_hazards(
                    &accesses,
                    &reads_since_barrier,
                    &atomics_since_barrier,
                    errors,
                );
                validate_sequence(body, errors);
                reads_since_barrier.extend(accesses.read_buffers);
                atomics_since_barrier.extend(accesses.atomic_buffers);
            }
            Node::Block(body) => {
                validate_sequence(body, errors);
            }
            Node::Region { body, .. } => {
                validate_sequence(body, errors);
            }
            _ => {
                let mut accesses = NodeAccesses::default();
                collect_node_accesses(node, &mut accesses);
                report_alias_hazards(
                    &accesses,
                    &reads_since_barrier,
                    &atomics_since_barrier,
                    errors,
                );
                reads_since_barrier.extend(accesses.read_buffers);
                atomics_since_barrier.extend(accesses.atomic_buffers);
            }
        }
    }
}

#[cfg(test)]
fn report_alias_hazards(
    accesses: &NodeAccesses,
    reads_since_barrier: &FxHashSet<Ident>,
    atomics_since_barrier: &FxHashSet<Ident>,
    errors: &mut Vec<ValidationError>,
) {
    let mut hazards = accesses
        .atomic_buffers
        .intersection(reads_since_barrier)
        .cloned()
        .collect::<Vec<_>>();
    hazards.extend(
        accesses
            .read_buffers
            .intersection(atomics_since_barrier)
            .cloned(),
    );
    // Sort by string content for deterministic error ordering.
    // Hazards are deduped immediately below, so unstable is fine.
    hazards.sort_unstable_by(|a, b| a.as_str().cmp(b.as_str()));
    hazards.dedup();

    for buffer in hazards {
        errors.push(err(format!(
            "fusion hazard on buffer `{buffer}`: one node reads it non-atomically while another issues an atomic access without an explicit barrier. Fix: insert `Node::barrier()` between the read path and the atomic path, or rename the buffers before fusion."
        )));
    }
}

#[cfg(test)]
pub(crate) fn collect_node_accesses(node: &Node, accesses: &mut NodeAccesses) {
    match node {
        Node::Let { value, .. } | Node::Assign { value, .. } => {
            collect_expr_accesses(value, accesses);
        }
        Node::Store {
            buffer,
            index,
            value,
        } => {
            accesses.read_buffers.insert(buffer.clone());
            collect_expr_accesses(index, accesses);
            collect_expr_accesses(value, accesses);
        }
        Node::IndirectDispatch { count_buffer, .. } => {
            accesses.read_buffers.insert(count_buffer.clone());
        }
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
            accesses.read_buffers.insert(source.clone());
            accesses.read_buffers.insert(destination.clone());
            collect_expr_accesses(offset, accesses);
            collect_expr_accesses(size, accesses);
        }
        Node::If {
            cond,
            then,
            otherwise,
        } => {
            collect_expr_accesses(cond, accesses);
            collect_node_sequence_accesses(then, accesses);
            collect_node_sequence_accesses(otherwise, accesses);
        }
        Node::Loop { from, to, body, .. } => {
            collect_expr_accesses(from, accesses);
            collect_expr_accesses(to, accesses);
            collect_node_sequence_accesses(body, accesses);
        }
        Node::Block(body) => {
            collect_node_sequence_accesses(body, accesses);
        }
        Node::Region { body, .. } => {
            collect_node_sequence_accesses(body, accesses);
        }
        Node::AllReduce { buffer, .. } | Node::Broadcast { buffer, .. } => {
            accesses.read_buffers.insert(buffer.clone());
        }
        Node::AllGather { input, output, .. } | Node::ReduceScatter { input, output, .. } => {
            accesses.read_buffers.insert(input.clone());
            accesses.read_buffers.insert(output.clone());
        }
        Node::Trap { .. }
        | Node::Resume { .. }
        | Node::Return
        | Node::Barrier { .. }
        | Node::Opaque(_) => {}
        Node::AsyncWait { .. } => {}
    }
}

#[cfg(test)]
fn collect_node_sequence_accesses(nodes: &[Node], accesses: &mut NodeAccesses) {
    for node in nodes {
        collect_node_accesses(node, accesses);
    }
}

pub(crate) fn collect_expr_accesses(expr: &Expr, accesses: &mut NodeAccesses) {
    let mut stack: SmallVec<[&Expr; 32]> = SmallVec::new();
    stack.push(expr);
    while let Some(expr) = stack.pop() {
        match expr {
            Expr::Load { buffer, index } => {
                accesses.read_buffers.insert(buffer.clone());
                stack.push(index);
            }
            Expr::BufLen { buffer } => {
                accesses.read_buffers.insert(buffer.clone());
            }
            Expr::Atomic {
                buffer,
                index,
                expected,
                value,
                ..
            } => {
                accesses.atomic_buffers.insert(buffer.clone());
                stack.push(value);
                if let Some(expected) = expected {
                    stack.push(expected);
                }
                stack.push(index);
            }
            Expr::BinOp { left, right, .. } => {
                stack.push(right);
                stack.push(left);
            }
            Expr::UnOp { operand, .. } | Expr::Cast { value: operand, .. } => {
                stack.push(operand);
            }
            Expr::Call { args, .. } => {
                stack.extend(args.iter());
            }
            Expr::Fma { a, b, c } => {
                stack.push(c);
                stack.push(b);
                stack.push(a);
            }
            Expr::Select {
                cond,
                true_val,
                false_val,
            } => {
                stack.push(false_val);
                stack.push(true_val);
                stack.push(cond);
            }
            Expr::SubgroupBallot { cond } => stack.push(cond),
            Expr::SubgroupShuffle { value, lane } => {
                stack.push(lane);
                stack.push(value);
            }
            Expr::SubgroupReduce { value, .. } => stack.push(value),
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
            | Expr::Opaque(_) => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{BufferAccess, BufferDecl, DataType, Program};

    fn validate(program: &Program) -> Vec<ValidationError> {
        let mut errors = Vec::new();
        validate_fusion_alias_hazards(program.entry(), &mut errors);
        errors
    }

    #[test]
    fn atomic_after_plain_read_requires_barrier() {
        let program = Program::wrapped(
            vec![BufferDecl::storage(
                "state",
                0,
                BufferAccess::ReadWrite,
                DataType::U32,
            )],
            [1, 1, 1],
            vec![
                Node::let_bind("plain", Expr::load("state", Expr::u32(0))),
                Node::let_bind(
                    "atomic_old",
                    Expr::atomic_add("state", Expr::u32(0), Expr::u32(1)),
                ),
            ],
        );

        let errors = validate(&program);
        assert!(errors
            .iter()
            .any(|error| error.message.contains("fusion hazard on buffer `state`")));
    }

    #[test]
    fn barrier_clears_atomic_plain_alias_hazard() {
        let program = Program::wrapped(
            vec![BufferDecl::storage(
                "state",
                0,
                BufferAccess::ReadWrite,
                DataType::U32,
            )],
            [1, 1, 1],
            vec![
                Node::let_bind("plain", Expr::load("state", Expr::u32(0))),
                Node::barrier(),
                Node::let_bind(
                    "atomic_old",
                    Expr::atomic_add("state", Expr::u32(0), Expr::u32(1)),
                ),
            ],
        );

        assert!(validate(&program).is_empty());
    }
}
