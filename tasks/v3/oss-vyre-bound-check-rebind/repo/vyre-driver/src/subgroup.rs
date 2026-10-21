//! Backend-neutral subgroup operation taxonomy.

/// Canonical subgroup intrinsic operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum SubgroupOp {
    /// Broadcast a value from one subgroup lane to all lanes.
    Broadcast,
    /// Reduce add across the subgroup.
    Add,
    /// Reduce max across the subgroup.
    Max,
    /// Reduce min across the subgroup.
    Min,
    /// Inclusive scan add across the subgroup.
    InclusiveAdd,
    /// Exclusive scan add across the subgroup.
    ExclusiveAdd,
    /// Shuffle-xor butterfly swap.
    ShuffleXor,
}

impl SubgroupOp {
    /// Iterate every canonical operation.
    #[must_use]
    pub const fn all() -> &'static [SubgroupOp] {
        &[
            SubgroupOp::Broadcast,
            SubgroupOp::Add,
            SubgroupOp::Max,
            SubgroupOp::Min,
            SubgroupOp::InclusiveAdd,
            SubgroupOp::ExclusiveAdd,
            SubgroupOp::ShuffleXor,
        ]
    }
}

/// Subgroup capability record shared by validation and optimizers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SubgroupCaps {
    /// Native subgroup operations are available for compute.
    pub supports_subgroup: bool,
    /// Subgroup operations are available in vertex-stage contexts.
    pub supports_subgroup_vertex: bool,
    /// Subgroup size in lanes; `0` means unknown.
    pub subgroup_size: u32,
}

impl SubgroupCaps {
    /// Capability record for native subgroup intrinsics.
    #[must_use]
    pub const fn native(subgroup_size: u32) -> Self {
        Self {
            supports_subgroup: true,
            supports_subgroup_vertex: false,
            subgroup_size,
        }
    }

    /// Capability record from a feature bit and reported lane-size range.
    #[must_use]
    pub const fn from_feature_range(
        supports_feature: bool,
        supports_vertex_stage: bool,
        min_size: u32,
        max_size: u32,
    ) -> Self {
        let supports_subgroup = supports_feature && min_size > 0 && max_size >= min_size;
        Self {
            supports_subgroup,
            supports_subgroup_vertex: supports_vertex_stage && supports_subgroup,
            subgroup_size: if supports_subgroup { min_size } else { 0 },
        }
    }

    /// Return true when native subgroup operations are usable.
    #[must_use]
    pub const fn is_usable(self) -> bool {
        self.supports_subgroup && self.subgroup_size > 0
    }
}

/// Canonical lane offsets for a power-of-two full-subgroup tree reduction.
#[must_use]
pub fn reduction_offsets(subgroup_size: u32) -> Vec<u32> {
    let mut offsets = Vec::new();
    reduction_offsets_into(subgroup_size, &mut offsets);
    offsets
}

/// Fallible canonical lane offsets for a full-subgroup tree reduction.
///
/// # Errors
///
/// Returns an error when the requested subgroup width cannot be rounded to a
/// power-of-two reduction width or the output vector cannot reserve storage.
pub fn try_reduction_offsets(subgroup_size: u32) -> Result<Vec<u32>, String> {
    let mut offsets = Vec::new();
    try_reduction_offsets_into(subgroup_size, &mut offsets)?;
    Ok(offsets)
}

/// Write canonical reduction offsets into caller-owned storage.
pub fn reduction_offsets_into(subgroup_size: u32, offsets: &mut Vec<u32>) {
    // Clearing to empty on failure silently produces a degenerate reduction
    // (no offsets -> the subgroup reduction reads nothing) — a silent fallback
    // (Law 10). Fail loud; callers use try_reduction_offsets_into.
    if let Err(error) = try_reduction_offsets_into(subgroup_size, offsets) {
        panic!("vyre-driver reduction offsets generation failed: {error}");
    }
}

/// Fallibly write canonical reduction offsets into caller-owned storage.
///
/// # Errors
///
/// Returns an error when the subgroup width overflows power-of-two rounding or
/// the output vector cannot reserve the required offsets.
pub fn try_reduction_offsets_into(
    subgroup_size: u32,
    offsets: &mut Vec<u32>,
) -> Result<(), String> {
    offsets.clear();
    let Some(rounded_width) = subgroup_size.checked_next_power_of_two() else {
        return Err(format!(
            "subgroup reduction width {subgroup_size} cannot be rounded to a power of two. Fix: clamp subgroup size to a valid backend-reported hardware width."
        ));
    };
    let offset_count = if subgroup_size == 0 {
        0
    } else {
        rounded_width.ilog2() as usize
    };
    crate::allocation::try_reserve_vec_to_capacity(offsets, offset_count).map_err(|error| {
        format!(
            "subgroup reduction offsets could not reserve {offset_count} slot(s): {error}. Fix: reuse caller-owned offset storage or clamp subgroup size."
        )
    })?;
    let mut width = rounded_width / 2;
    while width > 0 {
        offsets.push(width);
        width /= 2;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_enumerates_seven_ops() {
        assert_eq!(SubgroupOp::all().len(), 7);
    }

    #[test]
    fn try_reduction_offsets_reuses_storage() {
        let mut offsets = Vec::with_capacity(8);
        let ptr = offsets.as_ptr();

        try_reduction_offsets_into(32, &mut offsets).unwrap();

        assert_eq!(offsets, [16, 8, 4, 2, 1]);
        assert_eq!(offsets.as_ptr(), ptr);
    }

    #[test]
    fn try_reduction_offsets_rejects_overflowing_rounding() {
        let error = try_reduction_offsets(u32::MAX).unwrap_err();
        assert!(error.contains("cannot be rounded to a power of two"));
    }

    #[test]
    #[should_panic(expected = "reduction offsets generation failed")]
    fn reduction_offsets_into_fails_loud_on_invalid_width() {
        // An un-representable subgroup width must fail LOUD, not silently clear
        // the buffer (which would yield a degenerate reduction; Law 10).
        let mut offsets = vec![16, 8, 4];
        reduction_offsets_into(u32::MAX, &mut offsets);
    }

    #[test]
    #[should_panic(expected = "reduction offsets generation failed")]
    fn reduction_offsets_owned_fails_loud_on_invalid_width() {
        let _ = reduction_offsets(u32::MAX);
    }
}
