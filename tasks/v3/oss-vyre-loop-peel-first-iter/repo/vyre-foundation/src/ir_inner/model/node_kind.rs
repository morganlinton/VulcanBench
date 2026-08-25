//! Open statement IR model.

use crate::ir_eval::canonical_f32;
use crate::ir_inner::model::types::{BinOp, UnOp};
use rustc_hash::FxHashMap;
use std::fmt;
use std::sync::Arc;

/// Canonical operation identifier used by capability negotiation.
pub type OpId = Arc<str>;

/// Stable node id for graph-shaped IR.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct NodeId(pub u32);

/// Stable variable id for graph-shaped IR.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct VarId(pub u32);

/// Stable memory-region id for graph-shaped IR.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct RegionId(pub u32);

/// Scalar value carried by the generic interpreter.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Value {
    /// Unsigned 32-bit integer.
    U32(u32),
    /// Unsigned 64-bit integer.
    U64(u64),
    /// Signed 32-bit integer.
    I32(i32),
    /// IEEE-754 binary32.
    F32(f32),
    /// Boolean predicate.
    Bool(bool),
}

/// Generic interpreter failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvalError {
    message: String,
}

impl EvalError {
    /// Construct an actionable evaluator error.
    #[must_use]
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    /// Return the diagnostic message.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for EvalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for EvalError {}

/// Mutable context used by generic node interpreters.
#[derive(Debug, Default)]
pub struct InterpCtx {
    values: FxHashMap<NodeId, Value>,
    operands: Vec<NodeId>,
    regions: FxHashMap<RegionId, Vec<u8>>,
}

impl InterpCtx {
    /// Store a node result.
    pub fn set(&mut self, id: NodeId, value: Value) {
        self.values.insert(id, value);
    }

    /// Read a previously computed node result.
    ///
    /// # Errors
    ///
    /// Returns [`EvalError`] when `id` has not been produced in this context.
    pub fn get(&self, id: NodeId) -> Result<Value, EvalError> {
        self.values.get(&id).copied().ok_or_else(|| {
            EvalError::new(format!(
                "missing interpreter value for node {}. Fix: topologically sort the program before interpretation and ensure every operand node runs before its users.",
                id.0
            ))
        })
    }

    /// Set the node operands visible to the primitive currently being interpreted.
    pub fn set_operands<I>(&mut self, operands: I)
    where
        I: IntoIterator<Item = NodeId>,
    {
        self.operands.clear();
        self.operands.extend(operands);
    }

    /// Return the operand ids visible to the primitive currently being interpreted.
    #[must_use]
    pub fn operands(&self) -> &[NodeId] {
        &self.operands
    }

    /// Read an operand value by position.
    ///
    /// # Errors
    ///
    /// Returns [`EvalError`] when the operand index is absent or the referenced
    /// node has not produced a value.
    pub fn operand(&self, index: usize) -> Result<Value, EvalError> {
        let id = self.operands.get(index).copied().ok_or_else(|| {
            EvalError::new(format!(
                "missing operand {index}. Fix: bind the primitive with the arity declared by its metadata before interpretation."
            ))
        })?;
        self.get(id)
    }

    /// Store a byte region used by region-oriented primitives.
    pub fn set_region(&mut self, id: RegionId, bytes: Vec<u8>) {
        self.regions.insert(id, bytes);
    }

    /// Read a byte region used by region-oriented primitives.
    ///
    /// # Errors
    ///
    /// Returns [`EvalError`] when `id` has no initialized byte region.
    pub fn region(&self, id: RegionId) -> Result<&[u8], EvalError> {
        self.regions.get(&id).map(Vec::as_slice).ok_or_else(|| {
            EvalError::new(format!(
                "missing interpreter region {}. Fix: initialize every primitive input region before reference execution.",
                id.0
            ))
        })
    }

