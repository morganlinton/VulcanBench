//! Frozen known-answer vector records for bit-exact operation checks.

/// Known-answer test vector declared beside an operation in the frozen contract.
///
/// Example: a rotate-left primitive can publish input bytes, expected output
/// bytes, and the source reference that established the vector.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct KatVector {
    /// Test input bytes.
    pub input: &'static [u8],
    /// Expected output bytes.
    pub expected: &'static [u8],
    /// Source used to establish the vector.
    pub source: &'static str,
}
