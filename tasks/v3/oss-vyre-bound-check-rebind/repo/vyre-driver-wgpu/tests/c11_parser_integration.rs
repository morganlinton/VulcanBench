//! End-to-end smoke test for the existing GPU C11 parser stack.
//!
//! Pipeline under test:
//!   1. `c_grammar_gen::c11_lexer::build_c11_lexer_dfa`  -  host
//!      DFA table serialization remains wire-compatible for consumers that
//!      persist SGGC blobs.
//!   2. `vyre_libs::parsing::c11::lexer::c11_lexer`  -  construct the
//!      vyre `Program` that tokenizes C source on GPU.
//!   3. `vyre_libs::parsing::c11::structure::c11_extract_functions`  -
//!      structural pass that consumes the lexer's token stream.
//!   4. `vyre_emit_naga::program::emit_module`  -  prove
//!      both Programs lower to valid Naga + WGSL.
//!   5. `naga::valid::Validator`  -  reject any non-validating module.
//!
//! Answers "does the existing minimal parser work against the real
//! wgpu emitter or is more wiring required?" If both programs emit +
//! validate, the parser is ready for downstream compilers to consume
//! as-is for rule bodies that reference AST / tokens.
//!
//! Gated behind `feature = "c-parser"` because the parser modules
//! themselves are under the same gate in vyre-libs/src/parsing/c11.

#![cfg(feature = "c-parser")]

use c_grammar_gen::{c11_lexer::build_c11_lexer_dfa, wire::PackedBlob};
use vyre::DispatchConfig;
use vyre_emit_naga::program::emit_module;
use vyre_libs::parsing::c::lex::keyword::{c_keyword, C_KEYWORDS};
use vyre_libs::parsing::c::lex::lexer::c11_lexer;
use vyre_libs::parsing::c::lower::c_lower_ast_to_pg_nodes;
use vyre_libs::parsing::c::parse::structure::{c11_extract_calls, c11_extract_functions};
use vyre_libs::parsing::c::parse::vast::{c11_build_vast_nodes, c11_classify_vast_node_kinds};
use vyre_libs::parsing::c::sema::c_sema_scope;
use vyre_primitives::graph::program_graph::NAME_NODES;

const TEST_WORKGROUP_SIZE: [u32; 3] = [1, 1, 1];
const C11_PIPELINE_SMOKE_TOKENS: u32 = 9;

fn emit_wgsl(program: &vyre::ir::Program) -> String {
    let module = emit_module(program, &DispatchConfig::default(), TEST_WORKGROUP_SIZE)
        .expect("Program must lower to a valid Naga module");
    let info = naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::all(),
    )
    .validate(&module)
    .expect("Naga must accept the Program");
    naga::back::wgsl::write_string(&module, &info, naga::back::wgsl::WriterFlags::empty())
        .expect("Program must serialize to WGSL")
}

#[test]
fn c11_dfa_builder_produces_populated_table() {
    let table = build_c11_lexer_dfa();
    assert!(
        table.num_states >= 16,
        "C11 DFA must reach ≥16 states (identifiers + punctuation + keyword starts), got {}",
        table.num_states
    );
    assert!(
        table.num_classes >= 2,
        "C11 DFA must have ≥2 char classes populated, got {}",
        table.num_classes
    );

    // The packed wire blob must be non-empty and carry the lexer
    // header so the GPU lexer can identify it.
    let blob = PackedBlob::from_dfa(&table);
    assert!(!blob.bytes.is_empty(), "packed DFA blob must be non-empty");
    assert_eq!(
        &blob.bytes[0..4],
        b"SGGC",
        "packed DFA blob must start with the SGGC magic"
    );
}

