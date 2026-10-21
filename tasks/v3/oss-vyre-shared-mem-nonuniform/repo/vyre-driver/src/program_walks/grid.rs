//! Backend-neutral dispatch-grid inference.

use vyre_foundation::ir::Program;

use crate::backend::{BackendError, DispatchConfig};
use crate::binding::BindingPlan;
use crate::program_walks::dispatch_element_count_for_program;

/// Infer a concrete workgroup grid from a program ABI and dispatch inputs.
///
/// Explicit [`DispatchConfig::grid_override`] always wins. Otherwise this uses
/// the largest non-shared binding element count as the logical lane count and
/// derives a deterministic 1D/2D/3D grid from the effective workgroup shape.
///
/// # Errors
///
/// Returns when the program/input ABI cannot be planned or when inferred grid
/// dimensions overflow `u32`.
pub fn infer_dispatch_grid(
    program: &Program,
    inputs: &[Vec<u8>],
    config: &DispatchConfig,
) -> Result<[u32; 3], BackendError> {
    if let Some(grid) = config.grid_override {
        return Ok(grid);
    }
    let plan = BindingPlan::from_program(program, inputs)?;
    let element_count = dispatch_element_count_for_program(program, &plan.bindings);
    infer_dispatch_grid_for_count(
        element_count,
        config
            .workgroup_override
            .unwrap_or(program.workgroup_size()),
    )
}

/// Infer a grid size for a program based on its largest statically-known
/// non-shared binding and its workgroup size.
///
/// Bench cases and backends can use this when no explicit grid_override is provided.
///
/// # Errors
///
/// Returns when the program ABI cannot be planned or if inferred dimensions
/// overflow `u32`.
pub fn auto_grid(
    program: &Program,
    backend: &dyn crate::backend::VyreBackend,
) -> Result<[u32; 3], BackendError> {
    crate::validation::validate_program_for_backend(backend, program, &DispatchConfig::default())?;
    let plan = BindingPlan::build(program)?;
    let element_count = dispatch_element_count_for_program(program, &plan.bindings);

    infer_dispatch_grid_for_count(element_count, program.workgroup_size())
}

/// Infer a launch grid for a known logical element count and workgroup shape.
///
/// 1D kernels use a standard ceil-div over X lanes. 2D/3D kernels use a
/// square/cube-ish decomposition so common matrix-style programs with
/// `count = rows * cols` do not need driver-specific manual launch policy.
///
/// # Errors
///
/// Returns if any workgroup axis is zero or an inferred grid axis cannot fit
/// in `u32`.
pub fn infer_dispatch_grid_for_count(
    element_count: u32,
    workgroup: [u32; 3],
) -> Result<[u32; 3], BackendError> {
    if workgroup.contains(&0) {
        return Err(BackendError::new(
            "workgroup dimensions must be non-zero. Fix: set Program::workgroup_size and DispatchConfig::workgroup_override to positive values.",
        ));
    }
    let count = u64::from(element_count.max(1));
    if workgroup[1] == 1 && workgroup[2] == 1 {
        return Ok([ceil_div_u64(count, u64::from(workgroup[0]))?, 1, 1]);
    }
    if workgroup[2] == 1 {
        let side = ceil_sqrt_u64(count);
        return Ok([
            ceil_div_u64(side, u64::from(workgroup[0]))?,
            ceil_div_u64(
                u64::from(ceil_div_u64(count, side)?),
                u64::from(workgroup[1]),
            )?,
            1,
        ]);
    }
    let side = ceil_cuberoot_u64(count);
    let xy = side.checked_mul(side).ok_or_else(|| {
        BackendError::new(format!(
            "3D dispatch-grid side {side} overflows u64 square during shape planning. Fix: split the Program before GPU launch planning."
        ))
    })?;
    Ok([
        ceil_div_u64(side, u64::from(workgroup[0]))?,
        ceil_div_u64(side, u64::from(workgroup[1]))?,
        ceil_div_u64(u64::from(ceil_div_u64(count, xy)?), u64::from(workgroup[2]))?,
    ])
}