    /// Mutably read a byte region used by region-oriented primitives.
    ///
    /// # Errors
    ///
    /// Returns [`EvalError`] when `id` has no initialized mutable byte region.
    pub fn region_mut(&mut self, id: RegionId) -> Result<&mut Vec<u8>, EvalError> {
        self.regions.get_mut(&id).ok_or_else(|| {
            EvalError::new(format!(
                "missing mutable interpreter region {}. Fix: allocate primitive output regions before reference execution.",
                id.0
            ))
        })
    }
}

/// Compact storage for hot-path nodes plus an extension escape hatch.
#[derive(Debug, Clone)]
pub enum NodeStorage {
    /// Literal unsigned 32-bit integer.
    LitU32(u32),
    /// Literal unsigned 64-bit integer.
    LitU64(u64),
    /// Literal signed 32-bit integer.
    LitI32(i32),
    /// Literal floating-point value.
    LitF32(f32),
    /// Literal boolean.
    LitBool(bool),
    /// Binary operation over two prior node values.
    BinOp {
        /// Operator.
        op: BinOp,
        /// Left operand id.
        left: NodeId,
        /// Right operand id.
        right: NodeId,
    },
    /// Unary operation over one prior node value.
    UnOp {
        /// Operator.
        op: UnOp,
        /// Operand id.
        operand: NodeId,
    },
    /// Extension node stored by stable operation id and opaque payload.
    Extern {
        /// Stable operation id.
        op_id: OpId,
        /// Operand node ids.
        operands: Arc<[NodeId]>,
        /// Stable wire payload for this extension.
        payload: Arc<[u8]>,
    },
}

impl NodeStorage {
    /// Return node dependencies in storage order.
    #[must_use]
    pub fn input_ids(&self) -> Vec<NodeId> {
        match self {
            Self::BinOp { left, right, .. } => vec![*left, *right],
            Self::UnOp { operand, .. } => vec![*operand],
            Self::Extern { operands, .. } => operands.iter().copied().collect(),
            Self::LitU32(_)
            | Self::LitU64(_)
            | Self::LitI32(_)
            | Self::LitF32(_)
            | Self::LitBool(_) => Vec::new(),
        }
    }

