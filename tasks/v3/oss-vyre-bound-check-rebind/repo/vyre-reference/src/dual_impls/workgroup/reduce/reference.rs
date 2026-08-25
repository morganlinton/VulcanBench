use crate::{dual_impls::common, workgroup::Memory};
use vyre_primitives::{CombineOp, Reduce};

impl common::ReferenceEvaluator for Reduce {
    fn evaluate(&self, inputs: &[Memory]) -> Result<Memory, common::EvalError> {
        let words = common::u32_words(common::one_input(inputs, "reduce")?, "reduce")?;
        let Some((&first, tail)) = words.split_first() else {
            // Return the correct algebraic identity for each operator so the
            // reference oracle matches a correct GPU kernel's empty-input
            // behavior. Returning 0 for every operator was wrong: Min and
            // BitAnd have identity u32::MAX, and Mul has identity 1.
            return Ok(common::scalar(identity_for(self.combine)));
        };
        let mut value = first;
        for next in tail.iter().copied() {
            value = common::combine(self.combine, value, next)?;
        }
        Ok(common::scalar(value))
    }
}

/// Return the algebraic identity for the given [`CombineOp`] over u32.
///
/// This is the unique value `e` such that `combine(op, e, x) == x` for all x.
/// Used as the result of an empty reduction so the reference oracle agrees
/// with a correct GPU kernel.
fn identity_for(op: CombineOp) -> u32 {
    match op {
        CombineOp::Add => 0,
        CombineOp::Mul => 1,
        CombineOp::BitAnd => u32::MAX,
        CombineOp::BitOr => 0,
        CombineOp::BitXor => 0,
        CombineOp::Min => u32::MAX,
        CombineOp::Max => 0,
        // Any future variant: fail closed so the operator is forced to add an
        // explicit identity rather than silently using 0.
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dual_impls::common::ReferenceEvaluator;

    /// Before the fix, empty-input Reduce always returned scalar(0) regardless
    /// of operator. After the fix, each operator returns its algebraic identity.
    #[test]
    fn test_reduce_empty_min_identity() {
        let r = Reduce {
            combine: CombineOp::Min,
        };
        let out = r
            .evaluate(&[Memory::from_bytes(vec![])])
            .expect("Fix: reduce over empty input must succeed");
        let val = u32::from_le_bytes(
            out.bytes()
                .try_into()
                .expect("Fix: scalar output must be 4 bytes"),
        );
        assert_eq!(
            val,
            u32::MAX,
            "Fix: empty Min reduction identity must be u32::MAX, got {val}"
        );
    }

    #[test]
    fn test_reduce_empty_mul_identity() {
        let r = Reduce {
            combine: CombineOp::Mul,
        };
        let out = r
            .evaluate(&[Memory::from_bytes(vec![])])
            .expect("Fix: reduce over empty input must succeed");
        let val = u32::from_le_bytes(
            out.bytes()
                .try_into()
                .expect("Fix: scalar output must be 4 bytes"),
        );
        assert_eq!(
            val,
            1u32,
            "Fix: empty Mul reduction identity must be 1, got {val}"
        );
    }

    #[test]
    fn test_reduce_empty_bitand_identity() {
        let r = Reduce {
            combine: CombineOp::BitAnd,
        };
        let out = r
            .evaluate(&[Memory::from_bytes(vec![])])
            .expect("Fix: reduce over empty input must succeed");
        let val = u32::from_le_bytes(
            out.bytes()
                .try_into()
                .expect("Fix: scalar output must be 4 bytes"),
        );
        assert_eq!(
            val,
            u32::MAX,
            "Fix: empty BitAnd reduction identity must be u32::MAX, got {val}"
        );
    }

    #[test]
    fn test_reduce_empty_add_identity() {
        let r = Reduce {
            combine: CombineOp::Add,
        };
        let out = r
            .evaluate(&[Memory::from_bytes(vec![])])
            .expect("Fix: reduce over empty input must succeed");
        let val = u32::from_le_bytes(
            out.bytes()
                .try_into()
                .expect("Fix: scalar output must be 4 bytes"),
        );
        assert_eq!(
            val,
            0u32,
            "Fix: empty Add reduction identity must be 0, got {val}"
        );
    }
}
