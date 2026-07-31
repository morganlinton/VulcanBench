//! Frozen adversarial-input records shared by operation catalogs and test generators.

/// Adversarial input declared beside an operation in the frozen data contract.
///
/// Example: an unsigned-division operation can publish an input whose divisor
/// bytes are zero and whose reason names the defined divide-by-zero behavior.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AdversarialInput {
    /// Input bytes for the hostile or boundary case.
    pub input: &'static [u8],
    /// Why this input matters.
    pub reason: &'static str,
}
