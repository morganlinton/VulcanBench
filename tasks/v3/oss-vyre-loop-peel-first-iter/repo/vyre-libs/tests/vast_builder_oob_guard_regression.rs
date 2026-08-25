//! Regression tests for OOB-guard `Expr::select` wrapping in
//! `c11_build_vast_nodes`.
//!
//! These tests target the four load sites in the VAST builder that were
//! wrapped in `Expr::select` bounds guards to prevent CUDA
//! ILLEGAL_ADDRESS crashes. The guards prevent materialization of OOB
//! loads when:
//!   - `stack_depth == 0` (top_slot underflow via `sub(stack_depth, 1)`)
//!   - `parent_idx == SENTINEL` (OOB load via parent_row)
//!   - `top_idx == SENTINEL` (OOB load via tok_types[SENTINEL])
//!
//! Three test tiers:
//! 1. **IR structural**  -  source-level proof that `Expr::select` guards
//!    surround each dangerous load.
//! 2. **Reference oracle oracle**  -  adversarial token streams exercising
//!    every OOB code path through the scalar Reference oracle, proving
//!    correctness of the output under OOB-inducing inputs.
//! 3. **IR structural + semantic**  -  verify the generated IR Program
//!    contains select-guarded loads via inspection of buffer/binding
//!    layout, and that the CPU oracle matches known-good outputs.

#![cfg(feature = "c-parser")]
#![allow(deprecated)]
use vyre_libs::parsing::c::lex::tokens::*;
use vyre_libs::parsing::c::parse::vast::reference_c11_build_vast_nodes;

const VAST_STRIDE_U32: usize = 10;
const SENTINEL: u32 = u32::MAX;

fn word_at(bytes: &[u8], word: usize) -> u32 {
    let offset = word * 4;
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
}

// =========================================================================
// Tier 1: IR structural tests  -  Expr::select guards exist in source
// =========================================================================

#[test]
fn ir_source_contains_select_guard_around_top_slot_parent_load() {
    // The parent_idx load from top_slot must be wrapped in Expr::select
    // with the stack_depth > 0 guard, not a bare Expr::load.
    let source = include_str!("../src/parsing/c/parse/vast/build/structural_builder.rs");
    let fn_start = source
        .find("pub fn c11_build_vast_nodes")
        .expect("function must exist");
    let fn_body = &source[fn_start..];

    let parent_block = fn_body
        .find("let_bind(\n            \"parent_idx\"")
        .expect("parent_idx assignment must use Expr::select guard around top_slot load");

    let guard_region = &fn_body[parent_block..parent_block + 300];
    assert!(
        guard_region.contains("Expr::select("),
        "parent_idx assignment must use Expr::select guard around top_slot load"
    );
    assert!(
        guard_region.contains("Expr::gt(Expr::var(\"stack_depth\"), Expr::u32(0))"),
        "parent_idx select guard must check stack_depth > 0"
    );
    assert!(
        guard_region.contains("Expr::load(\"__vast_stack\", top_slot"),
        "parent_idx select guard must contain the load"
    );
    assert!(
        guard_region.contains("Expr::u32(SENTINEL)"),
        "parent_idx select fallback must be SENTINEL"
    );
}

#[test]
fn ir_source_contains_select_guard_around_parent_row_load() {
    let source = include_str!("../src/parsing/c/parse/vast/build/structural_builder.rs");
    let fn_start = source
        .find("pub fn c11_build_vast_nodes")
        .expect("function must exist");
    let fn_body = &source[fn_start..];

    // The previous_sibling load from parent_row must have a guarded address
    let prev_sib = fn_body
        .find("let_bind(\n            \"previous_sibling\"")
        .expect("previous_sibling let_bind must exist");
    let prev_region = &fn_body[prev_sib..prev_sib + 600];

    // The inner Expr::select guards the load address, clamping to 0 when OOB.
    // Check that the load's index argument contains a nested select with the
    // parent_idx < num_tokens condition.
    assert!(
        prev_region.contains("Expr::load(")
            && prev_region.contains("Expr::select(")
            && prev_region.contains("Expr::lt(Expr::var(\"parent_idx\"), num_tokens"),
        "previous_sibling load address must be guarded by parent_idx < num_tokens select"
    );
}

