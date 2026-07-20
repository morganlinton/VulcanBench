use crate::{dual_impls::common, workgroup::Memory};
use vyre_primitives::ShiftRight;

impl common::ReferenceEvaluator for ShiftRight {
    fn evaluate(&self, inputs: &[Memory]) -> Result<Memory, common::EvalError> {
        common::binary_u32_scalar(inputs, "shift_right", |left, right| left >> (right & 31))
    }
}
