use crate::{dual_impls::common, workgroup::Memory};
use vyre_primitives::ShiftLeft;

impl common::ReferenceEvaluator for ShiftLeft {
    fn evaluate(&self, inputs: &[Memory]) -> Result<Memory, common::EvalError> {
        common::binary_u32_scalar(inputs, "shift_left", |left, right| left << (right & 31))
    }
}
