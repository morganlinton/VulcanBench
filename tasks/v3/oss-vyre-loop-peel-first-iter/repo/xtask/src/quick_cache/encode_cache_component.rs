#![allow(
    missing_docs,
    dead_code,
    unused_imports,
    unused_variables,
    unreachable_patterns,
    clippy::all
)]
use crate::quick_cache::nibble;

pub(crate) fn encode_cache_component(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_' {
            out.push(byte as char);
        } else {
            out.push('%');
            out.push(nibble(byte >> 4));
            out.push(nibble(byte & 0x0f));
        }
    }
    out
}
