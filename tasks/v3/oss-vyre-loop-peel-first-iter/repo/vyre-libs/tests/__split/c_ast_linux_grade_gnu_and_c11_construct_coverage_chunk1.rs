// CPU-only reference tests for Linux-grade C constructs not covered by the
// existing GNU extension test suite.
//
// Constructs under test:
//   - `__auto_type` declarations
//   - `typeof` / `__typeof__` used as declaration type-specifiers
//   - `__attribute__` variants: `cleanup`, `constructor`, `destructor`, `mode`,
//     `packed`, `aligned`
//   - labels and computed-goto interactions
//   - statement expressions in initializer and declarator positions
//   - `_Static_assert`
//   - `_Alignas` / `_Alignof`
//
// These tests exercise the CPU reference pipeline (build -> annotate -> classify).


use vyre_libs::parsing::c::lex::keyword::reference_c_keyword_types;
use vyre_libs::parsing::c::lex::tokens::*;
use vyre_libs::parsing::c::parse::vast::{
    reference_c11_annotate_typedef_names, reference_c11_build_vast_nodes,
    reference_c11_classify_vast_node_kinds, C_AST_KIND_ASSIGN_EXPR, C_AST_KIND_ATTRIBUTE_ALIGNED,
    C_AST_KIND_ATTRIBUTE_CLEANUP, C_AST_KIND_ATTRIBUTE_CONSTRUCTOR,
    C_AST_KIND_ATTRIBUTE_DESTRUCTOR, C_AST_KIND_ATTRIBUTE_MODE, C_AST_KIND_ATTRIBUTE_PACKED,
    C_AST_KIND_FUNCTION_DEFINITION, C_AST_KIND_GNU_ATTRIBUTE, C_AST_KIND_GNU_LABEL_ADDRESS_EXPR,
    C_AST_KIND_GOTO_STMT, C_AST_KIND_LABEL_STMT,
};
use vyre_primitives::predicate::node_kind;

const VAST_STRIDE_U32: usize = 10;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

#[derive(Clone, Copy)]
struct FixtureToken {
    lexeme: &'static str,
    raw_kind: u32,
}

impl FixtureToken {
    const fn new(lexeme: &'static str, raw_kind: u32) -> Self {
        Self { lexeme, raw_kind }
    }
}

struct Fixture {
    source: String,
    tok_types: Vec<u32>,
    tok_starts: Vec<u32>,
    tok_lens: Vec<u32>,
}

fn build_fixture(tokens: &[FixtureToken]) -> Fixture {
    let mut source = String::new();
    let mut tok_starts = Vec::with_capacity(tokens.len());
    let mut tok_lens = Vec::with_capacity(tokens.len());
    let mut raw_kinds = Vec::with_capacity(tokens.len());

    for token in tokens {
        if !source.is_empty() && !source.ends_with('\n') {
            source.push(' ');
        }
        tok_starts.push(source.len() as u32);
        source.push_str(token.lexeme);
        tok_lens.push(token.lexeme.len() as u32);
        raw_kinds.push(token.raw_kind);
    }

    let promoted = reference_c_keyword_types(&raw_kinds, &tok_starts, &tok_lens, source.as_bytes());

    Fixture {
        source,
        tok_types: promoted,
        tok_starts,
        tok_lens,
    }
}

fn word_at(buf: &[u8], word: usize) -> u32 {
    let off = word * 4;
    u32::from_le_bytes(buf[off..off + 4].try_into().unwrap())
}

fn row_indices(rows: &[u8], kind: u32) -> Vec<usize> {
    rows.chunks_exact(VAST_STRIDE_U32 * 4)
        .enumerate()
        .filter_map(|(idx, row)| {
            let row_kind = u32::from_le_bytes(row[0..4].try_into().unwrap());
            (row_kind == kind).then_some(idx)
        })
        .collect()
}

fn classify(fix: &Fixture) -> Vec<u8> {
    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let annotated = reference_c11_annotate_typedef_names(&raw, fix.source.as_bytes());
    reference_c11_classify_vast_node_kinds(&annotated)
}

fn kind_at(rows: &[u8], idx: usize) -> u32 {
    word_at(rows, idx * VAST_STRIDE_U32)
}

// ---------------------------------------------------------------------------
// Fixture builders
// ---------------------------------------------------------------------------

fn fixture_auto_type_decl() -> Fixture {
    build_fixture(&[
        FixtureToken::new("__auto_type", TOK_IDENTIFIER),
        FixtureToken::new("x", TOK_IDENTIFIER),
        FixtureToken::new("=", TOK_ASSIGN),
        FixtureToken::new("1", TOK_INTEGER),
        FixtureToken::new(";", TOK_SEMICOLON),
    ])
}

