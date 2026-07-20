//! Reference-oracle contracts for the GPU function-extraction pass.
//!
//! Constructs under test:
//!   * bare function definitions (`int foo(void) {}`)
//!   * `__attribute__((...))` appearing between the return-type and the name
//!   * declarations without bodies must NOT be extracted
//!   * multiple specifiers and pointer return types

#![cfg(feature = "c-parser")]
#![allow(deprecated)]
mod common;

use common::decode_u32_words as words_from_bytes;
use common::u32_bytes as bytes;
use vyre::ir::BufferAccess;
use vyre_libs::parsing::c::lex::tokens::*;
use vyre_libs::parsing::c::parse::structure::c11_extract_functions;
use vyre_reference::value::Value;

fn run_extractor(tok_types: &[u32], paren_pairs: &[u32], brace_pairs: &[u32]) -> (Vec<u32>, u32) {
    let nt = tok_types.len() as u32;
    let program = c11_extract_functions(
        "tok_types",
        "paren_pairs",
        "brace_pairs",
        vyre::ir::Expr::u32(nt),
        "out_functions",
        "out_counts",
    );
    let fn_buf_size = (nt as usize).saturating_mul(3).max(3) * 4;
    let inputs = [
        Value::from(bytes(tok_types)),
        Value::from(bytes(paren_pairs)),
        Value::from(bytes(brace_pairs)),
        Value::from(vec![0u8; fn_buf_size]),
        Value::from(vec![0u8; 4]),
    ];
    let outputs = vyre_reference::reference_eval(&program, &inputs)
        .expect("c11_extract_functions must execute under the reference oracle");
    assert_eq!(outputs.len(), 2, "expected [out_functions, out_counts]");
    let functions = words_from_bytes(&outputs[0].to_bytes());
    let counts = words_from_bytes(&outputs[1].to_bytes());
    let slot_count = counts[0];
    (functions, slot_count)
}

#[test]
fn count_buffer_is_explicit_read_write_state_not_uninitialized_output() {
    let program = c11_extract_functions(
        "tok_types",
        "paren_pairs",
        "brace_pairs",
        vyre::ir::Expr::u32(8),
        "out_functions",
        "out_counts",
    );
    let counts = program
        .buffers
        .iter()
        .find(|buffer| buffer.name() == "out_counts")
        .expect("function extractor declares out_counts");
    assert_eq!(counts.access(), BufferAccess::ReadWrite);
    assert!(
        !counts.is_pipeline_live_out(),
        "out_counts must be caller-initialized read-write state; live-out-only output allocation can start from uninitialized device memory"
    );
}

/// Helper: decode function records from the SPARSE output array.
///
/// `c11_extract_functions` writes each function's 3-word record
/// `[name_idx, body_start, body_end]` at slot `name_idx * 3` and zero-initializes
/// every unoccupied slot; `out_counts[0]` is the array CAPACITY (`num_tokens*3`),
/// not a compacted match count (that is the `emit_atomic_record_append` variant,
/// which this extractor deliberately does not use to avoid GPU atomic contention).
/// So decoding means walking every slot and skipping the empty `(0,0,0)` ones.
/// A real record always has `name_idx >= 1` (a function name is never token 0 -
/// it needs a return-type prefix) and `body_end > 0`, so an all-zero tuple is
/// unambiguously an unoccupied slot, never a real function.
fn decode_functions(functions: &[u32], slot_count: u32) -> Vec<(u32, u32, u32)> {
    let n = (slot_count / 3) as usize;
    let mut out = Vec::new();
    for i in 0..n {
        let base = i * 3;
        let record = (functions[base], functions[base + 1], functions[base + 2]);
        if record != (0, 0, 0) {
            out.push(record);
        }
    }
    out
}

#[test]
fn bare_function_definition_extracted() {
    // int foo(void) { }
    let tok_types = vec![
        TOK_INT,
        TOK_IDENTIFIER,
        TOK_LPAREN,
        TOK_VOID,
        TOK_RPAREN,
        TOK_LBRACE,
        TOK_RBRACE,
    ];
    let paren_pairs = vec![
        u32::MAX, // int
        u32::MAX, // foo
        4,        // ( matches )
        u32::MAX, // void
        2,        // ) matches (
        u32::MAX, // {
        u32::MAX, // }
    ];
    let brace_pairs = vec![
        u32::MAX, // int
        u32::MAX, // foo
        u32::MAX, // (
        u32::MAX, // void
        u32::MAX, // )
        6,        // { matches }
        5,        // } matches {
    ];
    let (functions, slot_count) = run_extractor(&tok_types, &paren_pairs, &brace_pairs);
    let fns = decode_functions(&functions, slot_count);
    assert_eq!(fns.len(), 1, "expected exactly one function");
    assert_eq!(fns[0], (1, 5, 6), "foo at idx 1, body 5..6");
}

