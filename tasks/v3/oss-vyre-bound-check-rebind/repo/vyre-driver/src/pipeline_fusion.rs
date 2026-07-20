//! N4 substrate: cross-pipeline disjoint-binding fusion analysis.
//!
//! When two consecutive dispatches read/write disjoint slot sets,
//! they can fuse into one launch with a single workgroup-bounded
//! fence instead of going through the full grid-sync /
//! pipeline-barrier path. C4's [`crate::arm_independence`] already
//! detects the disjoint case for in-megakernel arms; this module
//! lifts the same analysis to cross-pipeline boundaries.
//!
//! Pure decision  -  no allocation in the disjoint path, no IR walk.
//! The runtime side (actually fusing the two pipelines into one
//! launch) lives in `runtime_megakernel` and `driver_shared` and is
//! out of this module's scope; this module just answers "would it
//! be safe to fuse?"
//!
//! ## Why not just reuse `arm_independence`?
//!
//! Same boolean answer ("disjoint => safe"), different verdict
//! semantics. `ArmIndependenceVerdict::Independent` means "launch
//! these two arms on independent streams"; `CrossPipelineFusionDecision::Fuse`
//! means "launch these two pipelines as one cooperative cluster
//! with a workgroup-bounded fence between them." A backend that
//! can do one but not the other reads the right verdict.

use crate::arm_independence::{
    can_dispatch_concurrently, ArmBindingSummary, ArmIndependenceVerdict,
};

/// Verdict from [`decide_cross_pipeline_fusion`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CrossPipelineFusionDecision {
    /// Both pipelines touch disjoint resources; the runtime can fuse
    /// them into one launch with a workgroup-bounded fence.
    Fuse,
    /// At least one binding race; the runtime must keep them as
    /// separate pipelines with a full grid-sync between them.
    KeepSeparate {
        /// Why fusion is unsafe; mirrors the underlying arm-independence
        /// reason so telemetry can attribute the missed fusion.
        reason: CrossPipelineConflict,
    },
}

/// Reason cross-pipeline fusion is unsafe between two consecutive
/// dispatches. Mirrors [`crate::arm_independence::ArmConflict`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CrossPipelineConflict {
    /// Both pipelines write the same binding slot.
    WriteWriteConflict,
    /// First pipeline writes a slot the second reads.
    ReadAfterWrite,
    /// First pipeline reads a slot the second writes.
    WriteAfterRead,
}

/// Decide whether two consecutive pipelines can fuse into one
/// launch with a workgroup-bounded fence. Pure set arithmetic on
/// the per-pipeline binding summaries; no allocation in the disjoint
/// path.
#[must_use]
pub fn decide_cross_pipeline_fusion(
    earlier: &ArmBindingSummary,
    later: &ArmBindingSummary,
) -> CrossPipelineFusionDecision {
    match can_dispatch_concurrently(earlier, later) {
        ArmIndependenceVerdict::Independent => CrossPipelineFusionDecision::Fuse,
        ArmIndependenceVerdict::SerializeRequired { reason } => {
            CrossPipelineFusionDecision::KeepSeparate {
                reason: match reason {
                    crate::arm_independence::ArmConflict::WriteWriteConflict => {
                        CrossPipelineConflict::WriteWriteConflict
                    }
                    crate::arm_independence::ArmConflict::ReadAfterWrite => {
                        CrossPipelineConflict::ReadAfterWrite
                    }
                    crate::arm_independence::ArmConflict::WriteAfterRead => {
                        CrossPipelineConflict::WriteAfterRead
                    }
                },
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn summary(reads: &[u32], writes: &[u32]) -> ArmBindingSummary {
        ArmBindingSummary {
            reads: reads.iter().copied().collect(),
            writes: writes.iter().copied().collect(),
        }
    }

    #[test]
    fn disjoint_pipelines_fuse() {
        let a = summary(&[0, 1], &[2]);
        let b = summary(&[3, 4], &[5]);
        assert_eq!(
            decide_cross_pipeline_fusion(&a, &b),
            CrossPipelineFusionDecision::Fuse
        );
    }

    #[test]
    fn write_write_conflict_keeps_separate() {
        let a = summary(&[0], &[2]);
        let b = summary(&[1], &[2]);
        assert_eq!(
            decide_cross_pipeline_fusion(&a, &b),
            CrossPipelineFusionDecision::KeepSeparate {
                reason: CrossPipelineConflict::WriteWriteConflict,
            }
        );
    }

    #[test]
    fn read_after_write_keeps_separate() {
        let a = summary(&[0], &[2]);
        let b = summary(&[2], &[3]);
        assert_eq!(
            decide_cross_pipeline_fusion(&a, &b),
            CrossPipelineFusionDecision::KeepSeparate {
                reason: CrossPipelineConflict::ReadAfterWrite,
            }
        );
    }

    #[test]
    fn read_only_share_same_slot_fuses() {
        // Two pipelines reading the same slot is always safe to fuse.
        let a = summary(&[0, 1], &[2]);
        let b = summary(&[0, 1], &[3]);
        assert_eq!(
            decide_cross_pipeline_fusion(&a, &b),
            CrossPipelineFusionDecision::Fuse
        );
    }
}
