pub(super) fn splice_line_continuations(source: &str) -> String {
    let mut out = String::with_capacity(source.len());
    for logical_line in source.split_inclusive('\n') {
        let (line, newline) = line_body_and_newline(logical_line);
        let trimmed_len = line.trim_end_matches([' ', '\t']).len();
        if line[..trimmed_len].ends_with('\\') {
            out.push_str(&line[..trimmed_len - 1]);
        } else {
            out.push_str(line);
            out.push_str(newline);
        }
    }
    out
}
pub(super) fn parse_directive(line: &str) -> Option<(&str, &str)> {
    let trimmed = line.trim_start();
    let after_hash = trimmed.strip_prefix('#')?.trim_start();
    let bytes = after_hash.as_bytes();
    let mut end = 0usize;
    while end < bytes.len() && bytes[end].is_ascii_alphabetic() {
        end += 1;
    }
    (end != 0).then(|| (&after_hash[..end], after_hash[end..].trim_start()))
}

#[cfg(feature = "cpu-oracle")]
pub(super) fn parse_include_literal(rest: &str) -> Result<Option<(&str, bool)>, String> {
    let trimmed = rest.trim_start();
    if let Some(s) = trimmed.strip_prefix('"') {
        let end = s
            .find('"')
            .ok_or_else(|| "vyre-frontend-c: unterminated #include \"...\"".to_string())?;
        Ok(Some((&s[..end], false)))
    } else if let Some(s) = trimmed.strip_prefix('<') {
        let end = s
            .find('>')
            .ok_or_else(|| "vyre-frontend-c: unterminated #include <...>".to_string())?;
        Ok(Some((&s[..end], true)))
    } else {
        Ok(None)
    }
}

pub(super) fn line_body_and_newline(line: &str) -> (&str, &str) {
    if let Some(body) = line.strip_suffix('\n') {
        if let Some(body) = body.strip_suffix('\r') {
            (body, "\r\n")
        } else {
            (body, "\n")
        }
    } else {
        (line, "")
    }
}

// Host include expansion lives behind the `cpu-oracle` feature.
