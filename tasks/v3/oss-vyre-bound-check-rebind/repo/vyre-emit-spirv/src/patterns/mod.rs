//! SPIR-V-specific emit-time patterns.
//!
//! These analyses walk the `KernelDescriptor` and produce
//! Vulkan/SPIR-V-specific reports (capability declarations,
//! descriptor-set normalization candidates, etc.) that emitters and
//! pipeline builders consume to make correct dispatch decisions.

pub mod subgroup_capabilities;
pub mod workgroup_size_validation;

use serde::{Deserialize, Serialize};
use vyre_lower::KernelDescriptor;

/// Unified SPIR-V-side pattern audit. Runs every shipped SPIR-V
/// pattern against the descriptor and bundles the reports. Mirror of
/// `vyre_emit_naga::patterns::audit` and `vyre_emit_ptx::patterns::audit`.
#[must_use]
pub fn audit(desc: &KernelDescriptor) -> SpirvAuditReport {
    SpirvAuditReport {
        kernel_id: desc.id.clone(),
        subgroup: subgroup_capabilities::analyze(desc),
        workgroup_validation: workgroup_size_validation::analyze(desc),
    }
}

/// Like [`audit`] but runs the standard rewrite pipeline first.
/// The workgroup-size validation produces the same result either way
/// (the rewrite stack doesn't change `dispatch.workgroup_size`), but
/// the subgroup capability detection may report fewer required caps
/// after dead-code elimination strips unused subgroup ops.
#[must_use]
pub fn audit_optimized(desc: &KernelDescriptor) -> SpirvAuditReport {
    let optimized = vyre_lower::rewrites::run_all(desc);
    audit(&optimized)
}

/// Combined SPIR-V-pattern report.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SpirvAuditReport {
    pub kernel_id: String,
    pub subgroup: subgroup_capabilities::SubgroupCapabilityReport,
    pub workgroup_validation: workgroup_size_validation::ValidationReport,
}

impl SpirvAuditReport {
    /// True iff at least one subgroup capability needs to be enabled
    /// OR at least one workgroup-size violation must be addressed.
    /// Both signals matter for pipeline construction.
    pub fn requires_action(&self) -> bool {
        let caps = &self.subgroup.capabilities;
        let needs_caps = caps.basic || caps.ballot || caps.shuffle || caps.arithmetic;
        let has_violations = !self.workgroup_validation.violations.is_empty();
        needs_caps || has_violations
    }

    /// Number of distinct findings across both patterns.
    pub fn total_findings(&self) -> usize {
        let caps = &self.subgroup.capabilities;
        let mut n = 0;
        if caps.basic {
            n += 1;
        }
        if caps.ballot {
            n += 1;
        }
        if caps.shuffle {
            n += 1;
        }
        if caps.arithmetic {
            n += 1;
        }
        n + self.workgroup_validation.violations.len()
    }

    /// One-line human-readable summary suitable for log lines.
    pub fn format_short(&self) -> String {
        let id = if self.kernel_id.is_empty() {
            "<unnamed>"
        } else {
            self.kernel_id.as_str()
        };
        let caps = &self.subgroup.capabilities;
        format!(
            "{id} (spirv): {} findings ({} subgroup caps, {} wg violations)",
            self.total_findings(),
            (caps.basic as usize)
                + (caps.ballot as usize)
                + (caps.shuffle as usize)
                + (caps.arithmetic as usize),
            self.workgroup_validation.violations.len(),
        )
    }

    /// True iff no SPIR-V-specific findings  -  no required capabilities,
    /// no workgroup-size violations.
    pub fn is_clean(&self) -> bool {
        !self.requires_action()
    }

    /// Identity element for `merge`  -  no required caps, no
    /// violations, baseline workgroup limits.
    pub fn zero() -> Self {
        Self {
            kernel_id: String::new(),
            subgroup: subgroup_capabilities::SubgroupCapabilityReport {
                kernel_id: String::new(),
                capabilities: subgroup_capabilities::SubgroupCapabilities {
                    basic: false,
                    ballot: false,
                    shuffle: false,
                    arithmetic: false,
                },
            },
            workgroup_validation: workgroup_size_validation::ValidationReport {
                kernel_id: String::new(),
                workgroup_size: [1, 1, 1],
                limits: workgroup_size_validation::VULKAN_BASELINE,
                violations: vec![],
            },
        }
    }

