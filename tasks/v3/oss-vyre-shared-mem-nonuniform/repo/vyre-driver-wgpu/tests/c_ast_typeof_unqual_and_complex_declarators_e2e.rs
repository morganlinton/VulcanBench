//! GPU/CPU parity end-to-end tests for typeof/typeof_unqual and complex
//! declarator constructs common in Linux headers and macro expansions.
//!
//! Constructs under test:
//!   - `typeof` in array, pointer, and function-pointer declarators
//!   - deeply parenthesised declarators with typeof type-specifiers
//!   - `typeof_unqual` treated as an identifier fallback (future C23 contract)
//!   - typeof combined with `_Atomic` and nested qualifiers
//!   - function-pointer arrays and function-returning-function pointers
//!
//! A missing GPU adapter is a configuration failure.

#![cfg(feature = "c-parser")]
#![allow(deprecated)]
mod c_ast_gpu_parity_support;

use c_ast_gpu_parity_support::{
    assert_full_pipeline_parity, build_fixture, row_indices, Fixture, FixtureToken,
};
use vyre_libs::parsing::c::lex::tokens::*;
use vyre_libs::parsing::c::parse::vast::{
    reference_c11_annotate_typedef_names, reference_c11_build_vast_nodes,
    reference_c11_classify_vast_node_kinds, C_AST_KIND_ARRAY_DECL, C_AST_KIND_FUNCTION_DECLARATOR,
    C_AST_KIND_POINTER_DECL,
};
use vyre_primitives::predicate::node_kind;

// ---------------------------------------------------------------------------
// Fixture builders
// ---------------------------------------------------------------------------