    /// Interpret this storage node without side effects.
    ///
    /// # Errors
    ///
    /// Returns [`EvalError`] when operands are missing or the operation has no
    /// registered reference semantics.
    pub fn interpret(&self, ctx: &mut InterpCtx) -> Result<Value, EvalError> {
        match self {
            Self::LitU32(value) => Ok(Value::U32(*value)),
            Self::LitU64(value) => Ok(Value::U64(*value)),
            Self::LitI32(value) => Ok(Value::I32(*value)),
            Self::LitF32(value) => Ok(Value::F32(*value)),
            Self::LitBool(value) => Ok(Value::Bool(*value)),
            Self::BinOp { op, left, right } => {
                interpret_bin_op(*op, ctx.get(*left)?, ctx.get(*right)?)
            }
            Self::UnOp { op, operand } => interpret_un_op(op, ctx.get(*operand)?),
            Self::Extern { op_id, .. } => Err(EvalError::new(format!(
                "extern node `{op_id}` has no linked interpreter. Fix: link the primitive crate that registered this op or lower it to a hot NodeStorage variant before reference execution."
            ))),
        }
    }
}

#[expect(
    clippy::too_many_lines,
    reason = "reference interpreter keeps each BinOp semantics in one exhaustive table"
)]
fn interpret_bin_op(op: BinOp, left: Value, right: Value) -> Result<Value, EvalError> {
    match (left, right) {
        (Value::U32(left), Value::U32(right)) => match op {
            BinOp::Add => Ok(Value::U32(left.wrapping_add(right))),
            BinOp::Sub => Ok(Value::U32(left.wrapping_sub(right))),
            BinOp::Mul => Ok(Value::U32(left.wrapping_mul(right))),
            BinOp::Div => {
                Ok(Value::U32(left.checked_div(right).unwrap_or(u32::MAX)))
            }
            BinOp::Mod => {
                Ok(Value::U32(left.checked_rem(right).unwrap_or(0)))
            }
            BinOp::BitAnd => Ok(Value::U32(left & right)),
            BinOp::BitOr => Ok(Value::U32(left | right)),
            BinOp::BitXor => Ok(Value::U32(left ^ right)),
            BinOp::Shl => Ok(Value::U32(left.wrapping_shl(right & 31))),
            BinOp::Shr => Ok(Value::U32(left.wrapping_shr(right & 31))),
            BinOp::Eq => Ok(Value::Bool(left == right)),
            BinOp::Ne => Ok(Value::Bool(left != right)),
            BinOp::Lt => Ok(Value::Bool(left < right)),
            BinOp::Le => Ok(Value::Bool(left <= right)),
            BinOp::Gt => Ok(Value::Bool(left > right)),
            BinOp::Ge => Ok(Value::Bool(left >= right)),
            BinOp::Min => Ok(Value::U32(left.min(right))),
            BinOp::Max => Ok(Value::U32(left.max(right))),
            BinOp::SaturatingAdd => Ok(Value::U32(left.saturating_add(right))),
            BinOp::SaturatingSub => Ok(Value::U32(left.saturating_sub(right))),
            BinOp::SaturatingMul => Ok(Value::U32(left.saturating_mul(right))),
            BinOp::AbsDiff => Ok(Value::U32(left.abs_diff(right))),
            BinOp::RotateLeft => Ok(Value::U32(left.rotate_left(right & 31))),
            BinOp::RotateRight => Ok(Value::U32(left.rotate_right(right & 31))),
            BinOp::MulHigh => Ok(Value::U32(u32::try_from((u64::from(left).wrapping_mul(u64::from(right))) >> 32).unwrap_or(u32::MAX))),
            BinOp::And => Ok(Value::Bool(left != 0 && right != 0)),
            BinOp::Or => Ok(Value::Bool(left != 0 || right != 0)),
            _ => Err(EvalError::new(format!(
                "unsupported u32 binary operation {op:?}. Fix: add interpreter semantics before registering this operation."
            ))),
        },
        (Value::U64(left), Value::U64(right)) => match op {
            BinOp::Add => Ok(Value::U64(left.wrapping_add(right))),
            BinOp::Sub => Ok(Value::U64(left.wrapping_sub(right))),
            BinOp::Mul => Ok(Value::U64(left.wrapping_mul(right))),
            BinOp::Div => Ok(Value::U64(if right == 0 {
                u64::MAX
            } else {
                left / right
            })),
            BinOp::Mod => Ok(Value::U64(if right == 0 { 0 } else { left % right })),
            BinOp::BitAnd => Ok(Value::U64(left & right)),
            BinOp::BitOr => Ok(Value::U64(left | right)),
            BinOp::BitXor => Ok(Value::U64(left ^ right)),
            BinOp::Shl => Ok(Value::U64(left.wrapping_shl((right & 63) as u32))),
            BinOp::Shr => Ok(Value::U64(left.wrapping_shr((right & 63) as u32))),
            BinOp::Eq => Ok(Value::Bool(left == right)),
            BinOp::Ne => Ok(Value::Bool(left != right)),
            BinOp::Lt => Ok(Value::Bool(left < right)),
            BinOp::Le => Ok(Value::Bool(left <= right)),
            BinOp::Gt => Ok(Value::Bool(left > right)),
            BinOp::Ge => Ok(Value::Bool(left >= right)),
            BinOp::Min => Ok(Value::U64(left.min(right))),
            BinOp::Max => Ok(Value::U64(left.max(right))),
            BinOp::SaturatingAdd => Ok(Value::U64(left.saturating_add(right))),
            BinOp::SaturatingSub => Ok(Value::U64(left.saturating_sub(right))),
            BinOp::SaturatingMul => Ok(Value::U64(left.saturating_mul(right))),
            BinOp::AbsDiff => Ok(Value::U64(left.abs_diff(right))),
            BinOp::WrappingAdd => Ok(Value::U64(left.wrapping_add(right))),
            BinOp::WrappingSub => Ok(Value::U64(left.wrapping_sub(right))),
            BinOp::MulHigh => Ok(Value::U64(((left as u128 * right as u128) >> 64) as u64)),
            BinOp::And => Ok(Value::Bool(left != 0 && right != 0)),
            BinOp::Or => Ok(Value::Bool(left != 0 || right != 0)),
            _ => Err(EvalError::new(format!(
                "unsupported u64 binary operation {op:?}. Fix: add interpreter semantics before registering this operation."
            ))),
        },
        (Value::Bool(left), Value::Bool(right)) => match op {
            BinOp::And => Ok(Value::Bool(left && right)),
            BinOp::Or => Ok(Value::Bool(left || right)),
            BinOp::Eq => Ok(Value::Bool(left == right)),
            BinOp::Ne => Ok(Value::Bool(left != right)),
            _ => Err(EvalError::new(format!(
                "unsupported bool binary operation {op:?}. Fix: add interpreter semantics before registering this operation."
            ))),
        },
        (Value::F32(left), Value::F32(right)) => {
            let left = canonical_f32(left);
            let right = canonical_f32(right);
            match op {
                BinOp::Add => Ok(Value::F32(canonical_f32(left + right))),
                BinOp::Sub => Ok(Value::F32(canonical_f32(left - right))),
                BinOp::Mul => Ok(Value::F32(canonical_f32(left * right))),
                BinOp::Div => Ok(Value::F32(canonical_f32(left / right))),
                BinOp::Eq => Ok(Value::Bool(left.partial_cmp(&right).is_some_and(std::cmp::Ordering::is_eq))),
                BinOp::Ne => Ok(Value::Bool(left.partial_cmp(&right).is_none_or(|ordering| !ordering.is_eq()))),
                BinOp::Lt => Ok(Value::Bool(left < right)),
                BinOp::Le => Ok(Value::Bool(left <= right)),
                BinOp::Gt => Ok(Value::Bool(left > right)),
                BinOp::Ge => Ok(Value::Bool(left >= right)),
                BinOp::Min => Ok(Value::F32(canonical_f32(left.min(right)))),
                BinOp::Max => Ok(Value::F32(canonical_f32(left.max(right)))),
                _ => Err(EvalError::new(format!(
                    "unsupported f32 binary operation {op:?}. Fix: add interpreter semantics before registering this operation."
                ))),
            }
        },
        (Value::I32(left), Value::I32(right)) => match op {
            BinOp::Add => Ok(Value::I32(left.wrapping_add(right))),
            BinOp::Sub => Ok(Value::I32(left.wrapping_sub(right))),
            BinOp::Mul => Ok(Value::I32(left.wrapping_mul(right))),
            BinOp::Div => {
                if right == 0 || (left == i32::MIN && right == -1) {
                    Err(undefined_i32_division("division", left, right))
                } else {
                    Ok(Value::I32(left / right))
                }
            }
            BinOp::Mod => {
                if right == 0 || (left == i32::MIN && right == -1) {
                    Err(undefined_i32_division("remainder", left, right))
                } else {
                    Ok(Value::I32(left % right))
                }
            }
            BinOp::BitAnd => Ok(Value::I32(left & right)),
            BinOp::BitOr => Ok(Value::I32(left | right)),
            BinOp::BitXor => Ok(Value::I32(left ^ right)),
            BinOp::Shl => Ok(Value::I32(left.wrapping_shl(u32::from_ne_bytes(right.to_ne_bytes()) & 31))),
            BinOp::Shr => Ok(Value::I32(left.wrapping_shr(u32::from_ne_bytes(right.to_ne_bytes()) & 31))),
            BinOp::Eq => Ok(Value::Bool(left == right)),
            BinOp::Ne => Ok(Value::Bool(left != right)),
            BinOp::Lt => Ok(Value::Bool(left < right)),
            BinOp::Le => Ok(Value::Bool(left <= right)),
            BinOp::Gt => Ok(Value::Bool(left > right)),
            BinOp::Ge => Ok(Value::Bool(left >= right)),
            BinOp::Min => Ok(Value::I32(left.min(right))),
            BinOp::Max => Ok(Value::I32(left.max(right))),
            BinOp::SaturatingAdd => Ok(Value::I32(left.saturating_add(right))),
            BinOp::SaturatingSub => Ok(Value::I32(left.saturating_sub(right))),
            BinOp::SaturatingMul => Ok(Value::I32(left.saturating_mul(right))),
            _ => Err(EvalError::new(format!(
                "unsupported i32 binary operation {op:?}. Fix: add interpreter semantics before registering this operation."
            ))),
        },
        _ => Err(EvalError::new(
            "type mismatch in binary operation. Fix: validate operand types before interpretation.",
        )),
    }
}

