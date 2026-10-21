pub(crate) fn compiler_bytes_from_sections(
    sections: &[&[u8]],
    max_words: usize,
) -> Result<(Vec<u8>, u32), String> {
    const SECTION_MARKER: u32 = 0x5659_5245; // "VYRE"
    const SECTION_HEADER_WORDS: usize = 4;

    let header_words = sections
        .len()
        .checked_mul(SECTION_HEADER_WORDS)
        .ok_or_else(|| {
            format!(
                "compiler section count {} overflows section-header word budget. Fix: split the compiler section table before ELF lowering.",
                sections.len()
            )
        })?;
    if max_words < header_words {
        return Err(format!(
            "compiler lowering capacity {max_words} words cannot hold {} section headers. \
             Fix: increase the ELF lowering input budget or reduce section count.",
            sections.len()
        ));
    }

    let non_empty_count = sections
        .iter()
        .filter(|section| !section.is_empty())
        .count();
    if non_empty_count == 0 {
        return Err(
            "compiler lowering input has no parser/lowering section data. \
             Fix: run VAST/ProgramGraph lowering before ELF lowering."
                .to_string(),
        );
    }

    let payload_budget = max_words - header_words;
    let per_section_budget = payload_budget / non_empty_count.max(1);
    let mut payload_remainder = payload_budget % non_empty_count.max(1);
    let mut bytes = Vec::with_capacity(max_words.saturating_mul(4));
    for (section_idx, section) in sections.iter().enumerate() {
        if section.len() % 4 != 0 {
            return Err(format!(
                "compiler section {section_idx} length is not u32-aligned: {} bytes. \
                 Fix: only feed packed parser/lowering u32 streams into ELF lowering.",
                section.len()
            ));
        }
        let section_word_count = section.len() / 4;
        let section_hash = fnv1a32_packed_u32_bytes(section);
        let section_idx_u32 = u32::try_from(section_idx).map_err(|error| {
            format!(
                "compiler section index {section_idx} does not fit u32: {error}. Fix: split the compiler section table before ELF lowering."
            )
        })?;
        push_u32_le(&mut bytes, SECTION_MARKER);
        push_u32_le(&mut bytes, section_idx_u32);
        push_u32_le(
            &mut bytes,
            u32::try_from(section_word_count)
                .map_err(|_| format!("compiler section {section_idx} exceeds u32 word count"))?,
        );
        push_u32_le(&mut bytes, section_hash);

        let mut take_words = per_section_budget.min(section_word_count);
        if payload_remainder != 0 && take_words < section_word_count {
            take_words = take_words.checked_add(1).ok_or_else(|| {
                format!(
                    "compiler section {section_idx} take-word count overflowed. Fix: split the compiler section table before ELF lowering."
                )
            })?;
            payload_remainder -= 1;
        }
        bytes.extend_from_slice(&section[..take_words * 4]);
    }
    let word_count = u32::try_from(bytes.len() / 4).map_err(|_| {
        "compiler lowering byte stream exceeds u32 word count. Fix: split the compiler section table before ELF lowering."
            .to_string()
    })?;
    Ok((bytes, word_count))
}

fn fnv1a32_packed_u32_bytes(bytes: &[u8]) -> u32 {
    let mut hash = 0x811c_9dc5u32;
    for byte in bytes {
        hash ^= u32::from(*byte);
        hash = hash.wrapping_mul(0x0100_0193);
    }
    hash
}

fn push_u32_le(bytes: &mut Vec<u8>, value: u32) {
    bytes.extend_from_slice(&value.to_le_bytes());
}
