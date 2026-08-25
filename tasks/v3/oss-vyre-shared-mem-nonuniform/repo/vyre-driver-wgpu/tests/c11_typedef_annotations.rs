//! Typedef-name identity tests for the C VAST semantic annotation pass.

#![cfg(feature = "c-parser")]
#![allow(deprecated)]
use std::sync::OnceLock;

use vyre::ir::Expr;
use vyre::VyreBackend;
use vyre_driver_wgpu::WgpuBackend;
use vyre_libs::parsing::c::lex::tokens::*;
use vyre_libs::parsing::c::parse::vast::{
    c11_annotate_typedef_names, c11_classify_vast_node_kinds, reference_c11_annotate_typedef_names,
    reference_c11_build_vast_nodes, reference_c11_classify_vast_node_kinds, C_AST_KIND_CAST_EXPR,
    C_AST_KIND_POINTER_DECL,
};

const VAST_STRIDE_U32: usize = 10;
const TYPEDEF_FLAGS_FIELD: usize = 7;
const TYPEDEF_FLAG_VISIBLE: u32 = 1;
const TYPEDEF_FLAG_DECL: u32 = 1 << 1;
const ORDINARY_FLAG_DECL: u32 = 1 << 2;

#[derive(Clone)]
enum Atom {
    Tok(u32),
    Ident(&'static str),
}

struct Fixture {
    tok_types: Vec<u32>,
    tok_starts: Vec<u32>,
    tok_lens: Vec<u32>,
    haystack: Vec<u8>,
}

fn tok(token: u32) -> Atom {
    Atom::Tok(token)
}

fn ident(name: &'static str) -> Atom {
    Atom::Ident(name)
}

fn fixture(atoms: &[Atom]) -> Fixture {
    let mut tok_types = Vec::with_capacity(atoms.len());
    let mut tok_starts = Vec::with_capacity(atoms.len());
    let mut tok_lens = Vec::with_capacity(atoms.len());
    let mut haystack = Vec::new();
    let mut cursor = 0u32;

    for atom in atoms {
        match atom {
            Atom::Tok(token) => {
                tok_types.push(*token);
                tok_starts.push(0);
                tok_lens.push(0);
            }
            Atom::Ident(name) => {
                tok_types.push(TOK_IDENTIFIER);
                tok_starts.push(cursor);
                tok_lens.push(name.len() as u32);
                haystack.extend_from_slice(name.as_bytes());
                cursor += name.len() as u32;
            }
        }
    }

    Fixture {
        tok_types,
        tok_starts,
        tok_lens,
        haystack,
    }
}

fn haystack_words(bytes: &[u8]) -> Vec<u8> {
    vyre_primitives::wire::pack_bytes_as_u32_slice(bytes)
}

fn word_at(buf: &[u8], word: usize) -> u32 {
    let offset = word * 4;
    u32::from_le_bytes(buf[offset..offset + 4].try_into().unwrap())
}

fn assert_words_eq(actual: &[u8], expected: &[u8]) {
    if actual != expected {
        let actual_words = actual.len() / 4;
        let expected_words = expected.len() / 4;
        let limit = actual_words.min(expected_words);
        for word in 0..limit {
            let actual_word = word_at(actual, word);
            let expected_word = word_at(expected, word);
            if actual_word != expected_word {
                panic!(
                    "word {word} differs: actual={actual_word}, expected={expected_word}, row={}, field={}",
                    word / VAST_STRIDE_U32,
                    word % VAST_STRIDE_U32
                );
            }
        }
        panic!(
            "byte lengths differ: actual={}, expected={}",
            actual.len(),
            expected.len()
        );
    }
}

fn kind_at(rows: &[u8], idx: usize) -> u32 {
    word_at(rows, idx * VAST_STRIDE_U32)
}

fn flags_at(rows: &[u8], idx: usize) -> u32 {
    word_at(rows, idx * VAST_STRIDE_U32 + TYPEDEF_FLAGS_FIELD)
}

fn gpu_backend() -> &'static WgpuBackend {
    static BACKEND: OnceLock<WgpuBackend> = OnceLock::new();
    BACKEND.get_or_init(|| {
        WgpuBackend::acquire().expect(
            "WgpuBackend::acquire failed on a machine that must have a GPU. \
             This is a configuration bug, not a graceful skip.",
        )
    })
}

