pub(crate) fn is_research_key(value: &str) -> bool {
    !value.is_empty()
        && value
            .chars()
            .all(|ch| ch.is_ascii_uppercase() || ch.is_ascii_digit() || ch == '_')
}

pub(crate) fn backtick_research_keys(text: &str) -> Vec<String> {
    text.split('`')
        .enumerate()
        .filter_map(|(index, part)| {
            if index % 2 == 1 && is_research_key(part) {
                Some(part.to_string())
            } else {
                None
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn research_keys_allow_uppercase_underscores_and_digits() {
        assert!(is_research_key("FLASH_ATTN2"));
        assert!(is_research_key("XAV_FPGA"));
        assert!(is_research_key("CUDA_GRAPHS"));
        assert!(!is_research_key(""));
        assert!(!is_research_key("Flash_Attn2"));
        assert!(!is_research_key("FLASH-ATTN2"));
    }

    #[test]
    fn backtick_research_keys_extract_only_valid_keys() {
        assert_eq!(
            backtick_research_keys("Uses `FLASH_ATTN2`, `docs/file.md`, and `XAV_FPGA`."),
            vec!["FLASH_ATTN2".to_string(), "XAV_FPGA".to_string()]
        );
    }
}