#[test]
fn declaration_without_body_not_extracted() {
    // int bar(void);
    let tok_types = vec![
        TOK_INT,
        TOK_IDENTIFIER,
        TOK_LPAREN,
        TOK_VOID,
        TOK_RPAREN,
        TOK_SEMICOLON,
    ];
    let paren_pairs = vec![
        u32::MAX, // int
        u32::MAX, // bar
        4,        // (
        u32::MAX, // void
        2,        // )
        u32::MAX, // ;
    ];
    let brace_pairs = vec![u32::MAX; 6];
    let (functions, slot_count) = run_extractor(&tok_types, &paren_pairs, &brace_pairs);
    let fns = decode_functions(&functions, slot_count);
    assert_eq!(
        fns.len(),
        0,
        "declaration without body must not be extracted"
    );
}

#[test]
fn attribute_between_return_type_and_name_extracted() {
    // void __attribute__((cold)) foo(void) { }
    let tok_types = vec![
        TOK_VOID,          // 0
        TOK_GNU_ATTRIBUTE, // 1
        TOK_LPAREN,        // 2
        TOK_LPAREN,        // 3
        TOK_IDENTIFIER,    // 4 (cold)
        TOK_RPAREN,        // 5
        TOK_RPAREN,        // 6
        TOK_IDENTIFIER,    // 7 (foo)
        TOK_LPAREN,        // 8
        TOK_VOID,          // 9
        TOK_RPAREN,        // 10
        TOK_LBRACE,        // 11
        TOK_RBRACE,        // 12
    ];
    let paren_pairs = vec![
        u32::MAX, // 0 void
        u32::MAX, // 1 __attribute__
        6,        // 2 ( matches 6
        5,        // 3 ( matches 5
        u32::MAX, // 4 cold
        3,        // 5 ) matches 3
        2,        // 6 ) matches 2
        u32::MAX, // 7 foo
        10,       // 8 ( matches 10
        u32::MAX, // 9 void
        8,        // 10 ) matches 8
        12,       // 11 { matches 12
        11,       // 12 } matches 11
    ];
    let brace_pairs = vec![
        u32::MAX, // 0
        u32::MAX, // 1
        u32::MAX, // 2
        u32::MAX, // 3
        u32::MAX, // 4
        u32::MAX, // 5
        u32::MAX, // 6
        u32::MAX, // 7
        u32::MAX, // 8
        u32::MAX, // 9
        u32::MAX, // 10
        12,       // 11
        11,       // 12
    ];
    let (functions, slot_count) = run_extractor(&tok_types, &paren_pairs, &brace_pairs);
    let fns = decode_functions(&functions, slot_count);
    assert_eq!(
        fns.len(),
        1,
        "expected exactly one function with attribute before name"
    );
    assert_eq!(fns[0], (7, 11, 12), "foo at idx 7, body 11..12");
}

#[test]
fn pointer_return_type_extracted() {
    // struct task_struct * get_regs(void) { }
    let tok_types = vec![
        TOK_STRUCT,
        TOK_IDENTIFIER, // task_struct
        TOK_STAR,
        TOK_IDENTIFIER, // get_regs
        TOK_LPAREN,
        TOK_VOID,
        TOK_RPAREN,
        TOK_LBRACE,
        TOK_RBRACE,
    ];
    let paren_pairs = vec![
        u32::MAX, // struct
        u32::MAX, // task_struct
        u32::MAX, // *
        u32::MAX, // get_regs
        6,        // (
        u32::MAX, // void
        4,        // )
        u32::MAX, // {
        u32::MAX, // }
    ];
    let brace_pairs = vec![
        u32::MAX, // struct
        u32::MAX, // task_struct
        u32::MAX, // *
        u32::MAX, // get_regs
        u32::MAX, // (
        u32::MAX, // void
        u32::MAX, // )
        8,        // {
        7,        // }
    ];
    let (functions, slot_count) = run_extractor(&tok_types, &paren_pairs, &brace_pairs);
    let fns = decode_functions(&functions, slot_count);
    assert_eq!(fns.len(), 1, "pointer return type must still extract");
    assert_eq!(fns[0], (3, 7, 8), "get_regs at idx 3, body 7..8");
}

#[test]
fn multiple_specifiers_extracted() {
    // static inline int foo(void) { }
    let tok_types = vec![
        TOK_STATIC,
        TOK_INLINE,
        TOK_INT,
        TOK_IDENTIFIER,
        TOK_LPAREN,
        TOK_VOID,
        TOK_RPAREN,
        TOK_LBRACE,
        TOK_RBRACE,
    ];
    let paren_pairs = vec![
        u32::MAX, // static
        u32::MAX, // inline
        u32::MAX, // int
        u32::MAX, // foo
        6,        // (
        u32::MAX, // void
        4,        // )
        u32::MAX, // {
        u32::MAX, // }
    ];
    let brace_pairs = vec![
        u32::MAX, // static
        u32::MAX, // inline
        u32::MAX, // int
        u32::MAX, // foo
        u32::MAX, // (
        u32::MAX, // void
        u32::MAX, // )
        8,        // {
        7,        // }
    ];
    let (functions, slot_count) = run_extractor(&tok_types, &paren_pairs, &brace_pairs);
    let fns = decode_functions(&functions, slot_count);
    assert_eq!(fns.len(), 1, "multiple specifiers must still extract");
    assert_eq!(fns[0], (3, 7, 8), "foo at idx 3, body 7..8");
}
