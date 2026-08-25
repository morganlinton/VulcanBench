//! Rust lexer kernel: CPU reference and GPU dispatch.

/// CPU reference lexer (hand-written, validated against `rustc_lexer`).
pub mod core;

/// GPU sparse-dispatch lexer plan builder.
pub mod plan;
