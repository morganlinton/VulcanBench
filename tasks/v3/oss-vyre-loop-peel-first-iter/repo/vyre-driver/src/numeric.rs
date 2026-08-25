//! Backend-neutral numeric boundary conversions.
//!
//! Concrete GPU backends cross the same host/API boundaries: host sizes become
//! API `u64`s, high-resolution timers become telemetry `u64`s, and device
//! timestamp deltas arrive as rounded floating-point nanoseconds. This module is
//! the single policy for those lossy or fallible conversions; backend crates add
//! only the backend label that makes the diagnostic actionable.

use std::time::Instant;

use crate::BackendError;

/// Integer basis-point denominator: 10_000 bps = 100%.
pub const BASIS_POINTS_DENOMINATOR: u32 = 10_000;

/// Backend-bound numeric conversion policy.
///
/// Backends should keep their label in one constant of this type instead of
/// cloning one local wrapper per numeric helper. The free functions below remain
/// available for backend-neutral callers and tests.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BackendNumericPolicy {
    backend: &'static str,
}

impl BackendNumericPolicy {
    /// Create a numeric policy that annotates diagnostics with `backend`.
    #[must_use]
    pub const fn new(backend: &'static str) -> Self {
        Self { backend }
    }

    /// Return the backend label used in numeric diagnostics.
    #[must_use]
    pub const fn backend(self) -> &'static str {
        self.backend
    }

    /// Convert a host `usize` to a backend/API `u64`.
    ///
    /// # Errors
    /// Returns [`BackendError::InvalidProgram`] when the value cannot fit in
    /// the backend/API boundary type.
    pub fn usize_to_u64(self, value: usize, label: &str) -> Result<u64, BackendError> {
        usize_to_u64(value, label, self.backend)
    }

    /// Convert a wide counter to telemetry `u64`.
    ///
    /// # Errors
    /// Returns [`BackendError::InvalidProgram`] when the counter does not fit in
    /// telemetry storage.
    pub fn u128_to_u64(self, value: u128, label: &str) -> Result<u64, BackendError> {
        u128_to_u64(value, label, self.backend)
    }

    /// Convert elapsed wall-clock time to telemetry nanoseconds.
    ///
    /// # Errors
    /// Returns [`BackendError::InvalidProgram`] when the elapsed nanoseconds
    /// cannot fit in telemetry storage.
    pub fn elapsed_nanos_u64(self, started: Instant, label: &str) -> Result<u64, BackendError> {
        elapsed_nanos_u64(started, label, self.backend)
    }

    /// Round a finite floating-point nanosecond value into telemetry storage.
    ///
    /// # Errors
    /// Returns [`BackendError::InvalidProgram`] when the rounded value is
    /// negative, non-finite, or too large for telemetry storage.
    pub fn rounded_f64_to_u64(self, value: f64, label: &str) -> Result<u64, BackendError> {
        rounded_f64_to_u64(value, label, self.backend)
    }

    /// Compute `part / whole` as floor basis points in a `u32` telemetry domain.
    #[must_use]
    pub fn ratio_basis_points_u64(
        self,
        part: u64,
        whole: u64,
        denominator_zero_value: u32,
        label: &str,
    ) -> u32 {
        ratio_basis_points_u64(part, whole, denominator_zero_value, label, self.backend)
    }

    /// Compute `part / whole` as floor basis points in a `u64` telemetry domain.
    #[must_use]
    pub fn ratio_basis_points_u64_wide(
        self,
        part: u64,
        whole: u64,
        denominator_zero_value: u64,
        label: &str,
    ) -> u64 {
        ratio_basis_points_u64_wide(part, whole, denominator_zero_value, label, self.backend)
    }

    /// Compute `part / whole` as floor parts-per-million.
    #[must_use]
    pub fn ratio_parts_per_million_u64(
        self,
        part: u64,
        whole: u64,
        denominator_zero_value: u32,
        label: &str,
    ) -> u32 {
        ratio_parts_per_million_u64(part, whole, denominator_zero_value, label, self.backend)
    }

    /// Compose two basis-point multipliers into a `u32` result.
    #[must_use]
    pub fn compose_basis_points_u32(self, left: u32, right: u32, label: &str) -> u32 {
        compose_basis_points_u32(left, right, label, self.backend)
    }

    /// Apply rounded basis-point scaling with optional high clamp.
    #[must_use]
    pub fn scale_u64_by_basis_points_round_clamped(
        self,
        base: u64,
        scale_bps: u32,
        zero_scale_value: u64,
        max_scale_bps: u32,
        label: &str,
    ) -> u64 {
        scale_u64_by_basis_points_round_clamped(
            base,
            scale_bps,
            zero_scale_value,
            max_scale_bps,
            label,
            self.backend,
        )
    }

    /// Apply floor basis-point scaling with a lower bound.
    #[must_use]
    pub fn scale_u64_by_basis_points_floor_min(
        self,
        base: u64,
        scale_bps: u32,
        min_value: u64,
        label: &str,
    ) -> u64 {
        scale_u64_by_basis_points_floor_min(base, scale_bps, min_value, label, self.backend)
    }

    /// Convert finite non-negative floating-point telemetry to `u32` by truncation.
    #[must_use]
    pub fn finite_f64_to_u32_trunc(self, value: f64, label: &str) -> u32 {
        finite_f64_to_u32_trunc(value, label, self.backend)
    }

    /// Convert finite non-negative floating-point telemetry to rounded `u32`.
    #[must_use]
    pub fn finite_f64_to_u32_round(self, value: f64, label: &str) -> u32 {
        finite_f64_to_u32_round(value, label, self.backend)
    }

    /// Convert a finite floating-point ratio into floor basis points.
    #[must_use]
    pub fn finite_f64_ratio_basis_points_trunc(
        self,
        numerator: f64,
        denominator: f64,
        invalid_numerator_value: u32,
        invalid_denominator_value: u32,
        label: &str,
    ) -> u32 {
        finite_f64_ratio_basis_points_trunc(
            numerator,
            denominator,
            invalid_numerator_value,
            invalid_denominator_value,
            label,
            self.backend,
        )
    }

    /// Convert a finite floating-point ratio into rounded basis points.
    #[must_use]
    pub fn finite_f64_ratio_basis_points_round(
        self,
        numerator: f64,
        denominator: f64,
        invalid_numerator_value: u32,
        invalid_denominator_value: u32,
        label: &str,
    ) -> u32 {
        finite_f64_ratio_basis_points_round(
            numerator,
            denominator,
            invalid_numerator_value,
            invalid_denominator_value,
            label,
            self.backend,
        )
    }

    /// Convert a finite scalar where `1.0 == 10_000 bps` into floor basis points.
    #[must_use]
    pub fn finite_f64_unit_basis_points_trunc(
        self,
        value: f64,
        invalid_value: u32,
        label: &str,
    ) -> u32 {
        finite_f64_unit_basis_points_trunc(value, invalid_value, label, self.backend)
    }

    /// Compute `ceil(value / divisor)` in `u64`, returning `None` for zero
    /// divisors or arithmetic overflow.
    #[must_use]
    pub fn checked_ceil_div_u64(self, value: u64, divisor: u64) -> Option<u64> {
        checked_ceil_div_u64(value, divisor)
    }

    /// Multiply three `u32` launch dimensions into a `u64` without wraparound.
    #[must_use]
    pub fn checked_dim_product_u64(self, dims: [u32; 3]) -> Option<u64> {
        checked_dim_product_u64(dims)
    }

    /// Multiply three `u32` launch dimensions into a `u32` without wraparound.
    #[must_use]
    pub fn checked_dim_product_u32(self, dims: [u32; 3]) -> Option<u32> {
        checked_dim_product_u32(dims)
    }

    /// Align `value` upward to `alignment`, after applying `min_value`.
    ///
    /// # Errors
    /// Returns [`BackendError::InvalidProgram`] when `alignment` is zero or the
    /// padded value would overflow `u64`.
    pub fn align_up_u64(
        self,
        value: u64,
        alignment: u64,
        min_value: u64,
        label: &str,
    ) -> Result<u64, BackendError> {
        align_up_u64(value, alignment, min_value, label, self.backend)
    }

    /// Align `value` upward to `alignment`, after applying `min_value`.
    ///
    /// # Errors
    /// Returns [`BackendError::InvalidProgram`] when `alignment` is zero or the
    /// padded value would overflow `usize`.
    pub fn align_up_usize(
        self,
        value: usize,
        alignment: usize,
        min_value: usize,
        label: &str,
    ) -> Result<usize, BackendError> {
        align_up_usize(value, alignment, min_value, label, self.backend)
    }
}

