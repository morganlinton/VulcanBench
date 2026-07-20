//! Preprocessing-token streams must survive lex → VAST → classification →
//! expression-shape → PG lowering **without** macro expansion: directive rows stay
//! `TOK_PREPROC` in raw VAST, `__LINE__` / `__FILE__` stay ordinary identifiers, and
//! macro-shaped calls stay `CALL` sites.

#![cfg(feature = "c-parser")]
#![allow(deprecated)]
#[path = "c_ast_gpu_parity_support/mod.rs"]
mod c_ast_gpu_parity_support;

use c_grammar_gen::lex_c11_max_munch_kinds;
use vyre::ir::Expr;
use vyre_libs::parsing::c::lex::keyword::reference_c_keyword_types;
use vyre_libs::parsing::c::lex::tokens::*;
use vyre_libs::parsing::c::lower::{c_lower_ast_to_pg_nodes, reference_ast_to_pg_nodes};
use vyre_libs::parsing::c::parse::vast::{
    reference_c11_build_expression_shape_nodes, reference_c11_build_vast_nodes,
    reference_c11_classify_vast_node_kinds, C_EXPR_ASSOC_NONE, C_EXPR_SHAPE_NONE,
    C_EXPR_SHAPE_STRIDE_U32,
};
use vyre_primitives::predicate::node_kind;
use vyre_reference::value::Value;

use c_ast_gpu_parity_support::{
    run_gpu_classifier, run_gpu_expr_shape, run_gpu_pg_lower, word_at, VAST_STRIDE_U32,
};

const PG_STRIDE_U32: usize = 6;

struct Assembled {
    source: String,
    raw_kinds: Vec<u32>,
    tok_types: Vec<u32>,
    tok_starts: Vec<u32>,
    tok_lens: Vec<u32>,
}

fn assemble(lexemes: &[(&str, u32)]) -> Assembled {
    let mut source = String::new();
    let mut tok_starts = Vec::new();
    let mut tok_lens = Vec::new();
    let mut raw_kinds = Vec::new();

    for (lex, kind) in lexemes {
        if *kind == TOK_WHITESPACE || *kind == TOK_COMMENT {
            source.push_str(lex);
            continue;
        }
        if !source.is_empty() && !source.ends_with('\n') {
            source.push(' ');
        }
        tok_starts.push(source.len() as u32);
        source.push_str(lex);
        tok_lens.push(lex.len() as u32);
        raw_kinds.push(*kind);
    }

    let tok_types =
        reference_c_keyword_types(&raw_kinds, &tok_starts, &tok_lens, source.as_bytes());

    Assembled {
        source,
        raw_kinds,
        tok_types,
        tok_starts,
        tok_lens,
    }
}

fn assert_lex_matches_non_ws(assembled: &Assembled) {
    let kinds = lex_c11_max_munch_kinds(assembled.source.as_bytes()).expect("lex fixture source");
    let filtered: Vec<u32> = kinds
        .into_iter()
        .filter(|k| *k != TOK_WHITESPACE && *k != TOK_COMMENT)
        .collect();
    assert_eq!(
        filtered, assembled.raw_kinds,
        "hand-built fixture must match max-munch lexer (no fake tokenization)"
    );
}

fn node_count_from_vast(vast: &[u8]) -> u32 {
    u32::try_from(vast.len() / (VAST_STRIDE_U32 * 4)).unwrap_or_default()
}

fn row_typed_kind(typed_vast: &[u8], idx: usize) -> u32 {
    word_at(typed_vast, idx * VAST_STRIDE_U32)
}

fn find_row_for_lexeme(assembled: &Assembled, needle: &str) -> usize {
    assembled
        .tok_starts
        .iter()
        .zip(&assembled.tok_lens)
        .position(|(start, len)| {
            let s = *start as usize;
            let e = s.saturating_add(*len as usize);
            assembled.source.as_bytes().get(s..e) == Some(needle.as_bytes())
        })
        .unwrap_or_else(|| panic!("lexeme {needle:?} not found in fixture"))
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
        .unwrap_or_else(|e| panic!("C AST PG lowerer must execute on CPU: {e}"));
    assert_eq!(outputs.len(), 1);
    outputs[0].to_bytes()
}

struct PipelineOut {
    raw_vast: Vec<u8>,
    typed_vast: Vec<u8>,
    expr_shape: Vec<u8>,
    pg: Vec<u8>,
}

fn run_cpu_pipeline(assembled: &Assembled) -> PipelineOut {
    assert_lex_matches_non_ws(assembled);
    let raw_vast = reference_c11_build_vast_nodes(
        &assembled.tok_types,
        &assembled.tok_starts,
        &assembled.tok_lens,
    );
    let typed_vast = reference_c11_classify_vast_node_kinds(&raw_vast);
    let expr_shape = reference_c11_build_expression_shape_nodes(&raw_vast, &typed_vast);
    let pg = run_reference_pg_lower(&typed_vast);
    assert_eq!(
        pg,
        reference_ast_to_pg_nodes(&typed_vast),
        "executable PG lowerer must match byte oracle"
    );
    PipelineOut {
        raw_vast,
        typed_vast,
        expr_shape,
        pg,
    }
}

