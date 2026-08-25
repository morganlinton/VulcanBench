//! Frozen subgroup (warp) reduction-operation contracts.
//!
//! Warp-scoped collective reductions across the active subgroup lanes. Distinct
//! from [`crate::collective_op::CollectiveOp`], which is distributed-scoped
//! (NCCL/MPI) and lacks the multiplicative reduction the warp ISA exposes.
//! This is the complete set the hardware/`naga::SubgroupOperation` surface
//! supports.
// TAG RESERVATIONS: Add=0x01, Mul=0x02, Min=0x03, Max=0x04, And=0x05,
// Or=0x06, Xor=0x07, 0x08..=0x7F reserved.

/// Reduction operator applied across the active subgroup lanes.
///
/// Stability: matches must include a fallback arm so the contract can grow
/// without breaking `SemVer`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Deserialize, serde::Serialize)]
#[non_exhaustive]
pub enum SubgroupReduceOp {
    /// Sum across the subgroup.
    Add,
    /// Product across the subgroup.
    Mul,
    /// Minimum across the subgroup.
    Min,
    /// Maximum across the subgroup.
    Max,
    /// Bitwise AND across the subgroup.
    And,
    /// Bitwise OR across the subgroup.
    Or,
    /// Bitwise XOR across the subgroup.
    Xor,
}

impl SubgroupReduceOp {
    /// Every builtin reduction operator, in wire-tag order.
    pub const ALL: [Self; 7] = [
        Self::Add,
        Self::Mul,
        Self::Min,
        Self::Max,
        Self::And,
        Self::Or,
        Self::Xor,
    ];

    /// Frozen builtin wire tag for this reduction operator.
    #[must_use]
    pub const fn builtin_wire_tag(self) -> u8 {
        match self {
            Self::Add => 0x01,
            Self::Mul => 0x02,
            Self::Min => 0x03,
            Self::Max => 0x04,
            Self::And => 0x05,
            Self::Or => 0x06,
            Self::Xor => 0x07,
        }
    }

    /// Decode a frozen builtin wire tag.
    ///
    /// # Errors
    ///
    /// Returns an actionable diagnostic when `tag` is not assigned.
    pub fn from_wire_tag(tag: u8) -> Result<Self, String> {
        match tag {
            0x01 => Ok(Self::Add),
            0x02 => Ok(Self::Mul),
            0x03 => Ok(Self::Min),
            0x04 => Ok(Self::Max),
            0x05 => Ok(Self::And),
            0x06 => Ok(Self::Or),
            0x07 => Ok(Self::Xor),
            value => Err(format!(
                "Fix: unknown SubgroupReduceOp tag {value}; use a Program serializer compatible with this vyre version."
            )),
        }
    }

