//! Subgroup operation lowering.

use naga::{Expression, Span, Statement};
use vyre_foundation::ir::SubgroupReduceOp;
use vyre_lower::{KernelOp, KernelOpKind};

use super::BodyBuilder;
use crate::EmitError;

/// Map a neutral [`SubgroupReduceOp`] to naga's collective operation.
fn naga_subgroup_operation(op: SubgroupReduceOp) -> Result<naga::SubgroupOperation, EmitError> {
    Ok(match op {
        SubgroupReduceOp::Add => naga::SubgroupOperation::Add,
        SubgroupReduceOp::Mul => naga::SubgroupOperation::Mul,
        SubgroupReduceOp::Min => naga::SubgroupOperation::Min,
        SubgroupReduceOp::Max => naga::SubgroupOperation::Max,
        SubgroupReduceOp::And => naga::SubgroupOperation::And,
        SubgroupReduceOp::Or => naga::SubgroupOperation::Or,
        SubgroupReduceOp::Xor => naga::SubgroupOperation::Xor,
        // SubgroupReduceOp is #[non_exhaustive]; a future operator with no
        // naga spelling fails loud rather than silently emitting Add.
        other => {
            return Err(EmitError::InvalidDescriptor(format!(
                "subgroup reduction operator {other:?} has no naga SubgroupOperation. Fix: extend naga_subgroup_operation."
            )))
        }
    })
}

