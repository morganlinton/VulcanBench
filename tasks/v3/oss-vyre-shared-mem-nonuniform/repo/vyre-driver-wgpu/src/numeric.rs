use vyre_driver::BackendError;

/// Convert a host `usize` into a GPU/API `u64` with a single loud boundary policy.
pub(crate) fn usize_to_u64(value: usize, label: &str) -> Result<u64, BackendError> {
    vyre_driver::numeric::usize_to_u64(value, label, "WGPU")
}

/// Convert a rounded finite nanosecond value into telemetry storage.
pub(crate) fn rounded_f64_to_u64(value: f64, label: &str) -> Result<u64, BackendError> {
    vyre_driver::numeric::rounded_f64_to_u64(value, label, "WGPU")
}

/// Compute basis points in a `u64` telemetry domain with WGPU-labelled
/// diagnostics.
pub(crate) fn ratio_basis_points_u64_wide(
    part: u64,
    whole: u64,
    denominator_zero_value: u64,
    label: &str,
) -> u64 {
    vyre_driver::numeric::ratio_basis_points_u64_wide(
        part,
        whole,
        denominator_zero_value,
        label,
        "WGPU",
    )
}

/// Pad a WGPU byte count to the 4-byte copy/alignment rule.
pub(crate) fn align_up_u64(value: u64, min_value: u64, label: &str) -> Result<u64, BackendError> {
    vyre_driver::numeric::align_up_u64(value, 4, min_value, label, "WGPU")
}

/// Pad a WGPU byte count to the 4-byte copy/alignment rule.
pub(crate) fn align_up_usize(
    value: usize,
    min_value: usize,
    label: &str,
) -> Result<usize, BackendError> {
    vyre_driver::numeric::align_up_usize(value, 4, min_value, label, "WGPU")
}