fn assert_pg_row(assembled: &Assembled, pg: &[u8], typed: &[u8], idx: usize, kind: u32) {
    assert_eq!(row_typed_kind(typed, idx), kind, "typed kind at {idx}");
    assert_eq!(word_at(pg, idx * PG_STRIDE_U32), kind, "PG kind at {idx}");
    assert_eq!(
        word_at(pg, idx * PG_STRIDE_U32 + 1),
        assembled.tok_starts[idx],
        "PG span_start at {idx}"
    );
    assert_eq!(
        word_at(pg, idx * PG_STRIDE_U32 + 2),
        assembled.tok_starts[idx] + assembled.tok_lens[idx],
        "PG span_end at {idx}"
    );
}

fn assert_shape_none(expr_shape: &[u8], idx: usize) {
    let base = idx * C_EXPR_SHAPE_STRIDE_U32 as usize;
    assert_eq!(
        word_at(expr_shape, base),
        C_EXPR_SHAPE_NONE,
        "preproc/structural rows stay shape-none"
    );
    assert_eq!(word_at(expr_shape, base + 4), C_EXPR_ASSOC_NONE);
}

#[test]
fn preprocessor_directive_rows_keep_preproc_raw_kind_and_survive_pg() {
    let a = assemble(&[
        ("#ifndef FOO", TOK_PREPROC),
        ("\n", TOK_WHITESPACE),
        ("#define FOO 1", TOK_PREPROC),
        ("\n", TOK_WHITESPACE),
        ("int", TOK_INT),
        ("x", TOK_IDENTIFIER),
        (";", TOK_SEMICOLON),
    ]);
    let out = run_cpu_pipeline(&a);
    for idx in [0usize, 1] {
        assert_eq!(
            word_at(&out.raw_vast, idx * VAST_STRIDE_U32),
            TOK_PREPROC,
            "raw VAST must preserve TOK_PREPROC (no expansion)"
        );
        assert_eq!(row_typed_kind(&out.typed_vast, idx), 0);
        assert_pg_row(&a, &out.pg, &out.typed_vast, idx, 0);
        assert_shape_none(&out.expr_shape, idx);
    }
    assert_eq!(
        word_at(&out.raw_vast, 2 * VAST_STRIDE_U32),
        TOK_INT,
        "`int` must stay keyword-promoted in raw VAST"
    );
    assert_eq!(
        row_typed_kind(&out.typed_vast, 2),
        0,
        "type-keyword rows stay unclassified (kind 0) in typed VAST"
    );
    assert_pg_row(&a, &out.pg, &out.typed_vast, 2, 0);
    assert_pg_row(&a, &out.pg, &out.typed_vast, 3, node_kind::VARIABLE);
}

#[test]
fn conditional_directive_token_rows_survive_without_expansion() {
    let a = assemble(&[
        ("#if 0", TOK_PREPROC),
        ("\n", TOK_WHITESPACE),
        ("#elif 1", TOK_PREPROC),
        ("\n", TOK_WHITESPACE),
        ("#else", TOK_PREPROC),
        ("\n", TOK_WHITESPACE),
        ("#endif", TOK_PREPROC),
        ("\n", TOK_WHITESPACE),
        ("int", TOK_INT),
        ("q", TOK_IDENTIFIER),
        (";", TOK_SEMICOLON),
    ]);
    let out = run_cpu_pipeline(&a);
    for idx in 0..4 {
        assert_eq!(
            word_at(&out.raw_vast, idx * VAST_STRIDE_U32),
            TOK_PREPROC,
            "conditional directive row {idx}"
        );
        assert_eq!(row_typed_kind(&out.typed_vast, idx), 0);
        assert_pg_row(&a, &out.pg, &out.typed_vast, idx, 0);
        assert_shape_none(&out.expr_shape, idx);
    }
}

#[test]
fn macro_shaped_call_survives_as_call_without_expansion() {
    // Split declaration from assignment so `SUM(` cannot inherit a declaration
    // prefix from `int y =` (which classifies the identifier as FUNCTION_DECL).
    let a = assemble(&[
        ("int", TOK_INT),
        ("y", TOK_IDENTIFIER),
        (";", TOK_SEMICOLON),
        ("y", TOK_IDENTIFIER),
        ("=", TOK_ASSIGN),
        ("SUM", TOK_IDENTIFIER),
        ("(", TOK_LPAREN),
        ("1", TOK_INTEGER),
        (",", TOK_COMMA),
        ("2", TOK_INTEGER),
        (")", TOK_RPAREN),
        (";", TOK_SEMICOLON),
    ]);
    let out = run_cpu_pipeline(&a);
    let sum_idx = find_row_for_lexeme(&a, "SUM");
    assert_eq!(row_typed_kind(&out.typed_vast, sum_idx), node_kind::CALL);
    assert_pg_row(&a, &out.pg, &out.typed_vast, sum_idx, node_kind::CALL);
}