/// typeof(int) *(*fp[4])(void);
fn fixture_typeof_function_pointer_array() -> Fixture {
    build_fixture(&[
        FixtureToken::new("typeof", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("int", TOK_IDENTIFIER),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new("*", TOK_STAR),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("*", TOK_STAR),
        FixtureToken::new("fp", TOK_IDENTIFIER),
        FixtureToken::new("[", TOK_LBRACKET),
        FixtureToken::new("4", TOK_INTEGER),
        FixtureToken::new("]", TOK_RBRACKET),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("void", TOK_IDENTIFIER),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new(";", TOK_SEMICOLON),
    ])
}

/// typeof(int) (((*ptr)));
fn fixture_typeof_deeply_parenthesised() -> Fixture {
    build_fixture(&[
        FixtureToken::new("typeof", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("int", TOK_IDENTIFIER),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("*", TOK_STAR),
        FixtureToken::new("ptr", TOK_IDENTIFIER),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new(";", TOK_SEMICOLON),
    ])
}

/// typeof(int) * const * volatile p;
fn fixture_typeof_nested_qualifiers() -> Fixture {
    build_fixture(&[
        FixtureToken::new("typeof", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("int", TOK_IDENTIFIER),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new("*", TOK_STAR),
        FixtureToken::new("const", TOK_IDENTIFIER),
        FixtureToken::new("*", TOK_STAR),
        FixtureToken::new("volatile", TOK_IDENTIFIER),
        FixtureToken::new("p", TOK_IDENTIFIER),
        FixtureToken::new(";", TOK_SEMICOLON),
    ])
}

/// __typeof_unqual__(int) z;
/// Simulate keyword promotion to verify the parser pipeline handles
/// typeof_unqual without panic.
fn fixture_typeof_unqual_simulated() -> Fixture {
    let mut fix = build_fixture(&[
        FixtureToken::new("__typeof_unqual__", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("int", TOK_IDENTIFIER),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new("z", TOK_IDENTIFIER),
        FixtureToken::new(";", TOK_SEMICOLON),
    ]);
    // Simulate keyword promotion so the parser sees it as typeof.
    fix.tok_types[0] = TOK_GNU_TYPEOF;
    fix
}

/// _Atomic typeof(int) *q;
fn fixture_atomic_typeof_pointer() -> Fixture {
    build_fixture(&[
        FixtureToken::new("_Atomic", TOK_IDENTIFIER),
        FixtureToken::new("typeof", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("int", TOK_IDENTIFIER),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new("*", TOK_STAR),
        FixtureToken::new("q", TOK_IDENTIFIER),
        FixtureToken::new(";", TOK_SEMICOLON),
    ])
}

/// typeof(int) *(*f(void))(float);
fn fixture_typeof_function_returning_fnptr() -> Fixture {
    build_fixture(&[
        FixtureToken::new("typeof", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("int", TOK_IDENTIFIER),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new("*", TOK_STAR),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("*", TOK_STAR),
        FixtureToken::new("f", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("void", TOK_IDENTIFIER),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("float", TOK_IDENTIFIER),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new(";", TOK_SEMICOLON),
    ])
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[test]
fn typeof_function_pointer_array_gpu_cpu_parity() {
    let fix = fixture_typeof_function_pointer_array();
    assert_eq!(fix.tok_types[0], TOK_GNU_TYPEOF, "typeof must promote");
    assert_full_pipeline_parity(&fix, "typeof_function_pointer_array");

    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let annotated = reference_c11_annotate_typedef_names(&raw, fix.source.as_bytes());
    let typed = reference_c11_classify_vast_node_kinds(&annotated);

    let ptrs = row_indices(&typed, C_AST_KIND_POINTER_DECL);
    assert_eq!(ptrs.len(), 2, "must contain two pointer declarators");
    assert!(
        row_indices(&typed, C_AST_KIND_ARRAY_DECL).contains(&8),
        "fp[4] must classify as ARRAY_DECL"
    );
    assert!(
        !row_indices(&typed, C_AST_KIND_FUNCTION_DECLARATOR).is_empty(),
        "must contain at least one function declarator"
    );
    assert!(
        row_indices(&typed, node_kind::VARIABLE).contains(&7),
        "fp must classify as VARIABLE"
    );
}

#[test]
fn typeof_deeply_parenthesised_pointer_gpu_cpu_parity() {
    let fix = fixture_typeof_deeply_parenthesised();
    assert_eq!(fix.tok_types[0], TOK_GNU_TYPEOF);
    assert_full_pipeline_parity(&fix, "typeof_deeply_parenthesised");

    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let annotated = reference_c11_annotate_typedef_names(&raw, fix.source.as_bytes());
    let typed = reference_c11_classify_vast_node_kinds(&annotated);

    assert!(
        row_indices(&typed, C_AST_KIND_POINTER_DECL).contains(&7),
        "deeply parenthesised pointer must classify as POINTER_DECL"
    );
    assert!(
        row_indices(&typed, node_kind::VARIABLE).contains(&8),
        "ptr must classify as VARIABLE"
    );
}

#[test]
fn typeof_nested_qualifiers_gpu_cpu_parity() {
    let fix = fixture_typeof_nested_qualifiers();
    assert_eq!(fix.tok_types[0], TOK_GNU_TYPEOF);
    assert_full_pipeline_parity(&fix, "typeof_nested_qualifiers");

    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let annotated = reference_c11_annotate_typedef_names(&raw, fix.source.as_bytes());
    let typed = reference_c11_classify_vast_node_kinds(&annotated);

    let ptrs = row_indices(&typed, C_AST_KIND_POINTER_DECL);
    assert_eq!(
        ptrs.len(),
        2,
        "must contain two pointer declarators for * const * volatile"
    );
    assert!(
        row_indices(&typed, node_kind::VARIABLE).contains(&8),
        "p must classify as VARIABLE"
    );
}

#[test]
fn typeof_unqual_simulated_promotion_gpu_cpu_parity() {
    let fix = fixture_typeof_unqual_simulated();
    // We manually promoted __typeof_unqual__ to TOK_GNU_TYPEOF to test
    // forward compatibility of the parser pipeline.
    assert_eq!(fix.tok_types[0], TOK_GNU_TYPEOF);
    assert_full_pipeline_parity(&fix, "typeof_unqual_simulated");

    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let annotated = reference_c11_annotate_typedef_names(&raw, fix.source.as_bytes());
    let typed = reference_c11_classify_vast_node_kinds(&annotated);
    assert!(
        row_indices(&typed, node_kind::VARIABLE).contains(&4),
        "z must classify as VARIABLE after simulated typeof_unqual"
    );
}

#[test]
fn atomic_typeof_pointer_combo_gpu_cpu_parity() {
    let fix = fixture_atomic_typeof_pointer();
    assert_eq!(fix.tok_types[0], TOK_ATOMIC, "_Atomic must promote");
    assert_eq!(fix.tok_types[1], TOK_GNU_TYPEOF, "typeof must promote");
    assert_full_pipeline_parity(&fix, "atomic_typeof_pointer");

    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let annotated = reference_c11_annotate_typedef_names(&raw, fix.source.as_bytes());
    let typed = reference_c11_classify_vast_node_kinds(&annotated);
    assert!(
        row_indices(&typed, C_AST_KIND_POINTER_DECL).contains(&5),
        "pointer declarator must be present"
    );
    assert!(
        row_indices(&typed, node_kind::VARIABLE).contains(&6),
        "q must classify as VARIABLE"
    );
}

#[test]
fn typeof_function_returning_fnptr_gpu_cpu_parity() {
    let fix = fixture_typeof_function_returning_fnptr();
    assert_eq!(fix.tok_types[0], TOK_GNU_TYPEOF);
    assert_full_pipeline_parity(&fix, "typeof_function_returning_fnptr");

    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let annotated = reference_c11_annotate_typedef_names(&raw, fix.source.as_bytes());
    let typed = reference_c11_classify_vast_node_kinds(&annotated);

    let fn_decls = row_indices(&typed, C_AST_KIND_FUNCTION_DECLARATOR);
    assert_eq!(
        fn_decls.len(),
        2,
        "must contain two function declarators (f(void) and (float))"
    );
    let ptrs = row_indices(&typed, C_AST_KIND_POINTER_DECL);
    assert_eq!(ptrs.len(), 2, "must contain two pointer declarators");
    assert!(
        row_indices(&typed, node_kind::FUNCTION_DECL).contains(&7),
        "f must classify as FUNCTION_DECL"
    );
}