#[test]
fn c11_lexer_program_validates_and_emits_wgsl() {
    let program = c11_lexer(
        /* haystack = */ "haystack",
        /* out_tok_types = */ "out_tok_types",
        /* out_tok_starts = */ "out_tok_starts",
        /* out_tok_lens = */ "out_tok_lens",
        /* out_counts = */ "out_counts",
        /* haystack_len = */ 4096,
    );

    let errors = vyre::validate(&program);
    assert!(errors.is_empty(), "c11_lexer must IR-validate: {errors:?}");

    let wgsl = emit_wgsl(&program);
    assert!(
        wgsl.contains("@compute"),
        "c11_lexer WGSL must declare a @compute entry: {wgsl}"
    );
    assert!(
        wgsl.contains("out_counts"),
        "c11_lexer must publish the compact token count for downstream parser stages: {wgsl}"
    );
}

#[test]
fn c11_keyword_program_validates_and_emits_wgsl() {
    let program = c_keyword(
        "tok_types",
        "tok_starts",
        "tok_lens",
        "counts",
        "haystack",
        "keyword_map",
        4096,
        C_KEYWORDS.len() as u32,
        4096,
    );

    let errors = vyre::validate(&program);
    assert!(errors.is_empty(), "c_keyword must IR-validate: {errors:?}");

    let wgsl = emit_wgsl(&program);
    assert!(
        wgsl.contains("@compute"),
        "c_keyword WGSL must declare a compute entry: {wgsl}"
    );
}

#[test]
fn c11_extract_functions_program_validates_and_emits_wgsl() {
    let program = c11_extract_functions(
        /* tok_types = */ "tok_types",
        /* paren_pairs = */ "paren_pairs",
        /* brace_pairs = */ "brace_pairs",
        /* num_tokens = */ vyre::ir::Expr::u32(4096),
        /* out_functions = */ "out_functions",
        /* out_counts = */ "out_counts",
    );

    let errors = vyre::validate(&program);
    assert!(
        errors.is_empty(),
        "c11_extract_functions must IR-validate: {errors:?}"
    );

    let wgsl = emit_wgsl(&program);
    assert!(
        wgsl.contains("@compute"),
        "c11_extract_functions WGSL must declare a compute entry: {wgsl}"
    );
}

#[test]
fn c11_extract_calls_program_validates_and_emits_wgsl() {
    let program = c11_extract_calls(
        /* tok_types = */ "tok_types",
        /* paren_pairs = */ "paren_pairs",
        /* functions = */ "functions",
        /* num_tokens = */ vyre::ir::Expr::u32(4096),
        /* num_functions = */ vyre::ir::Expr::u32(128),
        /* out_calls = */ "out_calls",
        /* out_counts = */ "out_counts",
    );

    let errors = vyre::validate(&program);
    assert!(
        errors.is_empty(),
        "c11_extract_calls must IR-validate: {errors:?}"
    );

    let wgsl = emit_wgsl(&program);
    assert!(
        wgsl.contains("@compute"),
        "c11_extract_calls WGSL must declare a compute entry: {wgsl}"
    );
}

#[test]
fn c11_build_vast_nodes_program_validates_and_emits_wgsl() {
    let program = c11_build_vast_nodes(
        "tok_types",
        "tok_starts",
        "tok_lens",
        vyre::ir::Expr::u32(C11_PIPELINE_SMOKE_TOKENS),
        "vast_nodes",
        "vast_count",
    );

    let errors = vyre::validate(&program);
    assert!(
        errors.is_empty(),
        "c11_build_vast_nodes must IR-validate: {errors:?}"
    );

    let wgsl = emit_wgsl(&program);
    assert!(
        wgsl.contains("@compute"),
        "c11_build_vast_nodes WGSL must declare a compute entry: {wgsl}"
    );
}

