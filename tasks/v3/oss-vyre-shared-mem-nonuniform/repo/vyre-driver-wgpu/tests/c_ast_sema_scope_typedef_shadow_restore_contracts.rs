//! Contract tests for typedef shadowing and restoration across nested scopes.
//!
//! Asserts standard C11 semantics:
//!   * typedef names live in the ordinary identifier namespace
//!   * an ordinary declaration in an inner block shadows a visible typedef
//!   * after the inner block exits, the typedef is visible again
//!   * parameter declarations shadow typedefs in the function body
//!   * typedefs declared inside a block are not visible outside it

#![cfg(feature = "c-parser")]
#![allow(deprecated)]
#[path = "c_ast_gpu_parity_support/mod.rs"]
mod c_ast_gpu_parity_support;

use vyre_libs::parsing::c::lex::tokens::*;
use vyre_libs::parsing::c::parse::vast::{
    reference_c11_annotate_typedef_names, reference_c11_build_vast_nodes,
    reference_c11_classify_vast_node_kinds, C_AST_KIND_CAST_EXPR, C_AST_KIND_POINTER_DECL,
    C_AST_KIND_UNARY_EXPR,
};
use vyre_libs::parsing::c::sema::lookup::{DECL_KIND_NONE, DECL_KIND_TYPEDEF, DECL_KIND_VARIABLE};
use vyre_libs::parsing::c::sema::reference_scope_tree;
use vyre_primitives::predicate::node_kind;

use c_ast_gpu_parity_support::{
    run_gpu_c_sema_scope_from_parts, run_gpu_classifier_with_count,
    run_gpu_full_typedef_annotation, word_at, VAST_STRIDE_U32,
};

const FLAGS_FIELD: usize = 7;
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

fn tok(t: u32) -> Atom {
    Atom::Tok(t)
}
fn ident(name: &'static str) -> Atom {
    Atom::Ident(name)
}

fn fixture(_name: &'static str, atoms: &[Atom]) -> Fixture {
    let mut tok_types = Vec::with_capacity(atoms.len());
    let mut tok_starts = Vec::with_capacity(atoms.len());
    let mut tok_lens = Vec::with_capacity(atoms.len());
    let mut haystack = Vec::new();
    let mut cursor = 0u32;
    for atom in atoms {
        match atom {
            Atom::Tok(t) => {
                tok_types.push(*t);
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

fn emit_u32_bytes(words: &[u32]) -> Vec<u8> {
    vyre_primitives::wire::pack_u32_slice(words)
}

fn kind_at(rows: &[u8], idx: usize) -> u32 {
    word_at(rows, idx * VAST_STRIDE_U32)
}

fn flags_at(rows: &[u8], idx: usize) -> u32 {
    word_at(rows, idx * VAST_STRIDE_U32 + FLAGS_FIELD)
}

fn scope_tree_word_at(buf: &[u8], token_idx: usize, field: usize) -> u32 {
    word_at(buf, token_idx * 4 + field)
}

fn scope_tree_for(fix: &Fixture) -> Vec<u8> {
    let haystack_u32: Vec<u32> = fix.haystack.iter().copied().map(u32::from).collect();
    let words = reference_scope_tree(
        &fix.tok_types,
        &fix.tok_starts,
        &fix.tok_lens,
        &haystack_u32,
    );
    emit_u32_bytes(&words)
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

fn run_gpu_scope_tree(fix: &Fixture) -> Vec<u8> {
    run_gpu_c_sema_scope_from_parts(
        &fix.tok_types,
        &fix.tok_starts,
        &fix.tok_lens,
        &fix.haystack,
    )
}

fn run_gpu_annotate(fix: &Fixture) -> Vec<u8> {
    let raw = raw_vast(fix);
    run_gpu_full_typedef_annotation(&fix.haystack, &raw)
}

fn run_gpu_classify(annotated: &[u8], node_count: usize) -> Vec<u8> {
    run_gpu_classifier_with_count(annotated, node_count as u32)
}

// ---------------------------------------------------------------------------
// Scope-tree contract tests
// ---------------------------------------------------------------------------

mod c_ast_sema_scope_typedef_shadow_restore_contracts_part1 {

    include!("__split/c_ast_sema_scope_typedef_shadow_restore_contracts_part1.rs");
}
mod c_ast_sema_scope_typedef_shadow_restore_contracts_part2 {
    include!("__split/c_ast_sema_scope_typedef_shadow_restore_contracts_part2.rs");
}
mod c_ast_sema_scope_typedef_shadow_restore_contracts_part3 {
    include!("__split/c_ast_sema_scope_typedef_shadow_restore_contracts_part3.rs");
}