/// Convert a host `usize` to a backend/API `u64`.
///
/// # Errors
/// Returns [`BackendError::InvalidProgram`] when the value cannot fit in the
/// backend/API boundary type.
pub fn usize_to_u64(value: usize, label: &str, backend: &str) -> Result<u64, BackendError> {
    u64::try_from(value).map_err(|source| BackendError::InvalidProgram {
        fix: format!(
            "Fix: {backend} {label} cannot fit u64: {source}; split the workload before crossing the host/device boundary."
        ),
    })
}

/// Convert a wide counter to telemetry `u64`.
///
/// # Errors
/// Returns [`BackendError::InvalidProgram`] when the counter does not fit in
/// telemetry storage.
pub fn u128_to_u64(value: u128, label: &str, backend: &str) -> Result<u64, BackendError> {
    u64::try_from(value).map_err(|source| BackendError::InvalidProgram {
        fix: format!(
            "Fix: {backend} {label} cannot fit u64: {source}; split the dispatch before telemetry overflows."
        ),
    })
}

/// Convert elapsed wall-clock time to telemetry nanoseconds.
///
/// # Errors
/// Returns [`BackendError::InvalidProgram`] when the elapsed nanoseconds cannot
/// fit in telemetry storage.
pub fn elapsed_nanos_u64(
    started: Instant,
    label: &str,
    backend: &str,
) -> Result<u64, BackendError> {
    u128_to_u64(started.elapsed().as_nanos(), label, backend)
}

