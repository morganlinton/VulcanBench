//! PTX-specific emit-time patterns.
//!
//! These rewrites operate at PTX emit time on the lowered
//! KernelDescriptor and produce PTX that takes advantage of
//! CUDA-specific features. They live in this crate because they are
//! specific to NVIDIA hardware; equivalent patterns for naga live in
//! `vyre-emit-naga::patterns`.

pub mod instruction_scheduling;
pub mod ldmatrix_cp_async;
pub mod predicated_execution;
pub mod tensor_core_fragment;
pub mod vec_load_fusion;
mod vec_memory_fusion;
pub mod vec_store_fusion;

use serde::{Deserialize, Serialize};
use vyre_lower::KernelDescriptor;

use crate::ComputeCapability;

/// Unified PTX-side audit: runs every shipped pattern against the
/// descriptor and returns the combined report. Mirror of
/// `vyre_lower::audit::audit` but for PTX-specific patterns.
///
/// `target` controls capability-gated patterns (tensor cores require
/// sm_70+; ldmatrix.cp.async requires sm_80+).
#[must_use]
pub fn audit(desc: &KernelDescriptor, target: ComputeCapability) -> PtxAuditReport {
    PtxAuditReport {
        kernel_id: desc.id.clone(),
        target,
        predication: predicated_execution::analyze(desc),
        vec_load: vec_load_fusion::analyze(desc),
        vec_store: vec_store_fusion::analyze(desc),
        async_copy: ldmatrix_cp_async::analyze(desc, target),
        tensor_core: tensor_core_fragment::analyze(desc, target),
        scheduling: instruction_scheduling::analyze(desc),
    }
}

/// Like [`audit`] but runs the standard rewrite pipeline first.
/// Shows what PTX-specific patterns still apply AFTER the
/// substrate-neutral optimization stack has run. A non-empty
/// post-optimization audit tells you the PTX layer is the only path
/// to recover the remaining perf  -  e.g., a vec_load_fusion candidate
/// that survives means scalar `ld.global.u32` instructions will be
/// emitted unless the PTX emit-side rewrite is taught to fuse.
#[must_use]
pub fn audit_optimized(desc: &KernelDescriptor, target: ComputeCapability) -> PtxAuditReport {
    let optimized = vyre_lower::rewrites::run_all(desc);
    audit(&optimized, target)
}

/// Combined PTX-pattern report. One `pub` field per shipped pattern.
/// Callers can drill into individual reports for details, or use
/// `total_candidates()` for a single-number "is anything actionable"
/// signal.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PtxAuditReport {
    pub kernel_id: String,
    pub target: ComputeCapability,
    pub predication: predicated_execution::PredicationPlan,
    pub vec_load: vec_load_fusion::FusionPlan,
    pub vec_store: vec_store_fusion::FusionPlan,
    pub async_copy: ldmatrix_cp_async::AsyncCopyPlan,
    pub tensor_core: tensor_core_fragment::TensorCorePlan,
    pub scheduling: instruction_scheduling::SchedulingHints,
}

impl PtxAuditReport {
    /// Sum of actionable findings across all patterns. `0` means no
    /// PTX-specific optimizations apply to this kernel.
    pub fn total_candidates(&self) -> usize {
        self.predication.candidates.len()
            + self.vec_load.candidates.len()
            + self.vec_store.candidates.len()
            + self.async_copy.candidates.len()
            + self.tensor_core.candidates.len()
            + self.scheduling.long_chains.len()
    }

