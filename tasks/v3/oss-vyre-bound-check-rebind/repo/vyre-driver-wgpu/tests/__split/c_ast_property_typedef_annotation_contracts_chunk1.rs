// Property and adversarial tests for C typedef annotation invariants.
// Covers flags, scope, symbol hash, and CPU/GPU parity.

// cfg(feature = "c-parser")  -  moved to parent

#[path = "../c_ast_gpu_parity_support/mod.rs"]
mod c_ast_gpu_parity_support;

use c_ast_gpu_parity_support::{
    run_gpu_full_typedef_annotation,
    run_gpu_vast_builder_from_parts as run_gpu_vast_builder, starts_for_lens, word_at,
};
use proptest::prelude::*;
use vyre_foundation::vast::{NODE_STRIDE_U32, SENTINEL};
use vyre_libs::parsing::c::lex::tokens::*;
use vyre_libs::parsing::c::parse::vast::{
    reference_c11_annotate_typedef_names, reference_c11_build_vast_nodes,
};

// Redefine private constants for test assertions.
const C_TYPEDEF_FLAG_VISIBLE_TYPEDEF_NAME: u32 = 1;
const C_TYPEDEF_FLAG_TYPEDEF_DECLARATOR: u32 = 1 << 1;
const C_TYPEDEF_FLAG_ORDINARY_DECLARATOR: u32 = 1 << 2;
const VAST_TYPEDEF_FLAGS_FIELD: usize = 7;
const VAST_TYPEDEF_SCOPE_FIELD: usize = 8;
const VAST_TYPEDEF_SYMBOL_FIELD: usize = 9;

const VAST_STRIDE_BYTES: usize = NODE_STRIDE_U32 * 4;

fn run_gpu_typedef_annotation(raw_vast: &[u8], haystack: &[u8], num_nodes: u32) -> Vec<u8> {
    let _ = num_nodes;
    run_gpu_full_typedef_annotation(haystack, raw_vast)
}

fn fnv1a32(bytes: &[u8]) -> u32 {
    let mut hash = 0x811c_9dc5u32;
    for b in bytes {
        hash ^= *b as u32;
        hash = hash.wrapping_mul(0x0100_0193);
    }
    hash
}

