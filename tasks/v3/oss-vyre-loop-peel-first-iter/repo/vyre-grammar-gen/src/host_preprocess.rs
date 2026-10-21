//! Host-side C preprocessor pass for parser fixtures: line splicing, comment
//! stripping, `#if 0` / `#endif` removal, and conservative object-like macro
//! expansion.

use std::collections::BTreeMap;

/// Apply a conservative host preprocess pass suitable before lexing
/// experiments on disk snippets.
///
/// Order: line splice (`\\\n`) → strip `/* */` → strip `//` → fold
/// `#if 0` … `#endif` blocks → expand object-like `#define` bindings.
#[must_use]
pub fn preprocess_c_host(input: &str) -> String {
    let spliced = splice_lines(input);
    let no_block = strip_block_comments(&spliced);
    let no_line = strip_line_comments(&no_block);
    let no_if_zero = strip_if_zero_blocks(&no_line);
    expand_object_like_macros(&no_if_zero)
}

fn splice_lines(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.peek() {
                Some('\r') => {
                    chars.next();
                    if chars.peek() == Some(&'\n') {
                        chars.next();
                    }
                    out.push(' ');
                    continue;
                }
                Some('\n') => {
                    chars.next();
                    out.push(' ');
                    continue;
                }
                _ => {}
            }
        }
        out.push(c);
    }
    out
}

fn strip_block_comments(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut rest = input;
    while let Some(pos) = rest.find("/*") {
        out.push_str(&rest[..pos]);
        rest = &rest[pos + 2..];
        match rest.find("*/") {
            Some(end) => {
                rest = &rest[end + 2..];
                out.push(' ');
            }
            None => break,
        }
    }
    out.push_str(rest);
    out
}

fn strip_line_comments(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for line in input.lines() {
        let mut cut = None;
        let b = line.as_bytes();
        let mut j = 0usize;
        while j + 1 < b.len() {
            if b[j] == b'/' && b[j + 1] == b'/' {
                cut = Some(j);
                break;
            }
            j += 1;
        }
        match cut {
            Some(idx) => {
                out.push_str(&line[..idx]);
                out.push('\n');
            }
            None => {
                out.push_str(line);
                out.push('\n');
            }
        }
    }
    if !input.ends_with('\n') && out.ends_with('\n') {
        out.pop();
    }
    out
}

fn strip_if_zero_blocks(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let lines: Vec<&str> = input.lines().collect();
    let mut i = 0usize;
    while i < lines.len() {
        let t = lines[i].trim_start();
        let is_if_zero = {
            let mut it = t.split_whitespace();
            it.next() == Some("#if") && it.next() == Some("0")
        };
        if is_if_zero {
            let mut depth = 1usize;
            i += 1;
            while i < lines.len() && depth > 0 {
                let u = lines[i].trim_start();
                let mut wit = u.split_whitespace();
                let head = wit.next();
                if head == Some("#if") {
                    depth += 1;
                } else if head == Some("#endif") {
                    depth -= 1;
                }
                i += 1;
            }
            continue;
        }
        out.push_str(lines[i]);
        out.push('\n');
        i += 1;
    }
    if !input.ends_with('\n') && out.ends_with('\n') {
        out.pop();
    }
    out
}

fn expand_object_like_macros(input: &str) -> String {
    let mut macros = BTreeMap::<String, String>::new();
    let mut out = String::with_capacity(input.len());
    for line in input.lines() {
        let trimmed = line.trim_start();
        if let Some(rest) = trimmed.strip_prefix("#define") {
            if let Some((name, replacement)) = parse_object_macro(rest) {
                macros.insert(name.to_string(), replacement.to_string());
            }
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("#undef") {
            if let Some(name) = rest.split_whitespace().next() {
                macros.remove(name);
            }
            continue;
        }
        if trimmed.starts_with("#include") {
            out.push_str(line);
            out.push('\n');
            continue;
        }
        out.push_str(&expand_line_identifiers(line, &macros));
        out.push('\n');
    }
    if !input.ends_with('\n') && out.ends_with('\n') {
        out.pop();
    }
    out
}

fn parse_object_macro(rest: &str) -> Option<(&str, &str)> {
    let rest = rest.trim_start();
    let name_end = rest
        .char_indices()
        .find_map(|(idx, ch)| (!is_ident_continue(ch)).then_some(idx))
        .unwrap_or(rest.len());
    if name_end == 0 {
        return None;
    }
    let name = &rest[..name_end];
    if !name.chars().next().is_some_and(is_ident_start) {
        return None;
    }
    let tail = &rest[name_end..];
    if tail.starts_with('(') {
        return None;
    }
    Some((name, tail.trim_start()))
}

fn expand_line_identifiers(line: &str, macros: &BTreeMap<String, String>) -> String {
    let mut out = String::with_capacity(line.len());
    let mut chars = line.char_indices().peekable();
    let mut in_string = false;
    let mut in_char = false;
    while let Some((idx, ch)) = chars.next() {
        if ch == '\\' {
            out.push(ch);
            if let Some((_, escaped)) = chars.next() {
                out.push(escaped);
            }
            continue;
        }
        if ch == '"' && !in_char {
            in_string = !in_string;
            out.push(ch);
            continue;
        }
        if ch == '\'' && !in_string {
            in_char = !in_char;
            out.push(ch);
            continue;
        }
        if !in_string && !in_char && is_ident_start(ch) {
            let start = idx;
            let mut end = idx + ch.len_utf8();
            while let Some(&(next_idx, next_ch)) = chars.peek() {
                if !is_ident_continue(next_ch) {
                    break;
                }
                chars.next();
                end = next_idx + next_ch.len_utf8();
            }
            let ident = &line[start..end];
            match macros.get(ident) {
                Some(replacement) => out.push_str(replacement),
                None => out.push_str(ident),
            }
            continue;
        }
        out.push(ch);
    }
    out
}

fn is_ident_start(ch: char) -> bool {
    ch == '_' || ch.is_ascii_alphabetic()
}

fn is_ident_continue(ch: char) -> bool {
    is_ident_start(ch) || ch.is_ascii_digit()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splices_backslash_newline() {
        assert_eq!(preprocess_c_host("a\\\nb"), "a b".to_string());
    }

    #[test]
    fn strips_block_comment() {
        assert_eq!(preprocess_c_host("x/*c*/y"), "x y".to_string());
    }

    #[test]
    fn strips_line_comment() {
        assert_eq!(preprocess_c_host("ok //x"), "ok ".to_string());
    }

    #[test]
    fn strips_if_zero() {
        let s = preprocess_c_host("#if 0\nBAD\n#endif\nOK");
        assert!(!s.contains("BAD"));
        assert!(s.contains("OK"));
    }

    #[test]
    fn expands_object_like_macro_identifiers() {
        let s = preprocess_c_host("#define SIZE 16\nint a[SIZE];\n");
        assert_eq!(s, "int a[16];\n");
    }

    #[test]
    fn does_not_expand_inside_string_literals() {
        let s = preprocess_c_host("#define SIZE 16\nchar *s = \"SIZE\"; SIZE\n");
        assert_eq!(s, "char *s = \"SIZE\"; 16\n");
    }

    #[test]
    fn undef_removes_macro_binding() {
        let s = preprocess_c_host("#define FLAG 1\nFLAG\n#undef FLAG\nFLAG\n");
        assert_eq!(s, "1\nFLAG\n");
    }
}