#[test]
fn c11_classify_vast_node_kinds_program_validates_and_emits_wgsl() {
    let program = c11_classify_vast_node_kinds(
        "vast_nodes",
        vyre::ir::Expr::u32(C11_PIPELINE_SMOKE_TOKENS),
        "typed_vast_nodes",
    );

    let errors = vyre::validate(&program);
    assert!(
        errors.is_empty(),
        "c11_classify_vast_node_kinds must IR-validate: {errors:?}"
    );

    let wgsl = emit_wgsl(&program);
    assert!(
        wgsl.contains("@compute"),
        "c11_classify_vast_node_kinds WGSL must declare a compute entry: {wgsl}"
    );
}

#[test]
fn c11_ast_to_program_graph_lowering_validates_and_emits_wgsl() {
    let program = c_lower_ast_to_pg_nodes(
        "vast_nodes",
        vyre::ir::Expr::u32(C11_PIPELINE_SMOKE_TOKENS),
        NAME_NODES,
    );

    let errors = vyre::validate(&program);
    assert!(
        errors.is_empty(),
        "c_lower_ast_to_pg_nodes must IR-validate: {errors:?}"
    );

    let wgsl = emit_wgsl(&program);
    assert!(
        wgsl.contains(NAME_NODES),
        "c_lower_ast_to_pg_nodes WGSL must write canonical ProgramGraph node buffer {NAME_NODES}: {wgsl}"
    );
}

#[test]
fn c11_parser_pipeline_integrates_without_stitching_errors() {
    // Lexer Program + both structural Programs exist as independent
    // compute entries today; each is a distinct dispatch in the
    // current pipeline. This test acts as the single assertion that
    // all three validate together  -  a regression in the emitter that
    // breaks one of them should FAIL here first, not at warpscan
    // runtime.
    let lexer = c11_lexer(
        "haystack",
        "out_tok_types",
        "out_tok_starts",
        "out_tok_lens",
        "out_counts",
        4096,
    );
    let functions = c11_extract_functions(
        "tok_types",
        "paren_pairs",
        "brace_pairs",
        vyre::ir::Expr::u32(4096),
        "out_functions",
        "out_counts",
    );
    let calls = c11_extract_calls(
        "tok_types",
        "paren_pairs",
        "functions",
        vyre::ir::Expr::u32(4096),
        vyre::ir::Expr::u32(128),
        "out_calls",
        "out_counts",
    );
    let sema = c_sema_scope(
        "tok_types",
        "tok_starts",
        "tok_lens",
        "haystack",
        vyre::ir::Expr::u32(4096),
        vyre::ir::Expr::u32(4096),
        "out_scope_tree",
    );
    let keywords = c_keyword(
        "tok_types",
        "tok_starts",
        "tok_lens",
        "counts",
        "haystack",
        "keyword_map",
        4096,
        C_KEYWORDS.len() as u32,
        4096,
    );
    let vast = c11_build_vast_nodes(
        "tok_types",
        "tok_starts",
        "tok_lens",
        vyre::ir::Expr::u32(C11_PIPELINE_SMOKE_TOKENS),
        "vast_nodes",
        "vast_count",
    );
    let typed_vast = c11_classify_vast_node_kinds(
        "vast_nodes",
        vyre::ir::Expr::u32(C11_PIPELINE_SMOKE_TOKENS),
        "typed_vast_nodes",
    );
    let pg_nodes = c_lower_ast_to_pg_nodes(
        "typed_vast_nodes",
        vyre::ir::Expr::u32(C11_PIPELINE_SMOKE_TOKENS),
        NAME_NODES,
    );

    for (name, prog) in [
        ("c11_lexer", &lexer),
        ("c_keyword", &keywords),
        ("c11_build_vast_nodes", &vast),
        ("c11_classify_vast_node_kinds", &typed_vast),
        ("c_lower_ast_to_pg_nodes", &pg_nodes),
        ("c11_extract_functions", &functions),
        ("c11_extract_calls", &calls),
        ("c11_sema_scope", &sema),
    ] {
        let errors = vyre::validate(prog);
        assert!(
            errors.is_empty(),
            "{name} must validate in the unified smoke: {errors:?}"
        );
        let _ = emit_wgsl(prog);
    }
}