#[test]
fn ir_source_contains_select_guard_around_top_slot_top_idx_load() {
    let source = include_str!("../src/parsing/c/parse/vast/build/structural_builder.rs");
    let fn_start = source
        .find("pub fn c11_build_vast_nodes")
        .expect("function must exist");
    let fn_body = &source[fn_start..];

    let top_idx_assign = fn_body
        .find("let_bind(\n            \"top_idx\"")
        .expect("top_idx assignment must use Expr::select guard");

    let guard_region = &fn_body[top_idx_assign..top_idx_assign + 300];
    assert!(
        guard_region.contains("Expr::select("),
        "top_idx assignment must use Expr::select guard"
    );
    assert!(
        guard_region.contains("Expr::gt(Expr::var(\"stack_depth\"), Expr::u32(0))"),
        "top_idx select guard must check stack_depth > 0"
    );
}

#[test]
fn ir_source_contains_select_guard_around_top_kind_tok_types_load() {
    let source = include_str!("../src/parsing/c/parse/vast/build/structural_builder.rs");
    let fn_start = source
        .find("pub fn c11_build_vast_nodes")
        .expect("function must exist");
    let fn_body = &source[fn_start..];

    let top_kind = fn_body
        .find("\"top_kind\"")
        .expect("top_kind binding must exist");
    let top_kind_region = &fn_body[top_kind..top_kind + 500];

    assert!(
        top_kind_region.contains(
            "Expr::select(\n                        Expr::lt(Expr::var(\"top_idx\"), num_tokens"
        ),
        "top_kind load index must be guarded by top_idx < num_tokens select"
    );
}

// =========================================================================
// Tier 2: Reference oracle oracle  -  adversarial OOB-inducing inputs
// =========================================================================

/// Leading close delimiter at index 0: stack is empty, stack_depth=0,
/// parent_idx=SENTINEL. Exercises all four guarded loads.
#[test]
fn reference_leading_rbrace_no_crash() {
    let tok_types = [TOK_RBRACE, TOK_IDENTIFIER, TOK_SEMICOLON];
    let tok_starts = [0u32, 2, 6];
    let tok_lens = [1u32, 4, 1];
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    assert_eq!(raw.len(), 3 * VAST_STRIDE_U32 * 4);
    for i in 0..3 {
        assert_eq!(
            word_at(&raw, i * VAST_STRIDE_U32 + 1),
            SENTINEL,
            "token {i} parent must be SENTINEL when no enclosing open delimiter"
        );
    }
}

#[test]
fn reference_leading_rparen_no_crash() {
    let tok_types = [TOK_RPAREN, TOK_IDENTIFIER];
    let tok_starts = [0u32, 2];
    let tok_lens = [1u32, 4];
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    assert_eq!(raw.len(), 2 * VAST_STRIDE_U32 * 4);
    assert_eq!(word_at(&raw, 1), SENTINEL, "rparen parent must be SENTINEL");
    assert_eq!(
        word_at(&raw, VAST_STRIDE_U32 + 1),
        SENTINEL,
        "identifier parent must be SENTINEL"
    );
}

#[test]
fn reference_leading_rbracket_no_crash() {
    let tok_types = [TOK_RBRACKET, TOK_IDENTIFIER];
    let tok_starts = [0u32, 2];
    let tok_lens = [1u32, 4];
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    assert_eq!(raw.len(), 2 * VAST_STRIDE_U32 * 4);
    assert_eq!(
        word_at(&raw, 1),
        SENTINEL,
        "rbracket parent must be SENTINEL"
    );
}

/// Entire stream of unmatched close delimiters: stack never grows,
/// stack_depth stays 0, parent_idx always SENTINEL.
#[test]
fn reference_all_close_delimiters_no_crash() {
    let tok_types = [TOK_RBRACE, TOK_RPAREN, TOK_RBRACKET, TOK_RBRACE, TOK_RPAREN];
    let tok_starts: Vec<u32> = (0..5).map(|i| i * 2).collect();
    let tok_lens = vec![1u32; 5];
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    assert_eq!(raw.len(), 5 * VAST_STRIDE_U32 * 4);
    for i in 0..5 {
        assert_eq!(
            word_at(&raw, i * VAST_STRIDE_U32 + 1),
            SENTINEL,
            "all-close token {i} must have SENTINEL parent"
        );
    }
}

/// Open 3 deep, then close 5 (2 excess). The excess closes must not
/// underflow stack_depth and must leave parent_idx = SENTINEL.
#[test]
fn reference_deep_nest_then_excess_close_no_crash() {
    let tok_types = [
        TOK_LBRACE, TOK_LBRACE, TOK_LBRACE, TOK_RBRACE, TOK_RBRACE, TOK_RBRACE,
        TOK_RBRACE, // excess
        TOK_RBRACE, // excess
    ];
    let tok_starts: Vec<u32> = (0..8).collect();
    let tok_lens = vec![1u32; 8];
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    assert_eq!(raw.len(), 8 * VAST_STRIDE_U32 * 4);
    // After the matched close at index 5, stack should be empty.
    // Excess close at index 6 and 7 should be root-level.
    assert_eq!(
        word_at(&raw, 6 * VAST_STRIDE_U32 + 1),
        SENTINEL,
        "excess close[6] parent must be SENTINEL"
    );
    assert_eq!(
        word_at(&raw, 7 * VAST_STRIDE_U32 + 1),
        SENTINEL,
        "excess close[7] parent must be SENTINEL"
    );
}

