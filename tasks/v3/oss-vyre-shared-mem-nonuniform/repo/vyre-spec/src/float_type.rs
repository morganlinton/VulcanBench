//! Frozen floating-point format tags used by verification evidence.

/// Floating-point type covered by an exhaustive verification pass.
///
/// Example: `FloatType::F32` records that a law was checked over IEEE 754
/// binary32 inputs.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum FloatType {
    /// IEEE 754 binary16.
    F16,
    /// bfloat16.
    BF16,
    /// IEEE 754 binary32.
    F32,
}
