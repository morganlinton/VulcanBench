// C parser contract tests for GNU inline asm with output operands, input
// operands, memory/cc clobbers, goto labels, and symbolic operand names  - 
// constructs likely to break VAST/PG lowering.
//
// Constructs under test:
//   - asm with multiple output operands
//   - asm with multiple input operands
//   - asm with memory and cc clobbers
//   - asm goto with multiple destination labels
//   - asm with earlyclobber and symbolic names
//   - PG lowering preservation and GPU/CPU parity
//
// A missing GPU adapter is a configuration failure; tests do not skip.

// cfg(feature = "c-parser")  -  moved to parent

#[path = "../c_ast_gpu_parity_support/mod.rs"]
mod c_ast_gpu_parity_support;

use c_ast_gpu_parity_support::{
    assert_full_pipeline_parity, build_fixture, row_indices, run_gpu_pg_lower, word_at, Fixture,
    FixtureToken, VAST_STRIDE_U32,
};
use vyre_libs::parsing::c::lex::tokens::*;
use vyre_libs::parsing::c::lower::reference_ast_to_pg_nodes;
use vyre_libs::parsing::c::parse::vast::{
    reference_c11_annotate_typedef_names, reference_c11_build_vast_nodes,
    reference_c11_classify_vast_node_kinds, C_AST_KIND_ASM_CLOBBERS_LIST,
    C_AST_KIND_ASM_GOTO_LABELS, C_AST_KIND_ASM_INPUT_OPERAND, C_AST_KIND_ASM_OUTPUT_OPERAND,
    C_AST_KIND_ASM_TEMPLATE, C_AST_KIND_INLINE_ASM,
};

const PG_STRIDE_U32: usize = 6;

fn classify(fix: &Fixture) -> Vec<u8> {
    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let annotated = reference_c11_annotate_typedef_names(&raw, fix.source.as_bytes());
    reference_c11_classify_vast_node_kinds(&annotated)
}

fn pg_word_at(buf: &[u8], idx: usize, field: usize) -> u32 {
    word_at(buf, idx * PG_STRIDE_U32 + field)
}

fn assert_pg_preserves_row(
    typed_vast: &[u8],
    pg: &[u8],
    fix: &Fixture,
    idx: usize,
    expected_kind: u32,
) {
    assert_eq!(
        pg_word_at(pg, idx, 0),
        expected_kind,
        "PG kind mismatch at row {idx}"
    );
    assert_eq!(
        pg_word_at(pg, idx, 1),
        fix.tok_starts[idx],
        "PG span_start mismatch at row {idx}"
    );
    assert_eq!(
        pg_word_at(pg, idx, 2),
        fix.tok_starts[idx] + fix.tok_lens[idx],
        "PG span_end mismatch at row {idx}"
    );
    assert_eq!(
        pg_word_at(pg, idx, 3),
        word_at(typed_vast, idx * VAST_STRIDE_U32 + 1),
        "PG parent mismatch at row {idx}"
    );
    assert_eq!(
        pg_word_at(pg, idx, 4),
        word_at(typed_vast, idx * VAST_STRIDE_U32 + 2),
        "PG first_child mismatch at row {idx}"
    );
    assert_eq!(
        pg_word_at(pg, idx, 5),
        word_at(typed_vast, idx * VAST_STRIDE_U32 + 3),
        "PG next_sibling mismatch at row {idx}"
    );
}

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