/// Round a finite floating-point nanosecond value into telemetry storage.
///
/// # Errors
/// Returns [`BackendError::InvalidProgram`] when the rounded value is negative,
/// non-finite, or too large for telemetry storage.
pub fn rounded_f64_to_u64(value: f64, label: &str, backend: &str) -> Result<u64, BackendError> {
    let rounded = value.round();
    if !rounded.is_finite() || rounded < 0.0 || rounded > u64::MAX as f64 {
        return Err(BackendError::InvalidProgram {
            fix: format!(
                "Fix: {backend} {label} value {value} cannot fit u64 after rounding; inspect device timing and split the dispatch before telemetry overflows."
            ),
        });
    }
    u64::try_from(rounded as u128).map_err(|source| BackendError::InvalidProgram {
        fix: format!(
            "Fix: {backend} {label} rounded value cannot fit u64: {source}; inspect device timing and split the dispatch before telemetry overflows."
        ),
    })
}

/// Compute `part / whole` as floor basis points with explicit zero-denominator
/// policy and saturating telemetry overflow.
///
/// CUDA release-path planners use the same ratio encoding for memory pressure,
/// readback savings, and device-side compaction. Keeping the arithmetic here
/// prevents each backend module from carrying its own unchecked `as u32` cast.
#[must_use]
pub fn ratio_basis_points_u64(
    part: u64,
    whole: u64,
    denominator_zero_value: u32,
    label: &str,
    backend: &str,
) -> u32 {
    let value = ratio_basis_points_u64_wide(
        part,
        whole,
        u64::from(denominator_zero_value),
        label,
        backend,
    );
    if value > u64::from(u32::MAX) {
        tracing::error!(
            "{backend} {label} basis-points value exceeded u32. Fix: shard or normalize the telemetry domain before release-path planning."
        );
        return u32::MAX;
    }
    value as u32
}

/// Compute `part / whole` as floor basis points in a `u64` telemetry domain
/// with explicit zero-denominator policy and loud overflow pinning.
#[must_use]
pub fn ratio_basis_points_u64_wide(
    part: u64,
    whole: u64,
    denominator_zero_value: u64,
    label: &str,
    backend: &str,
) -> u64 {
    if whole == 0 {
        return denominator_zero_value;
    }
    let value = (u128::from(part) * u128::from(BASIS_POINTS_DENOMINATOR)) / u128::from(whole);
    if value > u128::from(u64::MAX) {
        tracing::error!(
            "{backend} {label} basis-points value exceeded u64. Fix: shard or normalize the telemetry domain before release-path planning."
        );
        return u64::MAX;
    }
    value as u64
}

