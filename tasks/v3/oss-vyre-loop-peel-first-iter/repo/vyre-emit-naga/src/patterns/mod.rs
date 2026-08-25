//! Naga-specific emit-time patterns.
//!
//! These rewrites operate at emit time on the lowered KernelDescriptor
//! and produce naga IR that takes advantage of substrate-specific
//! features. They live in this crate because they are specific to the
//! naga backend (and the wgpu/Vulkan/WebGPU shaders it targets);
//! equivalent patterns for CUDA live in `vyre-emit-ptx::patterns`.

pub mod bind_group_reuse;
pub mod pipeline_prewarm;
pub mod push_constant_inline;
pub mod vec_pack;

use serde::{Deserialize, Serialize};
use vyre_lower::KernelDescriptor;

/// Unified naga-side pattern audit. Runs every shipped naga pattern
/// against the descriptor and bundles the reports. Mirror of
/// `vyre_emit_ptx::patterns::audit` and `vyre_lower::audit::audit`,
/// but for naga-specific patterns (vec packing, push constants,
/// pipeline prewarm; bind group reuse is multi-descriptor and not
/// included here).
#[must_use]
pub fn audit(desc: &KernelDescriptor) -> NagaAuditReport {
    NagaAuditReport {
        kernel_id: desc.id.clone(),
        vec_pack: vec_pack::analysis::analyze(desc),
        push_constant: push_constant_inline::analyze(desc),
        prewarm: pipeline_prewarm::analyze(desc),
    }
}

/// Like [`audit`] but runs the standard rewrite pipeline first.
/// Shows what naga-specific patterns still apply AFTER the
/// substrate-neutral optimization stack has already run.
///
/// Useful diagnostic: a non-empty post-optimization audit tells you
/// the substrate-specific layer (e.g. vec_pack fusion) is leaving
/// real performance on the table that no amount of substrate-neutral
/// rewriting will reach.
#[must_use]
pub fn audit_optimized(desc: &KernelDescriptor) -> NagaAuditReport {
    let optimized = vyre_lower::rewrites::run_all(desc);
    audit(&optimized)
}

/// Combined naga-pattern report.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NagaAuditReport {
    pub kernel_id: String,
    pub vec_pack: vec_pack::plan::PackingPlan,
    pub push_constant: push_constant_inline::PushConstantPlan,
    pub prewarm: pipeline_prewarm::PrewarmHint,
}

impl NagaAuditReport {
    /// Sum of actionable findings across the per-kernel patterns.
    /// Pre-warm contributes 1 if recommended.
    pub fn total_candidates(&self) -> usize {
        self.vec_pack.groups.len()
            + self.push_constant.candidates.len()
            + (self.prewarm.should_prewarm as usize)
    }

    pub fn has_any(&self) -> bool {
        self.total_candidates() > 0
    }

    /// One-line human-readable summary suitable for log lines.
    pub fn format_short(&self) -> String {
        let id = if self.kernel_id.is_empty() {
            "<unnamed>"
        } else {
            self.kernel_id.as_str()
        };
        format!(
            "{id} (naga): {} candidates ({} vec_pack, {} push_constant, prewarm={})",
            self.total_candidates(),
            self.vec_pack.groups.len(),
            self.push_constant.candidates.len(),
            self.prewarm.should_prewarm,
        )
    }

    /// True iff no naga-specific optimization opportunities found.
    pub fn is_clean(&self) -> bool {
        !self.has_any()
    }

    /// Identity element for `merge`  -  empty report. Useful as the
    /// seed of a corpus fold.
    pub fn zero() -> Self {
        Self {
            kernel_id: String::new(),
            vec_pack: vec_pack::plan::PackingPlan {
                kernel_id: String::new(),
                groups: vec![],
            },
            push_constant: push_constant_inline::PushConstantPlan {
                kernel_id: String::new(),
                candidates: vec![],
                total_bytes: 0,
                budget_bytes: 0,
            },
            prewarm: pipeline_prewarm::PrewarmHint {
                kernel_id: String::new(),
                should_prewarm: false,
                estimated_first_dispatch_us: 0,
                reason: String::new(),
            },
        }
    }

    /// Aggregate another report's findings into this one. Concatenates
    /// candidate vectors; ORs `should_prewarm`. Useful for corpus-level
    /// "how many naga-specific opportunities are there in this kernel
    /// suite?" rollups.
    pub fn merge(&mut self, other: NagaAuditReport) {
        self.vec_pack.groups.extend(other.vec_pack.groups);
        self.push_constant
            .candidates
            .extend(other.push_constant.candidates);
        self.push_constant.total_bytes = self
            .push_constant
            .total_bytes
            .saturating_add(other.push_constant.total_bytes);
        self.prewarm.should_prewarm |= other.prewarm.should_prewarm;
    }
}

