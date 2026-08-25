//! Output type for the vec-packing analysis.

use serde::{Deserialize, Serialize};
use vyre_lower::analyses::AccessKind;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PackKind {
    Vec2,
    Vec3,
    Vec4,
}

impl PackKind {
    #[must_use]
    pub const fn lane_count(&self) -> u32 {
        match self {
            Self::Vec2 => 2,
            Self::Vec3 => 3,
            Self::Vec4 => 4,
        }
    }

    /// Estimated throughput multiplier vs N scalar accesses.
    /// Most architectures get nearly the full N×; real-world
    /// observed 3-4x on Vec4 once cache effects settle in.
    #[must_use]
    pub fn throughput_factor(&self) -> f32 {
        // Conservative: assume hardware achieves 0.85 × the theoretical
        // peak.
        self.lane_count() as f32 * 0.85
    }
}

/// A detected group of adjacent ops that can fuse into one packed op.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PackGroup {
    /// Op-index range in the kernel body's flat ops vec. Inclusive on
    /// both ends. The entire range fuses into ONE packed op at the
    /// same logical position.
    pub start_op_index: usize,
    pub end_op_index: usize,
    pub kind: AccessKind,
    pub binding_slot: u32,
    pub pack: PackKind,
}

impl PackGroup {
    /// Number of scalar accesses that pack into one vector op. Matches
    /// `PackKind::lane_count()`  -  kept on PackGroup as a convenience.
    #[must_use]
    pub fn op_count(&self) -> usize {
        self.pack.lane_count() as usize
    }

    /// Op-stream span the group covers. NOT the number of accesses
    /// (use `op_count()` for that). Useful for emit-time slicing.
    #[must_use]
    pub fn op_stream_span(&self) -> usize {
        self.end_op_index - self.start_op_index + 1
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PackingPlan {
    pub kernel_id: String,
    pub groups: Vec<PackGroup>,
}

impl PackingPlan {
    /// Total scalar ops that would be eliminated if every group is
    /// packed (each Vec4 group eliminates 3 scalar ops, etc.).
    #[must_use]
    pub fn ops_eliminated(&self) -> usize {
        self.groups.iter().map(|g| g.op_count() - 1).sum()
    }

    /// Sum of `(lane_count - 1) * throughput_factor` per group  -
    /// a single score where higher means more wins available.
    #[must_use]
    pub fn estimated_savings_score(&self) -> f32 {
        self.groups
            .iter()
            .map(|g| (g.pack.lane_count() as f32 - 1.0) * g.pack.throughput_factor())
            .sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vec2_lane_count_is_2() {
        assert_eq!(PackKind::Vec2.lane_count(), 2);
    }

    #[test]
    fn vec4_lane_count_is_4() {
        assert_eq!(PackKind::Vec4.lane_count(), 4);
    }

    #[test]
    fn throughput_factor_grows_with_lane_count() {
        let v2 = PackKind::Vec2.throughput_factor();
        let v4 = PackKind::Vec4.throughput_factor();
        assert!(v4 > v2);
        assert!((v2 - 1.7).abs() < 1e-5); // 2 * 0.85
        assert!((v4 - 3.4).abs() < 1e-5); // 4 * 0.85
    }

    #[test]
    fn empty_plan_eliminates_no_ops() {
        let p = PackingPlan {
            kernel_id: "empty".into(),
            groups: vec![],
        };
        assert_eq!(p.ops_eliminated(), 0);
        assert!((p.estimated_savings_score() - 0.0).abs() < 1e-6);
    }

    #[test]
    fn vec4_group_eliminates_three_ops() {
        let p = PackingPlan {
            kernel_id: "k".into(),
            groups: vec![PackGroup {
                start_op_index: 0,
                end_op_index: 3,
                kind: AccessKind::Load,
                binding_slot: 0,
                pack: PackKind::Vec4,
            }],
        };
        assert_eq!(p.ops_eliminated(), 3);
    }

    #[test]
    fn savings_score_aggregates() {
        let p = PackingPlan {
            kernel_id: "k".into(),
            groups: vec![
                PackGroup {
                    start_op_index: 0,
                    end_op_index: 3,
                    kind: AccessKind::Load,
                    binding_slot: 0,
                    pack: PackKind::Vec4,
                },
                PackGroup {
                    start_op_index: 4,
                    end_op_index: 5,
                    kind: AccessKind::Store,
                    binding_slot: 1,
                    pack: PackKind::Vec2,
                },
            ],
        };
        // Vec4: 3 * 3.4 = 10.2; Vec2: 1 * 1.7 = 1.7; total 11.9.
        assert!((p.estimated_savings_score() - 11.9).abs() < 1e-4);
    }

    #[test]
    fn pack_group_op_count_inclusive() {
        let g = PackGroup {
            start_op_index: 5,
            end_op_index: 8,
            kind: AccessKind::Load,
            binding_slot: 0,
            pack: PackKind::Vec4,
        };
        assert_eq!(g.op_count(), 4);
    }
}