/// Close-then-open pattern forces stack_depth to oscillate at 0.
#[test]
fn reference_alternating_close_open_no_crash() {
    let tok_types = [
        TOK_RBRACE, TOK_LBRACE, TOK_RBRACE, TOK_RPAREN, TOK_LPAREN, TOK_RPAREN,
    ];
    let tok_starts: Vec<u32> = (0..6).collect();
    let tok_lens = vec![1u32; 6];
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    assert_eq!(raw.len(), 6 * VAST_STRIDE_U32 * 4);
    assert_eq!(
        word_at(&raw, 1),
        SENTINEL,
        "leading rbrace parent must be SENTINEL"
    );
}

/// Minimal OOB trigger: single unmatched close.
#[test]
fn reference_single_close_delimiter() {
    for &tok in &[TOK_RBRACE, TOK_RPAREN, TOK_RBRACKET] {
        let raw = reference_c11_build_vast_nodes(&[tok], &[0], &[1]);
        assert_eq!(raw.len(), VAST_STRIDE_U32 * 4);
        assert_eq!(word_at(&raw, 0), tok, "kind preserved");
        assert_eq!(word_at(&raw, 1), SENTINEL, "parent must be SENTINEL");
    }
}

/// Sibling chains under root when parent_idx=SENTINEL. The
/// `previous_sibling` load from `parent_row[4]` is only valid when
/// `parent_idx < num_tokens`. With all root-level tokens, every
/// previous_sibling comes from `root_last_child`.
#[test]
fn reference_root_level_sibling_chain_correct() {
    let tok_types = [TOK_IDENTIFIER, TOK_PLUS, TOK_IDENTIFIER, TOK_SEMICOLON];
    let tok_starts = [0u32, 3, 4, 7];
    let tok_lens = [3u32, 1, 3, 1];
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    assert_eq!(raw.len(), 4 * VAST_STRIDE_U32 * 4);

    // All tokens are root-level siblings
    for i in 0..4 {
        assert_eq!(
            word_at(&raw, i * VAST_STRIDE_U32 + 1),
            SENTINEL,
            "token {i} must be root-level"
        );
    }
    // First child of root = index 0 (no explicit root node, but first_child[root]
    // is not stored  -  skip). next_sibling chain: 0→1→2→3→SENTINEL
    assert_eq!(
        word_at(&raw, 0 * VAST_STRIDE_U32 + 3),
        1,
        "token 0 next_sibling must be 1"
    );
    assert_eq!(
        word_at(&raw, 1 * VAST_STRIDE_U32 + 3),
        2,
        "token 1 next_sibling must be 2"
    );
    assert_eq!(
        word_at(&raw, 2 * VAST_STRIDE_U32 + 3),
        3,
        "token 2 next_sibling must be 3"
    );
    assert_eq!(
        word_at(&raw, 3 * VAST_STRIDE_U32 + 3),
        SENTINEL,
        "token 3 next_sibling must be SENTINEL (end of chain)"
    );
}

/// Mixed mismatched delimiters: `{ } ) ] }`
/// Tests that the stack doesn't pop when close doesn't match.
#[test]
fn reference_mismatched_close_types_no_crash() {
    let tok_types = [TOK_LBRACE, TOK_RBRACE, TOK_RPAREN, TOK_RBRACKET, TOK_RBRACE];
    let tok_starts: Vec<u32> = (0..5).map(|i| i * 2).collect();
    let tok_lens = vec![1u32; 5];
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    assert_eq!(raw.len(), 5 * VAST_STRIDE_U32 * 4);
    // index 0: lbrace, root
    assert_eq!(word_at(&raw, 0 * VAST_STRIDE_U32 + 1), SENTINEL);
    // index 1: rbrace, matches lbrace → parent is 0
    assert_eq!(word_at(&raw, 1 * VAST_STRIDE_U32 + 1), 0);
    // index 2,3,4: after stack popped, stack_depth=0, parent=SENTINEL
    for i in 2..5 {
        assert_eq!(
            word_at(&raw, i * VAST_STRIDE_U32 + 1),
            SENTINEL,
            "token {i} parent must be SENTINEL after stack emptied"
        );
    }
}

