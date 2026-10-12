//! Focused C parser coverage for C99 for-init declarations and macro-shaped
//! `do { ... } while (0)` bodies.

#![cfg(feature = "c-parser")]
#![allow(deprecated)]
use c_grammar_gen::lex_c11_max_munch_kinds;
use vyre::ir::Expr;
use vyre_libs::parsing::c::lex::keyword::reference_c_keyword_types;
use vyre_libs::parsing::c::lex::tokens::*;
use vyre_libs::parsing::c::lower::{c_lower_ast_to_pg_nodes, reference_ast_to_pg_nodes};
use vyre_libs::parsing::c::parse::vast::{
    c11_classify_vast_node_kinds, reference_c11_build_vast_nodes,
    reference_c11_classify_vast_node_kinds, C_AST_KIND_ASSIGN_EXPR, C_AST_KIND_DO_STMT,
    C_AST_KIND_FOR_STMT, C_AST_KIND_WHILE_STMT,
};
use vyre_primitives::predicate::node_kind;
use vyre_reference::value::Value;

const VAST_STRIDE_U32: usize = 10;
const PG_STRIDE_U32: usize = 6;

struct Fixture {
    source: String,
    tok_types: Vec<u32>,
    tok_starts: Vec<u32>,
    tok_lens: Vec<u32>,
}

struct Rows {
    fixture: Fixture,
    typed_vast: Vec<u8>,
    pg: Vec<u8>,
}

fn assemble(lexemes: &[(&str, u32)]) -> Fixture {
    let mut source = String::new();
    let mut raw_kinds = Vec::new();
    let mut tok_starts = Vec::new();
    let mut tok_lens = Vec::new();

    for &(lexeme, kind) in lexemes {
        if kind == TOK_WHITESPACE || kind == TOK_COMMENT {
            source.push_str(lexeme);
            continue;
        }
        if !source.is_empty() && !source.ends_with('\n') {
            source.push(' ');
        }
        tok_starts.push(source.len() as u32);
        source.push_str(lexeme);
        tok_lens.push(lexeme.len() as u32);
        let raw_kind = if (100..200).contains(&kind) {
            TOK_IDENTIFIER
        } else {
            kind
        };
        raw_kinds.push(raw_kind);
    }

    let tok_types =
        reference_c_keyword_types(&raw_kinds, &tok_starts, &tok_lens, source.as_bytes());
    Fixture {
        source,
        tok_types,
        tok_starts,
        tok_lens,
    }
}

fn word_at(buf: &[u8], word: usize) -> u32 {
    let offset = word * 4;
    u32::from_le_bytes(buf[offset..offset + 4].try_into().unwrap())
}

fn node_count_from_vast(vast: &[u8]) -> u32 {
    u32::try_from(vast.len() / (VAST_STRIDE_U32 * 4)).unwrap_or_default()
}

fn row_kind(rows: &[u8], idx: usize) -> u32 {
    word_at(rows, idx * VAST_STRIDE_U32)
}

fn pg_word_at(pg: &[u8], idx: usize, field: usize) -> u32 {
    word_at(pg, idx * PG_STRIDE_U32 + field)
}

fn assert_lex_matches_fixture(fixture: &Fixture) {
    let kinds = lex_c11_max_munch_kinds(fixture.source.as_bytes())
        .expect("fixture source must be accepted by the C lexer");
    let filtered: Vec<u32> = kinds
        .into_iter()
        .filter(|kind| *kind != TOK_WHITESPACE && *kind != TOK_COMMENT)
        .collect();
    assert_eq!(
        filtered, fixture.tok_types,
        "fixture token rows must match max-munch lexer output"
    );
}

fn run_reference_pg_lower(typed_vast: &[u8]) -> Vec<u8> {
    let num_nodes = node_count_from_vast(typed_vast);
    let program = c_lower_ast_to_pg_nodes("vast_nodes", Expr::u32(num_nodes), "pg_nodes");
    let output_len = num_nodes.saturating_mul(PG_STRIDE_U32 as u32).max(1) as usize * 4;
    let values = [
        Value::from(typed_vast.to_vec()),
        Value::from(vec![0; output_len]),
    ];
    let outputs = vyre_reference::reference_eval(&program, &values)
        .unwrap_or_else(|error| panic!("Fix: C AST PG lowerer must execute on CPU: {error}"));
    assert_eq!(outputs.len(), 1, "Fix: PG lowerer must emit one buffer");
    outputs[0].to_bytes()
}

fn run_program_classifier(raw_vast: &[u8]) -> Vec<u8> {
    let num_nodes = node_count_from_vast(raw_vast);
    let program =
        c11_classify_vast_node_kinds("vast_nodes", Expr::u32(num_nodes), "out_typed_vast_nodes");
    let output_len = num_nodes.saturating_mul(VAST_STRIDE_U32 as u32).max(1) as usize * 4;
    let values = [
        Value::from(raw_vast.to_vec()),
        Value::from(vec![0; output_len]),
    ];
    let outputs = vyre_reference::reference_eval(&program, &values)
        .unwrap_or_else(|error| panic!("Fix: C AST classifier program must execute: {error}"));
    assert_eq!(outputs.len(), 1, "Fix: classifier must emit one buffer");
    outputs[0].to_bytes()
}

