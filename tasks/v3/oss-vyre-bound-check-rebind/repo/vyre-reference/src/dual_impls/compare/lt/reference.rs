use crate::{dual_impls::common, workgroup::Memory};
use vyre_primitives::CompareLt;

impl common::ReferenceEvaluator for CompareLt {
    fn evaluate(&self, inputs: &[Memory]) -> Result<Memory, common::EvalError> {
        common::binary_u32_predicate(inputs, "compare_lt", |left, right| left < right)
    }
}
