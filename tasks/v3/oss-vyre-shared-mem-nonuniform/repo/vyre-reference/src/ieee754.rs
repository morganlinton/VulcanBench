//! IEEE 754 float rules enforced by the parity engine.
//!
//! Until vyre IR gains full float variants, this module acts as a strict guard:
//! any code path that would require float semantics returns a deterministic error
//! rather than falling back to undefined or driver-dependent behavior. When float
//! support lands, this module will become the source of truth for rounding mode,
//! NaN propagation, and subnormal handling that the conform gate checks.

use vyre::Error;

/// Maximum accepted reference-oracle error against correctly rounded f32
/// transcendental results.
pub const REFERENCE_TRANSCENDENTAL_ULP_BUDGET: u32 = 4;

/// Maximum accepted backend-vs-reference error for programs containing f32
/// transcendental operations.
pub const BACKEND_TRANSCENDENTAL_ULP_BUDGET: u32 = 128;

/// Maximum accepted backend-vs-reference error for f32 programs without
/// transcendental operations under the default parity policy.
pub const BACKEND_ELEMENTARY_F32_ULP_BUDGET: u32 = 4;

/// Deterministic f32 sine used by the CPU parity oracle.
///
/// # Examples
///
/// ```
/// let y = vyre_reference::ieee754::canonical_sin(0.0);
/// assert_eq!(y.to_bits(), 0.0f32.to_bits());
/// ```
#[must_use]
#[inline]
pub fn canonical_sin(x: f32) -> f32 {
    libm::sinf(x)
}

/// Deterministic f32 cosine used by the CPU parity oracle.
///
/// # Examples
///
/// ```
/// let y = vyre_reference::ieee754::canonical_cos(0.0);
/// assert_eq!(y.to_bits(), 1.0f32.to_bits());
/// ```
#[must_use]
#[inline]
pub fn canonical_cos(x: f32) -> f32 {
    libm::cosf(x)
}

/// Deterministic f32 square root used by the CPU parity oracle.
///
/// # Examples
///
/// ```
/// let y = vyre_reference::ieee754::canonical_sqrt(4.0);
/// assert_eq!(y.to_bits(), 2.0f32.to_bits());
/// ```
#[must_use]
#[inline]
pub fn canonical_sqrt(x: f32) -> f32 {
    libm::sqrtf(x)
}

/// Deterministic f32 inverse square root used by the CPU parity oracle.
#[must_use]
#[inline]
pub fn canonical_inverse_sqrt(x: f32) -> f32 {
    1.0 / canonical_sqrt(x)
}

/// Deterministic f32 reciprocal used by the CPU parity oracle.
#[must_use]
#[inline]
pub fn canonical_reciprocal(x: f32) -> f32 {
    canonical_f32(1.0 / canonical_f32(x))
}

/// Deterministic f32 exponential used by the CPU parity oracle.
///
/// # Examples
///
/// ```
/// let y = vyre_reference::ieee754::canonical_exp(0.0);
/// assert_eq!(y.to_bits(), 1.0f32.to_bits());
/// ```
#[must_use]
#[inline]
pub fn canonical_exp(x: f32) -> f32 {
    libm::expf(x)
}

/// Deterministic f32 base-2 exponential used by the CPU parity oracle.
#[must_use]
#[inline]
pub fn canonical_exp2(x: f32) -> f32 {
    libm::exp2f(x)
}

/// Deterministic f32 natural logarithm used by the CPU parity oracle.
///
/// # Examples
///
/// ```
/// let y = vyre_reference::ieee754::canonical_log(1.0);
/// assert_eq!(y.to_bits(), 0.0f32.to_bits());
/// ```
#[must_use]
#[inline]
pub fn canonical_log(x: f32) -> f32 {
    libm::logf(x)
}

/// Deterministic f32 base-2 logarithm used by the CPU parity oracle.
#[must_use]
#[inline]
pub fn canonical_log2(x: f32) -> f32 {
    libm::log2f(x)
}

