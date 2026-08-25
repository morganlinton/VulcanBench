//! End-to-end example Programs that chain the C11 lex / preprocess
//! / parse stages. Fixtures for integration tests and grammar-gen
//! consumers; not a Cat-A op itself.

/// Content-keyed source pipeline cache.
pub mod source_cache;
/// Named GPU stages for embedders (`c11_lexer`, preprocess, …).
pub mod stages;

/// Megakernel sketch and inventory registration for the full C11 pipeline narrative.
pub mod examples;
