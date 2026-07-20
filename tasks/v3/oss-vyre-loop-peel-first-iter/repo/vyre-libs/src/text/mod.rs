//! Text-processing compositions for the GPU C parser pipeline.
//!
//! Phase L1 starts with byte classification. Later phases add UTF-8
//! validation, line indexing, and other host-fed parser helpers that
//! keep source-language parsing on CPU while pushing bulk analysis
//! onto GPU storage buffers.

pub mod char_class;

pub use char_class::{
    build_char_class_table, char_class, C_ALPHA, C_AMP, C_BACKSLASH, C_BANG, C_CARET,
    C_CLOSE_BRACE, C_CLOSE_BRACKET, C_CLOSE_PAREN, C_COMMA, C_DIGIT, C_DOT, C_DQUOTE, C_EOF,
    C_EQUALS, C_GT, C_HASH, C_LT, C_MINUS, C_NEWLINE, C_OPEN_BRACE, C_OPEN_BRACKET, C_OPEN_PAREN,
    C_OTHER, C_PERCENT, C_PIPE, C_PLUS, C_QUOTE, C_SEMICOLON, C_SLASH, C_STAR, C_TILDE, C_WS,
};
