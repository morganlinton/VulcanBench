#[cfg(feature = "cpu-oracle")]
use super::*;
#[cfg(feature = "cpu-oracle")]
use crate::tu_host::include_search::IncludeSearchDirs;
#[cfg(feature = "cpu-oracle")]
use crate::tu_host::source_text::line_body_and_newline;

#[cfg(feature = "cpu-oracle")]
#[derive(Clone, Copy, Debug)]
struct IncludeConditionalFrame {
    parent_active: bool,
    branch_taken: bool,
    current_active: bool,
}

#[cfg(feature = "cpu-oracle")]
/// Expand quote-style local includes for the host reference preprocessing path.
pub fn expand_local_includes(
    source: &str,
    tu_path: &Path,
    include_dirs: &[PathBuf],
    use_system_include_dirs: bool,
    system_include_sysroot: Option<&Path>,
    depth: u32,
    stack: &mut Vec<PathBuf>,
) -> Result<String, String> {
    expand_local_includes_with_search_dirs(
        source,
        tu_path,
        include_dirs,
        &[],
        &[],
        &[],
        use_system_include_dirs,
        system_include_sysroot,
        depth,
        stack,
    )
}

#[cfg(feature = "cpu-oracle")]
pub(super) fn expand_local_includes_with_search_dirs(
    source: &str,
    tu_path: &Path,
    include_dirs: &[PathBuf],
    quote_include_dirs: &[PathBuf],
    system_include_dirs: &[PathBuf],
    after_include_dirs: &[PathBuf],
    use_system_include_dirs: bool,
    system_include_sysroot: Option<&Path>,
    depth: u32,
    stack: &mut Vec<PathBuf>,
) -> Result<String, String> {
    let mut macros = HashMap::new();
    let include_dirs = expanded_include_search_dirs(
        include_dirs,
        quote_include_dirs,
        system_include_dirs,
        after_include_dirs,
        use_system_include_dirs,
        system_include_sysroot,
    )?;
    expand_local_includes_with_state(source, tu_path, &include_dirs, depth, stack, &mut macros)
}

