use super::*;
pub(in crate::tu_host) fn parse_define(rest: &str) -> Option<(String, MacroDef)> {
    let rest = rest.trim_start();
    let bytes = rest.as_bytes();
    let mut name_end = 0usize;
    if bytes.first().is_none_or(|b| !is_ident_start(*b)) {
        return None;
    }
    while name_end < bytes.len() && is_ident_continue(bytes[name_end]) {
        name_end += 1;
    }
    let name = rest[..name_end].to_string();
    let after_name = &rest[name_end..];
    if let Some(param_tail) = after_name.strip_prefix('(') {
        let close = param_tail.find(')')?;
        let mut params = Vec::new();
        #[cfg(feature = "cpu-oracle")]
        let mut variadic = None;
        for raw_param in param_tail[..close].split(',') {
            let param = raw_param.trim();
            if param.is_empty() {
                continue;
            }
            if param == "..." {
                #[cfg(feature = "cpu-oracle")]
                {
                    variadic = Some("__VA_ARGS__".to_string());
                }
            } else if let Some(name) = param.strip_suffix("...") {
                #[cfg(feature = "cpu-oracle")]
                {
                    let name = name.trim();
                    if !name.is_empty() {
                        variadic = Some(name.to_string());
                    }
                }
            } else {
                params.push(param.to_string());
            }
        }
        let replacement = param_tail[close + 1..].trim().to_string();
        Some((
            name,
            MacroDef {
                params: Some(params),
                #[cfg(feature = "cpu-oracle")]
                variadic,
                replacement,
            },
        ))
    } else {
        Some((
            name,
            MacroDef {
                params: None,
                #[cfg(feature = "cpu-oracle")]
                variadic: None,
                replacement: after_name.trim().to_string(),
            },
        ))
    }
}
