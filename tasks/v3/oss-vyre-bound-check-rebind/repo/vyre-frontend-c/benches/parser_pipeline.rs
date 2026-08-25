#![allow(missing_docs)]
//! End-to-end GPU C-frontend pipeline latency benchmark.
//!
//! Measures wall time of `compile()` (lex → digraph → keyword → mask →
//! functions → calls → ABI → AST → CFG → VAST → P-6 semantic graph →
//! ELF emit) on representative C11+GNU translation units.
//!
//! The bench writes the source to a tempfile, runs the full pipeline,
//! reads the resulting `.o`, and removes both. This is the real product
//! latency a `vyrec` invocation pays, not a synthetic stage measurement.

use std::path::PathBuf;

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
// Link both backends into the bench binary so `VYRE_BACKEND=cuda` (or wgpu)
// can pin the pipeline to one backend without recompiling. Without these
// pins the inventory registrations don't get linked and `acquire_preferred_*`
// returns "no usable dispatch backend".
use vyre_driver_cuda as _;
use vyre_driver_wgpu as _;
use vyre_frontend_c::api::{
    compile, parse_syntax_batch_bytes, parse_syntax_bytes, VyreCompileOptions,
};

const TINY_TU: &str = "int main(void){return 0;}\n";

const LINUX_TU: &str = r#"
typedef unsigned long ulong_t;

struct file_operations {
    int (*read)(void *f, void *buf, ulong_t len);
    void (*release)(void *f);
};

struct file {
    struct file_operations *f_op;
    int f_flags;
};

static int demo_read(void *f, void *buf, ulong_t len)
{
    (void)f;
    (void)buf;
    (void)len;
    return 0;
}

static void demo_release(void *f)
{
    (void)f;
}

static struct file_operations demo_fops __attribute__((unused)) = {
    .read = demo_read,
    .release = demo_release,
};

static int linux_fop_open(struct file *filp)
{
    struct file local = (struct file){
        .f_op = &demo_fops,
        .f_flags = 0,
    };
    int bump = ({
        int t = local.f_flags;
        t + 3;
    });
    if (filp && filp->f_op && filp->f_op->read)
        bump += filp->f_op->read(filp, 0, 0);
    return bump;
}
"#;

fn synth_medium_tu() -> String {
    // ~10 KB by repeating a non-trivial decl/function pair with varying names.
    let mut s = String::with_capacity(12_000);
    s.push_str("typedef unsigned long ulong_t;\n");
    for i in 0..40 {
        s.push_str(&format!(
            "static int helper_{i}(int x, int y){{ int z = x * 3 + y; if (z > 7) z -= 1; return z; }}\n"
        ));
        s.push_str(&format!(
            "static struct {{ int a; ulong_t b; }} pair_{i} = {{ .a = {i}, .b = {i}UL }};\n"
        ));
    }
    s.push_str("int main(void){ int s = 0; ");
    for i in 0..40 {
        s.push_str(&format!("s += helper_{i}(s, {i});"));
    }
    s.push_str(" return s; }\n");
    s
}

fn unique_tmp(stem: &str) -> PathBuf {
    let pid = std::process::id();
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    std::env::temp_dir().join(format!("vyre-frontend-c-bench-{stem}-{pid}-{nanos}"))
}

fn run_compile_once(stem: &str, source: &str) {
    let src = unique_tmp(stem).with_extension("c");
    let out = unique_tmp(stem).with_extension("o");
    std::fs::write(&src, source).expect("write bench source");
    let mut opts = VyreCompileOptions::default();
    opts.is_compile_only = true;
    opts.disable_system_include_dirs = true;
    opts.input_files = vec![src.clone()];
    opts.output_file = Some(out.clone());
    compile(opts).expect("vyre-frontend-c compile must succeed");
    remove_bench_temp_file(&src);
    remove_bench_temp_file(&out);
}

