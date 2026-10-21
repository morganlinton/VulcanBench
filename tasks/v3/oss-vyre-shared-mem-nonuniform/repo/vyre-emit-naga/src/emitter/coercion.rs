//! Pre-emit `Expression::Binary` operand-type unification + the
//! `append_expr` shim every other emit path goes through. Plus the
//! tiny `emit_builtin_axis` / `emit_scalar_builtin` wrappers  -  kept
//! here because they're the simplest consumers of `append_expr`.

use naga::{BinaryOperator, Expression, Span, Statement};
use vyre_lower::KernelOp;

use super::BodyBuilder;
use crate::EmitError;

impl BodyBuilder<'_> {
    /// Pre-emit fix-up for `Expression::Binary` operand-type mismatches.
    /// Naga's `Subtract` / `Add` / etc. require both operands to share
    /// scalar kind. Source builders (and emit-naga's own inserted
    /// comparisons) sometimes hand mixed types in. Coerce the right
    /// operand to match the left when they differ  -  value-preserving
    /// for bool↔u32 (via select) and integer↔integer (via As).
    fn unify_binary_operand_types(&mut self, expr: Expression) -> Expression {
        if let Expression::Binary { op, left, right } = expr {
            let arithmetic_or_compare = matches!(
                op,
                BinaryOperator::Add
                    | BinaryOperator::Subtract
                    | BinaryOperator::Multiply
                    | BinaryOperator::Divide
                    | BinaryOperator::Modulo
                    | BinaryOperator::And
                    | BinaryOperator::ExclusiveOr
                    | BinaryOperator::InclusiveOr
                    | BinaryOperator::ShiftLeft
                    | BinaryOperator::ShiftRight
                    | BinaryOperator::Equal
                    | BinaryOperator::NotEqual
                    | BinaryOperator::Less
                    | BinaryOperator::LessEqual
                    | BinaryOperator::Greater
                    | BinaryOperator::GreaterEqual
            );
            if arithmetic_or_compare {
                let left_kind = self.scalar_kind_of_expression(left, 0);
                let right_kind = self.scalar_kind_of_expression(right, 0);
                // Asymmetric bool-rescue: if one side is Bool and the other
                // resolves to a non-Bool (or doesn't resolve at all), coerce
                // the Bool side to u32. Naga rejects mixed bool/numeric
                // operands on every arithmetic op.
                let bool_arith_invalid = matches!(
                    op,
                    BinaryOperator::Add
                        | BinaryOperator::Subtract
                        | BinaryOperator::Multiply
                        | BinaryOperator::Divide
                        | BinaryOperator::Modulo
                        | BinaryOperator::ShiftLeft
                        | BinaryOperator::ShiftRight
                );
                if bool_arith_invalid {
                    let left_is_bool = left_kind == Some(naga::ScalarKind::Bool);
                    let right_is_bool = right_kind == Some(naga::ScalarKind::Bool);
                    if left_is_bool && right_is_bool {
                        let new_left = self.coerce_value_to_type(left, self.types.u32_ty);
                        let new_right = self.coerce_value_to_type(right, self.types.u32_ty);
                        return Expression::Binary {
                            op,
                            left: new_left,
                            right: new_right,
                        };
                    }
                    if left_is_bool {
                        let new_left = self.coerce_value_to_type(left, self.types.u32_ty);
                        return Expression::Binary {
                            op,
                            left: new_left,
                            right,
                        };
                    }
                    if right_is_bool {
                        let new_right = self.coerce_value_to_type(right, self.types.u32_ty);
                        return Expression::Binary {
                            op,
                            left,
                            right: new_right,
                        };
                    }
                }
                // Shifts are ASYMMETRIC: `e1 >> e2` takes its result type from
                // the value `e1` (whose signedness selects arithmetic vs logical
                // shift) and REQUIRES the amount `e2` to be u32. The symmetric
                // `lk != rk` unification below would coerce the amount to the
                // value's type, emitting e.g. `ShiftRight(i32, i32)` — which naga
                // rejects (InvalidBinaryOperandTypes), so signed arithmetic
                // shifts (and signed-value rotates, whose synthetic lowering
                // reuses this path) could not be emitted at all. Coerce ONLY the
                // amount, ONLY to u32, and never touch the value's type.
                if matches!(
                    op,
                    BinaryOperator::ShiftLeft | BinaryOperator::ShiftRight
                ) {
                    if right_kind != Some(naga::ScalarKind::Uint) {
                        let new_right = self.coerce_value_to_type(right, self.types.u32_ty);
                        return Expression::Binary {
                            op,
                            left,
                            right: new_right,
                        };
                    }
                    return Expression::Binary { op, left, right };
                }
                if let (Some(lk), Some(rk)) = (left_kind, right_kind) {
                    if lk != rk {
                        let target = match lk {
                            naga::ScalarKind::Bool => self.types.bool_ty,
                            naga::ScalarKind::Sint => self.types.i32_ty,
                            naga::ScalarKind::Float => self.types.f32_ty,
                            _ => self.types.u32_ty,
                        };
                        let new_right = self.coerce_value_to_type(right, target);
                        return Expression::Binary {
                            op,
                            left,
                            right: new_right,
                        };
                    }
                }
            }
            return Expression::Binary { op, left, right };
        }
        expr
    }

    pub(super) fn append_expr(&mut self, expr: Expression) -> naga::Handle<Expression> {
        let expr = self.unify_binary_operand_types(expr);
        let needs_emit = !expr.needs_pre_emit();
        let handle = self.function.expressions.append(expr, Span::UNDEFINED);
        if needs_emit {
            self.function.body.push(
                Statement::Emit(naga::Range::new_from_bounds(handle, handle)),
                Span::UNDEFINED,
            );
        }
        handle
    }

    pub(super) fn emit_builtin_axis(
        &mut self,
        op: &KernelOp,
        arg_index: u32,
    ) -> Result<(), EmitError> {
        let axis = self.inline_axis(op)?;
        let base = self.append_expr(Expression::FunctionArgument(arg_index));
        let value = self.append_expr(Expression::AccessIndex { base, index: axis });
        self.bind_result_typed(op, value, self.types.u32_ty)
    }

    pub(super) fn emit_scalar_builtin(
        &mut self,
        op: &KernelOp,
        arg_index: Option<u32>,
        name: &str,
    ) -> Result<(), EmitError> {
        let arg_index = arg_index.ok_or_else(|| {
            EmitError::InvalidDescriptor(format!(
                "{name} requires subgroup builtins, but descriptor scan did not enable them"
            ))
        })?;
        let value = self.append_expr(Expression::FunctionArgument(arg_index));
        self.bind_result_typed(op, value, self.types.u32_ty)
    }
}
