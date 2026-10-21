//! Output type for the workgroup-uniform branch analysis.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BranchUniformity {
    /// All threads in the workgroup take the same path. Emit can use
    /// uniform-control-flow optimizations.
    Uniform,
    /// Threads diverge. Default to standard branch emission.
    Divergent,
    /// Phase-1 analysis cannot prove uniformity (e.g., condition
    /// depends on a load whose source we can't reason about).
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct BranchSite {
    /// Op-index of the structured-if op in the kernel body.
    pub op_index: usize,
    /// Operand id of the condition expression.
    pub cond_operand_id: u32,
    pub uniformity: BranchUniformity,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BranchEmitHint {
    /// The condition is proven workgroup-uniform; emitters may use
    /// target-native uniform-control-flow metadata or scheduling.
    UniformControlFlow,
    /// The branch diverges across lanes; emitters should avoid
    /// uniform-only assumptions and may consider predication.
    DivergentControlFlow,
    /// The analysis could not prove either side. Emit standard branch
    /// code and keep this site visible to profiling/PGO.
    NeedsProfile,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct BranchHint {
    pub code: &'static str,
    pub site: BranchSite,
    pub hint: BranchEmitHint,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct WorkgroupUniformReport {
    pub kernel_id: String,
    pub branches: Vec<BranchSite>,
}

impl WorkgroupUniformReport {
    #[must_use]
    pub fn uniform_count(&self) -> usize {
        self.branches
            .iter()
            .filter(|b| matches!(b.uniformity, BranchUniformity::Uniform))
            .count()
    }

    #[must_use]
    pub fn divergent_count(&self) -> usize {
        self.branches
            .iter()
            .filter(|b| matches!(b.uniformity, BranchUniformity::Divergent))
            .count()
    }

    /// Convert raw branch uniformity into backend-neutral emit hints.
    #[must_use]
    pub fn hints(&self) -> Vec<BranchHint> {
        self.branches.iter().map(hint_for_branch).collect()
    }
}

fn hint_for_branch(site: &BranchSite) -> BranchHint {
    match site.uniformity {
        BranchUniformity::Uniform => BranchHint {
            code: "I-BRANCH-UNIFORM",
            site: site.clone(),
            hint: BranchEmitHint::UniformControlFlow,
            message: format!(
                "Fix: op {} condition result {} is workgroup-uniform; emit the backend's uniform-control-flow hint.",
                site.op_index, site.cond_operand_id
            ),
        },
        BranchUniformity::Divergent => BranchHint {
            code: "W-BRANCH-DIVERGENT",
            site: site.clone(),
            hint: BranchEmitHint::DivergentControlFlow,
            message: format!(
                "Fix: op {} condition result {} is lane-divergent; consider predication or branch flattening.",
                site.op_index, site.cond_operand_id
            ),
        },
        BranchUniformity::Unknown => BranchHint {
            code: "N-BRANCH-UNKNOWN",
            site: site.clone(),
            hint: BranchEmitHint::NeedsProfile,
            message: format!(
                "Fix: op {} condition result {} has unknown uniformity; keep standard branch emission and feed runtime profile data back into the optimizer.",
                site.op_index, site.cond_operand_id
            ),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_report_has_zero_counts() {
        let r = WorkgroupUniformReport {
            kernel_id: "empty".into(),
            branches: vec![],
        };
        assert_eq!(r.uniform_count(), 0);
        assert_eq!(r.divergent_count(), 0);
    }

    #[test]
    fn counts_aggregate_correctly() {
        let r = WorkgroupUniformReport {
            kernel_id: "k".into(),
            branches: vec![
                BranchSite {
                    op_index: 0,
                    cond_operand_id: 1,
                    uniformity: BranchUniformity::Uniform,
                },
                BranchSite {
                    op_index: 5,
                    cond_operand_id: 8,
                    uniformity: BranchUniformity::Divergent,
                },
                BranchSite {
                    op_index: 10,
                    cond_operand_id: 12,
                    uniformity: BranchUniformity::Unknown,
                },
                BranchSite {
                    op_index: 15,
                    cond_operand_id: 20,
                    uniformity: BranchUniformity::Uniform,
                },
            ],
        };
        assert_eq!(r.uniform_count(), 2);
        assert_eq!(r.divergent_count(), 1);
    }

    #[test]
    fn hints_map_every_uniformity_class() {
        let r = WorkgroupUniformReport {
            kernel_id: "k".into(),
            branches: vec![
                BranchSite {
                    op_index: 0,
                    cond_operand_id: 1,
                    uniformity: BranchUniformity::Uniform,
                },
                BranchSite {
                    op_index: 5,
                    cond_operand_id: 8,
                    uniformity: BranchUniformity::Divergent,
                },
                BranchSite {
                    op_index: 10,
                    cond_operand_id: 12,
                    uniformity: BranchUniformity::Unknown,
                },
            ],
        };
        let hints = r.hints();
        assert_eq!(hints.len(), 3);
        assert_eq!(hints[0].code, "I-BRANCH-UNIFORM");
        assert_eq!(hints[0].hint, BranchEmitHint::UniformControlFlow);
        assert_eq!(hints[1].code, "W-BRANCH-DIVERGENT");
        assert_eq!(hints[1].hint, BranchEmitHint::DivergentControlFlow);
        assert_eq!(hints[2].code, "N-BRANCH-UNKNOWN");
        assert_eq!(hints[2].hint, BranchEmitHint::NeedsProfile);
        assert!(hints[2].message.contains("runtime profile"));
    }
}
