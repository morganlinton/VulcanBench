//! Frozen golden-sample records shared by operation catalogs and conformance tests.

/// Hand-verified golden input/output pair declared beside an operation.
///
/// Lives in `vyre_spec` so both `vyre` can embed
/// the same type  -  previously this was a `conform`-only struct and
/// `std`-side generated fixtures could not name it. Example: an add operation
/// can publish input bytes for `1 + 2` and expected bytes for `3`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct GoldenSample {
    /// Op id this sample belongs to.
    pub op_id: &'static str,
    /// Input bytes.
    pub input: &'static [u8],
    /// Expected output bytes from the CPU reference.
    pub expected: &'static [u8],
    /// Human-readable explanation of why this value matters.
    pub reason: &'static str,
}
