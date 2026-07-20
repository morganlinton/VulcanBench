//! Shared `KernelOpKind` policy predicates.

use crate::KernelOpKind;

/// Return true when a result-producing op can be removed if all of its
/// result ids are unused.
///
/// This is stricter than descriptor-level "has side effects": structural
/// control ops, async protocol ops, calls, regions, and opaque nodes are kept
/// even when they expose no directly-used result id because their nested
/// bodies or backend contracts may carry observable behavior.
#[must_use]
pub(crate) fn kernel_op_kind_is_dce_pure(kind: &KernelOpKind) -> bool {
    !matches!(
        kind,
        KernelOpKind::StoreGlobal
            | KernelOpKind::StoreShared
            | KernelOpKind::LoopCarrierInit { .. }
            | KernelOpKind::LoopCarrierEnd { .. }
            | KernelOpKind::Barrier { .. }
            | KernelOpKind::Atomic { .. }
            | KernelOpKind::AsyncLoad { .. }
            | KernelOpKind::AsyncStore { .. }
            | KernelOpKind::AsyncWait { .. }
            | KernelOpKind::Trap { .. }
            | KernelOpKind::Resume { .. }
            | KernelOpKind::IndirectDispatch { .. }
            | KernelOpKind::Return
            | KernelOpKind::StructuredIfThen
            | KernelOpKind::StructuredIfThenElse
            | KernelOpKind::StructuredForLoop { .. }
            | KernelOpKind::StructuredBlock
            | KernelOpKind::Region { .. }
            | KernelOpKind::Call { .. }
            | KernelOpKind::OpaqueExpr(..)
            | KernelOpKind::OpaqueNode(..)
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::OpaqueNodeData;
    use vyre_foundation::{
        ir::{AtomicOp, BinOp},
        runtime::memory_model::MemoryOrdering,
    };

    #[test]
    fn arithmetic_and_literals_are_dead_eliminable_when_unused() {
        assert!(kernel_op_kind_is_dce_pure(&KernelOpKind::Literal));
        assert!(kernel_op_kind_is_dce_pure(&KernelOpKind::BinOpKind(
            BinOp::Add
        )));
    }

    #[test]
    fn side_effecting_and_structural_ops_are_not_dead_eliminable() {
        assert!(!kernel_op_kind_is_dce_pure(&KernelOpKind::StoreGlobal));
        assert!(!kernel_op_kind_is_dce_pure(&KernelOpKind::Atomic {
            op: AtomicOp::Add,
            ordering: MemoryOrdering::SeqCst,
        }));
        assert!(!kernel_op_kind_is_dce_pure(&KernelOpKind::AsyncLoad {
            tag: "copy".into(),
        }));
        assert!(!kernel_op_kind_is_dce_pure(
            &KernelOpKind::StructuredForLoop {
                loop_var: "i".into(),
            }
        ));
        assert!(!kernel_op_kind_is_dce_pure(&KernelOpKind::OpaqueNode(
            Box::new(OpaqueNodeData {
                extension_kind: "backend-specific".into(),
                payload: Vec::new(),
            })
        )));
    }
}
