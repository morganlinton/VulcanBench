use super::*;

pub(crate) fn macro_spelling_origin(
    macro_events: &[MacroEvent],
    symbol_id: [u8; 16],
    fallback_file: &std::path::Path,
    fallback_start: u32,
    fallback_len: u32,
) -> (std::path::PathBuf, u32, u32) {
    for event in macro_events.iter().rev() {
        if event.symbol_id != symbol_id {
            continue;
        }
        match event.kind {
            MacroEventKind::Define => {
                if let Some((start, len)) = event.replacement_range.or(event.name_range) {
                    return (event.file.clone(), start, len);
                }
            }
            MacroEventKind::Undef => {
                return (fallback_file.to_path_buf(), fallback_start, fallback_len)
            }
        }
    }
    (fallback_file.to_path_buf(), fallback_start, fallback_len)
}
