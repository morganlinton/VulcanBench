pub(crate) fn decode_u32_words_for_section(
    section: &[u8],
    section_name: &str,
) -> Result<Vec<u32>, String> {
    if section.len() % 4 != 0 {
        return Err(format!(
            "vyre-frontend-c {section_name} section length {} is not u32-aligned. Fix: regenerate the object.",
            section.len()
        ));
    }
    Ok(vyre_primitives::wire::decode_u32_le_bytes_all(section))
}
