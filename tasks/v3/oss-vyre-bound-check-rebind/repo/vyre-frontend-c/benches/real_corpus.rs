//! Criterion benchmarks for real Linux C corpus parser throughput.

#![allow(missing_docs)]

use std::hint::black_box;
use std::path::{Path, PathBuf};

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use vyre_driver_cuda as _;
use vyre_driver_wgpu as _;
use vyre_frontend_c::api::parse_syntax_batch_bytes;

fn collect_c_files(root: &Path, files: &mut Vec<PathBuf>) -> std::io::Result<()> {
    let entries = std::fs::read_dir(root)?;
    for entry in entries {
        let path = entry?.path();
        if path.is_dir() {
            collect_c_files(&path, files)?;
        } else if path.extension().is_some_and(|extension| extension == "c") {
            files.push(path);
        }
    }
    Ok(())
}

fn load_real_corpus() -> Vec<Vec<u8>> {
    let root = std::env::var_os("VYRE_FRONTEND_C_REAL_CORPUS_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/corpus/r2_kernel_scripts")
        });
    let mut paths = Vec::new();
    collect_c_files(&root, &mut paths).unwrap_or_else(|error| {
        panic!(
            "failed to enumerate real C corpus under {}: {error}",
            root.display()
        )
    });
    paths.sort();
    if let Ok(max_files) = std::env::var("VYRE_FRONTEND_C_REAL_CORPUS_MAX_FILES") {
        let max_files = max_files
            .parse::<usize>()
            .expect("VYRE_FRONTEND_C_REAL_CORPUS_MAX_FILES must be a positive integer");
        assert!(
            max_files > 0,
            "VYRE_FRONTEND_C_REAL_CORPUS_MAX_FILES must be positive"
        );
        paths.truncate(max_files);
    }
    assert!(
        !paths.is_empty(),
        "real C corpus has no .c files under {}",
        root.display()
    );
    paths
        .into_iter()
        .map(|path| {
            std::fs::read(&path)
                .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()))
        })
        .collect()
}

fn tree_sitter_c_cold_batch(sources: &[Vec<u8>]) -> (usize, bool) {
    let mut nodes = 0usize;
    let mut has_error = false;
    let language: tree_sitter::Language = tree_sitter_c::LANGUAGE.into();
    for source in sources {
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&language)
            .expect("tree-sitter C language must initialize");
        let tree = parser
            .parse(source.as_slice(), None)
            .expect("tree-sitter C parse should produce a tree");
        has_error |= tree.root_node().has_error();
        let mut cursor = tree.walk();
        nodes = nodes.saturating_add(count_tree_sitter_nodes(&mut cursor));
    }
    (nodes, has_error)
}

fn count_tree_sitter_nodes(cursor: &mut tree_sitter::TreeCursor<'_>) -> usize {
    let mut nodes = 0usize;
    loop {
        nodes = nodes.saturating_add(1);
        if cursor.goto_first_child() {
            continue;
        }
        loop {
            if cursor.goto_next_sibling() {
                break;
            }
            if !cursor.goto_parent() {
                return nodes;
            }
        }
    }
}

fn bench_real_corpus(c: &mut Criterion) {
    let corpus = load_real_corpus();
    let file_count = corpus.len();
    let total_bytes = corpus.iter().map(Vec::len).sum::<usize>();
    let refs = corpus.iter().map(Vec::as_slice).collect::<Vec<_>>();
    let mut group = c.benchmark_group("frontend.c.real_corpus");
    group.throughput(Throughput::Bytes(total_bytes as u64));
    group.bench_with_input(
        BenchmarkId::new("parse_syntax_batch_bytes", file_count),
        &refs,
        |b, sources| {
            b.iter(|| {
                let summary =
                    parse_syntax_batch_bytes(black_box(sources)).expect("real corpus parse failed");
                black_box(summary);
            });
        },
    );
    group.bench_with_input(
        BenchmarkId::new("tree_sitter_c_cold_batch", file_count),
        &corpus,
        |b, sources| {
            b.iter(|| {
                let baseline = tree_sitter_c_cold_batch(black_box(sources));
                black_box(baseline);
            });
        },
    );
    group.finish();
}

criterion_group!(benches, bench_real_corpus);
criterion_main!(benches);
