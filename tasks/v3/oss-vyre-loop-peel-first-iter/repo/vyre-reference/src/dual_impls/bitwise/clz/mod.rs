/// Count-leading-zero dual implementation reference.
pub mod reference {}

/// Operation ID for count-leading-zero dual references.
pub const OP_ID: &str = "primitive.bitwise.clz";

/// Direct word-oriented count-leading-zero reference.
pub mod reference_a {
    /// Evaluate `leading_zeros` over one little-endian u32 input.
    #[must_use]
    pub fn reference(input: &[u8]) -> Vec<u8> {
        super::super::common::unary_direct(input, u32::leading_zeros)
    }
}

/// Independent bit-walk count-leading-zero reference.
pub mod reference_b {
    /// Count leading zero bits by walking from the most significant bit.
    #[must_use]
    pub fn reference(input: &[u8]) -> Vec<u8> {
        super::super::common::clz_bits(input)
    }
}
