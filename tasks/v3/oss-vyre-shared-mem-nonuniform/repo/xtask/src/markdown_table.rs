pub(crate) fn markdown_cells(line: &str) -> Vec<&str> {
    let trimmed = line.trim();
    if !trimmed.starts_with('|') || !trimmed.ends_with('|') {
        return Vec::new();
    }
    trimmed
        .trim_matches('|')
        .split('|')
        .map(str::trim)
        .collect()
}

pub(crate) fn trim_code_ticks(value: &str) -> &str {
    value.trim().trim_start_matches('`').trim_end_matches('`')
}