/// Compute `part / whole` as floor parts-per-million with explicit
/// zero-denominator policy and loud `u32` overflow pinning.
#[must_use]
pub fn ratio_parts_per_million_u64(
    part: u64,
    whole: u64,
    denominator_zero_value: u32,
    label: &str,
    backend: &str,
) -> u32 {
    if whole == 0 {
        return denominator_zero_value;
    }
    let value = (u128::from(part) * 1_000_000) / u128::from(whole);
    if value > u128::from(u32::MAX) {
        tracing::error!(
            "{backend} {label} parts-per-million value exceeded u32. Fix: shard or normalize telemetry before release-path planning."
        );
        return u32::MAX;
    }
    value as u32
}

/// Compose two basis-point multipliers as `(left * right) / 10_000`, with
/// widened arithmetic and loud `u32` overflow pinning.
#[must_use]
pub fn compose_basis_points_u32(left: u32, right: u32, label: &str, backend: &str) -> u32 {
    let value = (u128::from(left) * u128::from(right)) / u128::from(BASIS_POINTS_DENOMINATOR);
    if value > u128::from(u32::MAX) {
        tracing::error!(
            "{backend} {label} composed basis-points value exceeded u32. Fix: normalize chained multipliers before release-path planning."
        );
        return u32::MAX;
    }
    value as u32
}

/// Compose two basis-point multipliers as `(left * right) / 10_000`, returning
/// `None` rather than saturating when the composed value cannot fit `u64`.
#[must_use]
pub fn checked_compose_basis_points_u64(left: u64, right: u64) -> Option<u64> {
    let value = (u128::from(left) * u128::from(right)) / u128::from(BASIS_POINTS_DENOMINATOR);
    u64::try_from(value).ok()
}

/// Apply a basis-point multiplier to a `u64` with nearest-integer rounding,
/// optional high clamp, and explicit zero-scale policy.
#[must_use]
pub fn scale_u64_by_basis_points_round_clamped(
    base: u64,
    scale_bps: u32,
    zero_scale_value: u64,
    max_scale_bps: u32,
    label: &str,
    backend: &str,
) -> u64 {
    if scale_bps == 0 {
        return zero_scale_value;
    }
    let clamped = if max_scale_bps == 0 {
        scale_bps
    } else {
        scale_bps.min(max_scale_bps)
    };
    let value = (u128::from(base) * u128::from(clamped) + u128::from(BASIS_POINTS_DENOMINATOR / 2))
        / u128::from(BASIS_POINTS_DENOMINATOR);
    if value > u128::from(u64::MAX) {
        tracing::error!(
            "{backend} {label} rounded basis-point scaling exceeded u64. Fix: shard or normalize the cost domain before extraction."
        );
        return u64::MAX;
    }
    value as u64
}

/// Apply a basis-point multiplier to a `u64` with floor rounding and an output
/// lower bound.
#[must_use]

pub fn scale_u64_by_basis_points_floor_min(
    base: u64,
    scale_bps: u32,
    min_value: u64,
    label: &str,
    backend: &str,
) -> u64 {
    let value = (u128::from(base) * u128::from(scale_bps)) / u128::from(BASIS_POINTS_DENOMINATOR);
    if value > u128::from(u64::MAX) {
        tracing::error!(
            "{backend} {label} floor basis-point scaling exceeded u64. Fix: shard or normalize the cost domain before extraction."
        );
        return u64::MAX;
    }
    (value as u64).max(min_value)
}

/// Weight a `u64` cost by basis points into a widened exact `u128` domain.
#[must_use]
pub fn weighted_u64_by_basis_points_u128(value: u64, basis_points: u32) -> u128 {
    (u128::from(value) * u128::from(basis_points)) / u128::from(BASIS_POINTS_DENOMINATOR)
}

/// Convert a finite non-negative floating-point telemetry value to `u32` by
/// truncating toward zero, with loud saturation on invalid or oversized input.
#[must_use]
pub fn finite_f64_to_u32_trunc(value: f64, label: &str, backend: &str) -> u32 {
    if !value.is_finite() {
        tracing::error!(
            "{backend} {label} value {value} is not finite. Fix: normalize telemetry before release-path planning."
        );
        return u32::MAX;
    }
    if value <= 0.0 {
        return 0;
    }
    if value > f64::from(u32::MAX) {
        tracing::error!(
            "{backend} {label} value {value} cannot fit u32. Fix: shard or normalize telemetry before release-path planning."
        );
        return u32::MAX;
    }
    value as u32
}

