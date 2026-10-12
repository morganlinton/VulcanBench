use crate::{dual_impls::common, workgroup::Memory};
use vyre_primitives::{ArithAdd, ArithMul, Clz, Popcount};

macro_rules! impl_binary_u32_reference {
    ($type:ty, $name:literal, $op:path) => {
        impl common::ReferenceEvaluator for $type {
            fn evaluate(&self, inputs: &[Memory]) -> Result<Memory, common::EvalError> {
                common::binary_u32_scalar(inputs, $name, $op)
            }
        }
    };
}

macro_rules! impl_unary_u32_reference {
    ($type:ty, $name:literal, $op:path) => {
        impl common::ReferenceEvaluator for $type {
            fn evaluate(&self, inputs: &[Memory]) -> Result<Memory, common::EvalError> {
                common::unary_u32_scalar(inputs, $name, $op)
            }
        }
    };
}

impl_binary_u32_reference!(ArithAdd, "arith_add", u32::wrapping_add);
impl_binary_u32_reference!(ArithMul, "arith_mul", u32::wrapping_mul);
impl_unary_u32_reference!(Clz, "clz", u32::leading_zeros);
impl_unary_u32_reference!(Popcount, "popcount", u32::count_ones);