    /// Aggregate another report's findings: ORs each subgroup
    /// capability bit, concatenates workgroup violations. Workgroup
    /// size + limits are kept from the SEED (merging mismatched
    /// dispatches doesn't make geometric sense).
    pub fn merge(&mut self, other: SpirvAuditReport) {
        let dst = &mut self.subgroup.capabilities;
        let src = &other.subgroup.capabilities;
        dst.basic |= src.basic;
        dst.ballot |= src.ballot;
        dst.shuffle |= src.shuffle;
        dst.arithmetic |= src.arithmetic;
        self.workgroup_validation
            .violations
            .extend(other.workgroup_validation.violations);
    }
}

impl std::fmt::Display for SpirvAuditReport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.format_short())
    }
}

#[cfg(test)]
mod audit_tests {
    use super::*;
    use vyre_lower::{
        BindingLayout, Dispatch, KernelBody, KernelDescriptor, KernelOp, KernelOpKind,
    };

    #[test]
    fn empty_kernel_yields_no_findings() {
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
        let report = audit(&desc);
        assert_eq!(report.kernel_id, "empty");
        assert_eq!(report.total_findings(), 0);
        assert!(!report.requires_action());
    }

    #[test]
    fn oversized_workgroup_shows_in_audit() {
        let desc = KernelDescriptor {
            id: "huge".into(),
            bindings: BindingLayout { slots: vec![] },
            dispatch: Dispatch::new(2048, 1, 1),
            body: KernelBody {
                ops: vec![],
                child_bodies: vec![],
                literals: vec![],
            },
        };
        let report = audit(&desc);
        assert!(report.requires_action());
        assert!(!report.workgroup_validation.violations.is_empty());
    }

    #[test]
    fn spirv_audit_merge_aggregates() {
        let mut acc = SpirvAuditReport::zero();
        let desc = KernelDescriptor {
            id: "k".into(),
            bindings: BindingLayout { slots: vec![] },
            dispatch: Dispatch::new(64, 1, 1),
            body: KernelBody {
                ops: vec![KernelOp {
                    kind: KernelOpKind::SubgroupBallot,
                    operands: vec![0],
                    result: Some(0),
                }],
                child_bodies: vec![],
                literals: vec![],
            },
        };
        acc.merge(audit(&desc));
        // After merging in a kernel that uses SubgroupBallot, the
        // ballot capability bit should be set on the aggregate.
        assert!(acc.subgroup.capabilities.ballot);
    }

    #[test]
    fn format_short_and_is_clean_on_empty() {
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
        let r = audit(&desc);
        assert!(r.is_clean());
        let s = r.format_short();
        assert!(s.contains("k (spirv)"));
        assert!(s.contains("0 findings"));
    }

    #[test]
    fn audit_optimized_returns_same_workgroup_as_audit() {
        // The rewrite stack doesn't change dispatch.workgroup_size,
        // so workgroup_validation should match between audit and
        // audit_optimized.
        let desc = KernelDescriptor {
            id: "wg".into(),
            bindings: BindingLayout { slots: vec![] },
            dispatch: Dispatch::new(2048, 1, 1), // intentionally over baseline
            body: KernelBody {
                ops: vec![],
                child_bodies: vec![],
                literals: vec![],
            },
        };
        let raw = audit(&desc);
        let optimized = audit_optimized(&desc);
        assert_eq!(raw.workgroup_validation, optimized.workgroup_validation);
    }

    #[test]
    fn subgroup_op_promotes_capability_in_audit() {
        let desc = KernelDescriptor {
            id: "sg".into(),
            bindings: BindingLayout { slots: vec![] },
            dispatch: Dispatch::new(64, 1, 1),
            body: KernelBody {
                ops: vec![KernelOp {
                    kind: KernelOpKind::SubgroupBallot,
                    operands: vec![0],
                    result: Some(0),
                }],
                child_bodies: vec![],
                literals: vec![],
            },
        };
        let report = audit(&desc);
        assert!(report.requires_action());
        assert!(report.subgroup.capabilities.ballot);
    }
}
