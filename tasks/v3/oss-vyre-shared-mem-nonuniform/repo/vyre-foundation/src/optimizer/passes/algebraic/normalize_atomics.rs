//! Atomic-normalization pass.
//!
//! Some backend emitters cannot lower an atomic expression directly inside a
//! branch predicate. This pass rewrites:
//!
//! ```text
//! If(Atomic(...), then, else)
//! ```
//!
//! into:
//!
//! ```text
//! Let __vyre_atomic_cond_N = Atomic(...);
//! If(__vyre_atomic_cond_N, then, else)
//! ```

use crate::ir::{Expr, Ident, Node, Program};
use crate::optimizer::{vyre_pass, PassAnalysis, PassResult};
use smallvec::SmallVec;

#[vyre_pass(
    name = "normalize_atomics",
    requires = [],
    invalidates = ["fusion"],
    phase = "sync",
    boundary_class = "abi_preserving",
    cost_model_family = "sync"
)]
/// Optimizer pass that hoists branch-condition atomics into explicit lets.
pub struct NormalizeAtomicsPass;

impl NormalizeAtomicsPass {
    /// Skip programs that do not contain atomics in `Node::If` conditions.
    fn analyze_impl(program: &Program) -> PassAnalysis {
        // Need both an If AND at least one atomic op anywhere. The
        // bitset already records If-presence; atomic_op_count is the
        // existing counter for atomic-presence on Expr-side.
        let stats = program.stats();
        if !stats.has_node_if() || stats.atomic_op_count == 0 {
            return PassAnalysis::SKIP;
        }
        if program.entry().iter().any(node_has_atomic_condition) {
            PassAnalysis::RUN
        } else {
            PassAnalysis::SKIP
        }
    }

    /// Hoist atomics out of branch conditions while preserving statement order.
    pub fn transform(program: Program) -> PassResult {
        let mut state = RewriteState::default();
        let program = program.map_entry(|entry| rewrite_nodes(entry, &mut state));
        PassResult {
            program,
            changed: state.changed,
        }
    }
}

#[derive(Default)]
struct RewriteState {
    next_temp: u32,
    changed: bool,
}

fn node_has_atomic_condition(node: &Node) -> bool {
    match node {
        Node::If {
            cond,
            then,
            otherwise,
        } => {
            expr_contains_atomic(cond)
                || then.iter().any(node_has_atomic_condition)
                || otherwise.iter().any(node_has_atomic_condition)
        }
        Node::Loop { body, .. } | Node::Block(body) => body.iter().any(node_has_atomic_condition),
        Node::Region { body, .. } => body.iter().any(node_has_atomic_condition),
        _ => false,
    }
}

