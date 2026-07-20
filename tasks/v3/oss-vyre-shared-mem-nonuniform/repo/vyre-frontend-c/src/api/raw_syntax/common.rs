pub(super) fn read_u32_at(bytes: &[u8], index: usize, label: &str) -> Result<u32, String> {
    // Canonical LEGO: vyre-primitives::wire owns the word-indexed LE u32 read with
    // the same checked bounds + diagnostics. Local reimplementation removed — one
    // implementation across the workspace, not re-rolled per crate.
    vyre_primitives::wire::read_u32_le_word(bytes, index, label)
}