fn run_pipeline(fixture: Fixture) -> Rows {
    assert_lex_matches_fixture(&fixture);
    let raw_vast =
        reference_c11_build_vast_nodes(&fixture.tok_types, &fixture.tok_starts, &fixture.tok_lens);
    let typed_vast = reference_c11_classify_vast_node_kinds(&raw_vast);
    assert_eq!(
        run_program_classifier(&raw_vast),
        typed_vast,
        "executable C AST classifier must match the byte oracle"
    );
    let pg = run_reference_pg_lower(&typed_vast);
    assert_eq!(
        pg,
        reference_ast_to_pg_nodes(&typed_vast),
        "executable PG lowerer must match the byte oracle"
    );

    Rows {
        fixture,
        typed_vast,
        pg,
    }
}

fn rows_of_kind(rows: &[u8], kind: u32) -> Vec<usize> {
    rows.chunks_exact(VAST_STRIDE_U32 * 4)
        .enumerate()
        .filter_map(|(idx, row)| {
            let row_kind = u32::from_le_bytes(row[0..4].try_into().unwrap());
            (row_kind == kind).then_some(idx)
        })
        .collect()
}

fn row_for_nth_lexeme(fixture: &Fixture, needle: &str, nth: usize) -> usize {
    fixture
        .tok_starts
        .iter()
        .zip(&fixture.tok_lens)
        .enumerate()
        .filter_map(|(idx, (start, len))| {
            let start = *start as usize;
            let end = start.saturating_add(*len as usize);
            (fixture.source.as_bytes().get(start..end) == Some(needle.as_bytes())).then_some(idx)
        })
        .nth(nth)
        .unwrap_or_else(|| panic!("lexeme {needle:?} occurrence {nth} not found"))
}

fn assert_kind_and_span(rows: &Rows, idx: usize, kind: u32) {
    assert_eq!(
        row_kind(&rows.typed_vast, idx),
        kind,
        "VAST kind at row {idx}"
    );
    assert_eq!(pg_word_at(&rows.pg, idx, 0), kind, "PG kind at row {idx}");
    assert_eq!(
        word_at(&rows.typed_vast, idx * VAST_STRIDE_U32 + 5),
        rows.fixture.tok_starts[idx],
        "VAST span_start at row {idx}"
    );
    assert_eq!(
        word_at(&rows.typed_vast, idx * VAST_STRIDE_U32 + 6),
        rows.fixture.tok_lens[idx],
        "VAST span_len at row {idx}"
    );
    assert_eq!(
        pg_word_at(&rows.pg, idx, 1),
        rows.fixture.tok_starts[idx],
        "PG span_start at row {idx}"
    );
    assert_eq!(
        pg_word_at(&rows.pg, idx, 2),
        rows.fixture.tok_starts[idx] + rows.fixture.tok_lens[idx],
        "PG span_end at row {idx}"
    );
    assert_eq!(
        pg_word_at(&rows.pg, idx, 3),
        word_at(&rows.typed_vast, idx * VAST_STRIDE_U32 + 1),
        "PG parent link at row {idx}"
    );
    assert_eq!(
        pg_word_at(&rows.pg, idx, 4),
        word_at(&rows.typed_vast, idx * VAST_STRIDE_U32 + 2),
        "PG first_child link at row {idx}"
    );
    assert_eq!(
        pg_word_at(&rows.pg, idx, 5),
        word_at(&rows.typed_vast, idx * VAST_STRIDE_U32 + 3),
        "PG next_sibling link at row {idx}"
    );
}

fn assert_each_kind_has_span(rows: &Rows, kind: u32) {
    let indices = rows_of_kind(&rows.typed_vast, kind);
    assert!(
        !indices.is_empty(),
        "expected at least one row for kind {kind:#x}"
    );
    for idx in indices {
        assert_kind_and_span(rows, idx, kind);
    }
}

