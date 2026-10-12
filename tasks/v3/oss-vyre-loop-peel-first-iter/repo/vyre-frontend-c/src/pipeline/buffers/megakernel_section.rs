use super::*;
pub(crate) fn megakernel_section_bytes(
    token_count: u32,
    function_count: u32,
    cfg_word_count: u32,
    section_tags: &[u32],
) -> Result<Vec<u8>, String> {
    let section_count = u32::try_from(section_tags.len()).map_err(|error| {
        format!(
            "megakernel section tag count {} does not fit u32: {error}. Fix: split the section tag table before VYRECOB2 emission.",
            section_tags.len()
        )
    })?;
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"MEGAKERN2");
    bytes.extend_from_slice(&protocol::SLOT_WORDS.to_le_bytes());
    bytes.extend_from_slice(&token_count.to_le_bytes());
    bytes.extend_from_slice(&function_count.to_le_bytes());
    bytes.extend_from_slice(&cfg_word_count.to_le_bytes());
    bytes.extend_from_slice(&section_count.to_le_bytes());
    // Tag table emits as a single LE bytemuck::cast_slice on LE hosts
    // via wire::append_u32_slice_le_bytes - no per-tag extend loop.
    vyre_primitives::wire::append_u32_slice_le_bytes(section_tags, &mut bytes);
    Ok(bytes)
}
