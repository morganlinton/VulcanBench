//! Runtime segment planning for cross-pipeline fusion.
//!
//! `vyre-driver::pipeline_fusion` answers whether two adjacent binding
//! summaries may fuse. This module turns a whole pipeline sequence into
//! launch segments, so concrete backends can record one dispatch per
//! segment instead of rediscovering the grouping policy.

use smallvec::SmallVec;
use vyre_driver::arm_independence::ArmBindingSummary;
use vyre_driver::pipeline_fusion::{
    decide_cross_pipeline_fusion, CrossPipelineConflict, CrossPipelineFusionDecision,
};

/// One contiguous runtime launch segment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PipelineFusionSegment {
    /// First pipeline index in the segment.
    pub start: usize,
    /// Exclusive end index in the segment.
    pub end: usize,
}

impl PipelineFusionSegment {
    /// Number of pipelines in this segment.
    #[must_use]
    pub fn len(&self) -> usize {
        self.end.saturating_sub(self.start)
    }

    /// True when this segment contains no pipelines.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// Conflict that ended a segment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PipelineFusionBreak {
    /// Earlier pipeline index.
    pub earlier: usize,
    /// Later pipeline index.
    pub later: usize,
    /// Conflict reason from the shared driver decision.
    pub reason: CrossPipelineConflict,
}

/// Full runtime plan for consecutive pipeline fusion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrossPipelineFusionPlan {
    /// Contiguous launch segments.
    pub segments: SmallVec<[PipelineFusionSegment; 8]>,
    /// Fusion breaks, useful for telemetry and missed-fusion counters.
    pub breaks: SmallVec<[PipelineFusionBreak; 8]>,
}

impl CrossPipelineFusionPlan {
    /// True when at least one segment contains more than one pipeline.
    #[must_use]
    pub fn contains_fused_segment(&self) -> bool {
        self.segments.iter().any(|segment| segment.len() > 1)
    }
}

/// Plan fused launch segments for a sequence of adjacent pipelines.
#[must_use]
pub fn plan_cross_pipeline_fusion(summaries: &[ArmBindingSummary]) -> CrossPipelineFusionPlan {
    if summaries.is_empty() {
        return CrossPipelineFusionPlan {
            segments: SmallVec::new(),
            breaks: SmallVec::new(),
        };
    }

    let mut segments = SmallVec::new();
    let mut breaks = SmallVec::new();
    let mut start = 0usize;
    for idx in 1..summaries.len() {
        match decide_cross_pipeline_fusion(&summaries[idx - 1], &summaries[idx]) {
            CrossPipelineFusionDecision::Fuse => {}
            CrossPipelineFusionDecision::KeepSeparate { reason } => {
                segments.push(PipelineFusionSegment { start, end: idx });
                breaks.push(PipelineFusionBreak {
                    earlier: idx - 1,
                    later: idx,
                    reason,
                });
                start = idx;
            }
        }
    }
    segments.push(PipelineFusionSegment {
        start,
        end: summaries.len(),
    });
    CrossPipelineFusionPlan { segments, breaks }
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
    fn empty_sequence_has_no_segments() {
        let plan = plan_cross_pipeline_fusion(&[]);
        assert!(plan.segments.is_empty());
        assert!(plan.breaks.is_empty());
    }

    #[test]
    fn disjoint_sequence_becomes_one_segment() {
        let plan = plan_cross_pipeline_fusion(&[
            summary(&[0], &[1]),
            summary(&[2], &[3]),
            summary(&[4], &[5]),
        ]);
        assert_eq!(
            plan.segments.as_slice(),
            &[PipelineFusionSegment { start: 0, end: 3 }]
        );
        assert!(plan.breaks.is_empty());
        assert!(plan.contains_fused_segment());
    }

    #[test]
    fn conflict_splits_segments_with_reason() {
        let plan = plan_cross_pipeline_fusion(&[
            summary(&[0], &[1]),
            summary(&[1], &[2]),
            summary(&[3], &[4]),
        ]);
        assert_eq!(
            plan.segments.as_slice(),
            &[
                PipelineFusionSegment { start: 0, end: 1 },
                PipelineFusionSegment { start: 1, end: 3 },
            ]
        );
        assert_eq!(
            plan.breaks.as_slice(),
            &[PipelineFusionBreak {
                earlier: 0,
                later: 1,
                reason: CrossPipelineConflict::ReadAfterWrite,
            }]
        );
    }

    #[test]
    fn read_only_overlap_stays_fused() {
        let plan = plan_cross_pipeline_fusion(&[summary(&[0], &[1]), summary(&[0, 2], &[3])]);
        assert_eq!(
            plan.segments.as_slice(),
            &[PipelineFusionSegment { start: 0, end: 2 }]
        );
    }
}