#[test]
fn line_and_file_spellings_remain_identifier_variables() {
    let a = assemble(&[
        ("int", TOK_INT),
        ("ln", TOK_IDENTIFIER),
        ("=", TOK_ASSIGN),
        ("__LINE__", TOK_IDENTIFIER),
        (";", TOK_SEMICOLON),
        ("const", TOK_CONST),
        ("char", TOK_CHAR_KW),
        ("*", TOK_STAR),
        ("fp", TOK_IDENTIFIER),
        ("=", TOK_ASSIGN),
        ("__FILE__", TOK_IDENTIFIER),
        (";", TOK_SEMICOLON),
    ]);
    let out = run_cpu_pipeline(&a);
    let line_idx = find_row_for_lexeme(&a, "__LINE__");
    let file_idx = find_row_for_lexeme(&a, "__FILE__");
    let ls = a.tok_starts[line_idx] as usize;
    let fs = a.tok_starts[file_idx] as usize;
    assert_eq!(&a.source.as_bytes()[ls..ls + 8], b"__LINE__");
    assert_eq!(&a.source.as_bytes()[fs..fs + 8], b"__FILE__");
    assert_eq!(
        row_typed_kind(&out.typed_vast, line_idx),
        node_kind::VARIABLE
    );
    assert_eq!(
        row_typed_kind(&out.typed_vast, file_idx),
        node_kind::VARIABLE
    );
    assert_pg_row(&a, &out.pg, &out.typed_vast, line_idx, node_kind::VARIABLE);
    assert_pg_row(&a, &out.pg, &out.typed_vast, file_idx, node_kind::VARIABLE);
}

#[test]
fn macro_statement_call_inside_compound_survives_as_call() {
    let a = assemble(&[
        ("void", TOK_VOID),
        ("f", TOK_IDENTIFIER),
        ("(", TOK_LPAREN),
        ("void", TOK_VOID),
        (")", TOK_RPAREN),
        ("{", TOK_LBRACE),
        ("\n", TOK_WHITESPACE),
        ("LOCK", TOK_IDENTIFIER),
        ("(", TOK_LPAREN),
        (")", TOK_RPAREN),
        (";", TOK_SEMICOLON),
        ("\n", TOK_WHITESPACE),
        ("}", TOK_RBRACE),
    ]);
    let out = run_cpu_pipeline(&a);
    let lock_idx = find_row_for_lexeme(&a, "LOCK");
    assert_eq!(row_typed_kind(&out.typed_vast, lock_idx), node_kind::CALL);
    assert_pg_row(&a, &out.pg, &out.typed_vast, lock_idx, node_kind::CALL);
}

#[test]
fn gpu_matches_cpu_for_classify_expr_shape_and_pg_on_preproc_stream() {
    let a = assemble(&[
        ("#define M(x) x", TOK_PREPROC),
        ("\n", TOK_WHITESPACE),
        ("int", TOK_INT),
        ("z", TOK_IDENTIFIER),
        (";", TOK_SEMICOLON),
        ("z", TOK_IDENTIFIER),
        ("=", TOK_ASSIGN),
        ("M", TOK_IDENTIFIER),
        ("(", TOK_LPAREN),
        ("42", TOK_INTEGER),
        (")", TOK_RPAREN),
        (";", TOK_SEMICOLON),
    ]);
    let out = run_cpu_pipeline(&a);
    let gpu_typed = run_gpu_classifier(&out.raw_vast);
    assert_eq!(gpu_typed, out.typed_vast, "GPU classifier must match CPU");

    assert_eq!(
        run_gpu_expr_shape(&out.raw_vast, &out.typed_vast),
        out.expr_shape,
        "GPU expression-shape must match CPU"
    );
    assert_eq!(
        run_gpu_pg_lower(&out.typed_vast),
        out.pg,
        "GPU PG lowering must match CPU"
    );

    let m_idx = find_row_for_lexeme(&a, "M");
    assert_eq!(row_typed_kind(&out.typed_vast, m_idx), node_kind::CALL);
    assert_eq!(
        word_at(&out.raw_vast, m_idx * VAST_STRIDE_U32),
        TOK_IDENTIFIER
    );
    assert_eq!(
        word_at(&out.raw_vast, 0),
        TOK_PREPROC,
        "directive row stays TOK_PREPROC in raw VAST"
    );
}
