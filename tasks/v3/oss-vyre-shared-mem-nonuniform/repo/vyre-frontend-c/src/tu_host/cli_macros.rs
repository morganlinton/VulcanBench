use super::*;
/// Build a legacy `-D` prefix for CPU-oracle compatibility paths.
pub fn apply_cli_defines_prefix(source: &str, macros: &[(String, Option<String>)]) -> String {
    if macros.is_empty() {
        return source.to_string();
    }
    let mut out = String::new();
    for (name, val) in macros {
        out.push_str("#define ");
        out.push_str(name);
        if let Some(v) = val {
            out.push(' ');
            out.push_str(v);
        } else {
            out.push_str(" 1");
        }
        out.push('\n');
    }
    out.push_str(source);
    out
}

#[cfg(feature = "cpu-oracle")]
pub(super) fn apply_cli_source_prefix(
    source: &str,
    options: &VyreCompileOptions,
) -> Result<String, String> {
    let actions = cli_macro_actions(options);
    if actions.is_empty() && options.forced_include_files.is_empty() {
        return Ok(source.to_string());
    }
    let mut out = String::new();
    for action in actions {
        match action {
            CliMacroAction::Define { name, value } => {
                out.push_str("#define ");
                out.push_str(&name);
                if let Some(v) = value {
                    out.push(' ');
                    out.push_str(&v);
                } else {
                    out.push_str(" 1");
                }
                out.push('\n');
            }
            CliMacroAction::DefineFunction {
                name,
                params,
                value,
            } => {
                out.push_str("#define ");
                out.push_str(&name);
                out.push('(');
                out.push_str(&params.join(","));
                out.push(')');
                if let Some(v) = value {
                    out.push(' ');
                    out.push_str(&v);
                } else {
                    out.push_str(" 1");
                }
                out.push('\n');
            }
            CliMacroAction::Undef { name } => {
                out.push_str("#undef ");
                out.push_str(&name);
                out.push('\n');
            }
        }
    }
    for path in &options.forced_include_files {
        out.push_str("#include \"");
        let path_text = path.to_str().ok_or_else(|| {
            format!(
                "vyre-frontend-c: forced include path {} is not valid UTF-8. Fix: pass -include operands as UTF-8 paths; lossy reference-helper source-prefix generation is forbidden.",
                path.display()
            )
        })?;
        out.push_str(&path_text.replace('\\', "\\\\").replace('"', "\\\""));
        out.push_str("\"\n");
    }
    out.push_str(source);
    Ok(out)
}

pub(super) fn apply_forced_include_prefix(
    source: &str,
    options: &VyreCompileOptions,
) -> Result<String, String> {
    if options.forced_include_files.is_empty() {
        return Ok(source.to_string());
    }
    let mut out = String::new();
    for path in &options.forced_include_files {
        out.push_str("#include \"");
        let path_text = path.to_str().ok_or_else(|| {
            format!(
                "vyre-frontend-c: forced include path {} is not valid UTF-8. Fix: pass -include operands as UTF-8 paths; lossy source-prefix generation is forbidden.",
                path.display()
            )
        })?;
        out.push_str(&path_text.replace('\\', "\\\\").replace('"', "\\\""));
        out.push_str("\"\n");
    }
    out.push_str(source);
    Ok(out)
}

pub(super) fn cli_macro_defs(options: &VyreCompileOptions) -> Vec<gpu_pipeline::MacroDef> {
    let actions = cli_macro_actions(options);
    let mut slots: Vec<Option<gpu_pipeline::MacroDef>> = Vec::with_capacity(actions.len());
    let mut active_by_name: std::collections::HashMap<Vec<u8>, usize> =
        std::collections::HashMap::with_capacity(actions.len());
    for action in actions {
        match action {
            CliMacroAction::Define { name, value } => {
                let name_bytes = name.into_bytes();
                if let Some(previous) = active_by_name.insert(name_bytes.clone(), slots.len()) {
                    slots[previous] = None;
                }
                slots.push(Some(gpu_pipeline::MacroDef {
                    name: name_bytes,
                    args: Vec::new(),
                    body: value.as_deref().unwrap_or("1").as_bytes().to_vec(),
                    is_function_like: false,
                }));
            }
            CliMacroAction::DefineFunction {
                name,
                params,
                value,
            } => {
                let name_bytes = name.into_bytes();
                if let Some(previous) = active_by_name.insert(name_bytes.clone(), slots.len()) {
                    slots[previous] = None;
                }
                slots.push(Some(gpu_pipeline::MacroDef {
                    name: name_bytes,
                    args: params.join(",").as_bytes().to_vec(),
                    body: value.as_deref().unwrap_or("1").as_bytes().to_vec(),
                    is_function_like: true,
                }));
            }
            CliMacroAction::Undef { name } => {
                if let Some(previous) = active_by_name.remove(name.as_bytes()) {
                    slots[previous] = None;
                }
            }
        }
    }
    slots.into_iter().flatten().collect()
}

pub(super) fn cli_macro_actions(options: &VyreCompileOptions) -> Vec<CliMacroAction> {
    let mut actions = target_predefines::predefined_macro_actions(options.target);
    if !options.macro_actions.is_empty() {
        actions.extend(options.macro_actions.clone());
        return actions;
    }
    actions.extend(
        options
            .macros
            .iter()
            .cloned()
            .map(|(name, value)| CliMacroAction::Define { name, value }),
    );
    actions.extend(
        options
            .undefs
            .iter()
            .cloned()
            .map(|name| CliMacroAction::Undef { name }),
    );
    actions
}

pub(super) fn reject_mixed_macro_transport(options: &VyreCompileOptions) -> Result<(), String> {
    if options.macro_actions.is_empty() || (options.macros.is_empty() && options.undefs.is_empty())
    {
        return Ok(());
    }
    Err(
        "vyre-frontend-c: mixed macro transport is ambiguous: macro_actions is ordered but macros/undefs are legacy unordered fields. Fix: move every -D/-U input into VyreCompileOptions::macro_actions and leave macros/undefs empty."
            .to_string(),
    )
}