fn undefined_i32_division(kind: &str, left: i32, right: i32) -> EvalError {
    EvalError::new(format!(
        "i32 {kind} `{left} / {right}` has undefined target-text semantics. Fix: guard the signed divisor/overflow case before interpretation, or use unsigned operands when zero-divisor semantics must be total."
    ))
}

fn interpret_un_op(op: &UnOp, operand: Value) -> Result<Value, EvalError> {
    match operand {
        Value::U32(value) => match op {
            UnOp::BitNot => Ok(Value::U32(!value)),
            UnOp::LogicalNot => Ok(Value::Bool(value == 0)),
            UnOp::Popcount => Ok(Value::U32(value.count_ones())),
            UnOp::Clz => Ok(Value::U32(value.leading_zeros())),
            UnOp::Ctz => Ok(Value::U32(value.trailing_zeros())),
            UnOp::ReverseBits => Ok(Value::U32(value.reverse_bits())),
            _ => Err(EvalError::new(format!(
                "unsupported u32 unary operation {op:?}. Fix: add interpreter semantics before registering this operation."
            ))),
        },
        Value::Bool(value) => match op {
            UnOp::LogicalNot => Ok(Value::Bool(!value)),
            _ => Err(EvalError::new(format!(
                "unsupported bool unary operation {op:?}. Fix: add interpreter semantics before registering this operation."
            ))),
        },
        Value::F32(value) => match op {
            UnOp::Negate => Ok(Value::F32(canonical_f32(-canonical_f32(value)))),
            UnOp::InverseSqrt => {
                let value = canonical_f32(value);
                Ok(Value::F32(canonical_f32(1.0 / value.sqrt())))
            }
            UnOp::Reciprocal => {
                let value = canonical_f32(value);
                Ok(Value::F32(canonical_f32(1.0 / value)))
            }
            _ => Err(EvalError::new(format!(
                "unsupported f32 unary operation {op:?}. Fix: add interpreter semantics before registering this operation."
            ))),
        },
        Value::I32(value) => match op {
            UnOp::Negate => Ok(Value::I32(value.wrapping_neg())),
            _ => Err(EvalError::new(format!(
                "unsupported i32 unary operation {op:?}. Fix: add interpreter semantics before registering this operation."
            ))),
        },
        Value::U64(value) => match op {
            UnOp::Negate => Ok(Value::U64(0u64.wrapping_sub(value))),
            UnOp::BitNot => Ok(Value::U64(!value)),
            UnOp::LogicalNot => Ok(Value::Bool(value == 0)),
            UnOp::Popcount => Ok(Value::U64(u64::from(value.count_ones()))),
            UnOp::Clz => Ok(Value::U64(u64::from(value.leading_zeros()))),
            UnOp::Ctz => Ok(Value::U64(u64::from(value.trailing_zeros()))),
            UnOp::ReverseBits => Ok(Value::U64(value.reverse_bits())),
            _ => Err(EvalError::new(format!(
                "unsupported u64 unary operation {op:?}. Fix: register explicit u64 semantics before interpreting this operation."
            ))),
        },
    }
}

