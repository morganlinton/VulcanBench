//! Release-evidence validation tests for differential preprocessing benchmark reports.

use vyre_frontend_c::api::{
    PreprocessBenchmarkGpuCounters, PreprocessBenchmarkTranslationUnit,
    PreprocessDifferentialBenchmarkReport,
};

#[test]
fn preprocess_benchmark_report_validates_against_frozen_linux_manifest() {
    let manifest: toml::Value = toml::from_str(include_str!("../parity/linux_math_v6_8.toml"))
        .expect("release parity manifest parses");
    let sources = manifest["files"]["sources"]
        .as_array()
        .expect("manifest source list exists");
    let units = sources
        .iter()
        .map(|source| PreprocessBenchmarkTranslationUnit {
            path: source.as_str().expect("source path is string").to_string(),
            input_bytes: 100,
            clang_output_bytes: 80,
            vyre_output_bytes: 80,
            clang_wall_ns: 1_000,
            vyre_wall_ns: 500,
        })
        .collect::<Vec<_>>();
    let report = PreprocessDifferentialBenchmarkReport {
        target_id: manifest["id"].as_str().expect("target id").to_string(),
        source_commit: manifest["commit"].as_str().expect("commit").to_string(),
        target_triple: manifest["target_triple"]
            .as_str()
            .expect("target triple")
            .to_string(),
        clang_version: "clang synthetic".to_string(),
        vyre_version: env!("CARGO_PKG_VERSION").to_string(),
        translation_units: units,
        gpu: PreprocessBenchmarkGpuCounters {
            kernel_launch_count: 12,
            host_write_bytes: 4096,
            host_readback_bytes: 2048,
            host_sync_points: 12,
        },
    };

    report
        .validate_release_evidence(
            manifest["id"].as_str().expect("target id"),
            manifest["commit"].as_str().expect("commit"),
            sources.len(),
        )
        .expect("report satisfies release evidence shape");
    assert_eq!(report.total_input_bytes(), 1200);
    assert!(report.clang_bytes_per_second() > 0);
    assert!(report.vyre_bytes_per_second() > report.clang_bytes_per_second());
}

#[test]
fn preprocess_benchmark_report_rejects_missing_gpu_counters() {
    let report = PreprocessDifferentialBenchmarkReport {
        target_id: "linux-lib-math-v6.8".to_string(),
        source_commit: "90d1f30371ae3337beb01666b226320728d35c70".to_string(),
        target_triple: "x86_64-unknown-linux-gnu".to_string(),
        clang_version: "clang synthetic".to_string(),
        vyre_version: env!("CARGO_PKG_VERSION").to_string(),
        translation_units: vec![PreprocessBenchmarkTranslationUnit {
            path: "lib/math/gcd.c".to_string(),
            input_bytes: 100,
            clang_output_bytes: 80,
            vyre_output_bytes: 80,
            clang_wall_ns: 1_000,
            vyre_wall_ns: 500,
        }],
        gpu: PreprocessBenchmarkGpuCounters {
            kernel_launch_count: 0,
            host_write_bytes: 0,
            host_readback_bytes: 0,
            host_sync_points: 0,
        },
    };

    let err = report
        .validate_release_evidence(
            "linux-lib-math-v6.8",
            "90d1f30371ae3337beb01666b226320728d35c70",
            1,
        )
        .expect_err("zero GPU counters must reject release evidence");
    assert!(err.contains("zero GPU launches"), "{err}");
}
