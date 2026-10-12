use crate::{dual_impls::common, workgroup::Memory};
use vyre_primitives::hash::fnv1a::fnv1a32;
use vyre_primitives::HashFnv1a;

impl common::ReferenceEvaluator for HashFnv1a {
    fn evaluate(&self, inputs: &[Memory]) -> Result<Memory, common::EvalError> {
        let input = common::one_input(inputs, "hash_fnv1a")?;
        let hash = fnv1a32(&input);
        Ok(Memory::from_bytes(hash.to_le_bytes().to_vec()))
    }
}