fn c99_for_init_fixture() -> Fixture {
    assemble(&[
        ("void", TOK_IDENTIFIER),
        ("f", TOK_IDENTIFIER),
        ("(", TOK_LPAREN),
        ("int", TOK_IDENTIFIER),
        ("n", TOK_IDENTIFIER),
        (")", TOK_RPAREN),
        ("{", TOK_LBRACE),
        ("\n", TOK_WHITESPACE),
        ("for", TOK_IDENTIFIER),
        ("(", TOK_LPAREN),
        ("int", TOK_IDENTIFIER),
        ("i", TOK_IDENTIFIER),
        ("=", TOK_ASSIGN),
        ("0", TOK_INTEGER),
        (";", TOK_SEMICOLON),
        ("i", TOK_IDENTIFIER),
        ("<", TOK_LT),
        ("n", TOK_IDENTIFIER),
        (";", TOK_SEMICOLON),
        ("i", TOK_IDENTIFIER),
        ("=", TOK_ASSIGN),
        ("i", TOK_IDENTIFIER),
        ("+", TOK_PLUS),
        ("1", TOK_INTEGER),
        (")", TOK_RPAREN),
        ("{", TOK_LBRACE),
        ("visit", TOK_IDENTIFIER),
        ("(", TOK_LPAREN),
        ("i", TOK_IDENTIFIER),
        (")", TOK_RPAREN),
        (";", TOK_SEMICOLON),
        ("}", TOK_RBRACE),
        ("}", TOK_RBRACE),
    ])
}

fn do_while_zero_fixture() -> Fixture {
    assemble(&[
        ("void", TOK_IDENTIFIER),
        ("g", TOK_IDENTIFIER),
        ("(", TOK_LPAREN),
        ("void", TOK_IDENTIFIER),
        (")", TOK_RPAREN),
        ("{", TOK_LBRACE),
        ("\n", TOK_WHITESPACE),
        ("do", TOK_IDENTIFIER),
        ("{", TOK_LBRACE),
        ("lock", TOK_IDENTIFIER),
        ("(", TOK_LPAREN),
        (")", TOK_RPAREN),
        (";", TOK_SEMICOLON),
        ("body", TOK_IDENTIFIER),
        ("(", TOK_LPAREN),
        (")", TOK_RPAREN),
        (";", TOK_SEMICOLON),
        ("}", TOK_RBRACE),
        ("while", TOK_IDENTIFIER),
        ("(", TOK_LPAREN),
        ("0", TOK_INTEGER),
        (")", TOK_RPAREN),
        (";", TOK_SEMICOLON),
        ("}", TOK_RBRACE),
    ])
}

#[test]
fn c99_for_init_declaration_preserves_statement_expression_and_span_rows() {
    let rows = run_pipeline(c99_for_init_fixture());

    let for_idx = row_for_nth_lexeme(&rows.fixture, "for", 0);
    let init_assign_idx = row_for_nth_lexeme(&rows.fixture, "=", 0);
    let increment_assign_idx = row_for_nth_lexeme(&rows.fixture, "=", 1);
    let zero_idx = row_for_nth_lexeme(&rows.fixture, "0", 0);
    let one_idx = row_for_nth_lexeme(&rows.fixture, "1", 0);
    let call_idx = row_for_nth_lexeme(&rows.fixture, "visit", 0);

    assert_kind_and_span(&rows, for_idx, C_AST_KIND_FOR_STMT);
    assert_kind_and_span(&rows, init_assign_idx, 0);
    assert_kind_and_span(&rows, increment_assign_idx, C_AST_KIND_ASSIGN_EXPR);
    assert_kind_and_span(&rows, zero_idx, node_kind::LITERAL);
    assert_kind_and_span(&rows, one_idx, node_kind::LITERAL);
    assert_kind_and_span(&rows, call_idx, node_kind::CALL);
}

#[test]
fn do_while_zero_macro_body_preserves_call_loop_literal_and_span_rows() {
    let rows = run_pipeline(do_while_zero_fixture());

    let do_idx = row_for_nth_lexeme(&rows.fixture, "do", 0);
    let while_idx = row_for_nth_lexeme(&rows.fixture, "while", 0);
    let lock_idx = row_for_nth_lexeme(&rows.fixture, "lock", 0);
    let body_idx = row_for_nth_lexeme(&rows.fixture, "body", 0);
    let zero_idx = row_for_nth_lexeme(&rows.fixture, "0", 0);

    assert_kind_and_span(&rows, do_idx, C_AST_KIND_DO_STMT);
    assert_kind_and_span(&rows, while_idx, C_AST_KIND_WHILE_STMT);
    assert_kind_and_span(&rows, lock_idx, node_kind::CALL);
    assert_kind_and_span(&rows, body_idx, node_kind::CALL);
    assert_kind_and_span(&rows, zero_idx, node_kind::LITERAL);
}

#[test]
fn focused_fixtures_cover_required_c_ast_kinds_with_valid_spans() {
    let for_rows = run_pipeline(c99_for_init_fixture());
    let do_rows = run_pipeline(do_while_zero_fixture());

    for kind in [
        C_AST_KIND_FOR_STMT,
        C_AST_KIND_ASSIGN_EXPR,
        node_kind::CALL,
        node_kind::LITERAL,
    ] {
        assert_each_kind_has_span(&for_rows, kind);
    }
    for kind in [
        C_AST_KIND_DO_STMT,
        C_AST_KIND_WHILE_STMT,
        node_kind::CALL,
        node_kind::LITERAL,
    ] {
        assert_each_kind_has_span(&do_rows, kind);
    }
}