#[cfg(test)]

mod tests {
    use super::*;

    fn eval_binary(op: BinOp, left: Value, right: Value) -> Result<Value, EvalError> {
        let mut ctx = InterpCtx::default();
        ctx.set(NodeId(0), left);
        ctx.set(NodeId(1), right);
        NodeStorage::BinOp {
            op,
            left: NodeId(0),
            right: NodeId(1),
        }
        .interpret(&mut ctx)
    }

    #[test]
    fn unsigned_zero_division_matches_reference_total_contract() {
        assert_eq!(
            eval_binary(BinOp::Div, Value::U32(9), Value::U32(0)).unwrap(),
            Value::U32(u32::MAX)
        );
        assert_eq!(
            eval_binary(BinOp::Mod, Value::U32(9), Value::U32(0)).unwrap(),
            Value::U32(0)
        );
    }

    #[test]
    fn signed_undefined_division_returns_errors() {
        for (op, left, right) in [
            (BinOp::Div, i32::MIN, -1),
            (BinOp::Mod, i32::MIN, -1),
            (BinOp::Div, 1, 0),
            (BinOp::Mod, 1, 0),
        ] {
            let error = eval_binary(op, Value::I32(left), Value::I32(right))
                .unwrap_err()
                .to_string();
            assert!(
                error.contains("undefined target-text semantics"),
                "unexpected error for {op:?}({left}, {right}): {error}"
            );
        }
    }

    #[test]
    fn f32_subnormal_results_are_canonicalized() {
        let result =
            eval_binary(BinOp::Div, Value::F32(f32::MIN_POSITIVE), Value::F32(2.0)).unwrap();
        assert!(matches!(result, Value::F32(value) if value.to_bits() == 0.0f32.to_bits()));
    }
}