fn expr_contains_atomic(expr: &Expr) -> bool {
    match expr {
        Expr::Atomic { .. } => true,
        Expr::Load { index, .. } => expr_contains_atomic(index),
        Expr::BinOp { left, right, .. } => {
            expr_contains_atomic(left) || expr_contains_atomic(right)
        }
        Expr::UnOp { operand, .. } => expr_contains_atomic(operand),
        Expr::Call { args, .. } => args.iter().any(expr_contains_atomic),
        Expr::Select {
            cond,
            true_val,
            false_val,
        } => {
            expr_contains_atomic(cond)
                || expr_contains_atomic(true_val)
                || expr_contains_atomic(false_val)
        }
        Expr::Cast { value, .. } | Expr::SubgroupReduce { value, .. } => expr_contains_atomic(value),
        Expr::Fma { a, b, c } => {
            expr_contains_atomic(a) || expr_contains_atomic(b) || expr_contains_atomic(c)
        }
        Expr::SubgroupBallot { cond } => expr_contains_atomic(cond),
        Expr::SubgroupShuffle { value, lane } => {
            expr_contains_atomic(value) || expr_contains_atomic(lane)
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
        | Expr::Opaque(_) => false,
    }
}

fn rewrite_nodes(nodes: Vec<Node>, state: &mut RewriteState) -> Vec<Node> {
    let mut out = Vec::with_capacity(nodes.len());
    for node in nodes {
        match node {
            Node::If {
                cond,
                then,
                otherwise,
            } => {
                let mut hoists = SmallVec::<[Node; 2]>::new();
                let cond = hoist_condition_atomics(cond, state, &mut hoists);
                out.extend(hoists);
                out.push(Node::If {
                    cond,
                    then: rewrite_nodes(then, state),
                    otherwise: rewrite_nodes(otherwise, state),
                });
            }
            Node::Loop {
                var,
                from,
                to,
                body,
            } => out.push(Node::Loop {
                var,
                from,
                to,
                body: rewrite_nodes(body, state),
            }),
            Node::Block(body) => out.push(Node::Block(rewrite_nodes(body, state))),
            Node::Region {
                generator,
                source_region,
                body,
            } => {
                let body_vec = match std::sync::Arc::try_unwrap(body) {
                    Ok(body) => body,
                    Err(body) => (*body).clone(),
                };
                out.push(Node::Region {
                    generator,
                    source_region,
                    body: std::sync::Arc::new(rewrite_nodes(body_vec, state)),
                });
            }
            other => out.push(other),
        }
    }
    out
}

fn hoist_condition_atomics(
    expr: Expr,
    state: &mut RewriteState,
    hoists: &mut SmallVec<[Node; 2]>,
) -> Expr {
    match expr {
        Expr::Atomic {
            op,
            buffer,
            index,
            expected,
            value,
            ordering,
        } => {
            let atomic = Expr::Atomic {
                op,
                buffer,
                index: Box::new(hoist_condition_atomics(*index, state, hoists)),
                expected: expected
                    .map(|expr| Box::new(hoist_condition_atomics(*expr, state, hoists))),
                value: Box::new(hoist_condition_atomics(*value, state, hoists)),
                ordering,
            };
            let temp = Ident::from(format!("__vyre_atomic_cond_{}", state.next_temp));
            state.next_temp += 1;
            state.changed = true;
            hoists.push(Node::Let {
                name: temp.clone(),
                value: atomic,
            });
            Expr::Var(temp)
        }
        Expr::Load { buffer, index } => Expr::Load {
            buffer,
            index: Box::new(hoist_condition_atomics(*index, state, hoists)),
        },
        Expr::BinOp { op, left, right } => Expr::BinOp {
            op,
            left: Box::new(hoist_condition_atomics(*left, state, hoists)),
            right: Box::new(hoist_condition_atomics(*right, state, hoists)),
        },
        Expr::UnOp { op, operand } => Expr::UnOp {
            op,
            operand: Box::new(hoist_condition_atomics(*operand, state, hoists)),
        },
        Expr::Call { op_id, args } => Expr::Call {
            op_id,
            args: args
                .into_iter()
                .map(|arg| hoist_condition_atomics(arg, state, hoists))
                .collect(),
        },
        Expr::Select {
            cond,
            true_val,
            false_val,
        } => Expr::Select {
            cond: Box::new(hoist_condition_atomics(*cond, state, hoists)),
            true_val: Box::new(hoist_condition_atomics(*true_val, state, hoists)),
            false_val: Box::new(hoist_condition_atomics(*false_val, state, hoists)),
        },
        Expr::Cast { target, value } => Expr::Cast {
            target,
            value: Box::new(hoist_condition_atomics(*value, state, hoists)),
        },
        Expr::Fma { a, b, c } => Expr::Fma {
            a: Box::new(hoist_condition_atomics(*a, state, hoists)),
            b: Box::new(hoist_condition_atomics(*b, state, hoists)),
            c: Box::new(hoist_condition_atomics(*c, state, hoists)),
        },
        Expr::SubgroupBallot { cond } => Expr::SubgroupBallot {
            cond: Box::new(hoist_condition_atomics(*cond, state, hoists)),
        },
        Expr::SubgroupShuffle { value, lane } => Expr::SubgroupShuffle {
            value: Box::new(hoist_condition_atomics(*value, state, hoists)),
            lane: Box::new(hoist_condition_atomics(*lane, state, hoists)),
        },
        Expr::SubgroupReduce { op, value } => Expr::SubgroupReduce { op,
            value: Box::new(hoist_condition_atomics(*value, state, hoists)),
        },
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{AtomicOp, BufferAccess, BufferDecl, DataType};

    fn atomic_cond_program() -> Program {
        Program::wrapped(
            vec![
                BufferDecl::storage("state", 0, BufferAccess::ReadWrite, DataType::U32)
                    .with_count(1),
            ],
            [1, 1, 1],
            vec![Node::If {
                cond: Expr::Atomic {
                    op: AtomicOp::Exchange,
                    buffer: Ident::from("state"),
                    index: Box::new(Expr::u32(0)),
                    expected: None,
                    value: Box::new(Expr::u32(1)),
                    ordering: crate::MemoryOrdering::SeqCst,
                },
                then: vec![Node::store("state", Expr::u32(0), Expr::u32(2))],
                otherwise: Vec::new(),
            }],
        )
    }

    #[test]
    fn analyze_runs_only_when_if_condition_contains_atomic() {
        assert_eq!(
            crate::optimizer::ProgramPass::analyze(&NormalizeAtomicsPass, &atomic_cond_program()),
            PassAnalysis::RUN
        );

        let program = Program::wrapped(Vec::new(), [1, 1, 1], Vec::new());
        assert_eq!(
            crate::optimizer::ProgramPass::analyze(&NormalizeAtomicsPass, &program),
            PassAnalysis::SKIP
        );
    }

    #[test]
    fn transform_hoists_atomic_condition_before_if() {
        let result = NormalizeAtomicsPass::transform(atomic_cond_program());
        assert!(result.changed);

        let entry = result.program.entry();
        assert_eq!(entry.len(), 1);
        let Node::Region { body, .. } = &entry[0] else {
            panic!("Fix: Program::wrapped must preserve a top-level region");
        };
        assert_eq!(body.len(), 2);
        assert!(matches!(
            &body[0],
            Node::Let {
                value: Expr::Atomic { .. },
                ..
            }
        ));
        assert!(matches!(
            &body[1],
            Node::If {
                cond: Expr::Var(_),
                ..
            }
        ));
    }
}