fn ceil_div_u64(value: u64, divisor: u64) -> Result<u32, BackendError> {
    let divided = value.div_ceil(divisor).max(1);
    u32::try_from(divided).map_err(|_| {
        BackendError::new(
            "inferred dispatch grid dimension overflowed u32. Fix: split the Program into smaller dispatches.",
        )
    })
}

fn ceil_sqrt_u64(value: u64) -> u64 {
    if value <= 1 {
        return 1;
    }
    let mut lo = 1_u64;
    let mut hi = 1_u64 << 32;
    while lo < hi {
        let mid = lo + ((hi - lo) / 2);
        match mid.checked_mul(mid) {
            Some(square) if square < value => lo = mid + 1,
            _ => hi = mid,
        }
    }
    lo
}

fn ceil_cuberoot_u64(value: u64) -> u64 {
    if value <= 1 {
        return 1;
    }
    let mut lo = 1_u64;
    let mut hi = 1_u64 << 22;
    while lo < hi {
        let mid = lo + ((hi - lo) / 2);
        match checked_cube_u64(mid) {
            Some(cube) if cube < value => lo = mid + 1,
            _ => hi = mid,
        }
    }
    lo
}

fn checked_cube_u64(value: u64) -> Option<u64> {
    value.checked_mul(value)?.checked_mul(value)
}

// ---------------------------------------------------------------------------
// N6 power-of-2 dispatch grid coercion + tail-mask
// ---------------------------------------------------------------------------

/// Result of coercing a logical element count up to the next power of two.
///
/// Backends that opt into the N6 substrate dispatch over `rounded_count`
/// lanes (so every workgroup is uniform-shape, no boundary divergence on
/// the last workgroup) and have the kernel guard each store with the
/// tail-mask predicate `lane_id < original_count`. Threads beyond the
/// original count no-op their stores.
///
/// The win is on tail handling for attention/softmax/reduce shapes where
/// the workload is not a multiple of the workgroup size  -  without
/// coercion the last workgroup runs with masked-out lanes that still
/// incur scheduling cost; with coercion every workgroup is identical
/// and the masked-out lanes are skipped via the predicate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TailMaskPolicy {
    /// Logical element count requested by the caller.
    pub original_count: u32,
    /// Element count after rounding up to the next power of two. Equal
    /// to `original_count` when it is already a power of two.
    pub rounded_count: u32,
    /// Convenience: `rounded_count - original_count`. Lanes in this
    /// suffix range must be predicated off by the kernel.
    pub tail_lanes: u32,
}

impl TailMaskPolicy {
    /// True when no rounding was needed; the dispatch can run as-is
    /// without a tail-mask predicate.
    #[must_use]
    pub fn is_aligned(&self) -> bool {
        self.tail_lanes == 0
    }
}

/// N6: round `element_count` up to the next power of two. Returns a
/// [`TailMaskPolicy`] that the lower/emit layer consumes to insert a
/// `lane_id < original_count` predicate around each store. Pure
/// arithmetic; no I/O.
///
/// `element_count == 0` is treated as 0 (rounded_count = 0, no tail).
/// `element_count == 1` rounds to 1 (already pow2).
/// `element_count` beyond `1 << 31` cannot be rounded inside `u32`; callers
/// that need to distinguish that condition must use
/// [`try_coerce_to_pow2_with_tail_mask`]. This legacy wrapper preserves the
/// original shape on overflow instead of panicking.
#[must_use]
pub fn coerce_to_pow2_with_tail_mask(element_count: u32) -> TailMaskPolicy {
    match try_coerce_to_pow2_with_tail_mask(element_count) {
        Ok(policy) => policy,
        Err(_error) => TailMaskPolicy {
            original_count: element_count,
            rounded_count: element_count,
            tail_lanes: 0,
        },
    }
}