fn run_parse_syntax_once(source: &str) {
    let summary = parse_syntax_bytes(source.as_bytes()).expect("vyre parse_syntax_bytes");
    std::hint::black_box(summary.token_count);
    std::hint::black_box(summary.ast_node_count);
}

fn run_tree_sitter_c_once(source: &str) {
    let mut parser = tree_sitter::Parser::new();
    let language: tree_sitter::Language = tree_sitter_c::LANGUAGE.into();
    parser
        .set_language(&language)
        .expect("tree-sitter-c language must load");
    let tree = parser.parse(source, None).expect("tree-sitter parse tree");
    std::hint::black_box(tree.root_node().has_error());
}

fn run_parse_syntax_batch_once(sources: &[String]) {
    let refs = sources.iter().map(String::as_bytes).collect::<Vec<_>>();
    let summary = parse_syntax_batch_bytes(&refs).expect("vyre parse_syntax_batch_bytes");
    std::hint::black_box(summary.file_count);
    std::hint::black_box(summary.token_count);
    std::hint::black_box(summary.ast_node_count);
}

fn run_tree_sitter_c_batch_once(sources: &[String]) {
    let mut parser = tree_sitter::Parser::new();
    let language: tree_sitter::Language = tree_sitter_c::LANGUAGE.into();
    parser
        .set_language(&language)
        .expect("tree-sitter-c language must load");
    let mut has_error = false;
    let mut node_count = 0usize;
    for source in sources {
        let tree = parser.parse(source, None).expect("tree-sitter parse tree");
        let root = tree.root_node();
        has_error |= root.has_error();
        node_count = node_count.saturating_add(root.descendant_count());
    }
    std::hint::black_box(has_error);
    std::hint::black_box(node_count);
}

fn synth_linux_batch(count: usize) -> Vec<String> {
    (0..count)
        .map(|i| LINUX_TU.replace("linux_fop_open", &format!("linux_fop_open_{i}")))
        .collect()
}

fn remove_bench_temp_file(path: &std::path::Path) {
    match std::fs::remove_file(path) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => panic!(
            "failed to remove bench temp file {}: {error}",
            path.display()
        ),
    }
}

fn bench_parser_pipeline(c: &mut Criterion) {
    let medium = synth_medium_tu();
    let batch64_sources = synth_linux_batch(64);
    let batch512_sources = synth_linux_batch(512);

    let mut group = c.benchmark_group("frontend.c.compile");
    group.sample_size(10);
    group.measurement_time(std::time::Duration::from_secs(20));

    for (stem, source) in [
        ("tiny", TINY_TU),
        ("linux_driver", LINUX_TU),
        ("synth_medium", medium.as_str()),
    ] {
        group.throughput(Throughput::Bytes(source.len() as u64));
        group.bench_with_input(BenchmarkId::from_parameter(stem), source, |b, src| {
            b.iter(|| run_compile_once(stem, src));
        });
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{stem}/parse_syntax_bytes")),
            source,
            |b, src| {
                b.iter(|| run_parse_syntax_once(src));
            },
        );
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{stem}/tree_sitter_c_cold")),
            source,
            |b, src| {
                b.iter(|| run_tree_sitter_c_once(src));
            },
        );
    }
    for (name, sources) in [
        ("linux_driver_batch64", &batch64_sources),
        ("linux_driver_batch512", &batch512_sources),
    ] {
        group.throughput(Throughput::Bytes(
            sources.iter().map(String::len).sum::<usize>() as u64,
        ));
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{name}/parse_syntax_batch_bytes")),
            sources,
            |b, sources| {
                b.iter(|| run_parse_syntax_batch_once(sources));
            },
        );
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{name}/tree_sitter_c_cold")),
            sources,
            |b, sources| {
                b.iter(|| run_tree_sitter_c_batch_once(sources));
            },
        );
    }
    group.finish();
}

criterion_group!(benches, bench_parser_pipeline);
criterion_main!(benches);