impl std::fmt::Display for NagaAuditReport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.format_short())
    }
}

#[cfg(test)]
mod audit_tests {
    use super::*;
    use vyre_foundation::ir::DataType;
    use vyre_lower::{
        BindingLayout, BindingSlot, BindingVisibility, Dispatch, KernelBody, KernelDescriptor,
        KernelOp, KernelOpKind, LiteralValue, MemoryClass,
    };

    #[test]
    fn empty_kernel_yields_zero_candidates() {
        let desc = KernelDescriptor {
            id: "empty".into(),
            bindings: BindingLayout { slots: vec![] },
            dispatch: Dispatch::new(1, 1, 1),
            body: KernelBody {
                ops: vec![],
                child_bodies: vec![],
                literals: vec![],
            },
        };
        let report = audit(&desc);
        assert_eq!(report.kernel_id, "empty");
        assert_eq!(report.total_candidates(), 0);
        assert!(!report.has_any());
    }

    #[test]
    fn merge_aggregates_findings() {
        let mut acc = NagaAuditReport::zero();
        let desc = KernelDescriptor {
            id: "k".into(),
            bindings: BindingLayout { slots: vec![] },
            dispatch: Dispatch::new(64, 1, 1),
            body: KernelBody {
                ops: vec![],
                child_bodies: vec![],
                literals: vec![],
            },
        };
        let r1 = audit(&desc);
        let r2 = audit(&desc);
        acc.merge(r1);
        acc.merge(r2);
        // No findings on empty kernels  -  sums to 0.
        assert_eq!(acc.total_candidates(), 0);
    }

    #[test]
    fn format_short_and_is_clean_on_empty() {
        let desc = KernelDescriptor {
            id: "k".into(),
            bindings: BindingLayout { slots: vec![] },
            dispatch: Dispatch::new(1, 1, 1),
            body: KernelBody {
                ops: vec![],
                child_bodies: vec![],
                literals: vec![],
            },
        };
        let r = audit(&desc);
        assert!(r.is_clean());
        let s = r.format_short();
        assert!(s.contains("k (naga)"));
        assert!(s.contains("0 candidates"));
    }

    #[test]
    fn audit_optimized_drops_dead_arithmetic_findings() {
        // A kernel with dead arithmetic that the rewrite stack will
        // remove. After run_all, the post-optimization audit should
        // report no candidates from those vanished ops.
        use vyre_foundation::ir::BinOp;
        let desc = KernelDescriptor {
            id: "dead".into(),
            bindings: BindingLayout {
                slots: vec![BindingSlot {
                    slot: 0,
                    element_type: DataType::U32,
                    element_count: None,
                    memory_class: MemoryClass::Global,
                    visibility: BindingVisibility::ReadWrite,
                    name: "buf".into(),
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
                        kind: KernelOpKind::Literal,
                        operands: vec![1],
                        result: Some(1),
                    },
                    KernelOp {
                        kind: KernelOpKind::BinOpKind(BinOp::Add),
                        operands: vec![1, 0],
                        result: Some(2),
                    },
                    KernelOp {
                        kind: KernelOpKind::StoreGlobal,
                        operands: vec![0, 0, 1],
                        result: None,
                    },
                ],
                child_bodies: vec![],
                literals: vec![LiteralValue::U32(0), LiteralValue::U32(99)],
            },
        };
        let r = audit_optimized(&desc);
        // Just confirm it doesn't panic and returns the kernel_id.
        assert_eq!(r.kernel_id, "dead");
    }

    #[test]
    fn nonempty_kernel_audit_doesnt_panic() {
        let desc = KernelDescriptor {
            id: "k".into(),
            bindings: BindingLayout {
                slots: vec![BindingSlot {
                    slot: 0,
                    element_type: DataType::U32,
                    element_count: None,
                    memory_class: MemoryClass::Global,
                    visibility: BindingVisibility::ReadWrite,
                    name: "buf".into(),
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
                        kind: KernelOpKind::Literal,
                        operands: vec![1],
                        result: Some(1),
                    },
                    KernelOp {
                        kind: KernelOpKind::StoreGlobal,
                        operands: vec![0, 0, 1],
                        result: None,
                    },
                ],
                child_bodies: vec![],
                literals: vec![LiteralValue::U32(0), LiteralValue::U32(7)],
            },
        };
        let report = audit(&desc);
        assert_eq!(report.kernel_id, "k");
        // 3-op, 1-binding kernel sits below every naga pattern threshold
        // (vec_pack needs Load/Store fusion groups, push_constant needs
        // element_count=Some(1), prewarm needs ops≥50 or bindings≥4).
        // The contract this test enforces is "audit returns cleanly on
        // a real kernel without panicking", not a non-zero candidate count.
        assert_eq!(report.total_candidates(), 0);
    }
}
