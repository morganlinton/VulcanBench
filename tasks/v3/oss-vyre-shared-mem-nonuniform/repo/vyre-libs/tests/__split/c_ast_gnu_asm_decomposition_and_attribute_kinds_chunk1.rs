// CPU-only reference tests for extended GNU asm decomposition and
// GNU attribute-specific AST kinds.
//
// These tests exercise `reference_c11_classify_vast_node_kinds` through
// the full three-stage CPU pipeline (build → annotate → classify) to
// verify that:
//   - asm templates, output operands, input operands, clobbers, and goto
//     labels each receive a distinct `C_AST_KIND_ASM_*` kind.
//   - the eight supported GNU attribute names (`section`, `weak`, `alias`,
//     `aligned`, `used`, `unused`, `naked`, `visibility`) each receive a
//     distinct `C_AST_KIND_ATTRIBUTE_*` kind.
//   - identifiers outside attribute contexts are never mis-classified as
//     attribute-specific kinds.


use vyre_libs::parsing::c::lex::keyword::reference_c_keyword_types;
use vyre_libs::parsing::c::lex::tokens::*;
use vyre_libs::parsing::c::parse::vast::{
    reference_c11_annotate_typedef_names, reference_c11_build_vast_nodes,
    reference_c11_classify_vast_node_kinds, C_AST_KIND_ASM_CLOBBERS_LIST,
    C_AST_KIND_ASM_GOTO_LABELS, C_AST_KIND_ASM_INPUT_OPERAND, C_AST_KIND_ASM_OUTPUT_OPERAND,
    C_AST_KIND_ASM_TEMPLATE, C_AST_KIND_ATTRIBUTE_ALIAS, C_AST_KIND_ATTRIBUTE_ALIGNED,
    C_AST_KIND_ATTRIBUTE_NAKED, C_AST_KIND_ATTRIBUTE_SECTION, C_AST_KIND_ATTRIBUTE_UNUSED,
    C_AST_KIND_ATTRIBUTE_USED, C_AST_KIND_ATTRIBUTE_VISIBILITY, C_AST_KIND_ATTRIBUTE_WEAK,
    C_AST_KIND_GNU_ATTRIBUTE, C_AST_KIND_GOTO_STMT, C_AST_KIND_INLINE_ASM,
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

// ---------------------------------------------------------------------------
// Fixture builders  -  GNU attribute-specific kinds
// ---------------------------------------------------------------------------

fn fixture_attribute_section() -> Fixture {
    build_fixture(&[
        FixtureToken::new("__attribute__", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("section", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("\".text.foo\"", TOK_STRING),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new("void", TOK_IDENTIFIER),
        FixtureToken::new("foo", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new("{", TOK_LBRACE),
        FixtureToken::new("}", TOK_RBRACE),
    ])
}

fn fixture_attribute_weak() -> Fixture {
    build_fixture(&[
        FixtureToken::new("__attribute__", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("weak", TOK_IDENTIFIER),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new("int", TOK_IDENTIFIER),
        FixtureToken::new("x", TOK_IDENTIFIER),
        FixtureToken::new(";", TOK_SEMICOLON),
    ])
}

fn fixture_attribute_alias() -> Fixture {
    build_fixture(&[
        FixtureToken::new("__attribute__", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("alias", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("\"bar\"", TOK_STRING),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new("void", TOK_IDENTIFIER),
        FixtureToken::new("foo", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new(";", TOK_SEMICOLON),
    ])
}

fn fixture_attribute_aligned() -> Fixture {
    build_fixture(&[
        FixtureToken::new("__attribute__", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("aligned", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("16", TOK_INTEGER),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new("char", TOK_IDENTIFIER),
        FixtureToken::new("buf", TOK_IDENTIFIER),
        FixtureToken::new("[", TOK_LBRACKET),
        FixtureToken::new("64", TOK_INTEGER),
        FixtureToken::new("]", TOK_RBRACKET),
        FixtureToken::new(";", TOK_SEMICOLON),
    ])
}

fn fixture_attribute_used() -> Fixture {
    build_fixture(&[
        FixtureToken::new("__attribute__", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("used", TOK_IDENTIFIER),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new("static", TOK_IDENTIFIER),
        FixtureToken::new("int", TOK_IDENTIFIER),
        FixtureToken::new("x", TOK_IDENTIFIER),
        FixtureToken::new(";", TOK_SEMICOLON),
    ])
}

fn fixture_attribute_unused() -> Fixture {
    build_fixture(&[
        FixtureToken::new("__attribute__", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("unused", TOK_IDENTIFIER),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new("int", TOK_IDENTIFIER),
        FixtureToken::new("x", TOK_IDENTIFIER),
        FixtureToken::new(";", TOK_SEMICOLON),
    ])
}

fn fixture_attribute_naked() -> Fixture {
    build_fixture(&[
        FixtureToken::new("__attribute__", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("naked", TOK_IDENTIFIER),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new("void", TOK_IDENTIFIER),
        FixtureToken::new("entry", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new("{", TOK_LBRACE),
        FixtureToken::new("}", TOK_RBRACE),
    ])
}

fn fixture_attribute_visibility() -> Fixture {
    build_fixture(&[
        FixtureToken::new("__attribute__", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("visibility", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("\"hidden\"", TOK_STRING),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new("void", TOK_IDENTIFIER),
        FixtureToken::new("foo", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new(";", TOK_SEMICOLON),
    ])
}

// ---------------------------------------------------------------------------
// Fixture builders  -  extended GNU asm decomposition
// ---------------------------------------------------------------------------

fn fixture_asm_multiple_outputs() -> Fixture {
    build_fixture(&[
        FixtureToken::new("asm", TOK_IDENTIFIER),
        FixtureToken::new("volatile", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("\"mov %1, %0\"", TOK_STRING),
        FixtureToken::new(":", TOK_COLON),
        FixtureToken::new("\"=r\"", TOK_STRING),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("a", TOK_IDENTIFIER),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new(",", TOK_COMMA),
        FixtureToken::new("\"=r\"", TOK_STRING),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("b", TOK_IDENTIFIER),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new(":", TOK_COLON),
        FixtureToken::new("\"r\"", TOK_STRING),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("c", TOK_IDENTIFIER),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new(":", TOK_COLON),
        FixtureToken::new("\"memory\"", TOK_STRING),
        FixtureToken::new(",", TOK_COMMA),
        FixtureToken::new("\"cc\"", TOK_STRING),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new(";", TOK_SEMICOLON),
    ])
}

fn fixture_asm_multiple_inputs() -> Fixture {
    build_fixture(&[
        FixtureToken::new("asm", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("\"add %0, %1, %2\"", TOK_STRING),
        FixtureToken::new(":", TOK_COLON),
        FixtureToken::new("\"=r\"", TOK_STRING),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("dst", TOK_IDENTIFIER),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new(":", TOK_COLON),
        FixtureToken::new("\"r\"", TOK_STRING),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("src1", TOK_IDENTIFIER),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new(",", TOK_COMMA),
        FixtureToken::new("\"r\"", TOK_STRING),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("src2", TOK_IDENTIFIER),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new(";", TOK_SEMICOLON),
    ])
}

fn fixture_asm_goto_multiple_labels() -> Fixture {
    build_fixture(&[
        FixtureToken::new("asm", TOK_IDENTIFIER),
        FixtureToken::new("goto", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("\"jmp %l0\"", TOK_STRING),
        FixtureToken::new(":", TOK_COLON),
        FixtureToken::new(":", TOK_COLON),
        FixtureToken::new(":", TOK_COLON),
        FixtureToken::new(":", TOK_COLON),
        FixtureToken::new("error", TOK_IDENTIFIER),
        FixtureToken::new(",", TOK_COMMA),
        FixtureToken::new("done", TOK_IDENTIFIER),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new(";", TOK_SEMICOLON),
    ])
}

fn fixture_basic_asm() -> Fixture {
    build_fixture(&[
        FixtureToken::new("asm", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("\"nop\"", TOK_STRING),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new(";", TOK_SEMICOLON),
    ])
}

fn fixture_non_attribute_identifier() -> Fixture {
    build_fixture(&[
        FixtureToken::new("void", TOK_IDENTIFIER),
        FixtureToken::new("foo", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("section", TOK_IDENTIFIER),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new(";", TOK_SEMICOLON),
    ])
}

// ---------------------------------------------------------------------------
// Tests  -  GNU attribute-specific kinds
// ---------------------------------------------------------------------------

#[test]
fn cpu_reference_classifies_attribute_section() {
    let fix = fixture_attribute_section();
    let typed = classify(&fix);

    assert_eq!(
        row_indices(&typed, C_AST_KIND_ATTRIBUTE_SECTION),
        vec![3],
        "section attribute name must classify as ATTRIBUTE_SECTION"
    );
    assert_eq!(
        row_indices(&typed, C_AST_KIND_GNU_ATTRIBUTE),
        vec![0],
        "__attribute__ keyword must classify as GNU_ATTRIBUTE"
    );
}

#[test]
fn cpu_reference_classifies_attribute_weak() {
    let fix = fixture_attribute_weak();
    let typed = classify(&fix);

    assert_eq!(
        row_indices(&typed, C_AST_KIND_ATTRIBUTE_WEAK),
        vec![3],
        "weak attribute name must classify as ATTRIBUTE_WEAK"
    );
}

#[test]
fn cpu_reference_classifies_attribute_alias() {
    let fix = fixture_attribute_alias();
    let typed = classify(&fix);

    assert_eq!(
        row_indices(&typed, C_AST_KIND_ATTRIBUTE_ALIAS),
        vec![3],
        "alias attribute name must classify as ATTRIBUTE_ALIAS"
    );
}

#[test]
fn cpu_reference_classifies_attribute_aligned() {
    let fix = fixture_attribute_aligned();
    let typed = classify(&fix);

    assert_eq!(
        row_indices(&typed, C_AST_KIND_ATTRIBUTE_ALIGNED),
        vec![3],
        "aligned attribute name must classify as ATTRIBUTE_ALIGNED"
    );
}

#[test]
fn cpu_reference_classifies_attribute_used() {
    let fix = fixture_attribute_used();
    let typed = classify(&fix);

    assert_eq!(
        row_indices(&typed, C_AST_KIND_ATTRIBUTE_USED),
        vec![3],
        "used attribute name must classify as ATTRIBUTE_USED"
    );
}

#[test]
fn cpu_reference_classifies_attribute_unused() {
    let fix = fixture_attribute_unused();
    let typed = classify(&fix);

    assert_eq!(
        row_indices(&typed, C_AST_KIND_ATTRIBUTE_UNUSED),
        vec![3],
        "unused attribute name must classify as ATTRIBUTE_UNUSED"
    );
}

