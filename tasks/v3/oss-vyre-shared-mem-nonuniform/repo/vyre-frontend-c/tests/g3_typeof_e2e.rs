//! g3_typeof_e2e  -  end-to-end typeof / __typeof__ type-specifier integration tests.

mod support;

use support::*;
use vyre_libs::parsing::c::parse::vast::C_AST_KIND_SIZEOF_EXPR;
use vyre_primitives::predicate::node_kind;

const TYPEOF_USES_SOURCE: &str = include_str!("corpus/g3_typeof/typeof_uses.c");

#[test]
fn typeof_specifier_in_declaration_compiles_and_classifies_variables() {
    let (object, resident) =
        compile_source_with_resident("typeof_uses", TYPEOF_USES_SOURCE, Vec::new(), Vec::new());
    object.assert_elf();

    let lex = object.lex();
    let vast_words = object.words(SECTION_VAST);
    let pg_words = object.words(SECTION_PROGRAM_GRAPH);

    assert_typed_vast_and_pg_rows(
        &lex.tok_types,
        &lex.starts,
        &lex.lens,
        &vast_words,
        &pg_words,
    );

    // __typeof__ token (spelled "__typeof__" in source).
    let typeof_idx = find_token(&resident, &lex.starts, &lex.lens, "__typeof__");
    assert_token_kind(
        &resident,
        &lex.starts,
        &lex.lens,
        &vast_words,
        typeof_idx,
        C_AST_KIND_SIZEOF_EXPR,
        "__typeof__ should be classified as an expression-ish operator node",
    );

    // Plain typeof token.
    let plain_idx = find_token_after(&resident, &lex.starts, &lex.lens, "typeof", typeof_idx);
    assert_token_kind(
        &resident,
        &lex.starts,
        &lex.lens,
        &vast_words,
        plain_idx,
        C_AST_KIND_SIZEOF_EXPR,
        "typeof should be classified as an expression-ish operator node",
    );

    // y and z should be ordinary variables (declarators after typeof type-specifier).
    let y_idx = find_token_after(&resident, &lex.starts, &lex.lens, "y", plain_idx);
    assert_token_kind(
        &resident,
        &lex.starts,
        &lex.lens,
        &vast_words,
        y_idx,
        node_kind::VARIABLE,
        "y declarator should be a variable",
    );

    let z_idx = find_token_after(&resident, &lex.starts, &lex.lens, "z", y_idx);
    assert_token_kind(
        &resident,
        &lex.starts,
        &lex.lens,
        &vast_words,
        z_idx,
        node_kind::VARIABLE,
        "z declarator should be a variable",
    );
}

#[test]
fn typeof_no_arg_does_not_panic_or_corrupt_pipeline() {
    // Empty typeof argument is invalid C, but the pipeline must not crash
    // and must still emit a well-formed object envelope.
    let source = include_str!("corpus/g3_typeof/negatives/typeof_no_arg.c");
    let (object, _resident) =
        compile_source_with_resident("typeof_no_arg", source, Vec::new(), Vec::new());
    object.assert_elf();

    let lex = object.lex();
    let vast_words = object.words(SECTION_VAST);
    assert_eq!(
        vast_words.len(),
        lex.tok_types.len() * VAST_STRIDE_U32,
        "VAST section length matches token count even for malformed typeof"
    );
}
