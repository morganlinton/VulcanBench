use crate::{dual_impls::common, workgroup::Memory};
use vyre_primitives::PatternMatchLiteral;

impl common::ReferenceEvaluator for PatternMatchLiteral {
    fn evaluate(&self, inputs: &[Memory]) -> Result<Memory, common::EvalError> {
        let haystack = common::one_input(inputs, "scan_literal")?;
        if self.literal.is_empty() {
            return Err(common::EvalError::new(
                "primitive `scan_literal` has empty literal. Fix: pass a non-empty literal.",
            ));
        }
        let mut offsets = Vec::new();
        for offset in 0..=haystack.len().saturating_sub(self.literal.len()) {
            if haystack[offset..].starts_with(&self.literal) {
                offsets.push(u32::try_from(offset).map_err(|_| {
                    common::EvalError::new(
                        "primitive `scan_literal` offset exceeds u32. Fix: split haystacks before 4 GiB.",
                    )
                })?);
            }
        }
        Ok(common::write_u32s(offsets))
    }
}
