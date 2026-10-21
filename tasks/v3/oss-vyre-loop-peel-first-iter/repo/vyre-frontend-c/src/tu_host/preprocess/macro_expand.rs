use super::*;
pub(super) fn expand_line_macros(
    line: &str,
    macros: &HashMap<String, MacroDef>,
    depth: u32,
) -> String {
    expand_line_macros_inner(line, macros, depth, &[])
}

#[cfg(feature = "cpu-oracle")]
pub(super) fn expand_line_macros_inner(
    line: &str,
    macros: &HashMap<String, MacroDef>,
    depth: u32,
    disabled: &[String],
) -> String {
    if depth > MAX_MACRO_EXPANSION_DEPTH {
        return line.to_string();
    }
    let bytes = line.as_bytes();
    let mut out = String::new();
    let mut i = 0usize;
    let mut changed = false;
    let mut next_disabled = disabled.to_vec();
    while i < bytes.len() {
        match bytes[i] {
            b'"' | b'\'' => {
                let quote = bytes[i];
                let start = i;
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
                out.push_str(&line[start..i.min(bytes.len())]);
            }
            b if is_ident_start(b) => {
                let start = i;
                i += 1;
                while i < bytes.len() && is_ident_continue(bytes[i]) {
                    i += 1;
                }
                let name = &line[start..i];
                if disabled.iter().any(|disabled| disabled == name) {
                    out.push_str(name);
                    continue;
                }
                let Some(def) = macros.get(name) else {
                    out.push_str(name);
                    continue;
                };
                match &def.params {
                    None => {
                        out.push_str(&def.replacement);
                        if !next_disabled.iter().any(|disabled| disabled == name) {
                            next_disabled.push(name.to_string());
                        }
                        changed = true;
                    }
                    Some(params) => {
                        let ws_start = i;
                        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
                            i += 1;
                        }
                        if bytes.get(i).copied() == Some(b'(') {
                            if let Some((args, end)) = parse_macro_args(line, i) {
                                out.push_str(&replace_macro_params(
                                    &def.replacement,
                                    params,
                                    def.variadic.as_deref(),
                                    &args,
                                ));
                                if !next_disabled.iter().any(|disabled| disabled == name) {
                                    next_disabled.push(name.to_string());
                                }
                                i = end;
                                changed = true;
                            } else {
                                out.push_str(name);
                                out.push_str(&line[ws_start..i]);
                            }
                        } else {
                            out.push_str(name);
                            out.push_str(&line[ws_start..i]);
                        }
                    }
                }
            }
            _ => {
                out.push(bytes[i] as char);
                i += 1;
            }
        }
    }
    if changed {
        expand_line_macros_inner(&out, macros, depth + 1, &next_disabled)
    } else {
        out
    }
}