fn raw_vast(fix: &Fixture) -> Vec<u8> {
    reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens)
}

fn annotate_cpu(fix: &Fixture) -> Vec<u8> {
    reference_c11_annotate_typedef_names(&raw_vast(fix), &fix.haystack)
}

fn classify_cpu_annotated(fix: &Fixture) -> Vec<u8> {
    reference_c11_classify_vast_node_kinds(&annotate_cpu(fix))
}

fn annotate_gpu(fix: &Fixture) -> Vec<u8> {
    let raw = raw_vast(fix);
    let haystack = haystack_words(&fix.haystack);
    let program = c11_annotate_typedef_names(
        "vast_nodes",
        "haystack",
        Expr::u32(fix.haystack.len() as u32),
        Expr::u32(fix.tok_types.len() as u32),
        "annotated_vast",
    );
    let inputs: Vec<&[u8]> = vec![&raw, &haystack];
    let outputs = gpu_backend()
        .dispatch_borrowed(&program, &inputs, &Default::default())
        .expect("GPU typedef annotation dispatch must succeed");
    assert_eq!(outputs.len(), 1);
    outputs[0].clone()
}

fn classify_gpu(annotated: &[u8], node_count: usize) -> Vec<u8> {
    let program = c11_classify_vast_node_kinds(
        "vast_nodes",
        Expr::u32(node_count as u32),
        "typed_vast_nodes",
    );
    let inputs: Vec<&[u8]> = vec![annotated];
    let outputs = gpu_backend()
        .dispatch_borrowed(&program, &inputs, &Default::default())
        .expect("GPU typedef-aware classifier dispatch must succeed");
    assert_eq!(outputs.len(), 1);
    outputs[0].clone()
}

#[test]
fn cpu_typedef_name_and_expression_identifier_are_distinct() {
    let fix = fixture(&[
        tok(TOK_TYPEDEF),
        tok(TOK_INT),
        ident("T"),
        tok(TOK_SEMICOLON),
        tok(TOK_VOID),
        ident("f"),
        tok(TOK_LPAREN),
        tok(TOK_VOID),
        tok(TOK_RPAREN),
        tok(TOK_LBRACE),
        ident("T"),
        tok(TOK_STAR),
        ident("p"),
        tok(TOK_SEMICOLON),
        tok(TOK_LPAREN),
        ident("T"),
        tok(TOK_RPAREN),
        tok(TOK_STAR),
        ident("p"),
        tok(TOK_SEMICOLON),
        tok(TOK_INT),
        ident("x"),
        tok(TOK_SEMICOLON),
        tok(TOK_LPAREN),
        ident("x"),
        tok(TOK_RPAREN),
        tok(TOK_STAR),
        ident("p"),
        tok(TOK_SEMICOLON),
        tok(TOK_RBRACE),
    ]);
    let annotated = annotate_cpu(&fix);
    let typed = classify_cpu_annotated(&fix);

    assert_ne!(flags_at(&annotated, 2) & TYPEDEF_FLAG_DECL, 0);
    assert_ne!(flags_at(&annotated, 10) & TYPEDEF_FLAG_VISIBLE, 0);
    assert_eq!(kind_at(&typed, 11), C_AST_KIND_POINTER_DECL);
    assert_eq!(kind_at(&typed, 14), C_AST_KIND_CAST_EXPR);
    assert_eq!(
        kind_at(&typed, 23),
        0,
        "ordinary expression identifier `(x)` must not become a cast"
    );
    assert_ne!(flags_at(&annotated, 21) & ORDINARY_FLAG_DECL, 0);
    assert_eq!(flags_at(&annotated, 24) & TYPEDEF_FLAG_VISIBLE, 0);
}

