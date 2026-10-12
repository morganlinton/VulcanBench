//! S0  -  AST equivalence oracle smoke harness.
//!
//! Confirms the support helpers (`clang_user_kinds`, `vyrec_user_kinds`,
//! `assert_kinds_contain`) behave correctly end-to-end on real fixtures.
//! Per-feature assertions live in the per-feature ticket tests (G1–G15,
//! P-tier, S-tier)  -  this file is just the smoke gate that proves the
//! helpers are wired correctly so those tests can rely on them.
//!
//! Why kind-presence is enough for Phase 1: every Tier-G / preprocessor /
//! C11 ticket promises "vyrec parses construct X to a VAST node of kind K".
//! That contract is verifiable by checking `vyrec_user_kinds` for K. A
//! structural diff (Phase 2) is open and lands later.

mod support;

use std::path::PathBuf;

use support::ast_oracle::{assert_kinds_contain, clang_user_kinds_required, vyrec_user_kinds};
use support::object::compile_source;

fn fixture(rel: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/corpus")
        .join(rel)
}

fn read(rel: &str) -> (PathBuf, String) {
    let path = fixture(rel);
    let source =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read fixture {rel}: {e}"));
    (path, source)
}

#[test]
fn clang_oracle_returns_user_file_kinds_for_typeof_fixture() {
    let path = fixture("g3_typeof/typeof_uses.c");
    let kinds = clang_user_kinds_required(&path);
    // Sanity: clang must surface at least one user-file declaration node.
    // The fixture contains `int f(void)` so a FunctionDecl is mandatory.
    // Failure here means the JSON walker / sticky-file inheritance is broken.
    assert_ne!(
        kinds.len(),
        0,
        "clang oracle returned no user-file kinds; walker is filtering everything out"
    );
    assert_kinds_contain(&kinds, &["FunctionDecl"]);
}

#[test]
fn vyrec_oracle_emits_classified_kind_for_typeof_fixture() {
    // The typeof_uses fixture is the canonical proof point: existing
    // `g3_typeof_e2e::typeof_specifier_in_declaration_compiles_and_classifies_variables`
    // already asserts the `__typeof__` token classifies to
    // `C_AST_KIND_SIZEOF_EXPR` at a known token index. The oracle helper
    // must surface that same kind in its flat label stream.
    let (_path, source) = read("g3_typeof/typeof_uses.c");
    let object = compile_source("ast_oracle_typeof", &source, Vec::new());
    let kinds = vyrec_user_kinds(&object);
    assert_ne!(
        kinds.len(),
        0,
        "vyrec oracle returned no classified kinds for a fixture with known classifications; \
         either compile_source did not run the classify pass or the VAST_STRIDE_U32 layout \
         changed and ast_oracle::vyrec_user_kinds drifted"
    );
    assert_kinds_contain(&kinds, &["SizeofExpr"]);
}

#[test]
fn vyrec_oracle_emits_attribute_node_for_g1_fixture() {
    // g1's decls.c uses `__attribute__((noreturn))`, `aligned(8)`, `packed`,
    // `format(printf,...)`, `unused`. The G1 ticket's contract is that the
    // GNU attribute parser produces a `C_AST_KIND_GNU_ATTRIBUTE` node for
    // each occurrence. If that contract regresses, this test fails before
    // any per-feature assertion does.
    let (_path, source) = read("g1_attribute/decls.c");
    let object = compile_source("ast_oracle_attribute", &source, Vec::new());
    let kinds = vyrec_user_kinds(&object);
    if !kinds.iter().any(|k| k == "GnuAttribute") {
        // Surface the actual kinds the parser emitted so the regression is
        // diagnosable  -  this tells us whether G1 silently drifted to a
        // different label or whether the entire classify path collapsed.
        let summary: Vec<&str> = kinds.iter().take(40).map(String::as_str).collect();
        panic!(
            "ast_oracle: g1 fixture must emit GnuAttribute kind; got {} kinds, first 40: {:?}",
            kinds.len(),
            summary,
        );
    }
}
