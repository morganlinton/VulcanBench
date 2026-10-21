//! Go frontend corpus regression tests.

#![cfg(feature = "go-parser")]
#![allow(deprecated)]

mod common;
use common::decode_u32_words;
use std::fs;
use std::path::{Path, PathBuf};

use tree_sitter::Parser;
use vyre::ir::Expr;
use vyre_libs::parsing::go::lex::go_lexer;
use vyre_libs::parsing::go::parse::ast_ops::{
    go_extract_channel_receives, go_extract_channel_sends, go_extract_defer_calls,
    go_extract_goroutine_calls,
};
use vyre_libs::parsing::go::parse::structure::{
    go_extract_declarations, go_extract_packages_and_imports, GO_DECL_RECORD_WORDS,
    GO_SPAN_RECORD_WORDS,
};
use vyre_reference::value::Value;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
struct Counts {
    packages: u32,
    imports: u32,
    decls: u32,
    goroutines: u32,
    sends: u32,
    receives: u32,
    deferred: u32,
}

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/go")
}

fn pack_source(source: &str) -> Vec<u8> {
    source
        .as_bytes()
        .iter()
        .flat_map(|byte| u32::from(*byte).to_le_bytes())
        .collect()
}

fn zeroed_u32_words(words: usize) -> Vec<u8> {
    vec![0u8; words * 4]
}

fn run(program: &vyre::Program, inputs: Vec<Vec<u8>>) -> Vec<Vec<u8>> {
    vyre_reference::reference_eval(
        program,
        &inputs.into_iter().map(Value::from).collect::<Vec<_>>(),
    )
    .expect("reference execution must succeed")
    .into_iter()
    .map(|value| value.to_bytes())
    .collect()
}

fn gpu_counts(source: &str) -> Counts {
    let haystack_words = source.len().max(1);
    let lexer = go_lexer(
        "haystack",
        "out_tok_types",
        "out_tok_starts",
        "out_tok_lens",
        "out_counts",
        haystack_words as u32,
    );
    let lexer_outputs = run(
        &lexer,
        vec![
            pack_source(source),
            zeroed_u32_words(haystack_words),
            zeroed_u32_words(haystack_words),
            zeroed_u32_words(haystack_words),
            zeroed_u32_words(1),
        ],
    );
    let tok_types = lexer_outputs[0].clone();
    let tok_starts = lexer_outputs[1].clone();
    let tok_lens = lexer_outputs[2].clone();
    let tok_count = decode_u32_words(&lexer_outputs[3])[0] as usize;
    assert!(tok_count > 0, "lexer must emit at least one token");

    let packages_and_imports = go_extract_packages_and_imports(
        "tok_types",
        "tok_starts",
        "tok_lens",
        "haystack",
        Expr::u32(tok_count as u32),
        "out_packages",
        "out_package_counts",
        "out_imports",
        "out_import_counts",
    );
    let pkg_outputs = run(
        &packages_and_imports,
        vec![
            tok_types.clone(),
            tok_starts.clone(),
            tok_lens.clone(),
            pack_source(source),
            zeroed_u32_words(tok_count.saturating_mul(GO_SPAN_RECORD_WORDS as usize)),
            zeroed_u32_words(1),
            zeroed_u32_words(tok_count.saturating_mul(GO_SPAN_RECORD_WORDS as usize)),
            zeroed_u32_words(1),
        ],
    );

    let decls = go_extract_declarations(
        "tok_types",
        "tok_starts",
        "tok_lens",
        "haystack",
        Expr::u32(tok_count as u32),
        "out_decls",
        "out_decl_counts",
    );
    let decl_outputs = run(
        &decls,
        vec![
            tok_types.clone(),
            tok_starts.clone(),
            tok_lens.clone(),
            pack_source(source),
            zeroed_u32_words(tok_count.saturating_mul(GO_DECL_RECORD_WORDS as usize)),
            zeroed_u32_words(1),
        ],
    );

    let goroutines = go_extract_goroutine_calls(
        "tok_types",
        "tok_starts",
        "tok_lens",
        "haystack",
        Expr::u32(tok_count as u32),
        "out_calls",
        "out_counts",
    );
    let goroutine_outputs = run(
        &goroutines,
        vec![
            tok_types.clone(),
            tok_starts.clone(),
            tok_lens.clone(),
            pack_source(source),
            zeroed_u32_words(tok_count.saturating_mul(GO_SPAN_RECORD_WORDS as usize)),
            zeroed_u32_words(1),
        ],
    );

    let sends = go_extract_channel_sends(
        "tok_types",
        "tok_starts",
        "tok_lens",
        Expr::u32(tok_count as u32),
        "out_ops",
        "out_counts",
    );
    let send_outputs = run(
        &sends,
        vec![
            tok_types.clone(),
            tok_starts.clone(),
            tok_lens.clone(),
            zeroed_u32_words(tok_count.saturating_mul(GO_SPAN_RECORD_WORDS as usize)),
            zeroed_u32_words(1),
        ],
    );

    let receives = go_extract_channel_receives(
        "tok_types",
        "tok_starts",
        "tok_lens",
        Expr::u32(tok_count as u32),
        "out_ops",
        "out_counts",
    );
    let receive_outputs = run(
        &receives,
        vec![
            tok_types.clone(),
            tok_starts.clone(),
            tok_lens.clone(),
            zeroed_u32_words(tok_count.saturating_mul(GO_SPAN_RECORD_WORDS as usize)),
            zeroed_u32_words(1),
        ],
    );

    let deferred = go_extract_defer_calls(
        "tok_types",
        "tok_starts",
        "tok_lens",
        "haystack",
        Expr::u32(tok_count as u32),
        "out_calls",
        "out_counts",
    );
    let defer_outputs = run(
        &deferred,
        vec![
            tok_types,
            tok_starts,
            tok_lens,
            pack_source(source),
            zeroed_u32_words(tok_count.saturating_mul(GO_SPAN_RECORD_WORDS as usize)),
            zeroed_u32_words(1),
        ],
    );

    Counts {
        packages: decode_u32_words(&pkg_outputs[1])[0] / GO_SPAN_RECORD_WORDS,
        imports: decode_u32_words(&pkg_outputs[3])[0] / GO_SPAN_RECORD_WORDS,
        decls: decode_u32_words(&decl_outputs[1])[0] / GO_DECL_RECORD_WORDS,
        goroutines: decode_u32_words(&goroutine_outputs[1])[0] / GO_SPAN_RECORD_WORDS,
        sends: decode_u32_words(&send_outputs[1])[0] / GO_SPAN_RECORD_WORDS,
        receives: decode_u32_words(&receive_outputs[1])[0] / GO_SPAN_RECORD_WORDS,
        deferred: decode_u32_words(&defer_outputs[1])[0] / GO_SPAN_RECORD_WORDS,
    }
}