    /// Whether any pattern fired.
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
            "{id} (ptx sm_{}_{}): {} candidates ({}p, {}vl, {}vs, {}ac, {}tc, {}sched)",
            self.target.major,
            self.target.minor,
            self.total_candidates(),
            self.predication.candidates.len(),
            self.vec_load.candidates.len(),
            self.vec_store.candidates.len(),
            self.async_copy.candidates.len(),
            self.tensor_core.candidates.len(),
            self.scheduling.long_chains.len(),
        )
    }

    /// True iff no PTX-specific optimization opportunities found.
    pub fn is_clean(&self) -> bool {
        !self.has_any()
    }

    /// Identity element for [`Self::merge`]  -  empty report. The `target`
    /// defaults to SM_70 (the broadest-compatibility floor); merging
    /// reports with different targets is allowed but the aggregate
    /// keeps the seed's target.
    pub fn zero() -> Self {
        Self {
            kernel_id: String::new(),
            target: ComputeCapability::SM_70,
            predication: predicated_execution::PredicationPlan {
                kernel_id: String::new(),
                candidates: vec![],
            },
            vec_load: vec_load_fusion::FusionPlan { candidates: vec![] },
            vec_store: vec_store_fusion::FusionPlan { candidates: vec![] },
            async_copy: ldmatrix_cp_async::AsyncCopyPlan {
                kernel_id: String::new(),
                target_supports_cp_async: false,
                target_supports_ldmatrix: false,
                candidates: vec![],
            },
            tensor_core: tensor_core_fragment::TensorCorePlan {
                kernel_id: String::new(),
                target_sm: String::new(),
                candidates: vec![],
            },
            scheduling: instruction_scheduling::SchedulingHints {
                kernel_id: String::new(),
                long_chains: vec![],
                total_op_count: 0,
            },
        }
    }

    /// Aggregate another report's findings into this one. Concatenates
    /// every candidate vector + long_chains. Useful for corpus-level
    /// rollups.
    pub fn merge(&mut self, other: PtxAuditReport) {
        self.predication
            .candidates
            .extend(other.predication.candidates);
        self.vec_load.candidates.extend(other.vec_load.candidates);
        self.vec_store.candidates.extend(other.vec_store.candidates);
        self.async_copy
            .candidates
            .extend(other.async_copy.candidates);
        self.tensor_core
            .candidates
            .extend(other.tensor_core.candidates);
        self.scheduling
            .long_chains
            .extend(other.scheduling.long_chains);
        self.scheduling.total_op_count = self
            .scheduling
            .total_op_count
            .saturating_add(other.scheduling.total_op_count);
    }
}

impl std::fmt::Display for PtxAuditReport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.format_short())
    }
}

#[cfg(test)]
mod tests {
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
        let report = audit(&desc, ComputeCapability::SM_70);
        assert_eq!(report.kernel_id, "empty");
        assert_eq!(report.total_candidates(), 0);
        assert!(!report.has_any());
    }

    #[test]
    fn vec_load_chain_shows_up_in_audit() {
        let desc = KernelDescriptor {
            id: "vload_chain".into(),
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
            dispatch: Dispatch::new(1, 1, 1),
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
                        kind: KernelOpKind::LoadGlobal,
                        operands: vec![0, 0],
                        result: Some(2),
                    },
                    KernelOp {
                        kind: KernelOpKind::BinOpKind(vyre_foundation::ir::BinOp::Add),
                        operands: vec![0, 1],
                        result: Some(3),
                    },
                    KernelOp {
                        kind: KernelOpKind::LoadGlobal,
                        operands: vec![0, 3],
                        result: Some(4),
                    },
                ],
                child_bodies: vec![],
                literals: vec![LiteralValue::U32(0), LiteralValue::U32(1)],
            },
        };
        let report = audit(&desc, ComputeCapability::SM_70);
        assert!(report.has_any());
        assert_eq!(report.vec_load.candidates.len(), 1);
        assert_eq!(report.total_candidates(), 1);
    }

    #[test]
    fn ptx_audit_merge_aggregates_candidates() {
        let mut acc = PtxAuditReport::zero();
        // Merge two empty reports  -  both have no findings, so aggregate
        // stays empty.
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
        acc.merge(audit(&desc, ComputeCapability::SM_70));
        acc.merge(audit(&desc, ComputeCapability::SM_70));
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
        let r = audit(&desc, ComputeCapability::SM_80);
        assert!(r.is_clean());
        let s = r.format_short();
        assert!(s.contains("k (ptx sm_8_0)"));
        assert!(s.contains("0 candidates"));
    }

    #[test]
    fn audit_optimized_runs_and_returns_report() {
        let desc = KernelDescriptor {
            id: "ao".into(),
            bindings: BindingLayout { slots: vec![] },
            dispatch: Dispatch::new(64, 1, 1),
            body: KernelBody {
                ops: vec![],
                child_bodies: vec![],
                literals: vec![],
            },
        };
        let r = audit_optimized(&desc, ComputeCapability::SM_70);
        assert_eq!(r.kernel_id, "ao");
        assert_eq!(r.total_candidates(), 0);
    }

    #[test]
    fn audit_carries_target_through() {
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
        let r80 = audit(&desc, ComputeCapability::SM_80);
        let r90 = audit(&desc, ComputeCapability::SM_90);
        assert_eq!(r80.target, ComputeCapability::SM_80);
        assert_eq!(r90.target, ComputeCapability::SM_90);
    }
}
