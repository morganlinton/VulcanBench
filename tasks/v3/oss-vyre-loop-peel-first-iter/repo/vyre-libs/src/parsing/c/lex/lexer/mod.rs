//! C11 lexer program builder.
//!
//! `c11_lexer` constructs a single `Vec<Node>` by appending classifier
//! sub-builders. Each sub-builder lives in its own file:
//!  - `helpers.rs`: byte-class predicates + `set_token` + `classify_keyword`
//!  - `sections.rs`: large extracted operator-table + epilogue builders
//!  - `core.rs`: top-level `c11_lexer` orchestrator
//!  - `digraphs.rs`: digraph + line-splice resolution pass

mod core;
mod core_sparse;
mod digraphs;
mod helpers;
mod sections;
mod single_pass;
mod sparse_compact;

pub use core::{c11_lexer, c11_lexer_regular, c11_lexer_regular_ranked, c11_lexer_regular_sparse};
pub use core_sparse::{
    c11_lexer_regular_sparse_no_directives_no_backscan,
    c11_lexer_regular_sparse_packed_haystack_with_block_totals,
    c11_lexer_regular_sparse_packed_haystack_with_flags,
    c11_lexer_regular_sparse_packed_haystack_with_flags_no_directives,
    c11_lexer_regular_sparse_packed_haystack_with_flags_no_directives_no_backscan,
    c11_lexer_regular_sparse_u8_haystack_with_flags,
};
pub use digraphs::c11_lex_digraphs;
pub use single_pass::{c11_lex_regular_single_pass, c11_lex_single_pass};
pub use sparse_compact::{c11_compact_sparse_tokens, c11_compact_sparse_tokens_output};

// Sibling re-exports keep each lexer submodule on one explicit helper surface.
// If a helper stops being shared by multiple active lexer builders, move it
// into the single module that owns it.
