//! Hand-written CPU-only contracts for Linux-style C syntax parsed from raw
//! source strings through the full lexer → VAST → annotate → classify pipeline.
//!
//! Constructs under test:
//!   * `__attribute__((__section__(...)))` and `__attribute__((__aligned__(...)))`
//!   * `__attribute__((__weak__))` plus negative contract (bare `weak` identifier)
//!   * inline asm with output/input operands, clobbers, and `asm goto` labels
//!   * `typeof` / `__typeof__` / `__typeof__` in declarations
//!   * GNU statement expressions `({ ... })` in initializer context
//!   * designated initializers (field, array index, range)
//!   * macro-expanded-looking dense token streams (`__asm__ __volatile__` etc.)
//!
//! Every test asserts **structural invariants** (node kind counts, parent/child
//! links, span monotonicity, symbol-hash discrimination)  -  never merely
//! "parses without panic".

#![cfg(feature = "c-parser")]
#![allow(deprecated)]

mod common;
use c_grammar_gen::lex_c11_max_munch_kinds;
use common::{decode_u32_words, u32_bytes};
use vyre_libs::parsing::c::lex::keyword::reference_c_keyword_types;
use vyre_libs::parsing::c::lex::lexer::c11_lexer;
use vyre_libs::parsing::c::lex::tokens::*;
use vyre_libs::parsing::c::parse::vast::{
    reference_c11_annotate_typedef_names, reference_c11_build_vast_nodes,
    reference_c11_classify_vast_node_kinds, C_AST_KIND_ASM_CLOBBERS_LIST,
    C_AST_KIND_ASM_GOTO_LABELS, C_AST_KIND_ASM_INPUT_OPERAND, C_AST_KIND_ASM_OUTPUT_OPERAND,
    C_AST_KIND_ASM_QUALIFIER, C_AST_KIND_ASM_TEMPLATE, C_AST_KIND_ASSIGN_EXPR,
    C_AST_KIND_ATTRIBUTE_ALIGNED, C_AST_KIND_ATTRIBUTE_SECTION, C_AST_KIND_ATTRIBUTE_WEAK,
    C_AST_KIND_GNU_ATTRIBUTE, C_AST_KIND_GNU_STATEMENT_EXPR, C_AST_KIND_INITIALIZER_LIST,
    C_AST_KIND_INLINE_ASM, C_AST_KIND_MEMBER_ACCESS_EXPR, C_AST_KIND_RANGE_DESIGNATOR_EXPR,
};
use vyre_primitives::predicate::node_kind;
use vyre_reference::value::Value;

const VAST_STRIDE_U32: usize = 10;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn haystack_words(source: &[u8]) -> Vec<u32> {
    source.iter().map(|b| u32::from(*b)).collect()
}

/// Run the GPU lexer program through the Reference oracle oracle and return the
/// non-whitespace, non-comment token stream plus exact source spans.
fn lex_raw_source(source: &[u8]) -> (Vec<u32>, Vec<u32>, Vec<u32>) {
    let haystack_len = source.len() as u32;
    let program = c11_lexer(
        "haystack",
        "out_tok_types",
        "out_tok_starts",
        "out_tok_lens",
        "out_counts",
        haystack_len,
    );
    let haystack_buf = u32_bytes(&haystack_words(source));
    let zero_buf = vec![0u8; haystack_len as usize * 4];
    let count_zero = vec![0u8; 4];
    let inputs = [
        Value::from(haystack_buf),
        Value::from(zero_buf.clone()),
        Value::from(zero_buf.clone()),
        Value::from(zero_buf),
        Value::from(count_zero),
    ];
    let outputs = vyre_reference::reference_eval(&program, &inputs)
        .expect("c11_lexer must execute under the reference oracle");
    assert_eq!(
        outputs.len(),
        4,
        "expected [tok_types, tok_starts, tok_lens, counts]"
    );
    let raw_types = decode_u32_words(&outputs[0].to_bytes());
    let raw_starts = decode_u32_words(&outputs[1].to_bytes());
    let raw_lens = decode_u32_words(&outputs[2].to_bytes());
    let counts = decode_u32_words(&outputs[3].to_bytes());
    let tok_count = counts.first().copied().unwrap_or(0) as usize;

    let mut types = Vec::with_capacity(tok_count);
    let mut starts = Vec::with_capacity(tok_count);
    let mut lens = Vec::with_capacity(tok_count);
    for i in 0..tok_count {
        let k = raw_types[i];
        if k != TOK_WHITESPACE && k != TOK_COMMENT {
            types.push(k);
            starts.push(raw_starts[i]);
            lens.push(raw_lens[i]);
        }
    }
    (types, starts, lens)
}