fn reference_counts(source: &str) -> Counts {
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_go::LANGUAGE.into())
        .expect("Go parser language must load");
    let tree = parser
        .parse(source, None)
        .expect("tree-sitter parse must succeed");
    let root = tree.root_node();
    assert!(
        !root.has_error(),
        "fixture must parse cleanly for reference"
    );

    fn walk(node: tree_sitter::Node<'_>, source: &str, counts: &mut Counts) {
        match node.kind() {
            "package_clause" => counts.packages += 1,
            "import_spec" => counts.imports += 1,
            "function_declaration" | "method_declaration" => counts.decls += 1,
            "interface_type" => counts.decls += 1,
            "go_statement" => counts.goroutines += 1,
            "defer_statement" => counts.deferred += 1,
            "send_statement" => counts.sends += 1,
            "unary_expression" => {
                let text = &source[node.byte_range()];
                if text.trim_start().starts_with("<-") {
                    counts.receives += 1;
                }
            }
            _ => {}
        }

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            walk(child, source, counts);
        }
    }

    let mut counts = Counts::default();
    walk(root, source, &mut counts);
    counts
}

fn fixture_files() -> Vec<PathBuf> {
    let mut files = fs::read_dir(fixtures_dir())
        .expect("fixtures dir")
        .map(|entry| entry.expect("dir entry").path())
        .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("go"))
        .collect::<Vec<_>>();
    files.sort();
    files
}

#[test]
fn go_frontend_corpus_meets_parse_success_and_count_parity() {
    let files = fixture_files();
    assert!(!files.is_empty(), "Go fixture corpus must not be empty");

    let mut successes = 0usize;
    for path in &files {
        let source = fs::read_to_string(path).expect("fixture source");
        let gpu = gpu_counts(&source);
        let reference = reference_counts(&source);
        if gpu.packages > 0 && gpu.decls > 0 {
            successes += 1;
        }
        assert_eq!(gpu, reference, "count drift for {}", path.display());
    }

    let success_rate = (successes as f64 / files.len() as f64) * 100.0;
    assert!(
        success_rate >= 99.5,
        "parse-success rate must be >= 99.5%, got {success_rate:.2}%"
    );
}

#[test]
fn go_fixture_corpus_is_substantial() {
    fn count_lines(path: &Path) -> usize {
        fs::read_to_string(path)
            .expect("fixture source")
            .lines()
            .count()
    }

    let total_lines: usize = fixture_files().iter().map(|path| count_lines(path)).sum();
    assert!(
        total_lines >= 500,
        "fixture corpus should remain non-trivial; expected >=500 lines, found {total_lines}"
    );
}
