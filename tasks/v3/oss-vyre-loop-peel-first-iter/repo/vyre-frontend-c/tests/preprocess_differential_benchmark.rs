//! Differential clang-vs-vyre preprocessing benchmark harness for release-plan item 30.

use std::path::PathBuf;
use std::time::Instant;

#[allow(unused_imports)]
use vyre_driver_cuda as _;
#[allow(unused_imports)]
use vyre_driver_wgpu as _;

use vyre_libs::parsing::c::preprocess::gpu_pipeline::gpu_preprocess_translation_unit;

#[path = "preprocess_differential_benchmark/preprocess_support.rs"]
mod preprocess_support;

use preprocess_support::{
    assert_required_preprocess_speedup, bytes_per_second, clang_command,
    clang_kernel_predefined_macros, clang_preprocess, format_report, linux_include_roots,
    CountingGpuDispatcher, DifferentialPreprocessBenchmarkReport, FilesystemLoader,
};

#[test]
fn differential_preprocess_benchmark_reports_clang_vyre_and_gpu_counters() {
    let manifest: toml::Value = toml::from_str(include_str!("../parity/linux_math_v6_8.toml"))
        .expect("release parity manifest parses");
    let target_id = manifest["id"]
        .as_str()
        .expect("manifest id exists")
        .to_string();
    let subsystem_translation_units = manifest["files"]["sources"]
        .as_array()
        .expect("manifest source list exists")
        .len();

    let root = std::env::temp_dir().join(format!("vyre-preprocess-bench-{}", std::process::id()));
    std::fs::create_dir_all(&root).expect("benchmark temp dir exists");
    let header = root.join("bench.h");
    let source = root.join("tu.c");
    std::fs::write(
        &header,
        concat!(
            "#pragma once\n",
            "#define SCALE 21\n",
            "int header_value;\n",
        ),
    )
    .expect("write benchmark header");
    let source_bytes = concat!(
        "#include \"bench.h\"\n",
        "#include \"bench.h\"\n",
        "#if SCALE\n",
        "int scaled_value = SCALE;\n",
        "#endif\n",
    )
    .as_bytes()
    .to_vec();
    std::fs::write(&source, &source_bytes).expect("write benchmark source");

    let clang_start = Instant::now();
    let clang = clang_command()
        .arg("-E")
        .arg("-P")
        .arg("-x")
        .arg("c")
        .arg("-I")
        .arg(&root)
        .arg(&source)
        .output()
        .expect("clang must be installed for differential preprocessing benchmark");
    let clang_wall_ns = clang_start.elapsed().as_nanos() as u64;
    assert!(
        clang.status.success(),
        "clang preprocessing failed: {}",
        String::from_utf8_lossy(&clang.stderr)
    );

    let backend =
        vyre::backend::acquire_preferred_dispatch_backend().expect("GPU backend must be available");
    let dispatcher = CountingGpuDispatcher::new(backend.as_ref());
    let loader = FilesystemLoader::new(vec![root.clone()]);

    let vyre_start = Instant::now();
    let vyre = gpu_preprocess_translation_unit(&dispatcher, &loader, &source, &source_bytes, &[])
        .expect("vyre GPU preprocessing succeeds");
    let vyre_wall_ns = vyre_start.elapsed().as_nanos() as u64;
    let counters = dispatcher.counters();
    let corpus_bytes = source_bytes.len() as u64 + loader.loaded_include_bytes();

    let report = DifferentialPreprocessBenchmarkReport {
        target_id,
        subsystem_translation_units,
        corpus_bytes,
        clang_wall_ns,
        vyre_wall_ns,
        clang_bytes_per_second: bytes_per_second(corpus_bytes, clang_wall_ns),
        vyre_bytes_per_second: bytes_per_second(corpus_bytes, vyre_wall_ns),
        gpu: counters,
    };

    let clang_text = String::from_utf8_lossy(&clang.stdout);
    let vyre_text = String::from_utf8_lossy(&vyre.bytes);
    assert!(clang_text.contains("scaled_value"));
    assert!(vyre_text.contains("scaled_value"));
    assert!(clang_text.contains("21"));
    assert!(vyre_text.contains("21"));
    assert!(vyre
        .include_acceleration_events
        .iter()
        .any(|event| event.skipped_include));
    report.validate();
}

