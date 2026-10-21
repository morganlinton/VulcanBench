#![allow(
    missing_docs,
    dead_code,
    unused_imports,
    unused_variables,
    unreachable_patterns,
    clippy::all
)]

pub(crate) fn nibble(value: u8) -> char {
    b"0123456789abcdef"[value as usize] as char
}