/// Fallible N6 power-of-two dispatch-grid coercion.
///
/// # Errors
/// Returns when `element_count` cannot be rounded up inside `u32`.
pub fn try_coerce_to_pow2_with_tail_mask(
    element_count: u32,
) -> Result<TailMaskPolicy, BackendError> {
    if element_count == 0 {
        return Ok(TailMaskPolicy {
            original_count: 0,
            rounded_count: 0,
            tail_lanes: 0,
        });
    }
    let rounded = next_pow2_u32_checked(element_count)?;
    Ok(TailMaskPolicy {
        original_count: element_count,
        rounded_count: rounded,
        tail_lanes: rounded - element_count,
    })
}

fn next_pow2_u32_checked(value: u32) -> Result<u32, BackendError> {
    if value.is_power_of_two() {
        return Ok(value);
    }
    if value > (1u32 << 31) {
        return Err(BackendError::new(format!(
            "cannot round element_count={value} up to a power-of-two u32 grid without overflow. Fix: split the workload before grid-shape planning; do not silently saturate or fall back to an under-dispatching shape."
        )));
    }
    Ok(value.next_power_of_two())
}

#[cfg(test)]
mod n6_tests {
    use super::*;

    #[test]
    fn already_pow2_is_identity_with_no_tail() {
        let p = coerce_to_pow2_with_tail_mask(64);
        assert_eq!(p.original_count, 64);
        assert_eq!(p.rounded_count, 64);
        assert_eq!(p.tail_lanes, 0);
        assert!(p.is_aligned());
    }

    #[test]
    fn non_pow2_rounds_up_and_reports_tail() {
        let p = coerce_to_pow2_with_tail_mask(100);
        assert_eq!(p.original_count, 100);
        assert_eq!(p.rounded_count, 128);
        assert_eq!(p.tail_lanes, 28);
        assert!(!p.is_aligned());
    }

    #[test]
    fn one_is_pow2_no_tail() {
        let p = coerce_to_pow2_with_tail_mask(1);
        assert_eq!(p.rounded_count, 1);
        assert_eq!(p.tail_lanes, 0);
    }

    #[test]
    fn zero_passes_through_with_no_tail() {
        let p = coerce_to_pow2_with_tail_mask(0);
        assert_eq!(p.rounded_count, 0);
        assert_eq!(p.tail_lanes, 0);
        assert!(p.is_aligned());
    }

    #[test]
    fn large_value_below_2_31_rounds_normally() {
        let p = coerce_to_pow2_with_tail_mask(1_000_000_000);
        // 2^30 = 1_073_741_824
        assert_eq!(p.rounded_count, 1u32 << 30);
        assert_eq!(p.tail_lanes, (1u32 << 30) - 1_000_000_000);
    }

    #[test]
    fn value_above_2_31_errors_instead_of_saturating() {
        let error = try_coerce_to_pow2_with_tail_mask(u32::MAX)
            .expect_err("oversized power-of-two coercion must fail loudly");
        let message = error.to_string();
        assert!(
            message.contains("Fix:"),
            "oversized grid-shape error must be actionable"
        );
    }

    #[test]
    fn root_helpers_are_exact_at_large_boundaries() {
        assert_eq!(ceil_sqrt_u64((1_u64 << 32) - 1), 65_536);
        assert_eq!(ceil_sqrt_u64(1_u64 << 32), 65_536);
        assert_eq!(ceil_cuberoot_u64(2_642_245_u64.pow(3)), 2_642_245);
        assert_eq!(ceil_cuberoot_u64(2_642_245_u64.pow(3) - 1), 2_642_245);
    }

    #[test]
    fn dispatch_grid_planning_uses_integer_roots_and_typed_errors() {
        let source = include_str!("grid.rs");
        let production = source
            .split("#[cfg(test)]")
            .next()
            .expect("Fix: dispatch-grid production source must precede tests");

        assert!(
            !production.contains(" as f64")
                && !production.contains(".sqrt()")
                && !production.contains(".cbrt()"),
            "Fix: dispatch-grid inference must use deterministic integer root arithmetic."
        );
        assert!(
            production.contains("try_coerce_to_pow2_with_tail_mask")
                && !production.contains("panic!("),
            "Fix: dispatch-grid planning should expose typed errors instead of production panics."
        );
    }
}
