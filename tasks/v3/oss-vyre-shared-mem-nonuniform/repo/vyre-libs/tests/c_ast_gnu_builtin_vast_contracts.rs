//! VAST structural contracts for GNU builtin expressions.
//!
//! Constructs under test:
//!   * `__builtin_expect` in expression position
//!   * `__builtin_choose_expr` with constant selector
//!   * `__builtin_types_compatible_p` with two type arguments
//!   * `__builtin_constant_p` with value argument
//!   * real-header libc builtin variants that must not be rejected as unknown

#![cfg(feature = "c-parser")]
#![allow(deprecated)]

mod common;
use common::{decode_u32_words, u32_bytes};

use vyre_libs::parsing::c::lex::keyword::reference_c_keyword_types;
use vyre_libs::parsing::c::lex::tokens::*;
use vyre_libs::parsing::c::parse::vast::{
    reference_c11_annotate_typedef_names, reference_c11_build_vast_nodes,
    reference_c11_classify_vast_node_kinds, C_AST_KIND_BUILTIN_BPF_CORE_INTRIN_EXPR,
    C_AST_KIND_BUILTIN_CHOOSE_EXPR, C_AST_KIND_BUILTIN_CONSTANT_P_EXPR,
    C_AST_KIND_BUILTIN_EXPECT_EXPR, C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR,
    C_AST_KIND_BUILTIN_TYPES_COMPATIBLE_P_EXPR, C_AST_KIND_BUILTIN_UNREACHABLE_STMT,
};

const VAST_STRIDE_U32: usize = 10;

fn haystack_words(source: &[u8]) -> Vec<u32> {
    source.iter().map(|b| u32::from(*b)).collect()
}

fn lex_raw_source(source: &[u8]) -> (Vec<u32>, Vec<u32>, Vec<u32>) {
    use vyre_libs::parsing::c::lex::lexer::c11_lexer;
    use vyre_reference::value::Value;

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

fn parse_source(source: &str) -> Vec<u8> {
    let source_bytes = source.as_bytes();
    let (raw_types, raw_starts, raw_lens) = lex_raw_source(source_bytes);
    let tok_types = reference_c_keyword_types(&raw_types, &raw_starts, &raw_lens, source_bytes);
    let raw_vast = reference_c11_build_vast_nodes(&tok_types, &raw_starts, &raw_lens);
    let annotated = reference_c11_annotate_typedef_names(&raw_vast, source_bytes);
    reference_c11_classify_vast_node_kinds(&annotated)
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

#[test]
fn builtin_expect_classified_in_expression() {
    let vast = parse_source(r#"int x = __builtin_expect(a, 1);"#);
    let idxs = indices_with_kind(&vast, C_AST_KIND_BUILTIN_EXPECT_EXPR);
    assert_eq!(
        idxs.len(),
        1,
        "__builtin_expect must produce exactly one BUILTIN_EXPECT_EXPR node"
    );
}

#[test]
fn builtin_choose_expr_classified_in_expression() {
    let vast = parse_source(r#"int x = __builtin_choose_expr(1, 2, 3);"#);
    let idxs = indices_with_kind(&vast, C_AST_KIND_BUILTIN_CHOOSE_EXPR);
    assert_eq!(
        idxs.len(),
        1,
        "__builtin_choose_expr must produce exactly one BUILTIN_CHOOSE_EXPR node"
    );
}

#[test]
fn builtin_types_compatible_p_classified_in_expression() {
    let vast = parse_source(r#"int x = __builtin_types_compatible_p(int, long);"#);
    let idxs = indices_with_kind(&vast, C_AST_KIND_BUILTIN_TYPES_COMPATIBLE_P_EXPR);
    assert_eq!(
        idxs.len(),
        1,
        "__builtin_types_compatible_p must produce exactly one BUILTIN_TYPES_COMPATIBLE_P_EXPR node"
    );
}

#[test]
fn builtin_constant_p_classified_in_expression() {
    let vast = parse_source(r#"int x = __builtin_constant_p(42);"#);
    let idxs = indices_with_kind(&vast, C_AST_KIND_BUILTIN_CONSTANT_P_EXPR);
    assert_eq!(
        idxs.len(),
        1,
        "__builtin_constant_p must produce exactly one BUILTIN_CONSTANT_P_EXPR node"
    );
}

#[test]
fn builtin_unreachable_classified_in_statement() {
    let vast = parse_source(r#"void f(void) { __builtin_unreachable(); }"#);
    let idxs = indices_with_kind(&vast, C_AST_KIND_BUILTIN_UNREACHABLE_STMT);
    assert_eq!(
        idxs.len(),
        1,
        "__builtin_unreachable must produce exactly one BUILTIN_UNREACHABLE_STMT node"
    );
}

#[test]
fn real_header_libc_builtins_parse_without_unknown_builtin_rejection() {
    let vast = parse_source(
        r#"
        int f(char *p, char *q) {
            return __builtin_memchr(p, 'x', 8) != 0
                || __builtin_strnlen(q, 16) > 4
                || __builtin___memcpy_chk(p, q, 4, 8) != 0;
        }
        "#,
    );
    let idxs = indices_with_kind(&vast, C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR);
    assert!(
        idxs.len() >= 3,
        "memchr/strnlen/memcpy_chk must each classify as BUILTIN_LIBC_INTRIN_EXPR, got {}",
        idxs.len()
    );
}

#[test]
fn real_header_libm_builtins_parse_without_unknown_builtin_rejection() {
    let vast = parse_source(
        r#"
        double f(double x, double y) {
            return __builtin_sqrt(x)
                + __builtin_pow(x, y)
                + __builtin_fma(x, y, 1.0)
                + __builtin_remainder(x, y);
        }
        "#,
    );
    let idxs = indices_with_kind(&vast, C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR);
    assert_eq!(
        idxs.len(),
        4,
        "libm GNU builtins must classify as explicit libc intrinsic VAST rows"
    );
}

#[test]
fn bpf_core_builtin_preserves_distinct_vast_semantics() {
    let vast = parse_source(r#"int f(int *p) { return __builtin_preserve_access_index(*p); }"#);
    let idxs = indices_with_kind(&vast, C_AST_KIND_BUILTIN_BPF_CORE_INTRIN_EXPR);
    assert_eq!(
        idxs.len(),
        1,
        "BPF CO-RE builtin calls must not collapse into generic assumption intrinsics"
    );
}