    /// Lower-case spelling used in op-id strings and diagnostics.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Add => "add",
            Self::Mul => "mul",
            Self::Min => "min",
            Self::Max => "max",
            Self::And => "and",
            Self::Or => "or",
            Self::Xor => "xor",
        }
    }

    /// True when this operator is bitwise (integer-only): `And`/`Or`/`Xor`.
    ///
    /// Bitwise reductions reject floating-point operands during type checking.
    #[must_use]
    pub const fn is_bitwise(self) -> bool {
        matches!(self, Self::And | Self::Or | Self::Xor)
    }

    /// Canonical integer reduction of `lanes` under this operator.
    ///
    /// This is the single source of truth for the operator's semantics:
    /// the CPU reference oracle, constant folding, and any host-side
    /// evaluation route through it so they cannot drift. Wrapping arithmetic
    /// matches the GPU ISA (`redux.sync` / `subgroupAdd` wrap on overflow).
    /// Neutral elements: `Add`=0, `Mul`=1, `Min`=`u32::MAX`, `Max`=0,
    /// `And`=`u32::MAX`, `Or`=0, `Xor`=0.
    #[must_use]
    pub fn reduce_u32(self, lanes: impl IntoIterator<Item = u32>) -> u32 {
        let lanes = lanes.into_iter();
        match self {
            Self::Add => lanes.fold(0u32, u32::wrapping_add),
            Self::Mul => lanes.fold(1u32, u32::wrapping_mul),
            Self::Min => lanes.fold(u32::MAX, u32::min),
            Self::Max => lanes.fold(0u32, u32::max),
            Self::And => lanes.fold(u32::MAX, |acc, lane| acc & lane),
            Self::Or => lanes.fold(0u32, |acc, lane| acc | lane),
            Self::Xor => lanes.fold(0u32, |acc, lane| acc ^ lane),
        }
    }

    /// Floating-point identity (neutral) element for this operator, or `None`
    /// for the bitwise operators (`And`/`Or`/`Xor`), which are integer-only.
    ///
    /// Callers fold with [`Self::combine_f32`] starting from this identity so
    /// they can apply their own per-step canonicalization (e.g. NaN folding).
    #[must_use]
    pub fn f32_identity(self) -> Option<f32> {
        match self {
            Self::Add => Some(0.0),
            Self::Mul => Some(1.0),
            Self::Min => Some(f32::INFINITY),
            Self::Max => Some(f32::NEG_INFINITY),
            Self::And | Self::Or | Self::Xor => None,
        }
    }

    /// Combine one f32 lane into a running accumulator, or `None` for the
    /// bitwise operators (`And`/`Or`/`Xor`), which are integer-only.
    #[must_use]
    pub fn combine_f32(self, acc: f32, lane: f32) -> Option<f32> {
        match self {
            Self::Add => Some(acc + lane),
            Self::Mul => Some(acc * lane),
            Self::Min => Some(acc.min(lane)),
            Self::Max => Some(acc.max(lane)),
            Self::And | Self::Or | Self::Xor => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wire_tag_roundtrips_every_op() {
        for op in SubgroupReduceOp::ALL {
            let tag = op.builtin_wire_tag();
            assert_eq!(
                SubgroupReduceOp::from_wire_tag(tag).unwrap(),
                op,
                "Fix: SubgroupReduceOp wire tag {tag:#04x} must round-trip"
            );
        }
    }

    #[test]
    fn wire_tags_are_distinct_and_dense() {
        let tags: Vec<u8> = SubgroupReduceOp::ALL
            .iter()
            .map(|op| op.builtin_wire_tag())
            .collect();
        assert_eq!(tags, vec![0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07]);
    }

    #[test]
    fn unknown_tag_is_rejected_loudly() {
        let err = SubgroupReduceOp::from_wire_tag(0x42).unwrap_err();
        assert!(err.contains("unknown SubgroupReduceOp tag 66"), "{err}");
    }

    #[test]
    fn reduce_u32_computes_exact_values() {
        let lanes = [3u32, 1, 4, 1, 5];
        assert_eq!(SubgroupReduceOp::Add.reduce_u32(lanes), 14);
        assert_eq!(SubgroupReduceOp::Mul.reduce_u32(lanes), 60);
        assert_eq!(SubgroupReduceOp::Min.reduce_u32(lanes), 1);
        assert_eq!(SubgroupReduceOp::Max.reduce_u32(lanes), 5);
        assert_eq!(SubgroupReduceOp::And.reduce_u32([0b1100u32, 0b1010]), 0b1000);
        assert_eq!(SubgroupReduceOp::Or.reduce_u32([0b1100u32, 0b1010]), 0b1110);
        assert_eq!(SubgroupReduceOp::Xor.reduce_u32([0b1100u32, 0b1010]), 0b0110);
    }

    #[test]
    fn reduce_u32_add_wraps_like_the_isa() {
        assert_eq!(SubgroupReduceOp::Add.reduce_u32([u32::MAX, 1]), 0);
        assert_eq!(SubgroupReduceOp::Mul.reduce_u32([u32::MAX, 2]), u32::MAX - 1);
    }

    #[test]
    fn reduce_u32_empty_yields_neutral() {
        assert_eq!(SubgroupReduceOp::Add.reduce_u32([]), 0);
        assert_eq!(SubgroupReduceOp::Mul.reduce_u32([]), 1);
        assert_eq!(SubgroupReduceOp::Min.reduce_u32([]), u32::MAX);
        assert_eq!(SubgroupReduceOp::Max.reduce_u32([]), 0);
        assert_eq!(SubgroupReduceOp::And.reduce_u32([]), u32::MAX);
    }

    #[test]
    fn f32_reduce_helpers_match_op_and_reject_bitwise() {
        let lanes = [3.0f32, 1.0, 4.0];
        let fold = |op: SubgroupReduceOp| {
            op.f32_identity()
                .map(|id| lanes.iter().fold(id, |acc, &l| op.combine_f32(acc, l).unwrap()))
        };
        assert_eq!(fold(SubgroupReduceOp::Add), Some(8.0));
        assert_eq!(fold(SubgroupReduceOp::Mul), Some(12.0));
        assert_eq!(fold(SubgroupReduceOp::Min), Some(1.0));
        assert_eq!(fold(SubgroupReduceOp::Max), Some(4.0));
        // Bitwise ops have no f32 identity.
        assert_eq!(SubgroupReduceOp::And.f32_identity(), None);
        assert_eq!(SubgroupReduceOp::Or.combine_f32(1.0, 2.0), None);
    }

    #[test]
    fn only_bitwise_ops_are_bitwise() {
        assert!(SubgroupReduceOp::And.is_bitwise());
        assert!(SubgroupReduceOp::Or.is_bitwise());
        assert!(SubgroupReduceOp::Xor.is_bitwise());
        assert!(!SubgroupReduceOp::Add.is_bitwise());
        assert!(!SubgroupReduceOp::Mul.is_bitwise());
        assert!(!SubgroupReduceOp::Min.is_bitwise());
        assert!(!SubgroupReduceOp::Max.is_bitwise());
    }
}
