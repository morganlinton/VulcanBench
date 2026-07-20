//! Pipeline pre-warm hint.
//!
//! Source-of-truth: `PERF_ROADMAP_2026-05-01.md` section B item B4.
//!
//! First-dispatch pipeline reflection is sync-blocking on wgpu  -  the
//! host has to wait for the driver to compile + reflect the shader
//! before issuing the first dispatch. Pre-warming during the
//! canonicalize / lower phase moves that cost off the dispatch path.
//!
//! This module computes a `PrewarmHint` indicating whether a kernel
//! is large/complex enough to merit pre-warm, plus the suggested
//! warm-up time budget. The host's pre-warm executor consumes this
//! to decide which kernels to dispatch a no-op for during canonicalize.

use serde::{Deserialize, Serialize};
use vyre_lower::{KernelBody, KernelDescriptor, KernelOp, KernelOpKind};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PrewarmHint {
    pub kernel_id: String,
    /// True if pre-warm is recommended.
    pub should_prewarm: bool,
    /// Estimated first-dispatch reflection cost in microseconds (very
    /// rough  -  anchored on op-count + binding-count proxies).
    pub estimated_first_dispatch_us: u32,
    /// Reason  -  useful for logging.
    pub reason: String,
}

/// Op-count threshold above which pre-warm is recommended. Below this
/// the reflection cost is small enough that pre-warm doesn't pay back.
pub const PREWARM_OP_THRESHOLD: u32 = 50;

#[must_use]
pub fn analyze(desc: &KernelDescriptor) -> PrewarmHint {
    let op_count = count_ops(&desc.body);
    let binding_count = desc.bindings.slots.len() as u32;
    // Crude cost model: ~10us baseline + 1us per op + 50us per binding
    // (driver work scales steeply with binding count).
    let estimated_us = 10 + op_count + 50 * binding_count;

    let (should_prewarm, reason) = if op_count >= PREWARM_OP_THRESHOLD {
        (
            true,
            format!("op-count {op_count} ≥ {PREWARM_OP_THRESHOLD}"),
        )
    } else if binding_count >= 4 {
        (true, format!("binding-count {binding_count} ≥ 4"))
    } else {
        (
            false,
            format!(
                "small kernel ({op_count} ops, {binding_count} bindings)  -  pre-warm not worth it"
            ),
        )
    };

    PrewarmHint {
        kernel_id: desc.id.clone(),
        should_prewarm,
        estimated_first_dispatch_us: estimated_us,
        reason,
    }
}

fn count_ops(body: &KernelBody) -> u32 {
    let mut total: u32 = body.ops.len() as u32;
    for op in &body.ops {
        if has_child_body(op) {
            for child in &body.child_bodies {
                total = total.saturating_add(count_ops(child));
            }
            break; // child bodies counted once
        }
    }
    total
}

fn has_child_body(op: &KernelOp) -> bool {
    matches!(
        op.kind,
        KernelOpKind::StructuredIfThen
            | KernelOpKind::StructuredIfThenElse
            | KernelOpKind::StructuredForLoop { .. }
            | KernelOpKind::StructuredBlock
            | KernelOpKind::Region { .. }
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use vyre_foundation::ir::DataType;
    use vyre_lower::{
        BindingLayout, BindingSlot, BindingVisibility, Dispatch, KernelBody, KernelDescriptor,
        KernelOp, KernelOpKind, LiteralValue, MemoryClass,
    };

    fn binding(slot: u32) -> BindingSlot {
        BindingSlot {
            slot,
            element_type: DataType::U32,
            element_count: None,
            memory_class: MemoryClass::Global,
            visibility: BindingVisibility::ReadWrite,
            name: format!("b{slot}"),
        }
    }

    fn small_kernel() -> KernelDescriptor {
        KernelDescriptor {
            id: "small".into(),
            bindings: BindingLayout {
                slots: vec![binding(0)],
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
                        kind: KernelOpKind::Return,
                        operands: vec![],
                        result: None,
                    },
                ],
                child_bodies: vec![],
                literals: vec![LiteralValue::U32(0)],
            },
        }
    }

    #[test]
    fn small_kernel_does_not_warrant_prewarm() {
        let h = analyze(&small_kernel());
        assert!(!h.should_prewarm);
        assert!(h.reason.contains("not worth it"));
    }

    #[test]
    fn many_op_kernel_warrants_prewarm() {
        let mut ops = Vec::with_capacity(60);
        for i in 0..60 {
            ops.push(KernelOp {
                kind: KernelOpKind::Literal,
                operands: vec![0],
                result: Some(i),
            });
        }
        let kernel = KernelDescriptor {
            id: "big".into(),
            bindings: BindingLayout { slots: vec![] },
            dispatch: Dispatch::new(1, 1, 1),
            body: KernelBody {
                ops,
                child_bodies: vec![],
                literals: vec![LiteralValue::U32(0)],
            },
        };
        let h = analyze(&kernel);
        assert!(h.should_prewarm);
        assert!(h.reason.contains("op-count"));
    }

    #[test]
    fn many_binding_kernel_warrants_prewarm() {
        let kernel = KernelDescriptor {
            id: "many_bindings".into(),
            bindings: BindingLayout {
                slots: (0..6).map(binding).collect(),
            },
            dispatch: Dispatch::new(1, 1, 1),
            body: KernelBody {
                ops: vec![],
                child_bodies: vec![],
                literals: vec![],
            },
        };
        let h = analyze(&kernel);
        assert!(h.should_prewarm);
        assert!(h.reason.contains("binding-count"));
    }

    #[test]
    fn estimated_us_grows_with_op_and_binding_counts() {
        let small = analyze(&small_kernel());
        // 10 baseline + 2 ops + 50 * 1 binding = 62us
        assert_eq!(small.estimated_first_dispatch_us, 62);
    }

    #[test]
    fn empty_kernel_estimated_at_baseline() {
        let kernel = KernelDescriptor {
            id: "empty".into(),
            bindings: BindingLayout { slots: vec![] },
            dispatch: Dispatch::new(64, 1, 1),
            body: KernelBody {
                ops: vec![],
                child_bodies: vec![],
                literals: vec![],
            },
        };
        let h = analyze(&kernel);
        assert_eq!(h.estimated_first_dispatch_us, 10); // baseline only
        assert!(!h.should_prewarm);
    }

    #[test]
    fn threshold_constant_is_documented_value() {
        assert_eq!(PREWARM_OP_THRESHOLD, 50);
    }

    #[test]
    fn kernel_id_echoed_in_hint() {
        let h = analyze(&small_kernel());
        assert_eq!(h.kernel_id, "small");
    }
}
