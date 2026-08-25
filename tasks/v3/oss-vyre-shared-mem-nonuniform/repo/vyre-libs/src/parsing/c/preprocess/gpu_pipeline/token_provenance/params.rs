use super::*;

pub(crate) fn parse_param_names(args: &[u8]) -> SmallVec<[&[u8]; 8]> {
    args.split(|byte| *byte == b',')
        .filter_map(|raw| {
            let trimmed = trim_ascii(raw);
            if trimmed.is_empty() || trimmed == b"..." {
                None
            } else if let Some(name) = trimmed.strip_prefix(b"...") {
                Some(name)
            } else {
                Some(trimmed)
            }
        })
        .collect()
}

pub(crate) fn trim_ascii(bytes: &[u8]) -> &[u8] {
    let mut start = 0_usize;
    let mut end = bytes.len();
    while start < end && bytes[start].is_ascii_whitespace() {
        start += 1;
    }
    while end > start && bytes[end - 1].is_ascii_whitespace() {
        end -= 1;
    }
    &bytes[start..end]
}

pub(crate) fn param_argument_span(
    token: &[u8],
    params: &[&[u8]],
    arg_spans: &[(usize, usize)],
) -> Option<(usize, usize)> {
    let idx = params.iter().position(|param| *param == token)?;
    arg_spans.get(idx).copied()
}
