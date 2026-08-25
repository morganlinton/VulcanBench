use crate::{dual_impls::common, workgroup::Memory};
use vyre_primitives::Scatter;

impl common::ReferenceEvaluator for Scatter {
    fn evaluate(&self, inputs: &[Memory]) -> Result<Memory, common::EvalError> {
        let (values, indices) = common::two_inputs(inputs, "scatter")?;
        let values = common::u32_words(values, "scatter")?;
        let indices = common::u32_words(indices, "scatter")?;
        if values.len() != indices.len() {
            return Err(common::EvalError::new(format!(
                "primitive `scatter` expected equal value/index counts, got {} and {}. Fix: make scatter inputs the same length.",
                values.len(),
                indices.len()
            )));
        }
        let max_index = indices.iter().copied().max().unwrap_or(0);
        let len = usize::try_from(max_index).map_err(|_| {
            common::EvalError::new(
                "primitive `scatter` max index does not fit usize. Fix: keep scatter indices addressable.",
            )
        })?;
        let mut output = vec![0; len.saturating_add(1)];
        for (value, index) in values.into_iter().zip(indices) {
            let slot = usize::try_from(index).map_err(|_| {
                common::EvalError::new(
                    "primitive `scatter` index does not fit usize. Fix: keep scatter indices addressable.",
                )
            })?;
            output[slot] = value;
        }
        Ok(common::write_u32s(output))
    }
}
