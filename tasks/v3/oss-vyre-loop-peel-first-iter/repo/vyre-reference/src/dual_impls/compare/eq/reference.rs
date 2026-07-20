use crate::{dual_impls::common, workgroup::Memory};
use vyre_primitives::CompareEq;

impl common::ReferenceEvaluator for CompareEq {
    fn evaluate(&self, inputs: &[Memory]) -> Result<Memory, common::EvalError> {
        common::binary_u32_predicate(inputs, "compare_eq", |left, right| left == right)
    }
}
