/// Population-count dual implementation reference.
pub mod reference {}

/// Operation ID for population-count dual references.
pub const OP_ID: &str = "primitive.bitwise.popcount";

/// Direct word-oriented population-count reference.
pub mod reference_a {
    /// Evaluate `count_ones` over one little-endian u32 input.
    #[must_use]
    pub fn reference(input: &[u8]) -> Vec<u8> {
        super::super::common::unary_direct(input, u32::count_ones)
    }
}

/// Independent bit-walk population-count reference.
pub mod reference_b {
    /// Count bits by walking every lane explicitly.
    #[must_use]
    pub fn reference(input: &[u8]) -> Vec<u8> {
        super::super::common::popcount_bits(input)
    }
}
