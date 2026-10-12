//! Macro-shaped calls with trailing commas must remain call expressions.

#![cfg(feature = "c-parser")]
#![allow(deprecated)]
mod c_ast_gpu_parity_support;

use c_ast_gpu_parity_support::{
    assert_full_pipeline_parity, build_fixture, row_indices, Fixture, FixtureToken,
};
use vyre_libs::parsing::c::lex::tokens::*;
use vyre_libs::parsing::c::parse::vast::{
    reference_c11_annotate_typedef_names, reference_c11_build_vast_nodes,
    reference_c11_classify_vast_node_kinds,
};
use vyre_primitives::predicate::node_kind;

fn fixture_macro_call_with_trailing_comma() -> Fixture {
    build_fixture(&[
        FixtureToken::new("int", TOK_IDENTIFIER),
        FixtureToken::new("v", TOK_IDENTIFIER),
        FixtureToken::new(";", TOK_SEMICOLON),
        FixtureToken::new("v", TOK_IDENTIFIER),
        FixtureToken::new("=", TOK_ASSIGN),
        FixtureToken::new("FOO", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("a", TOK_IDENTIFIER),
        FixtureToken::new(",", TOK_COMMA),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new(";", TOK_SEMICOLON),
    ])
}

#[test]
fn macro_call_with_trailing_comma_gpu_cpu_parity() {
    let fix = fixture_macro_call_with_trailing_comma();
    assert_full_pipeline_parity(&fix, "macro_call_trailing_comma");

    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let annotated = reference_c11_annotate_typedef_names(&raw, fix.source.as_bytes());
    let typed = reference_c11_classify_vast_node_kinds(&annotated);

    assert!(
        row_indices(&typed, node_kind::CALL).contains(&5),
        "FOO must classify as a CALL even when its argument list has a trailing comma"
    );
    assert!(
        row_indices(&typed, node_kind::VARIABLE).contains(&7),
        "argument identifier must remain a VARIABLE row"
    );
}
