//! GPU DFA lexer pipeline: classifier table, lexer kernel, token
//! constants, keyword recogniser.

/// Source-positioned diagnostics decoded from lexer error tokens.
pub mod diagnostics;
/// Post-lex keyword promotion (identifier → keyword token id).
pub mod keyword;
/// Maximally-munching DFA-driven lexer kernel.
pub mod lexer;
/// Token-id constants (`TOK_*`) shared by every C-parser stage.
pub mod tokens;
