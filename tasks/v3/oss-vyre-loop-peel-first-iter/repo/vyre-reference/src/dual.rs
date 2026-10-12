//! Dual-dispatch resolver for the parity engine.
//!
//! Every operation that claims conformance must produce byte-identical output on
//! two independently-written CPU references. The dual-reference registry picks
//! which pair runs for a given op ID, and the conform gate rejects any backend
//! that diverges from either reference on any witnessed input.

/// CPU reference function type used by the dual-reference registry.
pub type ReferenceFn = fn(&[u8]) -> Vec<u8>;

/// Two independently-written CPU references for one operation.
///
/// # Examples
///
/// ```rust,ignore
/// struct MyDual;
/// impl DualReference for MyDual {
///     fn reference_a(input: &[u8]) -> Vec<u8> { /* first impl */ vec![] }
///     fn reference_b(input: &[u8]) -> Vec<u8> { /* second impl */ vec![] }
/// }
/// ```
pub trait DualReference {
    /// First independently-written reference implementation.
    fn reference_a(input: &[u8]) -> Vec<u8>;

    /// Second independently-written reference implementation.
    fn reference_b(input: &[u8]) -> Vec<u8>;
}