/// Convert a finite non-negative floating-point telemetry value to `u32` after
/// rounding to the nearest integer, with loud saturation on invalid input.
#[must_use]
pub fn finite_f64_to_u32_round(value: f64, label: &str, backend: &str) -> u32 {
    let rounded = value.round();
    if !rounded.is_finite() {
        tracing::error!(
            "{backend} {label} rounded value {rounded} is not finite. Fix: normalize telemetry before release-path planning."
        );
        return u32::MAX;
    }
    if rounded <= 0.0 {
        return 0;
    }
    if rounded > f64::from(u32::MAX) {
        tracing::error!(
            "{backend} {label} rounded value {rounded} cannot fit u32. Fix: shard or normalize telemetry before release-path planning."
        );
        return u32::MAX;
    }
    rounded as u32
}

/// Convert a finite floating-point ratio into floor basis points, with separate
/// policies for invalid numerators and denominators.
#[must_use]
pub fn finite_f64_ratio_basis_points_trunc(
    numerator: f64,
    denominator: f64,
    invalid_numerator_value: u32,
    invalid_denominator_value: u32,
    label: &str,
    backend: &str,
) -> u32 {
    finite_f64_ratio_basis_points(
        numerator,
        denominator,
        invalid_numerator_value,
        invalid_denominator_value,
        label,
        backend,
        finite_f64_to_u32_trunc,
    )
}

/// Convert a finite floating-point ratio into rounded basis points, with
/// separate policies for invalid numerators and denominators.
#[must_use]
pub fn finite_f64_ratio_basis_points_round(
    numerator: f64,
    denominator: f64,
    invalid_numerator_value: u32,
    invalid_denominator_value: u32,
    label: &str,
    backend: &str,
) -> u32 {
    finite_f64_ratio_basis_points(
        numerator,
        denominator,
        invalid_numerator_value,
        invalid_denominator_value,
        label,
        backend,
        finite_f64_to_u32_round,
    )
}

/// Convert a finite scalar where `1.0 == 10_000 bps` into floor basis points.
#[must_use]
pub fn finite_f64_unit_basis_points_trunc(
    value: f64,
    invalid_value: u32,
    label: &str,
    backend: &str,
) -> u32 {
    if !value.is_finite() {
        tracing::error!(
            "{backend} {label} value {value} is not finite. Fix: normalize telemetry before release-path planning."
        );
        return invalid_value;
    }
    finite_f64_to_u32_trunc(
        value.max(0.0) * f64::from(BASIS_POINTS_DENOMINATOR),
        label,
        backend,
    )
}

fn finite_f64_ratio_basis_points(
    numerator: f64,
    denominator: f64,
    invalid_numerator_value: u32,
    invalid_denominator_value: u32,
    label: &str,
    backend: &str,
    convert: fn(f64, &str, &str) -> u32,
) -> u32 {
    if !numerator.is_finite() {
        tracing::error!(
            "{backend} {label} numerator {numerator} is not finite. Fix: record finite dispatch timing before release-path planning."
        );
        return invalid_numerator_value;
    }
    if !denominator.is_finite() || denominator <= 0.0 {
        tracing::error!(
            "{backend} {label} denominator {denominator} is not finite and positive. Fix: record finite dispatch timing before release-path planning."
        );
        return invalid_denominator_value;
    }
    if numerator <= 0.0 {
        return 0;
    }
    convert(
        (numerator / denominator) * f64::from(BASIS_POINTS_DENOMINATOR),
        label,
        backend,
    )
}

/// Compute `ceil(value / divisor)` in `u64`, returning `None` for zero divisors
/// or arithmetic overflow.
#[must_use]
pub fn checked_ceil_div_u64(value: u64, divisor: u64) -> Option<u64> {
    if divisor == 0 {
        return None;
    }
    if value == 0 {
        return Some(0);
    }
    ((value - 1) / divisor).checked_add(1)
}