impl BodyBuilder<'_> {
    pub(super) fn emit_subgroup_ballot(&mut self, op: &KernelOp) -> Result<(), EmitError> {
        let cond_id = *op
            .operands
            .first()
            .ok_or_else(|| EmitError::InvalidDescriptor("SubgroupBallot missing cond".into()))?;
        let cond = self.values.get(&cond_id).copied().ok_or_else(|| {
            EmitError::InvalidDescriptor(format!(
                "SubgroupBallot cond id {cond_id} not yet emitted"
            ))
        })?;
        // naga's `SubgroupBallot` requires a `bool` predicate, but the IR cond
        // is commonly an integer mask (e.g. the `BinOp::Ballot` spelling stores
        // to a u32 and passes a u32 predicate). Coerce non-bool predicates to
        // bool with `cond != 0`; `append_expr` unifies the literal's type to the
        // operand, so this is correct for u32/i32 conds and a no-op for bool.
        let predicate = if self.is_bool_expression(cond) {
            cond
        } else {
            let zero = self.literal_u32(0);
            self.append_expr(Expression::Binary {
                op: naga::BinaryOperator::NotEqual,
                left: cond,
                right: zero,
            })
        };
        let result = self
            .function
            .expressions
            .append(Expression::SubgroupBallotResult, Span::UNDEFINED);
        self.function.body.push(
            Statement::SubgroupBallot {
                result,
                predicate: Some(predicate),
            },
            Span::UNDEFINED,
        );
        let result_id = op
            .result
            .ok_or_else(|| EmitError::InvalidDescriptor("SubgroupBallot missing result".into()))?;
        let target_ty = self
            .value_types
            .get(&result_id)
            .copied()
            .unwrap_or(self.types.u32_ty);
        if target_ty == self.types.u64_ty {
            let low = self.append_expr(Expression::AccessIndex {
                base: result,
                index: 0,
            });
            let high = self.append_expr(Expression::AccessIndex {
                base: result,
                index: 1,
            });
            let low_u64 = self.append_expr(Expression::As {
                expr: low,
                kind: naga::ScalarKind::Uint,
                convert: Some(8),
            });
            let high_u64 = self.append_expr(Expression::As {
                expr: high,
                kind: naga::ScalarKind::Uint,
                convert: Some(8),
            });
            let thirty_two = self.literal_u32(32);
            let thirty_two_u64 = self.append_expr(Expression::As {
                expr: thirty_two,
                kind: naga::ScalarKind::Uint,
                convert: Some(8),
            });
            let high_shl = self.append_expr(Expression::Binary {
                op: naga::BinaryOperator::ShiftLeft,
                left: high_u64,
                right: thirty_two_u64,
            });
            let final_u64 = self.append_expr(Expression::Binary {
                op: naga::BinaryOperator::InclusiveOr,
                left: low_u64,
                right: high_shl,
            });
            self.bind_result_typed(op, final_u64, self.types.u64_ty)
        } else {
            let first_word = self.append_expr(Expression::AccessIndex {
                base: result,
                index: 0,
            });
            self.bind_result_typed(op, first_word, self.types.u32_ty)
        }
    }

    pub(super) fn emit_subgroup_reduce(&mut self, op: &KernelOp) -> Result<(), EmitError> {
        let reduce_op = match &op.kind {
            KernelOpKind::SubgroupReduce { op: reduce_op } => naga_subgroup_operation(*reduce_op)?,
            other => {
                return Err(EmitError::InvalidDescriptor(format!(
                    "emit_subgroup_reduce dispatched on non-SubgroupReduce op {other:?}"
                )))
            }
        };
        let value_id = *op
            .operands
            .first()
            .ok_or_else(|| EmitError::InvalidDescriptor("SubgroupReduce missing value".into()))?;
        let argument = self.values.get(&value_id).copied().ok_or_else(|| {
            EmitError::InvalidDescriptor(format!(
                "SubgroupReduce value id {value_id} not yet emitted"
            ))
        })?;
        let result = self.function.expressions.append(
            Expression::SubgroupOperationResult {
                ty: self.value_type_operand(op, 0)?,
            },
            Span::UNDEFINED,
        );
        self.function.body.push(
            Statement::SubgroupCollectiveOperation {
                op: reduce_op,
                collective_op: naga::CollectiveOperation::Reduce,
                argument,
                result,
            },
            Span::UNDEFINED,
        );
        let ty = self.value_type_operand(op, 0)?;
        self.bind_result_typed(op, result, ty)
    }

    pub(super) fn emit_subgroup_shuffle(&mut self, op: &KernelOp) -> Result<(), EmitError> {
        let value_id = *op
            .operands
            .first()
            .ok_or_else(|| EmitError::InvalidDescriptor("SubgroupShuffle missing value".into()))?;
        let lane_id = *op
            .operands
            .get(1)
            .ok_or_else(|| EmitError::InvalidDescriptor("SubgroupShuffle missing lane".into()))?;
        let argument = self.values.get(&value_id).copied().ok_or_else(|| {
            EmitError::InvalidDescriptor(format!(
                "SubgroupShuffle value id {value_id} not yet emitted"
            ))
        })?;
        let lane = self.values.get(&lane_id).copied().ok_or_else(|| {
            EmitError::InvalidDescriptor(format!(
                "SubgroupShuffle lane id {lane_id} not yet emitted"
            ))
        })?;
        let result = self.function.expressions.append(
            Expression::SubgroupOperationResult {
                ty: self.value_type_operand(op, 0)?,
            },
            Span::UNDEFINED,
        );
        self.function.body.push(
            Statement::SubgroupGather {
                mode: naga::GatherMode::Shuffle(lane),
                argument,
                result,
            },
            Span::UNDEFINED,
        );
        let ty = self.value_type_operand(op, 0)?;
        self.bind_result_typed(op, result, ty)
    }

    pub(super) fn emit_subgroup_broadcast(&mut self, op: &KernelOp) -> Result<(), EmitError> {
        let value_id = *op.operands.first().ok_or_else(|| {
            EmitError::InvalidDescriptor("SubgroupBroadcast missing value".into())
        })?;
        let lane_id = *op.operands.get(1).ok_or_else(|| {
            EmitError::InvalidDescriptor("SubgroupBroadcast missing lane".into())
        })?;
        let argument = self.values.get(&value_id).copied().ok_or_else(|| {
            EmitError::InvalidDescriptor(format!(
                "SubgroupBroadcast value id {value_id} not yet emitted"
            ))
        })?;
        let lane = self.values.get(&lane_id).copied().ok_or_else(|| {
            EmitError::InvalidDescriptor(format!(
                "SubgroupBroadcast lane id {lane_id} not yet emitted"
            ))
        })?;
        let result = self.function.expressions.append(
            Expression::SubgroupOperationResult {
                ty: self.value_type_operand(op, 0)?,
            },
            Span::UNDEFINED,
        );
        // Broadcast = gather from one uniform source lane (vs Shuffle's per-lane
        // source). naga lowers GatherMode::Broadcast to `subgroupBroadcast`.
        self.function.body.push(
            Statement::SubgroupGather {
                mode: naga::GatherMode::Broadcast(lane),
                argument,
                result,
            },
            Span::UNDEFINED,
        );
        let ty = self.value_type_operand(op, 0)?;
        self.bind_result_typed(op, result, ty)
    }
}
