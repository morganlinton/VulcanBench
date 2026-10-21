pub(super) fn read_output_u32(bytes: &[u8], label: &str) -> Result<u32, String> {
    if bytes.len() != 4 {
        return Err(format!(
            "{label}: malformed u32 output: expected exactly 4 bytes, got {}. Fix: backend must emit one u32 scalar and no trailing bytes.",
            bytes.len()
        ));
    }
    vyre_primitives::wire::read_u32_le_word(bytes, 0, label)
}
