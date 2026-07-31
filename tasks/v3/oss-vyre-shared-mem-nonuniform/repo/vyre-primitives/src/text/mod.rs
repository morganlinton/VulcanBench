//! Tier 2.5 text primitives.
//!
//! The path IS the interface. Callers write
//! `vyre_primitives::text::char_class::char_class(...)`  -  explicit
//! paths keep the LEGO substrate visible at every call site. No
//! wildcard re-exports; the subsystem exposes its sub-modules, not a
//! flat namespace.
//!
//! See `docs/primitives-tier.md` and `docs/lego-block-rule.md`.

/// 256-bin byte histogram over source bytes.
pub mod byte_histogram;
/// Byte classifier  -  host 256-entry lookup table classifies each source byte.
pub mod char_class;
/// Histogram-based encoding classifier.
#[cfg(feature = "reduce")]
pub mod encoding_classify;

/// UTF-8 byte classifier  -  single-pass sequence-shape detection.
pub mod utf8_validate;

/// Line-number-per-byte index for diagnostic-producing parsers.
pub mod line_index;
/// UTF-8 shape counters over byte histograms.
pub mod utf8_shape_counts;

#[cfg(any(test, feature = "cpu-parity"))]
pub use byte_histogram::reference_byte_histogram;
pub use byte_histogram::{
    byte_histogram_256, byte_histogram_256_body, byte_histogram_256_child, byte_histogram_256_u8,
    byte_histogram_256_u8_child, BYTE_HISTOGRAM_256_OP_ID,
};
#[cfg(any(test, feature = "cpu-parity", feature = "text"))]
pub use char_class::reference_char_class;
pub use char_class::{
    build_char_class_table, char_class, char_class_dispatch_grid, char_class_u8, pack_bytes_as_u32,
    pack_u32 as pack_classified_u32, CHAR_CLASS_OP_ID, CHAR_CLASS_WORKGROUP_SIZE, C_ALPHA, C_AMP,
    C_BACKSLASH, C_BANG, C_CARET, C_CLOSE_BRACE, C_CLOSE_BRACKET, C_CLOSE_PAREN, C_COMMA, C_DIGIT,
    C_DOT, C_DQUOTE, C_EOF, C_EQUALS, C_GT, C_HASH, C_LT, C_MINUS, C_NEWLINE, C_OPEN_BRACE,
    C_OPEN_BRACKET, C_OPEN_PAREN, C_OTHER, C_PERCENT, C_PIPE, C_PLUS, C_QUOTE, C_SEMICOLON,
    C_SLASH, C_STAR, C_TILDE, C_WS,
};
#[cfg(feature = "reduce")]
pub use encoding_classify::{
    classify_from_histogram as reference_encoding_classify, encoding_classify,
    encoding_classify_body, encoding_classify_child, ENCODING_CLASSIFY_OP_ID,
    ENCODING_CLASSIFY_WORKGROUP_SIZE, ENC_ASCII, ENC_BINARY, ENC_ISO8859_1, ENC_UTF16BE,
    ENC_UTF16LE, ENC_UTF8,
};
#[cfg(any(test, feature = "cpu-parity"))]
pub use line_index::line_index;
#[cfg(any(test, feature = "cpu-parity", feature = "text"))]
pub use line_index::reference_line_index;
#[cfg(any(test, feature = "cpu-parity"))]
pub use utf8_shape_counts::reference_utf8_shape_counts;
pub use utf8_shape_counts::{
    utf8_shape_counts, utf8_shape_counts_body, utf8_shape_counts_child, UTF8_SHAPE_COUNTS_OP_ID,
};
#[cfg(any(test, feature = "cpu-parity", feature = "text"))]
pub use utf8_validate::reference_utf8_validate;
pub use utf8_validate::{
    utf8_validate, utf8_validate_dispatch_grid, utf8_validate_u8, UTF8_ASCII, UTF8_CONT,
    UTF8_INVALID, UTF8_LEAD_2, UTF8_LEAD_3, UTF8_LEAD_4, UTF8_VALIDATE_WORKGROUP_SIZE,
};