/// Deterministic f32 tangent used by the CPU parity oracle.
#[must_use]
#[inline]
pub fn canonical_tan(x: f32) -> f32 {
    libm::tanf(x)
}

/// Deterministic f32 arc cosine used by the CPU parity oracle.
#[must_use]
#[inline]
pub fn canonical_acos(x: f32) -> f32 {
    libm::acosf(x)
}

/// Deterministic f32 arc sine used by the CPU parity oracle.
#[must_use]
#[inline]
pub fn canonical_asin(x: f32) -> f32 {
    libm::asinf(x)
}

/// Deterministic f32 arc tangent used by the CPU parity oracle.
#[must_use]
#[inline]
pub fn canonical_atan(x: f32) -> f32 {
    libm::atanf(x)
}

/// Deterministic f32 hyperbolic tangent used by the CPU parity oracle.
#[must_use]
#[inline]
pub fn canonical_tanh(x: f32) -> f32 {
    libm::tanhf(x)
}

/// Deterministic f32 hyperbolic sine used by the CPU parity oracle.
#[must_use]
#[inline]
pub fn canonical_sinh(x: f32) -> f32 {
    libm::sinhf(x)
}

/// Deterministic f32 hyperbolic cosine used by the CPU parity oracle.
#[must_use]
#[inline]
pub fn canonical_cosh(x: f32) -> f32 {
    libm::coshf(x)
}

/// Flush subnormal `f32` values to signed zero, preserving the sign bit.
///
/// This is the canonicalization step applied by the reference interpreter
/// before and after every f32 operation so that GPU backends that flush
/// subnormals, canonicalize NaN payloads, or preserve them all converge on
/// the same deterministic bit pattern.
#[must_use]
#[inline]
pub fn canonical_f32(value: f32) -> f32 {
    crate::execution::typed_ops::canonical_f32(value)
}

/// Compute the deterministic ULP distance after vyre f32 canonicalization.
///
/// NaN payloads collapse to one quiet NaN bit pattern, subnormals flush to
/// signed zero, and `+0.0`/`-0.0` compare at zero distance.
#[must_use]
#[inline]
pub fn canonical_ulp_distance(left: f32, right: f32) -> u32 {
    let left = canonical_f32(left);
    let right = canonical_f32(right);
    if left == right || left.to_bits() == right.to_bits() {
        return 0;
    }
    ordered_f32_key(left).abs_diff(ordered_f32_key(right))
}

#[inline]
fn ordered_f32_key(value: f32) -> u32 {
    let bits = value.to_bits();
    if bits & 0x8000_0000 == 0 {
        bits | 0x8000_0000
    } else {
        !bits
    }
}

/// Return the canonical float-pending error.
///
/// This function exists to make the reference interpreter intentionally fail on
/// float operations until the parity engine has a complete, testable IEEE 754
/// CPU reference to compare against GPU output.
///
/// # Examples
///
/// ```rust,ignore
/// let err = vyre::reference::ieee754::pending_float_types();
/// ```
pub fn pending_float_types() -> Error {
    Error::interp(
        "pending upstream float variants in vyre::ir; reference interpreter is integer-only until those variants land",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_f32_collapses_nan_payloads() {
        let quiet_payload = f32::from_bits(0x7FC1_2345);
        let signaling_payload = f32::from_bits(0x7F81_2345);
        assert_eq!(canonical_f32(quiet_payload).to_bits(), 0x7FC0_0000);
        assert_eq!(canonical_f32(signaling_payload).to_bits(), 0x7FC0_0000);
        assert_eq!(canonical_ulp_distance(quiet_payload, signaling_payload), 0);
    }

    #[test]
    fn canonical_ulp_distance_handles_zero_and_neighbors() {
        assert_eq!(canonical_ulp_distance(0.0, -0.0), 0);
        assert_eq!(
            canonical_ulp_distance(1.0, f32::from_bits(1.0f32.to_bits() + 1)),
            1
        );
    }
}