#[test]
fn cpu_inner_ordinary_declaration_shadows_typedef_until_scope_exit() {
    let fix = fixture(&[
        tok(TOK_TYPEDEF),
        tok(TOK_INT),
        ident("T"),
        tok(TOK_SEMICOLON),
        tok(TOK_VOID),
        ident("f"),
        tok(TOK_LPAREN),
        tok(TOK_VOID),
        tok(TOK_RPAREN),
        tok(TOK_LBRACE),
        tok(TOK_LBRACE),
        tok(TOK_INT),
        ident("T"),
        tok(TOK_SEMICOLON),
        tok(TOK_LPAREN),
        ident("T"),
        tok(TOK_RPAREN),
        tok(TOK_STAR),
        ident("p"),
        tok(TOK_SEMICOLON),
        tok(TOK_RBRACE),
        tok(TOK_LPAREN),
        ident("T"),
        tok(TOK_RPAREN),
        tok(TOK_STAR),
        ident("p"),
        tok(TOK_SEMICOLON),
        tok(TOK_RBRACE),
    ]);
    let annotated = annotate_cpu(&fix);
    let typed = classify_cpu_annotated(&fix);

    assert_ne!(flags_at(&annotated, 12) & ORDINARY_FLAG_DECL, 0);
    assert_eq!(flags_at(&annotated, 15) & TYPEDEF_FLAG_VISIBLE, 0);
    assert_eq!(kind_at(&typed, 14), 0);
    assert_ne!(flags_at(&annotated, 22) & TYPEDEF_FLAG_VISIBLE, 0);
    assert_eq!(kind_at(&typed, 21), C_AST_KIND_CAST_EXPR);
}

#[test]
fn cpu_struct_and_enum_tags_do_not_shadow_typedef_namespace() {
    let fix = fixture(&[
        tok(TOK_TYPEDEF),
        tok(TOK_INT),
        ident("S"),
        tok(TOK_SEMICOLON),
        tok(TOK_STRUCT),
        ident("S"),
        tok(TOK_LBRACE),
        tok(TOK_INT),
        ident("field"),
        tok(TOK_SEMICOLON),
        tok(TOK_RBRACE),
        tok(TOK_SEMICOLON),
        tok(TOK_ENUM),
        ident("S"),
        tok(TOK_LBRACE),
        ident("A"),
        tok(TOK_RBRACE),
        tok(TOK_SEMICOLON),
        tok(TOK_VOID),
        ident("f"),
        tok(TOK_LPAREN),
        tok(TOK_VOID),
        tok(TOK_RPAREN),
        tok(TOK_LBRACE),
        tok(TOK_LPAREN),
        ident("S"),
        tok(TOK_RPAREN),
        tok(TOK_STAR),
        ident("p"),
        tok(TOK_SEMICOLON),
        tok(TOK_RBRACE),
    ]);
    let annotated = annotate_cpu(&fix);
    let typed = classify_cpu_annotated(&fix);

    assert_eq!(flags_at(&annotated, 5) & ORDINARY_FLAG_DECL, 0);
    assert_eq!(flags_at(&annotated, 13) & ORDINARY_FLAG_DECL, 0);
    assert_ne!(flags_at(&annotated, 25) & TYPEDEF_FLAG_VISIBLE, 0);
    assert_eq!(kind_at(&typed, 24), C_AST_KIND_CAST_EXPR);
}

#[test]
fn gpu_annotation_and_classifier_match_cpu_for_shadowing() {
    let fix = fixture(&[
        tok(TOK_TYPEDEF),
        tok(TOK_INT),
        ident("T"),
        tok(TOK_SEMICOLON),
        tok(TOK_VOID),
        ident("f"),
        tok(TOK_LPAREN),
        tok(TOK_VOID),
        tok(TOK_RPAREN),
        tok(TOK_LBRACE),
        tok(TOK_LBRACE),
        tok(TOK_INT),
        ident("T"),
        tok(TOK_SEMICOLON),
        tok(TOK_LPAREN),
        ident("T"),
        tok(TOK_RPAREN),
        tok(TOK_STAR),
        ident("p"),
        tok(TOK_SEMICOLON),
        tok(TOK_RBRACE),
        tok(TOK_LPAREN),
        ident("T"),
        tok(TOK_RPAREN),
        tok(TOK_STAR),
        ident("p"),
        tok(TOK_SEMICOLON),
        tok(TOK_RBRACE),
    ]);
    let expected_annotations = annotate_cpu(&fix);
    let gpu_annotations = annotate_gpu(&fix);
    assert_words_eq(&gpu_annotations, &expected_annotations);

    let expected_typed = reference_c11_classify_vast_node_kinds(&expected_annotations);
    let gpu_typed = classify_gpu(&gpu_annotations, fix.tok_types.len());
    assert_words_eq(&gpu_typed, &expected_typed);
    assert_eq!(kind_at(&gpu_typed, 14), 0);
    assert_eq!(kind_at(&gpu_typed, 21), C_AST_KIND_CAST_EXPR);
}

