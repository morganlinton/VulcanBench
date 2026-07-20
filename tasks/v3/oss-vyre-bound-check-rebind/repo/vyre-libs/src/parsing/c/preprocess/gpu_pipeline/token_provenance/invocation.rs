use super::*;

pub(crate) fn matched_macro_invocation<'a>(
    candidate_macros: &[&'a MacroDef],
    source: &[u8],
    after_name: usize,
) -> Option<(&'a MacroDef, usize)> {
    for mac in candidate_macros {
        if mac.is_function_like {
            let Some(invocation_end) = function_like_invocation_end(source, after_name) else {
                continue;
            };
            return Some((mac, invocation_end));
        }
        return Some((mac, after_name));
    }
    None
}

pub(crate) fn function_like_invocation_end(source: &[u8], after_name: usize) -> Option<usize> {
    let mut pos = after_name;
    while source
        .get(pos)
        .is_some_and(|byte| byte.is_ascii_whitespace())
    {
        pos += 1;
    }
    if source.get(pos).copied() != Some(b'(') {
        return None;
    }
    let mut depth = 1_u32;
    pos += 1;
    while let Some(byte) = source.get(pos).copied() {
        match byte {
            b'(' => depth = depth.saturating_add(1),
            b')' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return pos.checked_add(1);
                }
            }
            _ => {}
        }
        pos += 1;
    }
    None
}

pub(crate) fn invocation_arg_spans(
    source: &[u8],
    after_name: usize,
) -> Option<SmallVec<[(usize, usize); 8]>> {
    let mut pos = after_name;
    while source
        .get(pos)
        .is_some_and(|byte| byte.is_ascii_whitespace())
    {
        pos += 1;
    }
    if source.get(pos).copied() != Some(b'(') {
        return None;
    }
    pos += 1;
    let mut args = SmallVec::new();
    let mut depth = 0_u32;
    let mut arg_start = pos;
    while let Some(byte) = source.get(pos).copied() {
        match byte {
            b'(' => depth = depth.saturating_add(1),
            b')' if depth == 0 => {
                push_trimmed_arg_span(source, arg_start, pos, &mut args);
                return Some(args);
            }
            b')' => depth = depth.saturating_sub(1),
            b',' if depth == 0 => {
                push_trimmed_arg_span(source, arg_start, pos, &mut args);
                arg_start = pos + 1;
            }
            _ => {}
        }
        pos += 1;
    }
    None
}

pub(crate) fn push_trimmed_arg_span(
    source: &[u8],
    start: usize,
    end: usize,
    args: &mut SmallVec<[(usize, usize); 8]>,
) {
    let mut trimmed_start = start;
    let mut trimmed_end = end;
    while trimmed_start < trimmed_end
        && source
            .get(trimmed_start)
            .is_some_and(|byte| byte.is_ascii_whitespace())
    {
        trimmed_start += 1;
    }
    while trimmed_end > trimmed_start
        && source
            .get(trimmed_end - 1)
            .is_some_and(|byte| byte.is_ascii_whitespace())
    {
        trimmed_end -= 1;
    }
    args.push((trimmed_start, trimmed_end.saturating_sub(trimmed_start)));
}