#[cfg(feature = "cpu-oracle")]
pub(super) fn expand_local_includes_with_state(
    source: &str,
    tu_path: &Path,
    include_dirs: &IncludeSearchDirs,
    depth: u32,
    stack: &mut Vec<PathBuf>,
    macros: &mut HashMap<String, MacroDef>,
) -> Result<String, String> {
    if depth > MAX_INCLUDE_DEPTH {
        return Err(format!(
            "vyre-frontend-c: #include depth exceeded {MAX_INCLUDE_DEPTH} (cycle or deep tree)."
        ));
    }
    let tu_dir = tu_path.parent().unwrap_or_else(|| Path::new("."));
    let mut out = String::with_capacity(source.len().saturating_mul(2));
    let mut conditionals = Vec::<IncludeConditionalFrame>::new();
    for logical_line in source.split_inclusive('\n') {
        let (line, newline) = line_body_and_newline(logical_line);
        let active = conditionals.last().is_none_or(|f| f.current_active);
        let directive_text = strip_directive_comments(line);
        let directive = parse_directive(&directive_text);

        if let Some((name, rest)) = directive {
            match name {
                "define" => {
                    if active {
                        if let Some((name, def)) = parse_define(rest) {
                            macros.insert(name, def);
                        }
                        out.push_str(line);
                        out.push_str(newline);
                    }
                    continue;
                }
                "undef" => {
                    if active {
                        macros.remove(rest.trim());
                        out.push_str(line);
                        out.push_str(newline);
                    }
                    continue;
                }
                "ifdef" => {
                    let parent_active = active;
                    let cond = macros.contains_key(rest.trim());
                    conditionals.push(IncludeConditionalFrame {
                        parent_active,
                        branch_taken: cond,
                        current_active: parent_active && cond,
                    });
                    out.push_str(line);
                    out.push_str(newline);
                    continue;
                }
                "ifndef" => {
                    let parent_active = active;
                    let cond = !macros.contains_key(rest.trim());
                    conditionals.push(IncludeConditionalFrame {
                        parent_active,
                        branch_taken: cond,
                        current_active: parent_active && cond,
                    });
                    out.push_str(line);
                    out.push_str(newline);
                    continue;
                }
                "if" => {
                    let parent_active = active;
                    let cond = eval_preprocessor_condition(rest, macros);
                    conditionals.push(IncludeConditionalFrame {
                        parent_active,
                        branch_taken: cond,
                        current_active: parent_active && cond,
                    });
                    out.push_str(line);
                    out.push_str(newline);
                    continue;
                }
                "elif" => {
                    if let Some(frame) = conditionals.last_mut() {
                        let cond = !frame.branch_taken && eval_preprocessor_condition(rest, macros);
                        frame.current_active = frame.parent_active && cond;
                        frame.branch_taken |= cond;
                    }
                    out.push_str(line);
                    out.push_str(newline);
                    continue;
                }
                "else" => {
                    if let Some(frame) = conditionals.last_mut() {
                        let cond = !frame.branch_taken;
                        frame.current_active = frame.parent_active && cond;
                        frame.branch_taken = true;
                    }
                    out.push_str(line);
                    out.push_str(newline);
                    continue;
                }
                "endif" => {
                    conditionals.pop();
                    out.push_str(line);
                    out.push_str(newline);
                    continue;
                }
                "include" if active => {
                    let Some((path_lit, is_system)) = parse_include_literal(rest)? else {
                        out.push_str(line);
                        out.push_str(newline);
                        continue;
                    };
                    let inc_path = if is_system {
                        search_system_include_file(path_lit, include_dirs).ok_or_else(|| {
                            format!(
                                "vyre-frontend-c: system #include <{path_lit}> not found in -I search path"
                            )
                        })?
                    } else {
                        search_include_file(path_lit, tu_dir, include_dirs).ok_or_else(|| {
                            format!(
                                "vyre-frontend-c: #include \"{path_lit}\" not found (tried TU dir and -I)"
                            )
                        })?
                    };
                    let expanded =
                        expand_one_include(&inc_path, include_dirs, depth, stack, macros)?;
                    let expanded_len = out.len().checked_add(expanded.len()).ok_or_else(|| {
                        "vyre-frontend-c: expanded include output length overflows host usize. Fix: split the translation unit before include expansion."
                            .to_string()
                    })?;
                    if expanded_len > MAX_INCLUDE_BYTES {
                        return Err(format!(
                            "vyre-frontend-c: expanded TU exceeds {MAX_INCLUDE_BYTES} bytes (include bomb guard)."
                        ));
                    }
                    out.push_str(&expanded);
                    if !expanded.ends_with('\n') {
                        out.push('\n');
                    }
                    continue;
                }
                _ => {}
            }
        }

        if active {
            out.push_str(line);
            out.push_str(newline);
        }
    }
    Ok(out)
}

#[cfg(feature = "cpu-oracle")]
pub(super) fn expand_one_include(
    inc_path: &Path,
    include_dirs: &IncludeSearchDirs,
    depth: u32,
    stack: &mut Vec<PathBuf>,
    macros: &mut HashMap<String, MacroDef>,
) -> Result<String, String> {
    let canon = fs::canonicalize(inc_path).map_err(|error| {
        format!(
            "vyre-frontend-c: failed to canonicalize include `{}` before reference-only include expansion: {error}. Fix: repair include path permissions or pass a stable include root.",
            inc_path.display()
        )
    })?;
    if stack.iter().any(|existing| existing == &canon) {
        return Err(format!(
            "vyre-frontend-c: #include cycle detected at {}.",
            canon.display()
        ));
    }
    let inner_bytes = read_include_bounded(inc_path)?;
    let inner = String::from_utf8(inner_bytes).map_err(|error| {
        format!(
            "vyre-frontend-c: include {} contains non-UTF-8 source bytes at offset {} during reference-only include expansion. Fix: preserve oracle inputs as bytes or reject the encoding before preprocessing; lossy replacement is forbidden.",
            inc_path.display(),
            error.utf8_error().valid_up_to()
        )
    })?;
    if inner.len() > MAX_INCLUDE_BYTES {
        return Err(format!(
            "vyre-frontend-c: include {} exceeds {MAX_INCLUDE_BYTES} bytes.",
            inc_path.display()
        ));
    }
    stack.push(canon);
    let expanded =
        expand_local_includes_with_state(&inner, inc_path, include_dirs, depth + 1, stack, macros)?;
    stack.pop();
    Ok(expanded)
}

// Reference-only resident prep moved behind the `cpu-oracle` feature in
// `resident_prepare`; production callers use GPU resident preprocessing.