#[test]
fn gpu_typedef_visibility_walks_deeper_than_four_block_scopes() {
    let fix = fixture(&[
        tok(TOK_VOID),
        ident("f"),
        tok(TOK_LPAREN),
        tok(TOK_VOID),
        tok(TOK_RPAREN),
        tok(TOK_LBRACE),
        tok(TOK_TYPEDEF),
        tok(TOK_INT),
        ident("T"),
        tok(TOK_SEMICOLON),
        tok(TOK_LBRACE),
        tok(TOK_LBRACE),
        tok(TOK_LBRACE),
        tok(TOK_LBRACE),
        tok(TOK_LBRACE),
        ident("T"),
        tok(TOK_STAR),
        ident("p"),
        tok(TOK_SEMICOLON),
        tok(TOK_RBRACE),
        tok(TOK_RBRACE),
        tok(TOK_RBRACE),
        tok(TOK_RBRACE),
        tok(TOK_RBRACE),
        tok(TOK_RBRACE),
    ]);
    let expected_annotations = annotate_cpu(&fix);
    let gpu_annotations = annotate_gpu(&fix);
    assert_words_eq(&gpu_annotations, &expected_annotations);

    assert_ne!(
        flags_at(&gpu_annotations, 15) & TYPEDEF_FLAG_VISIBLE,
        0,
        "typedef declared in the outer function block must remain visible five block scopes down"
    );
    let expected_typed = reference_c11_classify_vast_node_kinds(&expected_annotations);
    let gpu_typed = classify_gpu(&gpu_annotations, fix.tok_types.len());
    assert_words_eq(&gpu_typed, &expected_typed);
    assert_eq!(kind_at(&gpu_typed, 16), C_AST_KIND_POINTER_DECL);
}

#[test]
fn gpu_annotation_and_classifier_match_cpu_for_tags() {
    let fix = fixture(&[
        tok(TOK_TYPEDEF),
        tok(TOK_INT),
        ident("S"),
        tok(TOK_SEMICOLON),
        tok(TOK_STRUCT),
        ident("S"),
        tok(TOK_LBRACE),
        tok(TOK_INT),
        ident("field"),
        tok(TOK_SEMICOLON),
        tok(TOK_RBRACE),
        tok(TOK_SEMICOLON),
        tok(TOK_ENUM),
        ident("S"),
        tok(TOK_LBRACE),
        ident("A"),
        tok(TOK_RBRACE),
        tok(TOK_SEMICOLON),
        tok(TOK_VOID),
        ident("f"),
        tok(TOK_LPAREN),
        tok(TOK_VOID),
        tok(TOK_RPAREN),
        tok(TOK_LBRACE),
        tok(TOK_LPAREN),
        ident("S"),
        tok(TOK_RPAREN),
        tok(TOK_STAR),
        ident("p"),
        tok(TOK_SEMICOLON),
        tok(TOK_RBRACE),
    ]);
    let expected_annotations = annotate_cpu(&fix);
    let gpu_annotations = annotate_gpu(&fix);
    assert_words_eq(&gpu_annotations, &expected_annotations);

    let expected_typed = reference_c11_classify_vast_node_kinds(&expected_annotations);
    let gpu_typed = classify_gpu(&gpu_annotations, fix.tok_types.len());
    assert_words_eq(&gpu_typed, &expected_typed);
    assert_eq!(kind_at(&gpu_typed, 24), C_AST_KIND_CAST_EXPR);
}