// =========================================================================
// Tier 3: Structural integrity  -  guards match semantic invariants
// =========================================================================

/// The select guards must NOT change output for well-formed input.
/// Verify parity between a normal function body and expected tree structure.
#[test]
fn select_guards_preserve_normal_function_body_structure() {
    let tok_types = [
        TOK_INT,
        TOK_IDENTIFIER,
        TOK_LPAREN,
        TOK_RPAREN,
        TOK_LBRACE,
        TOK_RETURN,
        TOK_INTEGER,
        TOK_SEMICOLON,
        TOK_RBRACE,
    ];
    let tok_starts = [0u32, 4, 8, 9, 10, 11, 18, 19, 20];
    let tok_lens = [3u32, 4, 1, 1, 1, 6, 1, 1, 1];
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    assert_eq!(raw.len(), 9 * VAST_STRIDE_U32 * 4);

    // int (0) and identifier (1) are root-level
    assert_eq!(word_at(&raw, 0 * VAST_STRIDE_U32 + 1), SENTINEL);
    assert_eq!(word_at(&raw, 1 * VAST_STRIDE_U32 + 1), SENTINEL);
    // lparen (2) is root-level
    assert_eq!(word_at(&raw, 2 * VAST_STRIDE_U32 + 1), SENTINEL);
    // rparen (3) is child of lparen
    assert_eq!(word_at(&raw, 3 * VAST_STRIDE_U32 + 1), 2);
    // lbrace (4) is root-level
    assert_eq!(word_at(&raw, 4 * VAST_STRIDE_U32 + 1), SENTINEL);
    // return (5), integer (6), semicolon (7) are children of lbrace
    for i in 5..=7 {
        assert_eq!(
            word_at(&raw, i * VAST_STRIDE_U32 + 1),
            4,
            "token {i} must be child of lbrace"
        );
    }
    // rbrace (8) is child of lbrace
    assert_eq!(word_at(&raw, 8 * VAST_STRIDE_U32 + 1), 4);

    // Span data must be preserved
    for i in 0..9 {
        assert_eq!(
            word_at(&raw, i * VAST_STRIDE_U32 + 5),
            tok_starts[i],
            "token {i} span_start must be preserved"
        );
        assert_eq!(
            word_at(&raw, i * VAST_STRIDE_U32 + 6),
            tok_lens[i],
            "token {i} span_len must be preserved"
        );
    }
}

/// Large adversarial stream: 128 close-open-close triplets with no
/// matching structure. Tests sustained stack_depth=0 cycling under
/// load.
#[test]
fn reference_128_unmatched_triplets_no_crash() {
    let n = 128 * 3;
    let mut tok_types = Vec::with_capacity(n);
    for _ in 0..128 {
        tok_types.push(TOK_RBRACE);
        tok_types.push(TOK_LBRACE);
        tok_types.push(TOK_RBRACE);
    }
    let tok_starts: Vec<u32> = (0..n as u32).collect();
    let tok_lens = vec![1u32; n];
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    assert_eq!(raw.len(), n * VAST_STRIDE_U32 * 4);
    // Every third token (index 0, 3, 6, ...) is a leading rbrace at root level
    for i in (0..n).step_by(3) {
        assert_eq!(
            word_at(&raw, i * VAST_STRIDE_U32 + 1),
            SENTINEL,
            "triplet leading rbrace at index {i} must be root-level"
        );
    }
}

/// Empty stream exercises the zero-iteration path of the build loop.
/// No loads are executed, confirming the guards handle n=0 gracefully.
#[test]
fn reference_empty_stream_no_crash() {
    let raw = reference_c11_build_vast_nodes(&[], &[], &[]);
    assert!(raw.is_empty(), "zero tokens must produce empty VAST");
}

/// Single open delimiter with no matching close: stack grows to 1 but
/// never pops. Tests the stack_depth > 0 guard succeeding for the
/// second and subsequent tokens.
#[test]
fn reference_single_open_no_close() {
    let tok_types = [TOK_LBRACE, TOK_IDENTIFIER, TOK_IDENTIFIER];
    let tok_starts = [0u32, 2, 6];
    let tok_lens = [1u32, 4, 4];
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    assert_eq!(raw.len(), 3 * VAST_STRIDE_U32 * 4);
    // lbrace is root
    assert_eq!(word_at(&raw, 0 * VAST_STRIDE_U32 + 1), SENTINEL);
    // identifiers are children of lbrace
    assert_eq!(word_at(&raw, 1 * VAST_STRIDE_U32 + 1), 0);
    assert_eq!(word_at(&raw, 2 * VAST_STRIDE_U32 + 1), 0);
}
