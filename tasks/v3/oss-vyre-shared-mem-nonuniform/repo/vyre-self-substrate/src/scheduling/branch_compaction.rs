//! Branch compaction planning for predicate-frontier execution.

use std::collections::BTreeSet;

/// One branch candidate before CUDA launch compaction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BranchArm {
    /// Stable branch id from the producer IR.
    pub id: u32,
    /// Predicate-lane count that will execute this arm.
    pub active_lanes: u32,
    /// Total predicate-lane count observed for the branch site.
    pub total_lanes: u32,
    /// Bytes of launch parameter payload for this arm.
    pub parameter_bytes: u32,
}

/// One retained branch arm after compaction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompactedBranchArm {
    /// Stable branch id from the producer IR.
    pub id: u32,
    /// Predicate-lane count that will execute this arm.
    pub active_lanes: u32,
    /// Active/total density in basis points.
    pub density_bps: u32,
    /// Launch parameter byte offset in the compacted parameter slab.
    pub parameter_offset: u32,
    /// Launch parameter byte length.
    pub parameter_bytes: u32,
}

/// Deterministic CUDA branch-compaction plan.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BranchCompactionPlan {
    /// Non-empty arms in execution order.
    pub arms: Vec<CompactedBranchArm>,
    /// Total active predicate lanes retained.
    pub retained_lanes: u64,
    /// Inactive predicate lanes eliminated before CUDA launch.
    pub eliminated_lanes: u64,
    /// Bytes required by the compacted parameter slab.
    pub compacted_parameter_bytes: u32,
}

/// Branch compaction errors.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BranchCompactionError {
    /// Duplicate branch id.
    DuplicateBranch {
        /// Duplicate id.
        id: u32,
    },
    /// Active lane count exceeds total lane count.
    ActiveExceedsTotal {
        /// Branch id.
        id: u32,
        /// Active lanes.
        active_lanes: u32,
        /// Total lanes.
        total_lanes: u32,
    },
    /// Parameter-slab offset overflowed.
    ParameterByteOverflow,
}

impl std::fmt::Display for BranchCompactionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DuplicateBranch { id } => write!(
                f,
                "branch compaction received duplicate branch id {id}. Fix: preserve stable unique branch ids before CUDA launch planning."
            ),
            Self::ActiveExceedsTotal {
                id,
                active_lanes,
                total_lanes,
            } => write!(
                f,
                "branch {id} has active_lanes={active_lanes} greater than total_lanes={total_lanes}. Fix: compute predicate histograms before branch compaction."
            ),
            Self::ParameterByteOverflow => write!(
                f,
                "branch compaction parameter slab overflowed u32 offsets. Fix: shard branch arms before CUDA launch planning."
            ),
        }
    }
}

impl std::error::Error for BranchCompactionError {}

/// Plan branch compaction by dropping zero-lane arms and packing parameters.
pub fn plan_branch_compaction(
    branches: &[BranchArm],
) -> Result<BranchCompactionPlan, BranchCompactionError> {
    let mut ids = BTreeSet::new();
    let mut ordered = branches.to_vec();
    ordered.sort_unstable_by_key(|branch| (std::cmp::Reverse(branch.active_lanes), branch.id));

    let mut arms = Vec::new();
    let mut retained_lanes = 0_u64;
    let mut eliminated_lanes = 0_u64;
    let mut parameter_offset = 0_u32;

    for branch in ordered {
        if !ids.insert(branch.id) {
            return Err(BranchCompactionError::DuplicateBranch { id: branch.id });
        }
        if branch.active_lanes > branch.total_lanes {
            return Err(BranchCompactionError::ActiveExceedsTotal {
                id: branch.id,
                active_lanes: branch.active_lanes,
                total_lanes: branch.total_lanes,
            });
        }
        retained_lanes += u64::from(branch.active_lanes);
        eliminated_lanes += u64::from(branch.total_lanes - branch.active_lanes);
        if branch.active_lanes == 0 {
            continue;
        }
        let density_bps = if branch.total_lanes == 0 {
            0
        } else {
            ((u64::from(branch.active_lanes) * 10_000) / u64::from(branch.total_lanes)) as u32
        };
        arms.push(CompactedBranchArm {
            id: branch.id,
            active_lanes: branch.active_lanes,
            density_bps,
            parameter_offset,
            parameter_bytes: branch.parameter_bytes,
        });
        parameter_offset = parameter_offset
            .checked_add(branch.parameter_bytes)
            .ok_or(BranchCompactionError::ParameterByteOverflow)?;
    }

    Ok(BranchCompactionPlan {
        arms,
        retained_lanes,
        eliminated_lanes,
        compacted_parameter_bytes: parameter_offset,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn branch_compaction_skips_empty_arms_and_packs_parameters() {
        let plan =
            plan_branch_compaction(&[arm(10, 0, 64, 8), arm(20, 48, 64, 16), arm(30, 16, 64, 12)])
                .expect("Fix: valid branches should compact");

        assert_eq!(
            plan.arms,
            vec![
                CompactedBranchArm {
                    id: 20,
                    active_lanes: 48,
                    density_bps: 7_500,
                    parameter_offset: 0,
                    parameter_bytes: 16,
                },
                CompactedBranchArm {
                    id: 30,
                    active_lanes: 16,
                    density_bps: 2_500,
                    parameter_offset: 16,
                    parameter_bytes: 12,
                },
            ]
        );
        assert_eq!(plan.retained_lanes, 64);
        assert_eq!(plan.eliminated_lanes, 128);
        assert_eq!(plan.compacted_parameter_bytes, 28);
    }

    #[test]
    fn branch_compaction_orders_equal_density_by_stable_id() {
        let plan = plan_branch_compaction(&[arm(3, 4, 8, 4), arm(1, 4, 8, 4)])
            .expect("Fix: valid branches should compact deterministically");

        assert_eq!(plan.arms[0].id, 1);
        assert_eq!(plan.arms[1].id, 3);
    }

    #[test]
    fn branch_compaction_rejects_invalid_histograms() {
        assert_eq!(
            plan_branch_compaction(&[arm(1, 1, 2, 4), arm(1, 1, 2, 4)])
                .expect_err("duplicate branch ids should fail"),
            BranchCompactionError::DuplicateBranch { id: 1 }
        );
        assert_eq!(
            plan_branch_compaction(&[arm(2, 3, 2, 4)])
                .expect_err("active lanes above total should fail"),
            BranchCompactionError::ActiveExceedsTotal {
                id: 2,
                active_lanes: 3,
                total_lanes: 2,
            }
        );
    }

    fn arm(id: u32, active_lanes: u32, total_lanes: u32, parameter_bytes: u32) -> BranchArm {
        BranchArm {
            id,
            active_lanes,
            total_lanes,
            parameter_bytes,
        }
    }
}