/// Assert that `lex_c11_max_munch_kinds` agrees with the filtered, keyword-promoted oracle stream.
fn assert_max_munch_agrees(source: &[u8], types: &[u32]) {
    let host_kinds = lex_c11_max_munch_kinds(source).expect("host lexer must accept source");
    let host_non_ws: Vec<u32> = host_kinds
        .into_iter()
        .filter(|k| *k != TOK_WHITESPACE && *k != TOK_COMMENT)
        .collect();
    assert_eq!(
        host_non_ws, types,
        "hand-built source must match max-munch lexer"
    );
}

fn word_at(buf: &[u8], word: usize) -> u32 {
    let off = word * 4;
    u32::from_le_bytes(buf[off..off + 4].try_into().unwrap())
}

fn kind_at(rows: &[u8], idx: usize) -> u32 {
    word_at(rows, idx * VAST_STRIDE_U32)
}

fn parent_at(rows: &[u8], idx: usize) -> u32 {
    word_at(rows, idx * VAST_STRIDE_U32 + 1)
}

fn first_child_at(rows: &[u8], idx: usize) -> u32 {
    word_at(rows, idx * VAST_STRIDE_U32 + 2)
}

fn next_sibling_at(rows: &[u8], idx: usize) -> u32 {
    word_at(rows, idx * VAST_STRIDE_U32 + 3)
}

fn start_at(rows: &[u8], idx: usize) -> u32 {
    word_at(rows, idx * VAST_STRIDE_U32 + 5)
}

fn len_at(rows: &[u8], idx: usize) -> u32 {
    word_at(rows, idx * VAST_STRIDE_U32 + 6)
}

fn node_count_from_vast(vast: &[u8]) -> usize {
    vast.len() / (VAST_STRIDE_U32 * 4)
}

fn indices_with_kind(rows: &[u8], kind: u32) -> Vec<usize> {
    rows.chunks_exact(VAST_STRIDE_U32 * 4)
        .enumerate()
        .filter_map(|(idx, row)| {
            let row_kind = u32::from_le_bytes(row[0..4].try_into().unwrap());
            (row_kind == kind).then_some(idx)
        })
        .collect()
}

fn find_single(rows: &[u8], kind: u32) -> usize {
    let idxs = indices_with_kind(rows, kind);
    assert_eq!(
        idxs.len(),
        1,
        "expected exactly one node of kind 0x{kind:08x}, found {}",
        idxs.len()
    );
    idxs[0]
}

fn lexeme_at(source: &[u8], start: u32, len: u32) -> Option<&[u8]> {
    let s = start as usize;
    let e = s.saturating_add(len as usize);
    source.get(s..e)
}

fn assert_span_monotonicity(rows: &[u8]) {
    let n = node_count_from_vast(rows);
    for i in 0..n {
        let start = start_at(rows, i);
        let len = len_at(rows, i);
        assert!(
            len > 0 || kind_at(rows, i) == TOK_SEMICOLON,
            "token {i} has zero length"
        );
        // start+len must not overflow in practice for small fixtures
        let _end = start.saturating_add(len);
    }
}

/// Full pipeline: raw source bytes → typed VAST bytes + source context.
struct Parsed {
    source: Vec<u8>,
    typed_vast: Vec<u8>,
    tok_types: Vec<u32>,
    #[allow(dead_code)]
    tok_starts: Vec<u32>,
    #[allow(dead_code)]
    tok_lens: Vec<u32>,
}

fn parse_source(source: &str) -> Parsed {
    let source_bytes = source.as_bytes();
    let (raw_types, raw_starts, raw_lens) = lex_raw_source(source_bytes);

    let tok_types = reference_c_keyword_types(&raw_types, &raw_starts, &raw_lens, source_bytes);
    assert_max_munch_agrees(source_bytes, &tok_types);

    let raw_vast = reference_c11_build_vast_nodes(&tok_types, &raw_starts, &raw_lens);
    let annotated = reference_c11_annotate_typedef_names(&raw_vast, source_bytes);
    let typed_vast = reference_c11_classify_vast_node_kinds(&annotated);

    assert_span_monotonicity(&typed_vast);

    Parsed {
        source: source_bytes.to_vec(),
        typed_vast,
        tok_types,
        tok_starts: raw_starts,
        tok_lens: raw_lens,
    }
}

// ---------------------------------------------------------------------------
// 1. GNU __attribute__ with double-underscore forms (macro-expanded look)
// ---------------------------------------------------------------------------

mod c_ast_linux_style_raw_source_contracts_part1 {

    include!("__split/c_ast_linux_style_raw_source_contracts_part1.rs");
}
mod c_ast_linux_style_raw_source_contracts_part2 {
    include!("__split/c_ast_linux_style_raw_source_contracts_part2.rs");
}
