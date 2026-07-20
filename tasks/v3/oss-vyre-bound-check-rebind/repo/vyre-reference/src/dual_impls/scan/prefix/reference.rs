use crate::{dual_impls::common, workgroup::Memory};
use vyre_primitives::Scan;

impl common::ReferenceEvaluator for Scan {
    fn evaluate(&self, inputs: &[Memory]) -> Result<Memory, common::EvalError> {
        let words = common::u32_words(common::one_input(inputs, "scan")?, "scan")?;
        let mut iter = words.into_iter();
        let Some(first) = iter.next() else {
            return Ok(Memory::from_bytes(Vec::new()));
        };
        let mut acc = first;
        let mut output = vec![acc];
        for value in iter {
            acc = common::combine(self.combine, acc, value)?;
            output.push(acc);
        }
        Ok(common::write_u32s(output))
    }
}