/// ```c
/// asm volatile ("mov %2, %0\n\tadd %1, %0"
///   : "=r" (out0), "=r" (out1)
///   : "r" (in0), "r" (in1));
/// ```
fn fixture_asm_multiple_output_input_operands() -> Fixture {
    build_fixture(&[
        FixtureToken::new("asm", TOK_GNU_ASM),
        FixtureToken::new("volatile", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("\"mov %2, %0\\n\\tadd %1, %0\"", TOK_STRING),
        FixtureToken::new(":", TOK_COLON),
        FixtureToken::new("\"=r\"", TOK_STRING),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("out0", TOK_IDENTIFIER),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new(",", TOK_COMMA),
        FixtureToken::new("\"=r\"", TOK_STRING),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("out1", TOK_IDENTIFIER),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new(":", TOK_COLON),
        FixtureToken::new("\"r\"", TOK_STRING),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("in0", TOK_IDENTIFIER),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new(",", TOK_COMMA),
        FixtureToken::new("\"r\"", TOK_STRING),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("in1", TOK_IDENTIFIER),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new(";", TOK_SEMICOLON),
    ])
}

/// ```c
/// asm ("" : : : "memory", "cc");
/// ```
fn fixture_asm_memory_and_cc_clobbers() -> Fixture {
    build_fixture(&[
        FixtureToken::new("asm", TOK_GNU_ASM),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("\"\"", TOK_STRING),
        FixtureToken::new(":", TOK_COLON),
        FixtureToken::new(":", TOK_COLON),
        FixtureToken::new(":", TOK_COLON),
        FixtureToken::new("\"memory\"", TOK_STRING),
        FixtureToken::new(",", TOK_COMMA),
        FixtureToken::new("\"cc\"", TOK_STRING),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new(";", TOK_SEMICOLON),
    ])
}

/// ```c
/// asm goto ("jmp %l0\n\tjmp %l1"
///   :
///   :
///   :
///   : fail, ok);
/// ```
fn fixture_asm_goto_multiple_labels() -> Fixture {
    build_fixture(&[
        FixtureToken::new("asm", TOK_GNU_ASM),
        FixtureToken::new("goto", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("\"jmp %l0\\n\\tjmp %l1\"", TOK_STRING),
        FixtureToken::new(":", TOK_COLON),
        FixtureToken::new(":", TOK_COLON),
        FixtureToken::new(":", TOK_COLON),
        FixtureToken::new(":", TOK_COLON),
        FixtureToken::new("fail", TOK_IDENTIFIER),
        FixtureToken::new(",", TOK_COMMA),
        FixtureToken::new("ok", TOK_IDENTIFIER),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new(";", TOK_SEMICOLON),
    ])
}

/// ```c
/// asm ("mov %[src], %[dst]"
///   : [dst] "=&r" (out)
///   : [src] "r" (in));
/// ```
fn fixture_asm_symbolic_names_and_earlyclobber() -> Fixture {
    build_fixture(&[
        FixtureToken::new("asm", TOK_GNU_ASM),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("\"mov %[src], %[dst]\"", TOK_STRING),
        FixtureToken::new(":", TOK_COLON),
        FixtureToken::new("[dst]", TOK_IDENTIFIER),
        FixtureToken::new("\"=&r\"", TOK_STRING),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("out", TOK_IDENTIFIER),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new(":", TOK_COLON),
        FixtureToken::new("[src]", TOK_IDENTIFIER),
        FixtureToken::new("\"r\"", TOK_STRING),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("in", TOK_IDENTIFIER),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new(";", TOK_SEMICOLON),
    ])
}

/// ```c
/// __asm__ __volatile__ ("rdtsc" : "=A" (ticks));
/// ```
fn fixture_asm_extended_output_only() -> Fixture {
    build_fixture(&[
        FixtureToken::new("__asm__", TOK_GNU_ASM),
        FixtureToken::new("__volatile__", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("\"rdtsc\"", TOK_STRING),
        FixtureToken::new(":", TOK_COLON),
        FixtureToken::new("\"=A\"", TOK_STRING),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("ticks", TOK_IDENTIFIER),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new(";", TOK_SEMICOLON),
    ])
}

/// ```c
/// asm goto ("" :::: label1, label2, label3);
/// ```
fn fixture_asm_goto_three_labels() -> Fixture {
    build_fixture(&[
        FixtureToken::new("asm", TOK_GNU_ASM),
        FixtureToken::new("goto", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("\"\"", TOK_STRING),
        FixtureToken::new(":", TOK_COLON),
        FixtureToken::new(":", TOK_COLON),
        FixtureToken::new(":", TOK_COLON),
        FixtureToken::new(":", TOK_COLON),
        FixtureToken::new("label1", TOK_IDENTIFIER),
        FixtureToken::new(",", TOK_COMMA),
        FixtureToken::new("label2", TOK_IDENTIFIER),
        FixtureToken::new(",", TOK_COMMA),
        FixtureToken::new("label3", TOK_IDENTIFIER),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new(";", TOK_SEMICOLON),
    ])
}

// ---------------------------------------------------------------------------
// CPU reference contracts
// ---------------------------------------------------------------------------

#[test]
fn cpu_asm_multiple_output_input_operands_classifies() {
    let fix = fixture_asm_multiple_output_input_operands();
    let typed = classify(&fix);
    assert_eq!(
        row_indices(&typed, C_AST_KIND_INLINE_ASM),
        vec![0],
        "asm must classify as INLINE_ASM"
    );
    assert_eq!(
        row_indices(&typed, C_AST_KIND_ASM_TEMPLATE),
        vec![3],
        "template must classify"
    );
    let outputs = row_indices(&typed, C_AST_KIND_ASM_OUTPUT_OPERAND);
    assert_eq!(
        outputs.len(),
        2,
        "two output operands must classify, got {outputs:?}"
    );
    let inputs = row_indices(&typed, C_AST_KIND_ASM_INPUT_OPERAND);
    assert_eq!(
        inputs.len(),
        2,
        "two input operands must classify, got {inputs:?}"
    );
}

#[test]
fn cpu_asm_memory_and_cc_clobbers_classifies() {
    let fix = fixture_asm_memory_and_cc_clobbers();
    let typed = classify(&fix);
    assert_eq!(
        row_indices(&typed, C_AST_KIND_INLINE_ASM),
        vec![0],
        "asm must classify as INLINE_ASM"
    );
    let clobbers = row_indices(&typed, C_AST_KIND_ASM_CLOBBERS_LIST);
    assert_eq!(
        clobbers.len(),
        2,
        "memory and cc clobbers must classify, got {clobbers:?}"
    );
}

#[test]
fn cpu_asm_goto_multiple_labels_classifies() {
    let fix = fixture_asm_goto_multiple_labels();
    let typed = classify(&fix);
    assert_eq!(
        row_indices(&typed, C_AST_KIND_INLINE_ASM),
        vec![0],
        "asm goto must classify as INLINE_ASM"
    );
    let labels = row_indices(&typed, C_AST_KIND_ASM_GOTO_LABELS);
    assert_eq!(
        labels.len(),
        2,
        "fail and ok must classify as ASM_GOTO_LABELS, got {labels:?}"
    );
}

#[test]
fn cpu_asm_symbolic_names_and_earlyclobber_classifies() {
    let fix = fixture_asm_symbolic_names_and_earlyclobber();
    let typed = classify(&fix);
    assert_eq!(
        row_indices(&typed, C_AST_KIND_INLINE_ASM),
        vec![0],
        "asm must classify as INLINE_ASM"
    );
    assert_eq!(
        row_indices(&typed, C_AST_KIND_ASM_OUTPUT_OPERAND),
        vec![6],
        "output operand with earlyclobber must classify"
    );
    assert_eq!(
        row_indices(&typed, C_AST_KIND_ASM_INPUT_OPERAND),
        vec![12],
        "input operand with symbolic name must classify"
    );
}

#[test]
fn cpu_asm_extended_output_only_classifies() {
    let fix = fixture_asm_extended_output_only();
    let typed = classify(&fix);
    assert_eq!(
        row_indices(&typed, C_AST_KIND_INLINE_ASM),
        vec![0],
        "__asm__ must classify as INLINE_ASM"
    );
    assert_eq!(
        row_indices(&typed, C_AST_KIND_ASM_OUTPUT_OPERAND),
        vec![6],
        "output-only operand must classify"
    );
}

#[test]
fn cpu_asm_goto_three_labels_classifies() {
    let fix = fixture_asm_goto_three_labels();
    let typed = classify(&fix);
    let labels = row_indices(&typed, C_AST_KIND_ASM_GOTO_LABELS);
    assert_eq!(
        labels.len(),
        3,
        "three goto labels must classify, got {labels:?}"
    );
}

// ---------------------------------------------------------------------------
// PG lowering preservation contracts
// ---------------------------------------------------------------------------

#[test]
fn pg_lower_preserves_asm_multiple_output_input_operands() {
    let fix = fixture_asm_multiple_output_input_operands();
    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let annotated = reference_c11_annotate_typedef_names(&raw, fix.source.as_bytes());
    let typed = reference_c11_classify_vast_node_kinds(&annotated);
    let pg = reference_ast_to_pg_nodes(&typed);

    assert_pg_preserves_row(&typed, &pg, &fix, 0, C_AST_KIND_INLINE_ASM);
    for idx in row_indices(&typed, C_AST_KIND_ASM_OUTPUT_OPERAND) {
        assert_pg_preserves_row(&typed, &pg, &fix, idx, C_AST_KIND_ASM_OUTPUT_OPERAND);
    }
    for idx in row_indices(&typed, C_AST_KIND_ASM_INPUT_OPERAND) {
        assert_pg_preserves_row(&typed, &pg, &fix, idx, C_AST_KIND_ASM_INPUT_OPERAND);
    }
}

#[test]
fn pg_lower_preserves_asm_memory_and_cc_clobbers() {
    let fix = fixture_asm_memory_and_cc_clobbers();
    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let annotated = reference_c11_annotate_typedef_names(&raw, fix.source.as_bytes());
    let typed = reference_c11_classify_vast_node_kinds(&annotated);
    let pg = reference_ast_to_pg_nodes(&typed);

    assert_pg_preserves_row(&typed, &pg, &fix, 0, C_AST_KIND_INLINE_ASM);
    for idx in row_indices(&typed, C_AST_KIND_ASM_CLOBBERS_LIST) {
        assert_pg_preserves_row(&typed, &pg, &fix, idx, C_AST_KIND_ASM_CLOBBERS_LIST);
    }
}

#[test]
fn pg_lower_preserves_asm_goto_multiple_labels() {
    let fix = fixture_asm_goto_multiple_labels();
    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let annotated = reference_c11_annotate_typedef_names(&raw, fix.source.as_bytes());
    let typed = reference_c11_classify_vast_node_kinds(&annotated);
    let pg = reference_ast_to_pg_nodes(&typed);

    assert_pg_preserves_row(&typed, &pg, &fix, 0, C_AST_KIND_INLINE_ASM);
    for idx in row_indices(&typed, C_AST_KIND_ASM_GOTO_LABELS) {
        assert_pg_preserves_row(&typed, &pg, &fix, idx, C_AST_KIND_ASM_GOTO_LABELS);
    }
}