#[test]
#[ignore = "requires VYRE_LINUX_V68_ROOT pointing at Linux v6.8 source tree"]
fn full_linux_lib_math_preprocess_benchmark_report_when_root_is_configured() {
    let manifest: toml::Value = toml::from_str(include_str!("../parity/linux_math_v6_8.toml"))
        .expect("release parity manifest parses");
    let root = std::env::var_os("VYRE_LINUX_V68_ROOT")
        .map(PathBuf::from)
        .expect("set VYRE_LINUX_V68_ROOT to the Linux v6.8 source root");
    let target_id = manifest["id"]
        .as_str()
        .expect("manifest id exists")
        .to_string();
    let mut sources = manifest["files"]["sources"]
        .as_array()
        .expect("manifest source list exists")
        .iter()
        .map(|value| {
            value
                .as_str()
                .expect("source path must be string")
                .to_string()
        })
        .collect::<Vec<_>>();
    let full_source_count = sources.len();
    if let Ok(max_tus) = std::env::var("VYRE_LINUX_V68_MAX_TUS") {
        let max_tus = max_tus
            .parse::<usize>()
            .expect("VYRE_LINUX_V68_MAX_TUS must be a positive integer");
        assert!(max_tus > 0, "VYRE_LINUX_V68_MAX_TUS must be positive");
        sources.truncate(max_tus);
    }
    let include_roots = linux_include_roots(&root);
    let backend =
        vyre::backend::acquire_preferred_dispatch_backend().expect("GPU backend must be available");
    let dispatcher = CountingGpuDispatcher::new(backend.as_ref());
    let loader = FilesystemLoader::new(include_roots.clone());
    let kernel_macros = clang_kernel_predefined_macros();

    let mut corpus_bytes = 0_u64;
    let clang_start = Instant::now();
    let mut clang_output_bytes = 0_u64;
    for source in &sources {
        let path = root.join(source);
        let output = clang_preprocess(&root, &include_roots, &path);
        clang_output_bytes = clang_output_bytes.saturating_add(output.len() as u64);
    }
    let clang_wall_ns = clang_start.elapsed().as_nanos() as u64;

    let vyre_start = Instant::now();
    let mut vyre_output_bytes = 0_u64;
    for source in &sources {
        let path = root.join(source);
        let source_bytes =
            std::fs::read(&path).unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
        let mut bytes = b"#include <linux/kconfig.h>\n".to_vec();
        bytes.extend_from_slice(&source_bytes);
        let preprocessed =
            gpu_preprocess_translation_unit(&dispatcher, &loader, &path, &bytes, &kernel_macros)
                .unwrap_or_else(|error| panic!("vyre preprocess {}: {error}", path.display()));
        vyre_output_bytes = vyre_output_bytes.saturating_add(preprocessed.bytes.len() as u64);
    }
    corpus_bytes = corpus_bytes
        .saturating_add(
            sources
                .iter()
                .map(|source| {
                    let path = root.join(source);
                    let source_len = std::fs::metadata(&path)
                        .unwrap_or_else(|error| panic!("metadata {}: {error}", path.display()))
                        .len();
                    source_len + b"#include <linux/kconfig.h>\n".len() as u64
                })
                .sum::<u64>(),
        )
        .saturating_add(loader.loaded_include_bytes());
    let vyre_wall_ns = vyre_start.elapsed().as_nanos() as u64;
    let counters = dispatcher.counters();
    let report = DifferentialPreprocessBenchmarkReport {
        target_id,
        subsystem_translation_units: sources.len(),
        corpus_bytes,
        clang_wall_ns,
        vyre_wall_ns,
        clang_bytes_per_second: bytes_per_second(corpus_bytes, clang_wall_ns),
        vyre_bytes_per_second: bytes_per_second(corpus_bytes, vyre_wall_ns),
        gpu: counters,
    };

    if std::env::var_os("VYRE_LINUX_V68_MAX_TUS").is_none() {
        assert_eq!(sources.len(), 12);
    }
    assert!(clang_output_bytes > 0);
    assert!(vyre_output_bytes > 0);
    if sources.len() == full_source_count {
        report.validate();
        assert_required_preprocess_speedup(&report);
    }
    eprintln!("{}", format_report(&report));
    if std::env::var_os("VYRE_PREPROC_OP_COUNTS").is_some() {
        eprintln!(
            "{}",
            dispatcher
                .format_top_ops(40)
                .expect("format benchmark op counts")
        );
    }
}
