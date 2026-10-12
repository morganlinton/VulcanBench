use crate::{dual_impls::common, workgroup::Memory};
use vyre_primitives::HashBlake3;

impl common::ReferenceEvaluator for HashBlake3 {
    fn evaluate(&self, inputs: &[Memory]) -> Result<Memory, common::EvalError> {
        let input = common::one_input(inputs, "hash_blake3")?;
        Ok(Memory::from_bytes(blake3::hash(&input).as_bytes().to_vec()))
    }
}
