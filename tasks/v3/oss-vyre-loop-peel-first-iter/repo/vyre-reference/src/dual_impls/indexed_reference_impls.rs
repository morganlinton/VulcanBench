use crate::{dual_impls::common, workgroup::Memory};
use vyre_primitives::{Gather, Shuffle};

macro_rules! impl_indexed_select_reference {
    ($type:ty, $name:literal) => {
        impl common::ReferenceEvaluator for $type {
            fn evaluate(&self, inputs: &[Memory]) -> Result<Memory, common::EvalError> {
                indexed_select_u32(inputs, $name)
            }
        }
    };
}

fn indexed_select_u32(
    inputs: &[Memory],
    op_name: &'static str,
) -> Result<Memory, common::EvalError> {
    let (values, indices) = common::two_inputs(inputs, op_name)?;
    let values = common::u32_words(values, op_name)?;
    let indices = common::u32_words(indices, op_name)?;
    let mut output = Vec::with_capacity(indices.len());
    for index in indices {
        output.push(values[common::checked_index(index, values.len(), op_name)?]);
    }
    Ok(common::write_u32s(output))
}

impl_indexed_select_reference!(Gather, "gather");
impl_indexed_select_reference!(Shuffle, "shuffle");