// ---------------------------------------------------------------------------
// Helpers to build fixtures with named identifiers
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
enum Atom {
    Tok(u32),
    Ident(&'static str),
}

fn build_fixture(atoms: &[Atom]) -> (Vec<u32>, Vec<u32>, Vec<u32>, Vec<u8>) {
    let mut tok_types = Vec::new();
    let mut tok_lens = Vec::new();
    let mut source = String::new();
    for atom in atoms {
        if !source.is_empty() {
            source.push(' ');
        }
        match atom {
            Atom::Tok(t) => {
                tok_types.push(*t);
                let len = match *t {
                    TOK_EQ | TOK_NE | TOK_LE | TOK_GE | TOK_AND | TOK_OR | TOK_LSHIFT
                    | TOK_RSHIFT | TOK_INC | TOK_DEC | TOK_PLUS_EQ | TOK_MINUS_EQ | TOK_STAR_EQ
                    | TOK_SLASH_EQ | TOK_ARROW => 2,
                    TOK_ELLIPSIS => 3,
                    _ => 1,
                };
                tok_lens.push(len);
                for _ in 0..len {
                    source.push('x');
                }
            }
            Atom::Ident(name) => {
                tok_types.push(TOK_IDENTIFIER);
                tok_lens.push(name.len() as u32);
                source.push_str(name);
            }
        }
    }
    let tok_starts = starts_for_lens(&tok_lens);
    (tok_types, tok_starts, tok_lens, source.into_bytes())
}

fn assert_annotation_invariants(node_bytes: &[u8], _haystack: &[u8]) {
    let node_count = node_bytes.len() / VAST_STRIDE_BYTES;
    for i in 0..node_count {
        let flags = word_at(node_bytes, i * NODE_STRIDE_U32 + VAST_TYPEDEF_FLAGS_FIELD);
        let scope = word_at(node_bytes, i * NODE_STRIDE_U32 + VAST_TYPEDEF_SCOPE_FIELD);
        let sym = word_at(node_bytes, i * NODE_STRIDE_U32 + VAST_TYPEDEF_SYMBOL_FIELD);

        // scope must be in-bounds or SENTINEL
        if scope != SENTINEL {
            assert!(
                (scope as usize) < node_count,
                "node {i} scope {scope} out of bounds"
            );
        }

        // If any flag is set, the node should be an identifier (kind == TOK_IDENTIFIER)
        // Note: classification hasn't run yet, so kind is still the raw token type.
        let kind = word_at(node_bytes, i * NODE_STRIDE_U32);
        if flags != 0 {
            assert_eq!(
                kind, TOK_IDENTIFIER,
                "node {i} has typedef flags {flags} but kind {kind} is not an identifier"
            );
        }

        // symbol hash must be non-zero when a name is expected
        if flags & C_TYPEDEF_FLAG_VISIBLE_TYPEDEF_NAME != 0
            || flags & C_TYPEDEF_FLAG_TYPEDEF_DECLARATOR != 0
        {
            assert!(
                sym != 0,
                "node {i} with flags {flags} must have non-zero symbol hash"
            );
        }

        // flags must be a subset of known bits
        let known = C_TYPEDEF_FLAG_VISIBLE_TYPEDEF_NAME
            | C_TYPEDEF_FLAG_TYPEDEF_DECLARATOR
            | C_TYPEDEF_FLAG_ORDINARY_DECLARATOR;
        assert_eq!(
            flags & !known,
            0,
            "node {i} has unknown flag bits set: {flags}"
        );
    }
}

// ---------------------------------------------------------------------------
// Deterministic adversarial tables
// ---------------------------------------------------------------------------

#[test]
fn typedef_simple_visible_name() {
    // typedef int T; T x;
    let atoms = vec![
        Atom::Tok(TOK_TYPEDEF),
        Atom::Tok(TOK_INT),
        Atom::Ident("T"),
        Atom::Tok(TOK_SEMICOLON),
        Atom::Ident("T"),
        Atom::Ident("x"),
        Atom::Tok(TOK_SEMICOLON),
    ];
    let (tok_types, tok_starts, tok_lens, haystack) = build_fixture(&atoms);
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let annotated = reference_c11_annotate_typedef_names(&raw, &haystack);
    assert_annotation_invariants(&annotated, &haystack);

    // The second T (index 4) should be marked as visible typedef name.
    let flags = word_at(&annotated, 4 * NODE_STRIDE_U32 + VAST_TYPEDEF_FLAGS_FIELD);
    assert!(
        flags & C_TYPEDEF_FLAG_VISIBLE_TYPEDEF_NAME != 0,
        "second T must be marked as visible typedef name"
    );
}

#[test]
fn typedef_shadowing_nested_block() {
    // typedef int T; void f(void) { int T; T * x; }
    let atoms = vec![
        Atom::Tok(TOK_TYPEDEF),
        Atom::Tok(TOK_INT),
        Atom::Ident("T"),
        Atom::Tok(TOK_SEMICOLON),
        Atom::Tok(TOK_VOID),
        Atom::Ident("f"),
        Atom::Tok(TOK_LPAREN),
        Atom::Tok(TOK_VOID),
        Atom::Tok(TOK_RPAREN),
        Atom::Tok(TOK_LBRACE),
        Atom::Tok(TOK_INT),
        Atom::Ident("T"),
        Atom::Tok(TOK_SEMICOLON),
        Atom::Ident("T"),
        Atom::Tok(TOK_STAR),
        Atom::Ident("x"),
        Atom::Tok(TOK_SEMICOLON),
        Atom::Tok(TOK_RBRACE),
    ];
    let (tok_types, tok_starts, tok_lens, haystack) = build_fixture(&atoms);
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let annotated = reference_c11_annotate_typedef_names(&raw, &haystack);
    assert_annotation_invariants(&annotated, &haystack);

    // The T inside the block (index 11, declarator) should be ORDINARY_DECLARATOR.
    let flags_decl = word_at(&annotated, 11 * NODE_STRIDE_U32 + VAST_TYPEDEF_FLAGS_FIELD);
    assert!(
        flags_decl & C_TYPEDEF_FLAG_ORDINARY_DECLARATOR != 0,
        "shadowed T declarator must be ordinary"
    );

    // The T used in T * x (index 13) should NOT be VISIBLE_TYPEDEF_NAME because it's shadowed.
    let flags_use = word_at(&annotated, 13 * NODE_STRIDE_U32 + VAST_TYPEDEF_FLAGS_FIELD);
    assert!(
        flags_use & C_TYPEDEF_FLAG_VISIBLE_TYPEDEF_NAME == 0,
        "shadowed T use must not be visible typedef name"
    );
}

#[test]
fn typedef_struct_tag_vs_typedef_name() {
    // typedef struct S { int x; } S;
    // S *a;
    let atoms = vec![
        Atom::Tok(TOK_TYPEDEF),
        Atom::Tok(TOK_STRUCT),
        Atom::Ident("S"),
        Atom::Tok(TOK_LBRACE),
        Atom::Tok(TOK_INT),
        Atom::Ident("x"),
        Atom::Tok(TOK_SEMICOLON),
        Atom::Tok(TOK_RBRACE),
        Atom::Ident("S"),
        Atom::Tok(TOK_SEMICOLON),
        Atom::Ident("S"),
        Atom::Ident("a"),
        Atom::Tok(TOK_SEMICOLON),
    ];
    let (tok_types, tok_starts, tok_lens, haystack) = build_fixture(&atoms);
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let annotated = reference_c11_annotate_typedef_names(&raw, &haystack);
    assert_annotation_invariants(&annotated, &haystack);

    // The first S (index 2) is a struct tag, not a typedef declarator.
    let flags_tag = word_at(&annotated, 2 * NODE_STRIDE_U32 + VAST_TYPEDEF_FLAGS_FIELD);
    assert_eq!(flags_tag, 0, "struct tag S must not have typedef flags");

    // The second S (index 8) is the typedef declarator in standard C.
    let flags_decl = word_at(&annotated, 8 * NODE_STRIDE_U32 + VAST_TYPEDEF_FLAGS_FIELD);
    assert!(
        flags_decl & C_TYPEDEF_FLAG_TYPEDEF_DECLARATOR != 0,
        "typedef declarator after a struct body must preserve typedef context"
    );

    // The usage S (index 10) resolves to the visible typedef name.
    let flags_use = word_at(&annotated, 10 * NODE_STRIDE_U32 + VAST_TYPEDEF_FLAGS_FIELD);
    assert!(
        flags_use & C_TYPEDEF_FLAG_VISIBLE_TYPEDEF_NAME != 0,
        "usage of typedef S after struct body must be marked visible"
    );
}

#[test]
fn typedef_many_in_sequence() {
    // 66 typedefs to exercise scan-limit boundaries
    let mut atoms = Vec::new();
    for i in 0..66 {
        atoms.push(Atom::Tok(TOK_TYPEDEF));
        atoms.push(Atom::Tok(TOK_INT));
        atoms.push(Atom::Ident(if i % 2 == 0 { "A" } else { "B" }));
        atoms.push(Atom::Tok(TOK_SEMICOLON));
    }
    // Use the last typedef name.
    atoms.push(Atom::Ident("B"));
    atoms.push(Atom::Ident("x"));
    atoms.push(Atom::Tok(TOK_SEMICOLON));

    let (tok_types, tok_starts, tok_lens, haystack) = build_fixture(&atoms);
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let annotated = reference_c11_annotate_typedef_names(&raw, &haystack);
    assert_annotation_invariants(&annotated, &haystack);
}

#[test]
fn typedef_symbol_hash_matches_fnv1a() {
    // typedef int Foo;
    let atoms = vec![
        Atom::Tok(TOK_TYPEDEF),
        Atom::Tok(TOK_INT),
        Atom::Ident("Foo"),
        Atom::Tok(TOK_SEMICOLON),
    ];
    let (tok_types, tok_starts, tok_lens, haystack) = build_fixture(&atoms);
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let annotated = reference_c11_annotate_typedef_names(&raw, &haystack);

    let sym = word_at(&annotated, 2 * NODE_STRIDE_U32 + VAST_TYPEDEF_SYMBOL_FIELD);
    let expected = fnv1a32(b"Foo");
    assert_eq!(sym, expected, "symbol hash for Foo must match FNV-1a");
}

// ---------------------------------------------------------------------------
// GPU parity tests
// ---------------------------------------------------------------------------

#[test]
fn gpu_parity_typedef_simple() {
    let atoms = vec![
        Atom::Tok(TOK_TYPEDEF),
        Atom::Tok(TOK_INT),
        Atom::Ident("T"),
        Atom::Tok(TOK_SEMICOLON),
        Atom::Ident("T"),
        Atom::Ident("x"),
        Atom::Tok(TOK_SEMICOLON),
    ];
    let (tok_types, tok_starts, tok_lens, haystack) = build_fixture(&atoms);
    let raw_cpu = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let annotated_cpu = reference_c11_annotate_typedef_names(&raw_cpu, &haystack);

    let raw_gpu = run_gpu_vast_builder(&tok_types, &tok_starts, &tok_lens);
    let annotated_gpu = run_gpu_typedef_annotation(&raw_gpu, &haystack, tok_types.len() as u32);

    assert_eq!(
        annotated_gpu, annotated_cpu,
        "GPU typedef annotation must match CPU for simple typedef"
    );
}

#[test]
fn gpu_parity_typedef_shadowing() {
    let atoms = vec![
        Atom::Tok(TOK_TYPEDEF),
        Atom::Tok(TOK_INT),
        Atom::Ident("T"),
        Atom::Tok(TOK_SEMICOLON),
        Atom::Tok(TOK_VOID),
        Atom::Ident("f"),
        Atom::Tok(TOK_LPAREN),
        Atom::Tok(TOK_VOID),
        Atom::Tok(TOK_RPAREN),
        Atom::Tok(TOK_LBRACE),
        Atom::Tok(TOK_INT),
        Atom::Ident("T"),
        Atom::Tok(TOK_SEMICOLON),
        Atom::Ident("T"),
        Atom::Tok(TOK_STAR),
        Atom::Ident("x"),
        Atom::Tok(TOK_SEMICOLON),
        Atom::Tok(TOK_RBRACE),
    ];
    let (tok_types, tok_starts, tok_lens, haystack) = build_fixture(&atoms);
    let raw_cpu = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let annotated_cpu = reference_c11_annotate_typedef_names(&raw_cpu, &haystack);

    let raw_gpu = run_gpu_vast_builder(&tok_types, &tok_starts, &tok_lens);
    let annotated_gpu = run_gpu_typedef_annotation(&raw_gpu, &haystack, tok_types.len() as u32);

    assert_eq!(
        annotated_gpu, annotated_cpu,
        "GPU typedef annotation must match CPU for shadowed typedef"
    );
}

#[test]
fn gpu_parity_typedef_struct_tag() {
    let atoms = vec![
        Atom::Tok(TOK_TYPEDEF),
        Atom::Tok(TOK_STRUCT),
        Atom::Ident("S"),
        Atom::Tok(TOK_LBRACE),
        Atom::Tok(TOK_INT),
        Atom::Ident("x"),
        Atom::Tok(TOK_SEMICOLON),
        Atom::Tok(TOK_RBRACE),
        Atom::Ident("S"),
        Atom::Tok(TOK_SEMICOLON),
        Atom::Ident("S"),
        Atom::Ident("a"),
        Atom::Tok(TOK_SEMICOLON),
    ];
    let (tok_types, tok_starts, tok_lens, haystack) = build_fixture(&atoms);
    let raw_cpu = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let annotated_cpu = reference_c11_annotate_typedef_names(&raw_cpu, &haystack);

    let raw_gpu = run_gpu_vast_builder(&tok_types, &tok_starts, &tok_lens);
    let annotated_gpu = run_gpu_typedef_annotation(&raw_gpu, &haystack, tok_types.len() as u32);

    assert_eq!(
        annotated_gpu, annotated_cpu,
        "GPU typedef annotation must match CPU for struct tag typedef"
    );
}
