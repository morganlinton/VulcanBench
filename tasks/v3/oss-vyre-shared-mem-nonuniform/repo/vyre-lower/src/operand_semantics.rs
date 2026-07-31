//! Operand namespace semantics for lowered kernel ops.
//!
//! `KernelOp` operands are a compact `u32` vector, but not every entry is
//! a result-id. Some positions are binding slots, literal-pool indices,
//! child-body indices, axes, or metadata. Optimizer analyses and rewrites
//! must agree on this classifier or they will silently miscompile by
//! treating metadata as SSA values, or by missing real data dependencies.

use crate::KernelOpKind;

/// True when `kind.operands[pos]` is a result-id reference in the lowered
/// kernel SSA namespace.
#[must_use]
pub(crate) fn operand_is_result_reference(kind: &KernelOpKind, pos: usize) -> bool {
    use KernelOpKind::*;
    match kind {
        Literal => false,
        LocalInvocationId | GlobalInvocationId | WorkgroupId => false,
        SubgroupLocalId | SubgroupSize | LoopIndex { .. } => false,
        LoopCarrierInit { .. } | LoopCarrier { .. } | LoopCarrierEnd { .. } => pos == 0,
        LoadGlobal | LoadShared | LoadConstant => pos != 0,
        BufferLength => false,
        StoreGlobal | StoreShared => pos != 0,
        Copy | BinOpKind(_) | UnOpKind(_) | Fma | MatrixMma { .. } | Select | Cast { .. } => true,
        Atomic { .. } => pos != 0,
        SubgroupBallot | SubgroupShuffle | SubgroupBroadcast | SubgroupReduce { .. } => true,
        StructuredIfThen | StructuredIfThenElse => pos == 0,
        StructuredForLoop { .. } => pos != 2,
        StructuredBlock | Region { .. } => false,
        Return | Barrier { .. } => false,
        AsyncLoad { .. } | AsyncStore { .. } => pos >= 2,
        AsyncWait { .. } => false,
        Trap { .. } => pos == 0,
        Resume { .. } => false,
        IndirectDispatch { .. } => false,
        Call { .. } => true,
        OpaqueExpr(..) | OpaqueNode(..) => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::KernelOpKind;

    #[test]
    fn operand_classifier_separates_indices_from_result_ids() {
        assert!(!operand_is_result_reference(&KernelOpKind::Literal, 0));
        assert!(!operand_is_result_reference(&KernelOpKind::LoadGlobal, 0));
        assert!(operand_is_result_reference(&KernelOpKind::LoadGlobal, 1));
        assert!(!operand_is_result_reference(
            &KernelOpKind::StructuredForLoop {
                loop_var: "i".into(),
            },
            2,
        ));
        assert!(operand_is_result_reference(
            &KernelOpKind::StructuredForLoop {
                loop_var: "i".into(),
            },
            1,
        ));
        assert!(operand_is_result_reference(
            &KernelOpKind::AsyncStore { tag: "copy".into() },
            2,
        ));
        assert!(!operand_is_result_reference(
            &KernelOpKind::IndirectDispatch { count_offset: 0 },
            0,
        ));
    }
}
