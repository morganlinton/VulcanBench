//! Reverse-mode expression adjoint emission.

use crate::ir::{DataType, Expr, Node};

use super::{AdjointEnv, PullbackMap};
use crate::transform::autodiff::error::AutodiffError;
use crate::transform::autodiff::rules::{binop_adjoints, fma_adjoints, unop_adjoint};

pub(super) fn insert_pullback(
    pullbacks: &mut PullbackMap,
    next_pullback_id: &mut usize,
    expr: Expr,
) {
    let id = *next_pullback_id;
    *next_pullback_id = next_pullback_id.saturating_add(1);
    pullbacks.insert(id, expr);
}

/// Propagate adjoint through an expression tree, emitting accumulation nodes.
#[expect(
    clippy::too_many_lines,
    reason = "autodiff expression cases are kept in one exhaustive match so new IR expression variants are reviewed in one place"
)]
pub(super) fn emit_adjoint_expr(
    expr: &Expr,
    adjoint: &Expr,
    body: &mut Vec<Node>,
    env: &mut AdjointEnv,
) -> Result<(), AutodiffError> {
    match expr {
        // Leaf: variable reference.
        // Accumulate adjoint into the variable's adjoint accumulator.
        Expr::Var(name) => {
            let adj_var = env.ensure_adjoint_var(name.as_str());
            body.push(Node::Assign {
                name: adj_var.clone().into(),
                value: Expr::add(Expr::Var(adj_var.into()), adjoint.clone()),
            });
        }
        // Leaf: buffer load.
        // If this buffer is a tracked input, accumulate into its grad buffer.
        Expr::Load { buffer, index } => {
            let buf_name = buffer.as_str();
            if env.has_grad_buffer(buf_name) {
                if env.buffer_type(buf_name) != Some(DataType::F32) {
                    let source = env
                        .buffer_type(buf_name)
                        .as_ref()
                        .map(ToString::to_string)
                        .unwrap_or_else(|| "unknown".to_string());
                    return Err(AutodiffError::NotDifferentiable {
                        op: format!("Expr::Load({buf_name}: {source})"),
                        fix: "only f32 buffer loads can receive reverse-mode adjoints; cast-free integer/bool memory is a discrete path and needs an explicit differentiable relaxation"
                            .into(),
                    });
                }
                let grad_buf = format!("grad_{buf_name}");
                // Atomic add to handle multiple gradient contributions.
                body.push(Node::Store {
                    buffer: grad_buf.into(),
                    index: *index.clone(),
                    value: Expr::add(
                        Expr::Load {
                            buffer: format!("grad_{buf_name}").into(),
                            index: index.clone(),
                        },
                        adjoint.clone(),
                    ),
                });
            }
        }
        // Leaf: literal  -  zero gradient, nothing to propagate.
        Expr::LitF32(_)
        | Expr::LitU32(_)
        | Expr::LitI32(_)
        | Expr::LitBool(_)
        | Expr::InvocationId { .. }
        | Expr::WorkgroupId { .. }
        | Expr::LocalId { .. }
        | Expr::SubgroupLocalId
        | Expr::SubgroupSize
        | Expr::BufLen { .. } => {}
        // BinOp: apply chain rule.
        Expr::BinOp { op, left, right } => {
            let contribs = binop_adjoints(*op, left, right, adjoint)?;
            for contrib in contribs {
                emit_adjoint_expr(&contrib.child, &contrib.adjoint, body, env)?;
            }
        }
        // UnOp: apply chain rule.
        Expr::UnOp { op, operand } => {
            let contrib = unop_adjoint(op, operand, adjoint)?;
            emit_adjoint_expr(&contrib.child, &contrib.adjoint, body, env)?;
        }
        // Select: route adjoint to the taken branch.
        Expr::Select {
            cond,
            true_val,
            false_val,
        } => {
            let true_adj = Expr::Select {
                cond: cond.clone(),
                true_val: Box::new(adjoint.clone()),
                false_val: Box::new(Expr::f32(0.0)),
            };
            let false_adj = Expr::Select {
                cond: cond.clone(),
                true_val: Box::new(Expr::f32(0.0)),
                false_val: Box::new(adjoint.clone()),
            };
            emit_adjoint_expr(true_val, &true_adj, body, env)?;
            emit_adjoint_expr(false_val, &false_adj, body, env)?;
        }
        // Fma: a*b + c.
        Expr::Fma { a, b, c } => {
            let contribs = fma_adjoints(a, b, c, adjoint);
            for contrib in contribs {
                emit_adjoint_expr(&contrib.child, &contrib.adjoint, body, env)?;
            }
        }
        // Casts are only differentiable when they are an explicit f32 identity.
        // Integer/bool/precision-changing casts are quantization boundaries, not
        // smooth maps. Passing gradients through them silently corrupts results.
        Expr::Cast { target, value } => {
            let source = env.expr_type(value);
            if target == &DataType::F32 && source == Some(DataType::F32) {
                emit_adjoint_expr(value, adjoint, body, env)?;
            } else {
                let source = source
                    .as_ref()
                    .map(ToString::to_string)
                    .unwrap_or_else(|| "unknown".to_string());
                return Err(AutodiffError::NotDifferentiable {
                    op: format!("Expr::Cast({source} -> {target})"),
                    fix: "only f32-to-f32 identity casts preserve reverse-mode adjoints; keep integer/bool casts outside the gradient path or define an explicit differentiable relaxation"
                        .into(),
                });
            }
        }
        // Non-differentiable expression nodes.
        Expr::Call { op_id, .. } => {
            return Err(AutodiffError::NotDifferentiable {
                op: format!("Expr::Call({op_id})"),
                fix:
                    "inline the call before running autodiff, or register a derivative for this op"
                        .into(),
            });
        }
        Expr::Atomic { .. } => {
            return Err(AutodiffError::NotDifferentiable {
                op: "Expr::Atomic".into(),
                fix: "atomics are not differentiable; restructure to use non-atomic accumulation"
                    .into(),
            });
        }
        Expr::SubgroupBallot { .. } | Expr::SubgroupShuffle { .. } | Expr::SubgroupReduce { .. } => {
            return Err(AutodiffError::NotDifferentiable {
                op: format!("{expr:?}").chars().take(40).collect(),
                fix: "subgroup ops are not differentiable in the general case".into(),
            });
        }
        Expr::Opaque(_) => {
            return Err(AutodiffError::NotDifferentiable {
                op: "Expr::Opaque".into(),
                fix: "register a derivative rule for this opaque expression".into(),
            });
        }
    }
    Ok(())
}