fn fixture_typeof_in_decl() -> Fixture {
    build_fixture(&[
        FixtureToken::new("__typeof__", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("int", TOK_IDENTIFIER),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new("y", TOK_IDENTIFIER),
        FixtureToken::new(";", TOK_SEMICOLON),
        FixtureToken::new("typeof", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("long", TOK_IDENTIFIER),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new("z", TOK_IDENTIFIER),
        FixtureToken::new(";", TOK_SEMICOLON),
    ])
}

fn fixture_attribute_cleanup() -> Fixture {
    build_fixture(&[
        FixtureToken::new("__attribute__", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("cleanup", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("free", TOK_IDENTIFIER),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new("void", TOK_IDENTIFIER),
        FixtureToken::new("*", TOK_STAR),
        FixtureToken::new("p", TOK_IDENTIFIER),
        FixtureToken::new(";", TOK_SEMICOLON),
    ])
}

fn fixture_attribute_constructor() -> Fixture {
    build_fixture(&[
        FixtureToken::new("__attribute__", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("constructor", TOK_IDENTIFIER),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new("void", TOK_IDENTIFIER),
        FixtureToken::new("init", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("void", TOK_IDENTIFIER),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new("{", TOK_LBRACE),
        FixtureToken::new("}", TOK_RBRACE),
    ])
}

fn fixture_attribute_destructor() -> Fixture {
    build_fixture(&[
        FixtureToken::new("__attribute__", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("destructor", TOK_IDENTIFIER),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new("void", TOK_IDENTIFIER),
        FixtureToken::new("fini", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("void", TOK_IDENTIFIER),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new("{", TOK_LBRACE),
        FixtureToken::new("}", TOK_RBRACE),
    ])
}

fn fixture_attribute_mode() -> Fixture {
    build_fixture(&[
        FixtureToken::new("__attribute__", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("mode", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("__word__", TOK_IDENTIFIER),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new("unsigned", TOK_IDENTIFIER),
        FixtureToken::new("int", TOK_IDENTIFIER),
        FixtureToken::new("w", TOK_IDENTIFIER),
        FixtureToken::new(";", TOK_SEMICOLON),
    ])
}

fn fixture_attribute_packed_and_aligned() -> Fixture {
    build_fixture(&[
        FixtureToken::new("struct", TOK_IDENTIFIER),
        FixtureToken::new("__attribute__", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("packed", TOK_IDENTIFIER),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new("s", TOK_IDENTIFIER),
        FixtureToken::new("{", TOK_LBRACE),
        FixtureToken::new("char", TOK_IDENTIFIER),
        FixtureToken::new("c", TOK_IDENTIFIER),
        FixtureToken::new(";", TOK_SEMICOLON),
        FixtureToken::new("}", TOK_RBRACE),
        FixtureToken::new(";", TOK_SEMICOLON),
        FixtureToken::new("__attribute__", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("aligned", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("8", TOK_INTEGER),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new("long", TOK_IDENTIFIER),
        FixtureToken::new("l", TOK_IDENTIFIER),
        FixtureToken::new(";", TOK_SEMICOLON),
    ])
}

fn fixture_computed_goto() -> Fixture {
    build_fixture(&[
        FixtureToken::new("void", TOK_IDENTIFIER),
        FixtureToken::new("f", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("void", TOK_IDENTIFIER),
        FixtureToken::new("*", TOK_STAR),
        FixtureToken::new("p", TOK_IDENTIFIER),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new("{", TOK_LBRACE),
        FixtureToken::new("goto", TOK_GOTO),
        FixtureToken::new("*", TOK_STAR),
        FixtureToken::new("p", TOK_IDENTIFIER),
        FixtureToken::new(";", TOK_SEMICOLON),
        FixtureToken::new("}", TOK_RBRACE),
    ])
}

fn fixture_label_and_computed_goto_interaction() -> Fixture {
    build_fixture(&[
        FixtureToken::new("void", TOK_IDENTIFIER),
        FixtureToken::new("g", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("void", TOK_IDENTIFIER),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new("{", TOK_LBRACE),
        FixtureToken::new("void", TOK_IDENTIFIER),
        FixtureToken::new("*", TOK_STAR),
        FixtureToken::new("t", TOK_IDENTIFIER),
        FixtureToken::new("=", TOK_ASSIGN),
        FixtureToken::new("&&", TOK_AND),
        FixtureToken::new("lbl", TOK_IDENTIFIER),
        FixtureToken::new(";", TOK_SEMICOLON),
        FixtureToken::new("goto", TOK_GOTO),
        FixtureToken::new("*", TOK_STAR),
        FixtureToken::new("t", TOK_IDENTIFIER),
        FixtureToken::new(";", TOK_SEMICOLON),
        FixtureToken::new("lbl", TOK_IDENTIFIER),
        FixtureToken::new(":", TOK_COLON),
        FixtureToken::new("return", TOK_RETURN),
        FixtureToken::new(";", TOK_SEMICOLON),
        FixtureToken::new("}", TOK_RBRACE),
    ])
}

fn fixture_stmt_expr_initializer() -> Fixture {
    build_fixture(&[
        FixtureToken::new("int", TOK_IDENTIFIER),
        FixtureToken::new("a", TOK_IDENTIFIER),
        FixtureToken::new("=", TOK_ASSIGN),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("{", TOK_LBRACE),
        FixtureToken::new("int", TOK_IDENTIFIER),
        FixtureToken::new("b", TOK_IDENTIFIER),
        FixtureToken::new("=", TOK_ASSIGN),
        FixtureToken::new("1", TOK_INTEGER),
        FixtureToken::new(";", TOK_SEMICOLON),
        FixtureToken::new("b", TOK_IDENTIFIER),
        FixtureToken::new("+", TOK_PLUS),
        FixtureToken::new("2", TOK_INTEGER),
        FixtureToken::new(";", TOK_SEMICOLON),
        FixtureToken::new("}", TOK_RBRACE),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new(";", TOK_SEMICOLON),
    ])
}

fn fixture_stmt_expr_in_declarator() -> Fixture {
    // GNU C allows statement expressions in array sizes:
    // int arr[({ int x = 2; x; })];
    build_fixture(&[
        FixtureToken::new("int", TOK_IDENTIFIER),
        FixtureToken::new("arr", TOK_IDENTIFIER),
        FixtureToken::new("[", TOK_LBRACKET),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("{", TOK_LBRACE),
        FixtureToken::new("int", TOK_IDENTIFIER),
        FixtureToken::new("x", TOK_IDENTIFIER),
        FixtureToken::new("=", TOK_ASSIGN),
        FixtureToken::new("2", TOK_INTEGER),
        FixtureToken::new(";", TOK_SEMICOLON),
        FixtureToken::new("x", TOK_IDENTIFIER),
        FixtureToken::new(";", TOK_SEMICOLON),
        FixtureToken::new("}", TOK_RBRACE),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new("]", TOK_RBRACKET),
        FixtureToken::new(";", TOK_SEMICOLON),
    ])
}

fn fixture_static_assert() -> Fixture {
    build_fixture(&[
        FixtureToken::new("_Static_assert", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("1", TOK_INTEGER),
        FixtureToken::new(",", TOK_COMMA),
        FixtureToken::new("\"ok\"", TOK_STRING),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new(";", TOK_SEMICOLON),
    ])
}

fn fixture_alignas_var() -> Fixture {
    build_fixture(&[
        FixtureToken::new("_Alignas", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("16", TOK_INTEGER),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new("char", TOK_IDENTIFIER),
        FixtureToken::new("buf", TOK_IDENTIFIER),
        FixtureToken::new("[", TOK_LBRACKET),
        FixtureToken::new("64", TOK_INTEGER),
        FixtureToken::new("]", TOK_RBRACKET),
        FixtureToken::new(";", TOK_SEMICOLON),
    ])
}

fn fixture_alignof_expr() -> Fixture {
    build_fixture(&[
        FixtureToken::new("int", TOK_IDENTIFIER),
        FixtureToken::new("a", TOK_IDENTIFIER),
        FixtureToken::new("=", TOK_ASSIGN),
        FixtureToken::new("_Alignof", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("int", TOK_IDENTIFIER),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new(";", TOK_SEMICOLON),
    ])
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[test]
fn cpu_reference_auto_type_decl_parses_without_panic() {
    let fix = fixture_auto_type_decl();
    let typed = classify(&fix);

    assert!(
        !typed.is_empty(),
        "__auto_type fixture must produce a non-empty typed VAST"
    );
    assert_eq!(
        fix.tok_types[0], TOK_GNU_AUTO_TYPE,
        "__auto_type must promote to a declaration type token"
    );
}

#[test]
fn cpu_reference_typeof_in_decl_position() {
    let fix = fixture_typeof_in_decl();
    let typed = classify(&fix);

    assert!(
        !typed.is_empty(),
        "typeof declaration fixture must produce a non-empty typed VAST"
    );
    assert_eq!(
        fix.tok_types[0], TOK_GNU_TYPEOF,
        "__typeof__ must promote to TOK_GNU_TYPEOF"
    );
    assert_eq!(
        fix.tok_types[6], TOK_GNU_TYPEOF,
        "typeof must promote to TOK_GNU_TYPEOF"
    );

    // Both y and z should classify as variables (declarator identifiers)
    let vars = row_indices(&typed, node_kind::VARIABLE);
    assert!(vars.contains(&4), "y declarator must classify as VARIABLE");
    assert!(vars.contains(&10), "z declarator must classify as VARIABLE");
}

#[test]
fn cpu_reference_attribute_cleanup_parses() {
    let fix = fixture_attribute_cleanup();
    let typed = classify(&fix);

    assert!(
        !typed.is_empty(),
        "cleanup attribute fixture must produce a non-empty typed VAST"
    );
    assert_eq!(
        row_indices(&typed, C_AST_KIND_GNU_ATTRIBUTE),
        vec![0],
        "__attribute__ must classify as GNU_ATTRIBUTE"
    );
    assert_eq!(
        row_indices(&typed, C_AST_KIND_ATTRIBUTE_CLEANUP),
        vec![3],
        "cleanup must classify as a specific GNU attribute kind"
    );
}

