#![allow(
    missing_docs,
    dead_code,
    unused_imports,
    unused_variables,
    unreachable_patterns,
    clippy::all
)]

pub(crate) fn json_string_field(content: &str, field: &str) -> Option<String> {
    let needle = format!("\"{}\":\"", field);
    let start = content.find(&needle)? + needle.len();
    let rest = &content[start..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}
