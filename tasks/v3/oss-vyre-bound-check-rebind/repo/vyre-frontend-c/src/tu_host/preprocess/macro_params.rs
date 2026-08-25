use super::*;
pub(super) fn parse_macro_args(src: &str, open_idx: usize) -> Option<(Vec<String>, usize)> {
    let bytes = src.as_bytes();
    if bytes.get(open_idx).copied() != Some(b'(') {
        return None;
    }
    let mut args = Vec::new();
    let mut depth = 0u32;
    let mut start = open_idx + 1;
    let mut i = open_idx + 1;
    while i < bytes.len() {
        match bytes[i] {
            b'"' | b'\'' => {
                let quote = bytes[i];
                i += 1;
                while i < bytes.len() {
                    if bytes[i] == b'\\' {
                        i = i.saturating_add(2);
                        continue;
                    }
                    if bytes[i] == quote {
                        i += 1;
                        break;
                    }
                    i += 1;
                }
                continue;
            }
            b'(' => depth = depth.saturating_add(1),
            b')' if depth == 0 => {
                args.push(src[start..i].trim().to_string());
                return Some((args, i + 1));
            }
            b')' => depth = depth.saturating_sub(1),
            b',' if depth == 0 => {
                args.push(src[start..i].trim().to_string());
                start = i + 1;
            }
            _ => {}
        }
        i += 1;
    }
    None
}

#[cfg(feature = "cpu-oracle")]
pub(super) fn stringify_arg(arg: &str) -> String {
    let mut out = String::from("\"");
    for ch in arg.trim().chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            _ => out.push(ch),
        }
    }
    out.push('"');
    out
}

#[cfg(feature = "cpu-oracle")]
pub(super) fn variadic_arg(
    name: &str,
    variadic: Option<&str>,
    params: &[String],
    args: &[String],
) -> Option<String> {
    let variadic_name = variadic?;
    if name != "__VA_ARGS__" && name != variadic_name {
        return None;
    }
    if args.len() < params.len() {
        return Some(String::new());
    }
    Some(
        args[params.len()..]
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>()
            .join(", "),
    )
}

#[cfg(feature = "cpu-oracle")]
pub(super) fn macro_arg<'a>(args: &'a [String], idx: usize, name: &str) -> &'a str {
    let _ = name;
    args.get(idx).map(String::as_str).unwrap_or("")
}

#[cfg(feature = "cpu-oracle")]
pub(super) fn replace_macro_params(
    replacement: &str,
    params: &[String],
    variadic: Option<&str>,
    args: &[String],
) -> String {
    let param_index: std::collections::HashMap<&str, usize> = params
        .iter()
        .enumerate()
        .map(|(idx, param)| (param.as_str(), idx))
        .collect();
    let mut out = String::new();
    let bytes = replacement.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        if bytes[i] == b'#'
            && bytes.get(i + 1).copied() != Some(b'#')
            && i.checked_sub(1).and_then(|prev| bytes.get(prev)).copied() != Some(b'#')
        {
            let mut j = i + 1;
            while bytes.get(j).is_some_and(|b| b.is_ascii_whitespace()) {
                j += 1;
            }
            if bytes.get(j).is_some_and(|b| is_ident_start(*b)) {
                let start = j;
                j += 1;
                while bytes.get(j).is_some_and(|b| is_ident_continue(*b)) {
                    j += 1;
                }
                let name = &replacement[start..j];
                if let Some(value) = variadic_arg(name, variadic, params, args) {
                    out.push_str(&stringify_arg(&value));
                    i = j;
                    continue;
                } else if let Some(idx) = param_index.get(name).copied() {
                    out.push_str(&stringify_arg(macro_arg(args, idx, name)));
                    i = j;
                    continue;
                }
            }
        }
        if is_ident_start(bytes[i]) {
            let start = i;
            i += 1;
            while i < bytes.len() && is_ident_continue(bytes[i]) {
                i += 1;
            }
            let name = &replacement[start..i];
            if let Some(value) = variadic_arg(name, variadic, params, args) {
                out.push_str(&value);
            } else if let Some(idx) = param_index.get(name).copied() {
                out.push_str(macro_arg(args, idx, name));
            } else {
                out.push_str(name);
            }
            continue;
        }
        out.push(bytes[i] as char);
        i += 1;
    }
    collapse_token_paste(&out)
}

#[cfg(feature = "cpu-oracle")]
pub(super) fn collapse_token_paste(src: &str) -> String {
    let mut parts = src.split("##");
    let Some(first) = parts.next() else {
        return String::new();
    };
    let mut out = first.trim_end().to_string();
    for part in parts {
        out.push_str(part.trim());
    }
    out
}