/// Multiply three `u32` dimensions into a `u64` without wraparound.
///
/// CUDA, WGPU, and runtime launch geometry all cross this same host/device
/// boundary. Keeping the primitive here prevents each backend from carrying a
/// slightly different overflow policy for `[x, y, z]` launch dimensions.
#[must_use]
pub fn checked_dim_product_u64(dims: [u32; 3]) -> Option<u64> {
    u64::from(dims[0])
        .checked_mul(u64::from(dims[1]))
        .and_then(|xy| xy.checked_mul(u64::from(dims[2])))
}

/// Multiply three `u32` dimensions into a `u32` without wraparound.
#[must_use]
pub fn checked_dim_product_u32(dims: [u32; 3]) -> Option<u32> {
    u32::try_from(checked_dim_product_u64(dims)?).ok()
}

/// Align `value` upward to `alignment`, after applying `min_value`.
///
/// # Errors
/// Returns [`BackendError::InvalidProgram`] when `alignment` is zero or the
/// padded value would overflow `u64`.
pub fn align_up_u64(
    value: u64,
    alignment: u64,
    min_value: u64,
    label: &str,
    backend: &str,
) -> Result<u64, BackendError> {
    if alignment == 0 {
        return Err(BackendError::InvalidProgram {
            fix: format!("Fix: {backend} {label} alignment must be non-zero before padding."),
        });
    }
    let normalized = value.max(min_value);
    let remainder = normalized % alignment;
    if remainder == 0 {
        return Ok(normalized);
    }
    normalized
        .checked_add(alignment - remainder)
        .ok_or_else(|| BackendError::InvalidProgram {
            fix: format!(
                "Fix: {backend} {label} overflows u64 while padding to {alignment}-byte alignment; split the workload before crossing the host/device boundary."
            ),
        })
}

