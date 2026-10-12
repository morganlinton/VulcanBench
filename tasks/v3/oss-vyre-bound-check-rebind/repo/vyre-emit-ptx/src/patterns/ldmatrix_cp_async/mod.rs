//! PERF B7: ldmatrix / cp.async detection for async tile loads.
//!
//! `cp.async` (sm_80+) lets a thread issue a global-to-shared transfer
//! that completes asynchronously, freeing the thread for compute while
//! the load is in flight. Combined with `ldmatrix` for shared-to-register
//! tile loads, this hides memory latency on tiled ops.
//!
//! Phase-1 detection: identify load-then-store-to-shared op sequences
//! that match the cp.async pattern. The sequence is:
//!   `LoadGlobal(g, idx) → result_id`
//!   `StoreShared(s, idx, result_id)`
//! When found in adjacent positions on the same logical index, the
//! emitter can replace both with a single `cp.async.ca.shared.global`
//! issue + an `AsyncWait` later in the kernel.

use serde::{Deserialize, Serialize};
use vyre_lower::{KernelBody, KernelDescriptor, KernelOpKind};

use crate::ComputeCapability;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AsyncCopyCandidate {
    /// Op-index of the LoadGlobal op.
    pub load_op_index: usize,
    /// Op-index of the StoreShared op (must immediately follow).
    pub store_op_index: usize,
    /// Global binding slot read by the candidate load.
    pub global_binding_slot: u32,
    /// Shared-memory binding slot written by the candidate store.
    pub shared_binding_slot: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AsyncCopyPlan {
    /// Descriptor id that was analyzed.
    pub kernel_id: String,
    /// True when the selected PTX target can emit native `cp.async`.
    pub target_supports_cp_async: bool,
    /// True when the selected PTX target can use `ldmatrix` for matrix fragments.
    pub target_supports_ldmatrix: bool,
    /// Detected global-load to shared-store pairs that can be staged asynchronously.
    pub candidates: Vec<AsyncCopyCandidate>,
}

impl AsyncCopyPlan {
    #[must_use]
    pub fn candidate_count(&self) -> usize {
        self.candidates.len()
    }
}

#[must_use]
pub fn analyze(desc: &KernelDescriptor, target: ComputeCapability) -> AsyncCopyPlan {
    let cp_async_supported = target.supports_async_copy();
    let ldmatrix_supported = target.supports_ldmatrix();
    let mut candidates = Vec::new();
    if cp_async_supported {
        scan_body(&desc.body, &mut candidates, 0);
    }
    AsyncCopyPlan {
        kernel_id: desc.id.clone(),
        target_supports_cp_async: cp_async_supported,
        target_supports_ldmatrix: ldmatrix_supported,
        candidates,
    }
}

fn scan_body(body: &KernelBody, candidates: &mut Vec<AsyncCopyCandidate>, op_index_offset: usize) {
    for window in body.ops.windows(2).enumerate() {
        let (i, [load, store]) = (window.0, window.1) else {
            continue;
        };
        if let (KernelOpKind::LoadGlobal, KernelOpKind::StoreShared) = (&load.kind, &store.kind) {
            // store_value_id must equal load_result_id (the load feeds
            // the store), and both ops must use the same logical index.
            let load_result = load.result;
            let store_value = store.operands.get(2).copied();
            let same_index = load.operands.get(1) == store.operands.get(1);
            if load_result.is_some() && load_result.map(Some) == Some(store_value) && same_index {
                let Some(global_slot) = load.operands.first().copied() else {
                    continue;
                };
                let Some(shared_slot) = store.operands.first().copied() else {
                    continue;
                };
                candidates.push(AsyncCopyCandidate {
                    load_op_index: op_index_offset + i,
                    store_op_index: op_index_offset + i + 1,
                    global_binding_slot: global_slot,
                    shared_binding_slot: shared_slot,
                });
            }
        }
    }
    // Recurse into structured-control-flow children.
    for op in &body.ops {
        match &op.kind {
            KernelOpKind::StructuredIfThen
            | KernelOpKind::StructuredIfThenElse
            | KernelOpKind::StructuredForLoop { .. }
            | KernelOpKind::StructuredBlock
            | KernelOpKind::Region { .. } => {
                if let Some(child_id) = op.operands.last() {
                    if let Some(child) = body.child_bodies.get(*child_id as usize) {
                        scan_body(child, candidates, op_index_offset + body.ops.len());
                    }
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vyre_foundation::ir::DataType;
    use vyre_lower::{
        BindingLayout, BindingSlot, BindingVisibility, Dispatch, KernelBody, KernelDescriptor,
        KernelOp, LiteralValue, MemoryClass,
    };

    fn cp_async_kernel() -> KernelDescriptor {
        // load(global, 0) → r0; store(shared, 0, r0)
        KernelDescriptor {
            id: "cp_async".into(),
            bindings: BindingLayout {
                slots: vec![
                    BindingSlot {
                        slot: 0,
                        element_type: DataType::F32,
                        element_count: None,
                        memory_class: MemoryClass::Global,
                        visibility: BindingVisibility::ReadOnly,
                        name: "g".into(),
                    },
                    BindingSlot {
                        slot: 1,
                        element_type: DataType::F32,
                        element_count: Some(64),
                        memory_class: MemoryClass::Shared,
                        visibility: BindingVisibility::ReadWrite,
                        name: "s".into(),
                    },
                ],
            },
            dispatch: Dispatch::new(64, 1, 1),
            body: KernelBody {
                ops: vec![
                    KernelOp {
                        kind: KernelOpKind::Literal,
                        operands: vec![0],
                        result: Some(0),
                    },
                    KernelOp {
                        kind: KernelOpKind::LoadGlobal,
                        operands: vec![0, 0],
                        result: Some(1),
                    },
                    KernelOp {
                        kind: KernelOpKind::StoreShared,
                        operands: vec![1, 0, 1],
                        result: None,
                    },
                ],
                child_bodies: vec![],
                literals: vec![LiteralValue::U32(0)],
            },
        }
    }

    #[test]
    fn cp_async_unsupported_on_volta() {
        let p = analyze(&cp_async_kernel(), ComputeCapability::SM_70);
        assert!(!p.target_supports_cp_async);
        assert!(p.candidates.is_empty());
    }

    #[test]
    fn cp_async_supported_on_ampere() {
        let p = analyze(&cp_async_kernel(), ComputeCapability::SM_80);
        assert!(p.target_supports_cp_async);
        assert_eq!(p.candidates.len(), 1);
        assert_eq!(p.candidates[0].load_op_index, 1);
        assert_eq!(p.candidates[0].store_op_index, 2);
        assert_eq!(p.candidates[0].global_binding_slot, 0);
        assert_eq!(p.candidates[0].shared_binding_slot, 1);
    }

    #[test]
    fn empty_kernel_yields_no_candidates() {
        let desc = KernelDescriptor {
            id: "empty".into(),
            bindings: BindingLayout { slots: vec![] },
            dispatch: Dispatch::new(64, 1, 1),
            body: KernelBody {
                ops: vec![],
                child_bodies: vec![],
                literals: vec![],
            },
        };
        let p = analyze(&desc, ComputeCapability::SM_80);
        assert!(p.candidates.is_empty());
    }

    #[test]
    fn load_without_immediate_store_no_candidate() {
        let desc = KernelDescriptor {
            id: "load_only".into(),
            bindings: BindingLayout {
                slots: vec![BindingSlot {
                    slot: 0,
                    element_type: DataType::F32,
                    element_count: None,
                    memory_class: MemoryClass::Global,
                    visibility: BindingVisibility::ReadOnly,
                    name: "g".into(),
                }],
            },
            dispatch: Dispatch::new(64, 1, 1),
            body: KernelBody {
                ops: vec![
                    KernelOp {
                        kind: KernelOpKind::Literal,
                        operands: vec![0],
                        result: Some(0),
                    },
                    KernelOp {
                        kind: KernelOpKind::LoadGlobal,
                        operands: vec![0, 0],
                        result: Some(1),
                    },
                ],
                child_bodies: vec![],
                literals: vec![LiteralValue::U32(0)],
            },
        };
        let p = analyze(&desc, ComputeCapability::SM_80);
        assert!(p.candidates.is_empty());
    }

    #[test]
    fn store_to_global_not_shared_no_candidate() {
        let desc = KernelDescriptor {
            id: "store_global".into(),
            bindings: BindingLayout {
                slots: vec![BindingSlot {
                    slot: 0,
                    element_type: DataType::F32,
                    element_count: None,
                    memory_class: MemoryClass::Global,
                    visibility: BindingVisibility::ReadWrite,
                    name: "g".into(),
                }],
            },
            dispatch: Dispatch::new(64, 1, 1),
            body: KernelBody {
                ops: vec![
                    KernelOp {
                        kind: KernelOpKind::Literal,
                        operands: vec![0],
                        result: Some(0),
                    },
                    KernelOp {
                        kind: KernelOpKind::LoadGlobal,
                        operands: vec![0, 0],
                        result: Some(1),
                    },
                    KernelOp {
                        kind: KernelOpKind::StoreGlobal,
                        operands: vec![0, 0, 1],
                        result: None,
                    },
                ],
                child_bodies: vec![],
                literals: vec![LiteralValue::U32(0)],
            },
        };
        let p = analyze(&desc, ComputeCapability::SM_80);
        assert!(
            p.candidates.is_empty(),
            "global→global not a cp.async candidate"
        );
    }

    #[test]
    fn mismatched_load_store_index_no_candidate() {
        let mut desc = cp_async_kernel();
        desc.id = "cp_async_mismatched_index".into();
        desc.body.ops[2].operands[1] = 99;
        let p = analyze(&desc, ComputeCapability::SM_80);
        assert!(
            p.candidates.is_empty(),
            "cp.async requires the global load and shared store to use the same logical index"
        );
    }
}