/// Align `value` upward to `alignment`, after applying `min_value`.
///
/// # Errors
/// Returns [`BackendError::InvalidProgram`] when `alignment` is zero or the
/// padded value would overflow `usize`.
pub fn align_up_usize(
    value: usize,
    alignment: usize,
    min_value: usize,
    label: &str,
    backend: &str,
) -> Result<usize, BackendError> {
    if alignment == 0 {
        return Err(BackendError::InvalidProgram {
            fix: format!("Fix: {backend} {label} alignment must be non-zero before padding."),
        });
    }
    let normalized = value.max(min_value);
    let remainder = normalized % alignment;
    if remainder == 0 {
        return Ok(normalized);
    }
    normalized
        .checked_add(alignment - remainder)
        .ok_or_else(|| BackendError::InvalidProgram {
            fix: format!(
                "Fix: {backend} {label} overflows usize while padding to {alignment}-byte alignment; split the workload before crossing the host/device boundary."
            ),
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn usize_boundary_accepts_fit_values() {
        assert_eq!(usize_to_u64(17, "bytes", "test").unwrap(), 17);
    }

    #[test]
    fn backend_numeric_policy_carries_backend_label_without_local_wrappers() {
        let policy = BackendNumericPolicy::new("CUDA");
        assert_eq!(policy.backend(), "CUDA");
        assert_eq!(policy.usize_to_u64(17, "bytes").unwrap(), 17);
        assert_eq!(policy.ratio_basis_points_u64(1, 4, 0, "pressure"), 2_500);
        assert_eq!(
            policy.finite_f64_ratio_basis_points_round(1.0, 6.0, 99, 77, "ratio"),
            1_667
        );
        assert_eq!(policy.checked_ceil_div_u64(65_537, 65_536), Some(2));
        assert_eq!(
            policy.checked_dim_product_u64([65_535, 2, 3]),
            Some(393_210)
        );
        assert_eq!(
            policy.checked_dim_product_u32([65_535, 2, 3]),
            Some(393_210)
        );

        let err = policy
            .u128_to_u64(u128::from(u64::MAX) + 1, "resident bytes")
            .unwrap_err();
        let rendered = err.to_string();
        assert!(
            rendered.contains("CUDA resident bytes"),
            "backend policy diagnostics must carry the backend label and boundary name: {rendered}"
        );
    }

    #[test]
    fn u128_boundary_rejects_overflow_with_backend_label() {
        let err = u128_to_u64(u128::from(u64::MAX) + 1, "counter", "test").unwrap_err();
        let rendered = err.to_string();
        assert!(
            rendered.contains("test counter"),
            "numeric boundary diagnostics must identify the backend and label: {rendered}"
        );
    }

    #[test]
    fn rounded_f64_rejects_non_finite_values() {
        let err = rounded_f64_to_u64(f64::NAN, "timestamp", "test").unwrap_err();
        let rendered = err.to_string();
        assert!(
            rendered.contains("timestamp"),
            "rounded timestamp diagnostics must include the failing label: {rendered}"
        );
    }

    #[test]
    fn ratio_basis_points_preserves_zero_denominator_policy() {
        assert_eq!(
            ratio_basis_points_u64(1, 0, u32::MAX, "pressure", "test"),
            u32::MAX
        );
        assert_eq!(ratio_basis_points_u64(0, 0, 0, "savings", "test"), 0);
    }

    #[test]
    fn ratio_basis_points_uses_wide_arithmetic_before_clamping() {
        assert_eq!(
            ratio_basis_points_u64(u64::MAX, u64::MAX / 2, 0, "wide", "test"),
            20_000
        );
        assert_eq!(
            ratio_basis_points_u64(u64::MAX, 1, 0, "overflow", "test"),
            u32::MAX
        );
    }

    #[test]
    fn wide_ratio_basis_points_retains_u64_telemetry_domain() {
        assert_eq!(ratio_basis_points_u64_wide(3, 2, 0, "wide", "test"), 15_000);
        assert_eq!(
            ratio_basis_points_u64_wide(u64::MAX, u64::MAX / 4, 0, "wide", "test"),
            40_000
        );
        assert_eq!(
            ratio_basis_points_u64_wide(u64::MAX, 1, 0, "overflow", "test"),
            u64::MAX
        );
    }

    #[test]
    fn finite_f64_to_u32_helpers_pin_invalid_values() {
        assert_eq!(finite_f64_to_u32_trunc(12.9, "value", "test"), 12);
        assert_eq!(finite_f64_to_u32_round(12.5, "value", "test"), 13);
        assert_eq!(finite_f64_to_u32_trunc(-1.0, "value", "test"), 0);
        assert_eq!(
            finite_f64_to_u32_round(f64::INFINITY, "value", "test"),
            u32::MAX
        );
        assert_eq!(
            finite_f64_to_u32_trunc(f64::from(u32::MAX) * 2.0, "value", "test"),
            u32::MAX
        );
    }

    #[test]
    fn finite_f64_basis_point_helpers_pin_invalid_policies() {
        assert_eq!(
            finite_f64_ratio_basis_points_trunc(1.0, 4.0, 99, 77, "ratio", "test"),
            2_500
        );
        assert_eq!(
            finite_f64_ratio_basis_points_round(1.0, 6.0, 99, 77, "ratio", "test"),
            1_667
        );
        assert_eq!(
            finite_f64_ratio_basis_points_trunc(f64::NAN, 1.0, 99, 77, "ratio", "test"),
            99
        );
        assert_eq!(
            finite_f64_ratio_basis_points_trunc(1.0, 0.0, 99, 77, "ratio", "test"),
            77
        );
        assert_eq!(
            finite_f64_ratio_basis_points_round(-1.0, 1.0, 99, 77, "ratio", "test"),
            0
        );
        assert_eq!(
            finite_f64_unit_basis_points_trunc(0.25, 33, "unit", "test"),
            2_500
        );
        assert_eq!(
            finite_f64_unit_basis_points_trunc(f64::INFINITY, 33, "unit", "test"),
            33
        );
    }

    #[test]
    fn alignment_helpers_pad_minimums_and_reject_overflow() {
        assert_eq!(align_up_u64(0, 4, 4, "copy", "test").unwrap(), 4);
        assert_eq!(align_up_u64(5, 4, 0, "copy", "test").unwrap(), 8);
        assert_eq!(align_up_usize(0, 4, 4, "copy", "test").unwrap(), 4);
        assert_eq!(align_up_usize(5, 4, 0, "copy", "test").unwrap(), 8);

        let zero_alignment = align_up_u64(1, 0, 0, "copy", "test").unwrap_err();
        assert!(
            zero_alignment
                .to_string()
                .contains("alignment must be non-zero"),
            "zero-alignment diagnostics must be actionable: {zero_alignment}"
        );

        let overflow_u64 = align_up_u64(u64::MAX, 4, 0, "copy", "test").unwrap_err();
        assert!(
            overflow_u64.to_string().contains("overflows u64"),
            "u64 alignment overflow diagnostics must name the target type: {overflow_u64}"
        );

        let overflow_usize = align_up_usize(usize::MAX, 4, 0, "copy", "test").unwrap_err();
        assert!(
            overflow_usize.to_string().contains("overflows usize"),
            "usize alignment overflow diagnostics must name the target type: {overflow_usize}"
        );
    }

    #[test]
    fn checked_ceil_div_u64_handles_cuda_queue_boundaries() {
        assert_eq!(checked_ceil_div_u64(0, 64), Some(0));
        assert_eq!(checked_ceil_div_u64(1, 64), Some(1));
        assert_eq!(checked_ceil_div_u64(65_537, 65_536), Some(2));
        assert_eq!(
            checked_ceil_div_u64(u64::MAX, 65_536),
            Some(281_474_976_710_656)
        );
        assert_eq!(checked_ceil_div_u64(u64::MAX, 1), Some(u64::MAX));
        assert_eq!(checked_ceil_div_u64(1, 0), None);
    }

    #[test]
    fn checked_dim_product_helpers_cover_cuda_launch_boundaries() {
        assert_eq!(checked_dim_product_u64([1, 1, 1]), Some(1));
        assert_eq!(checked_dim_product_u64([0, 999, 999]), Some(0));
        assert_eq!(checked_dim_product_u64([65_535, 2, 3]), Some(393_210));
        assert_eq!(checked_dim_product_u32([65_535, 2, 3]), Some(393_210));
        assert_eq!(
            checked_dim_product_u64([u32::MAX, u32::MAX, u32::MAX]),
            None
        );
        assert_eq!(checked_dim_product_u32([u32::MAX, 2, 1]), None);
    }

    #[test]
    fn generated_dim_product_matrix_matches_wide_integer_reference() {
        const VALUES: [u32; 9] = [0, 1, 2, 3, 7, 32, 255, 65_535, u32::MAX];
        for x in VALUES {
            for y in VALUES {
                for z in VALUES {
                    let wide = u128::from(x) * u128::from(y) * u128::from(z);
                    let expected_u64 = u64::try_from(wide).ok();
                    let expected_u32 = u32::try_from(wide).ok();
                    assert_eq!(checked_dim_product_u64([x, y, z]), expected_u64);
                    assert_eq!(checked_dim_product_u32([x, y, z]), expected_u32);
                }
            }
        }
    }

    #[test]
    fn ratio_parts_per_million_uses_wide_arithmetic_and_pins_overflow() {
        assert_eq!(
            ratio_parts_per_million_u64(1, 4, 0, "commit-rate", "test"),
            250_000
        );
        assert_eq!(
            ratio_parts_per_million_u64(1, 0, 7, "commit-rate", "test"),
            7
        );
        assert_eq!(
            ratio_parts_per_million_u64(u64::MAX, 1, 0, "commit-rate", "test"),
            u32::MAX
        );
    }

    #[test]
    fn basis_point_composition_and_scaling_helpers_are_widened() {
        assert_eq!(
            compose_basis_points_u32(15_000, 2_500, "compose", "test"),
            3_750
        );
        assert_eq!(
            compose_basis_points_u32(u32::MAX, u32::MAX, "compose", "test"),
            u32::MAX
        );
        assert_eq!(
            checked_compose_basis_points_u64(50_000, 20_000),
            Some(100_000)
        );
        assert_eq!(checked_compose_basis_points_u64(u64::MAX, u64::MAX), None);
        assert_eq!(
            scale_u64_by_basis_points_round_clamped(10, 1_000_000, 10, 40_000, "scale", "test"),
            40
        );
        assert_eq!(
            scale_u64_by_basis_points_round_clamped(7, 0, 7, 40_000, "scale", "test"),
            7
        );
        assert_eq!(
            scale_u64_by_basis_points_floor_min(1, 1, 1, "scale", "test"),
            1
        );
        assert_eq!(
            weighted_u64_by_basis_points_u128(u64::MAX, 10_000),
            u128::from(u64::MAX)
        );
    }
}
