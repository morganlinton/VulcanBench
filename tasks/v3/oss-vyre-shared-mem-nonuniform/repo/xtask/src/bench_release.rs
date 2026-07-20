//! R1  -  single release-bench surface.
//!
//! `cargo_full run --bin xtask -- bench-release` runs the canonical cold + warm + scale
//! + correctness sweep and prints **one number per axis** as the final
//! output. Designed to be pasted into release notes / marketing without
//! editing  -  every line is `axis_name=value units`.
//!
//! This is intentionally a thin coordinator. The actual measurement is
//! delegated to existing benches and probes (criterion harnesses in
//! `vyre-bench`, GPU dispatch latency probes in `vyre-driver-wgpu`,
//! ULP differential in `vyre-harness`). The xtask's value is collapsing
//! N marketing surfaces into one canonical entry point so v0.4.1 release
//! numbers are reproducible by anyone running one command.
//!
//! Substrate-attribution per-axis (which optimization fired and saved
//! how much) lives behind `VYRE_TRACE=1` and the substrate audit log
//! (R4  -  `DriverObservability::to_audit_log`); this xtask just surfaces
//! the headline numbers.

use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Instant;

use serde_json::Value;

use crate::benchmark_evidence_semantics::{
    benchmark_evidence_blocker_issues, cuda_release_axes_source_artifact_issues,
};

/// Axis identifier emitted in the final report. Stable string so
/// downstream graphs / regression gates can key on it.
const AXIS_WARM_US_PER_FILE: &str = "warm_us_per_file";
const AXIS_COLD_PIPELINE_BUILD_MS: &str = "cold_pipeline_build_ms";
const AXIS_GBS_SCAN_THROUGHPUT: &str = "gbs_scan_throughput";
const AXIS_ULP_DRIFT_MAX: &str = "ulp_drift_max";
const AXIS_MAX_VRAM_MIB: &str = "max_vram_mib";
const MAX_BENCH_RELEASE_REPORT_BYTES: u64 = 16_777_216;

pub(crate) fn run(args: &[String]) {
    let started = Instant::now();
    eprintln!("vyre xtask bench-release  -  canonical v0.4.1 release axes");
    eprintln!("  source: release benchmark evidence artifacts");
    eprintln!();

    let evidence_dir = parse_evidence_dir(args).unwrap_or_else(|message| fatal(&message));
    eprintln!("==> reading evidence from {}", evidence_dir.display());
    let axes = load_release_axes(&evidence_dir).unwrap_or_else(|message| fatal(&message));

    let report = ReleaseReport {
        warm_us_per_file: require_f64_axis(&axes, AXIS_WARM_US_PER_FILE)
            .unwrap_or_else(|message| fatal(&message)),
        cold_pipeline_build_ms: require_f64_axis(&axes, AXIS_COLD_PIPELINE_BUILD_MS)
            .unwrap_or_else(|message| fatal(&message)),
        gbs_scan_throughput: require_f64_axis(&axes, AXIS_GBS_SCAN_THROUGHPUT)
            .unwrap_or_else(|message| fatal(&message)),
        ulp_drift_max: require_u32_axis(&axes, AXIS_ULP_DRIFT_MAX)
            .unwrap_or_else(|message| fatal(&message)),
        max_vram_mib: require_u64_axis(&axes, AXIS_MAX_VRAM_MIB)
            .unwrap_or_else(|message| fatal(&message)),
    };

    let elapsed = started.elapsed();
    eprintln!();
    eprintln!("==> bench-release complete in {elapsed:.2?}");
    eprintln!();
    report.print_release_notes_block();
}

struct ReleaseReport {
    warm_us_per_file: f64,
    cold_pipeline_build_ms: f64,
    gbs_scan_throughput: f64,
    ulp_drift_max: u32,
    max_vram_mib: u64,
}

impl ReleaseReport {
    fn print_release_notes_block(&self) {
        println!("# vyre v0.4.1  -  bench-release axes");
        println!();
        println!("{AXIS_WARM_US_PER_FILE}={} us", self.warm_us_per_file);
        println!(
            "{AXIS_COLD_PIPELINE_BUILD_MS}={} ms",
            self.cold_pipeline_build_ms
        );
        println!(
            "{AXIS_GBS_SCAN_THROUGHPUT}={} GiB/s",
            self.gbs_scan_throughput
        );
        println!("{AXIS_ULP_DRIFT_MAX}={} ulp", self.ulp_drift_max);
        println!("{AXIS_MAX_VRAM_MIB}={} MiB", self.max_vram_mib);
    }
}

#[allow(dead_code)]
fn _ignore_exit_code_warning() -> ExitCode {
    ExitCode::SUCCESS
}

fn parse_evidence_dir(args: &[String]) -> Result<PathBuf, String> {
    let mut evidence_dir = PathBuf::from("release/evidence/benchmarks");
    let mut i = 2usize;
    while i < args.len() {
        match args[i].as_str() {
            "--evidence-dir" => {
                let value = args.get(i + 1).ok_or_else(|| {
                    "Fix: --evidence-dir requires a path to release benchmark artifacts."
                        .to_string()
                })?;
                evidence_dir = PathBuf::from(value);
                i += 2;
            }
            "--help" | "-h" => {
                println!(
                    "USAGE:\n  cargo_full run --bin xtask -- bench-release [--evidence-dir PATH]\n\n\
                     Reads release benchmark evidence and prints the canonical v0.4.1 axes.\n\
                     Generate evidence first with `cargo_full run --bin xtask -- release-benchmarks --backend cuda`."
                );
                std::process::exit(0);
            }
            other => {
                return Err(format!(
                    "Fix: unknown bench-release argument `{other}`. Use --evidence-dir PATH."
                ));
            }
        }
    }
    Ok(evidence_dir)
}

fn require_f64_axis(axes: &Value, axis: &str) -> Result<f64, String> {
    let raw = require_axis_text(axes, axis)?;
    raw.parse::<f64>().map_err(|_| {
        format!("Fix: axis `{axis}` value `{raw}` is not a floating-point benchmark number.")
    })
}

fn require_u32_axis(axes: &Value, axis: &str) -> Result<u32, String> {
    let raw = require_axis_text(axes, axis)?;
    raw.parse::<u32>()
        .map_err(|_| format!("Fix: axis `{axis}` value `{raw}` is not an unsigned integer."))
}

fn require_u64_axis(axes: &Value, axis: &str) -> Result<u64, String> {
    let raw = require_axis_text(axes, axis)?;
    raw.parse::<u64>()
        .map_err(|_| format!("Fix: axis `{axis}` value `{raw}` is not an unsigned integer."))
}

fn require_axis_text(axes: &Value, axis: &str) -> Result<String, String> {
    json_axis_text(axes, axis).ok_or_else(|| {
        format!(
            "Fix: canonical bench-release axes are missing `{axis}`. Run `cargo_full run --bin xtask -- release-benchmarks --backend cuda` with current CUDA evidence."
        )
    })
}

fn load_release_axes(evidence_dir: &Path) -> Result<Value, String> {
    let axes_path = evidence_dir.join("bench-release-axes.json");
    let axes = read_json_report(&axes_path, "canonical bench-release axes")?;
    reject_report_blockers(&axes_path, &axes)?;
    let suite_path = evidence_dir.join("cuda-release-suite.json");
    let cuda_suite = read_json_report(&suite_path, "CUDA release suite")?;
    reject_report_blockers(&suite_path, &cuda_suite)?;
    let workspace_root = workspace_root_for_evidence_dir(evidence_dir);
    let issues = cuda_release_axes_source_artifact_issues(&workspace_root, &axes, &cuda_suite);
    if let Some(first) = issues.first() {
        return Err(format!(
            "Fix: canonical bench-release axes `{}` failed CUDA source artifact validation with {} issue(s); first issue: {first}",
            axes_path.display(),
            issues.len()
        ));
    }
    Ok(axes)
}

fn read_json_report(path: &Path, label: &str) -> Result<Value, String> {
    let contents =
        read_text_bounded(path).map_err(|error| format!("Fix: cannot read {label} `{}`: {error}. Run `cargo_full run --bin xtask -- release-benchmarks --backend cuda` first.", path.display()))?;
    serde_json::from_str::<Value>(&contents)
        .map_err(|error| format!("Fix: invalid {label} JSON `{}`: {error}.", path.display()))
}

fn workspace_root_for_evidence_dir(evidence_dir: &Path) -> PathBuf {
    let benchmarks = evidence_dir;
    let evidence = benchmarks.parent();
    let release = evidence.and_then(Path::parent);
    if benchmarks.file_name().and_then(|name| name.to_str()) == Some("benchmarks")
        && evidence
            .and_then(Path::file_name)
            .and_then(|name| name.to_str())
            == Some("evidence")
        && release
            .and_then(Path::file_name)
            .and_then(|name| name.to_str())
            == Some("release")
    {
        return release
            .and_then(Path::parent)
            .map_or_else(PathBuf::new, Path::to_path_buf);
    }
    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_canonical_axes_fixture(
        benchmark_dir: &Path,
        workspace_root: &Path,
        poisoned_index: Option<usize>,
    ) -> Vec<String> {
        fs::write(workspace_root.join("Cargo.toml"), "[workspace]\n")
            .expect("Fix: write temporary workspace manifest.");
        fs::create_dir_all(benchmark_dir)
            .expect("Fix: create temporary benchmark evidence directory.");
        let git = vyre_bench::probes::capture_git_info_at(workspace_root);
        let source_fingerprint = vyre_bench::probes::source_fingerprint(&git);
        let source_tree_fingerprint =
            vyre_bench::probes::source_tree_fingerprint_at(workspace_root);
        let mut artifacts = Vec::new();
        for index in 1..=12 {
            let artifact = format!("release/evidence/benchmarks/workload-{index:02}.json");
            let selected_backend = if poisoned_index == Some(index) {
                "wgpu"
            } else {
                "cuda"
            };
            fs::write(
                workspace_root.join(&artifact),
                serde_json::to_string_pretty(&serde_json::json!({
                    "selected_backend": selected_backend,
                    "source_fingerprint": &source_fingerprint,
                    "source_tree_fingerprint": &source_tree_fingerprint,
                    "summary": {"total_cases": 1, "passed": 1, "failed": 0},
                    "cases": [
                        {
                            "id": format!("release.axis.{index:02}"),
                            "backend_id": selected_backend,
                            "status": "pass",
                            "metrics": {
                                "wall_ns": {"p50": 17_000},
                                "cold_compile_ns": {"p50": 2_000_000},
                                "wall_gb_s_x1000": {"p50": 4_000},
                                "memory_total_mib": {"p50": 24_576}
                            }
                        }
                    ]
                }))
                .expect("Fix: serialize temporary benchmark artifact."),
            )
            .expect("Fix: write temporary benchmark artifact.");
            artifacts.push(artifact);
        }
        let artifact_statuses = artifacts
            .iter()
            .map(|artifact| {
                serde_json::json!({
                    "path": artifact,
                    "blockers": []
                })
            })
            .collect::<Vec<_>>();
        fs::write(
            benchmark_dir.join("cuda-release-suite.json"),
            serde_json::to_string_pretty(&serde_json::json!({
                "schema_version": 2,
                "backend": "cuda",
                "artifacts": artifacts.clone(),
                "artifact_statuses": artifact_statuses,
                "blockers": []
            }))
            .expect("Fix: serialize temporary CUDA release suite."),
        )
        .expect("Fix: write temporary CUDA release suite.");
        fs::write(
            benchmark_dir.join("bench-release-axes.json"),
            serde_json::to_string_pretty(&serde_json::json!({
                "schema_version": 1,
                "warm_us_per_file": 17.0,
                "cold_pipeline_build_ms": 2.0,
                "gbs_scan_throughput": 4.0,
                "ulp_drift_max": 0,
                "max_vram_mib": 24576,
                "source_artifacts": artifacts,
                "blockers": []
            }))
            .expect("Fix: serialize temporary canonical release axes."),
        )
        .expect("Fix: write temporary canonical release axes.");
        artifacts
    }

    #[test]
    fn bench_release_reads_canonical_axes_instead_of_directory_decoys() {
        let dir = tempfile::TempDir::new()
            .expect("Fix: create temporary workspace for bench-release test.");
        let benchmark_dir = dir.path().join("release/evidence/benchmarks");
        write_canonical_axes_fixture(&benchmark_dir, dir.path(), None);
        fs::write(
            benchmark_dir.join("aaa-decoy-axis.json"),
            serde_json::to_string_pretty(&serde_json::json!({
                "warm_us_per_file": 0.001,
                "blockers": []
            }))
            .expect("Fix: serialize temporary decoy axis."),
        )
        .expect("Fix: write temporary decoy axis.");

        let axes = load_release_axes(&benchmark_dir)
            .expect("Fix: canonical CUDA release axes fixture should load.");

        assert_eq!(
            require_f64_axis(&axes, AXIS_WARM_US_PER_FILE),
            Ok(17.0),
            "Fix: bench-release must print the canonical bench-release-axes value, not whichever JSON directory entry exposes a top-level axis."
        );
    }

    #[test]
    fn bench_release_rejects_wgpu_source_artifacts_under_cuda_axes() {
        let dir = tempfile::TempDir::new()
            .expect("Fix: create temporary workspace for bench-release poison test.");
        let benchmark_dir = dir.path().join("release/evidence/benchmarks");
        write_canonical_axes_fixture(&benchmark_dir, dir.path(), Some(7));

        let error = load_release_axes(&benchmark_dir)
            .expect_err("Fix: WGPU artifacts must not satisfy CUDA bench-release axes.");

        assert!(
            error.contains("selected_backend must be cuda"),
            "Fix: bench-release must reject backend drift inside source_artifacts; error={error}"
        );
    }

    #[test]
    fn bench_release_rejects_case_backend_drift_under_cuda_axes() {
        let dir = tempfile::TempDir::new()
            .expect("Fix: create temporary workspace for bench-release case backend drift test.");
        let benchmark_dir = dir.path().join("release/evidence/benchmarks");
        let artifacts = write_canonical_axes_fixture(&benchmark_dir, dir.path(), None);
        let drift_path = dir.path().join(&artifacts[6]);
        let mut artifact = serde_json::from_str::<Value>(
            &fs::read_to_string(&drift_path).expect("Fix: read temporary CUDA axis artifact."),
        )
        .expect("Fix: temporary CUDA axis artifact must be JSON.");
        artifact["cases"][0]["backend_id"] = Value::String("wgpu".to_string());
        fs::write(
            &drift_path,
            serde_json::to_string_pretty(&artifact)
                .expect("Fix: serialize drifted temporary CUDA axis artifact."),
        )
        .expect("Fix: write drifted temporary CUDA axis artifact.");

        let error = load_release_axes(&benchmark_dir)
            .expect_err("Fix: WGPU cases must not satisfy CUDA bench-release axes.");

        assert!(
            error.contains("backend_id `wgpu` does not match selected_backend `cuda`"),
            "Fix: bench-release must reject case-level backend drift inside CUDA source_artifacts; error={error}"
        );
    }

    #[test]
    fn bench_release_rejects_borrowed_resident_fallback_source_artifacts() {
        let dir = tempfile::TempDir::new()
            .expect("Fix: create temporary workspace for bench-release CUDA telemetry test.");
        let benchmark_dir = dir.path().join("release/evidence/benchmarks");
        let artifacts = write_canonical_axes_fixture(&benchmark_dir, dir.path(), None);
        let polluted_path = dir.path().join(&artifacts[6]);
        let mut artifact = serde_json::from_str::<Value>(
            &fs::read_to_string(&polluted_path).expect("Fix: read temporary CUDA axis artifact."),
        )
        .expect("Fix: temporary CUDA axis artifact must be JSON.");
        artifact["cases"][0]["optimization_passes_applied"] =
            serde_json::json!(["cuda-resident-borrowed-escape-hatch"]);
        artifact["cases"][0]["metrics"]["cuda_resident_borrowed_fallback_dispatches"] =
            serde_json::json!({"p50": 2.0});
        fs::write(
            &polluted_path,
            serde_json::to_string_pretty(&artifact)
                .expect("Fix: serialize polluted temporary CUDA axis artifact."),
        )
        .expect("Fix: write polluted temporary CUDA axis artifact.");

        let error = load_release_axes(&benchmark_dir)
            .expect_err("Fix: borrowed resident fallback source artifacts must not load.");

        assert!(
            error.contains("cuda_resident_borrowed_fallback_dispatches p50=2")
                && error.contains("canonical CUDA release axes must use native resident dispatch"),
            "Fix: bench-release must reject canonical axes backed by borrowed resident fallback dispatches; error={error}"
        );
    }

    #[test]
    fn bench_release_rejects_source_artifacts_missing_axis_metrics() {
        let dir = tempfile::TempDir::new()
            .expect("Fix: create temporary workspace for bench-release axis metric test.");
        let benchmark_dir = dir.path().join("release/evidence/benchmarks");
        let artifacts = write_canonical_axes_fixture(&benchmark_dir, dir.path(), None);
        let missing_metric_path = dir.path().join(&artifacts[5]);
        let mut artifact = serde_json::from_str::<Value>(
            &fs::read_to_string(&missing_metric_path)
                .expect("Fix: read temporary CUDA axis artifact."),
        )
        .expect("Fix: temporary CUDA axis artifact must be JSON.");
        artifact["cases"][0]["metrics"]["cold_compile_ns"] = Value::Null;
        artifact["cases"][0]["metrics"]["wall_gb_s_x1000"] = Value::Null;
        artifact["environment"] = Value::Null;
        artifact["cases"][0]["metrics"]["memory_total_mib"] = Value::Null;
        fs::write(
            &missing_metric_path,
            serde_json::to_string_pretty(&artifact)
                .expect("Fix: serialize metric-poisoned temporary CUDA axis artifact."),
        )
        .expect("Fix: write metric-poisoned temporary CUDA axis artifact.");

        let error = load_release_axes(&benchmark_dir)
            .expect_err("Fix: source artifacts missing axis metrics must not load.");

        assert!(
            error.contains("has no positive p50 cold/compile metric for cold_pipeline_build_ms"),
            "Fix: bench-release must reject old clean axes when a source artifact lacks required release-axis metrics; error={error}"
        );
    }

    #[test]
    fn bench_release_rejects_axes_scalar_drift_from_source_artifacts() {
        let dir = tempfile::TempDir::new()
            .expect("Fix: create temporary workspace for bench-release axes drift test.");
        let benchmark_dir = dir.path().join("release/evidence/benchmarks");
        let artifacts = write_canonical_axes_fixture(&benchmark_dir, dir.path(), None);
        fs::write(
            benchmark_dir.join("bench-release-axes.json"),
            serde_json::to_string_pretty(&serde_json::json!({
                "schema_version": 1,
                "warm_us_per_file": 17.0,
                "cold_pipeline_build_ms": 2.0,
                "gbs_scan_throughput": 999.0,
                "ulp_drift_max": 0,
                "max_vram_mib": 24576,
                "source_artifacts": artifacts,
                "blockers": []
            }))
            .expect("Fix: serialize drifted temporary release axes."),
        )
        .expect("Fix: write drifted temporary release axes.");

        let error = load_release_axes(&benchmark_dir)
            .expect_err("Fix: drifted release axes must not load.");

        assert!(
            error.contains("gbs_scan_throughput=999 does not match source artifacts 4"),
            "Fix: bench-release must reject stale or inflated release axes values; error={error}"
        );
    }

    #[test]
    fn bench_release_rejects_axes_missing_blockers_array() {
        let dir = tempfile::TempDir::new()
            .expect("Fix: create temporary workspace for bench-release blocker schema test.");
        let benchmark_dir = dir.path().join("release/evidence/benchmarks");
        let artifacts = write_canonical_axes_fixture(&benchmark_dir, dir.path(), None);
        fs::write(
            benchmark_dir.join("bench-release-axes.json"),
            serde_json::to_string_pretty(&serde_json::json!({
                "schema_version": 1,
                "warm_us_per_file": 17.0,
                "cold_pipeline_build_ms": 2.0,
                "gbs_scan_throughput": 4.0,
                "ulp_drift_max": 0,
                "max_vram_mib": 24576,
                "source_artifacts": artifacts
            }))
            .expect("Fix: serialize blockerless temporary release axes."),
        )
        .expect("Fix: write blockerless temporary release axes.");

        let error = load_release_axes(&benchmark_dir)
            .expect_err("Fix: release axes without blockers array must not load.");

        assert!(
            error.contains("bench-release-axes.json` is missing blockers array"),
            "Fix: bench-release must fail closed when canonical axes omit blockers; error={error}"
        );
    }

    #[test]
    fn bench_release_rejects_suite_missing_artifact_statuses() {
        let dir = tempfile::TempDir::new()
            .expect("Fix: create temporary workspace for bench-release suite inventory test.");
        let benchmark_dir = dir.path().join("release/evidence/benchmarks");
        let artifacts = write_canonical_axes_fixture(&benchmark_dir, dir.path(), None);
        fs::write(
            benchmark_dir.join("cuda-release-suite.json"),
            serde_json::to_string_pretty(&serde_json::json!({
                "schema_version": 2,
                "backend": "cuda",
                "artifacts": artifacts,
                "blockers": []
            }))
            .expect("Fix: serialize temporary CUDA release suite without inventory."),
        )
        .expect("Fix: write temporary CUDA release suite without inventory.");

        let error = load_release_axes(&benchmark_dir)
            .expect_err("Fix: suite evidence without artifact_statuses must not load.");

        assert!(
            error.contains("cuda-release-suite.json` is missing artifact_statuses array"),
            "Fix: bench-release must fail closed when CUDA suite evidence omits artifact_statuses; error={error}"
        );
    }

    #[test]
    fn bench_release_rejects_suite_status_blockers() {
        let dir = tempfile::TempDir::new()
            .expect("Fix: create temporary workspace for bench-release suite blocker test.");
        let benchmark_dir = dir.path().join("release/evidence/benchmarks");
        let artifacts = write_canonical_axes_fixture(&benchmark_dir, dir.path(), None);
        fs::write(
            benchmark_dir.join("cuda-release-suite.json"),
            serde_json::to_string_pretty(&serde_json::json!({
                "schema_version": 2,
                "backend": "cuda",
                "artifacts": artifacts,
                "artifact_statuses": [
                    {
                        "path": "release/evidence/benchmarks/workload-01.json",
                        "blockers": ["case `release.condition_eval.1m` failed: wrong answer"]
                    }
                ],
                "blockers": []
            }))
            .expect("Fix: serialize temporary CUDA release suite with nested blocker."),
        )
        .expect("Fix: write temporary CUDA release suite with nested blocker.");

        let error = load_release_axes(&benchmark_dir)
            .expect_err("Fix: suite evidence with nested blockers must not load.");

        assert!(
            error.contains("artifact_statuses[0]")
                && error.contains("case `release.condition_eval.1m` failed: wrong answer"),
            "Fix: bench-release must reject CUDA suite status rows that carry blockers; error={error}"
        );
    }

    #[test]
    fn bench_release_rejects_suite_status_inventory_drift() {
        let dir = tempfile::TempDir::new().expect(
            "Fix: create temporary workspace for bench-release suite inventory drift test.",
        );
        let benchmark_dir = dir.path().join("release/evidence/benchmarks");
        let artifacts = write_canonical_axes_fixture(&benchmark_dir, dir.path(), None);
        let mut artifact_statuses = artifacts
            .iter()
            .map(|artifact| {
                serde_json::json!({
                    "path": artifact,
                    "blockers": []
                })
            })
            .collect::<Vec<_>>();
        artifact_statuses[11]["path"] =
            Value::String("release/evidence/benchmarks/wgpu-workload-12.json".to_string());
        fs::write(
            benchmark_dir.join("cuda-release-suite.json"),
            serde_json::to_string_pretty(&serde_json::json!({
                "schema_version": 2,
                "backend": "cuda",
                "artifacts": artifacts,
                "artifact_statuses": artifact_statuses,
                "blockers": []
            }))
            .expect("Fix: serialize temporary CUDA release suite with inventory drift."),
        )
        .expect("Fix: write temporary CUDA release suite with inventory drift.");

        let error = load_release_axes(&benchmark_dir)
            .expect_err("Fix: suite evidence with status inventory drift must not load.");

        assert!(
            error.contains(
                "cuda-release-suite lists artifact `release/evidence/benchmarks/workload-12.json` without matching artifact_statuses entry"
            ),
            "Fix: bench-release must reject CUDA suite inventory drift before printing axes; error={error}"
        );
    }

    #[test]
    fn bench_release_rejects_mislabeled_cuda_suite_backend() {
        let dir = tempfile::TempDir::new()
            .expect("Fix: create temporary workspace for bench-release suite backend test.");
        let benchmark_dir = dir.path().join("release/evidence/benchmarks");
        let artifacts = write_canonical_axes_fixture(&benchmark_dir, dir.path(), None);
        fs::write(
            benchmark_dir.join("cuda-release-suite.json"),
            serde_json::to_string_pretty(&serde_json::json!({
                "schema_version": 2,
                "backend": "wgpu",
                "artifacts": artifacts,
                "artifact_statuses": [
                    {
                        "path": "release/evidence/benchmarks/workload-01.json",
                        "blockers": []
                    }
                ],
                "blockers": []
            }))
            .expect("Fix: serialize mislabeled temporary CUDA release suite."),
        )
        .expect("Fix: write mislabeled temporary CUDA release suite.");

        let error = load_release_axes(&benchmark_dir)
            .expect_err("Fix: mislabeled CUDA release suites must not satisfy bench-release axes.");

        assert!(
            error.contains("cuda-release-suite backend `wgpu` does not match required `cuda`"),
            "Fix: bench-release must reject suite backend identity drift even when old axes have no blockers; error={error}"
        );
    }
}

fn reject_report_blockers(path: &Path, value: &Value) -> Result<(), String> {
    let evidence = path.to_string_lossy();
    let blockers = benchmark_evidence_blocker_issues(&evidence, value);
    match blockers.first() {
        Some(first) => Err(format!(
            "Fix: benchmark evidence `{}` reports {} blocker contract issue(s); first issue: {first}",
            path.display(),
            blockers.len()
        )),
        None => Ok(()),
    }
}

fn json_axis_text(value: &Value, axis: &str) -> Option<String> {
    match value.get(axis)? {
        Value::Number(number) => Some(number.to_string()),
        Value::String(text) if !text.trim().is_empty() => Some(text.clone()),
        _ => None,
    }
}

fn fatal(message: &str) -> ! {
    eprintln!("error: {message}");
    std::process::exit(1);
}

fn read_text_bounded(path: &Path) -> io::Result<String> {
    let mut reader = fs::File::open(path)?.take(MAX_BENCH_RELEASE_REPORT_BYTES.saturating_add(1));
    let mut text = String::new();
    reader.read_to_string(&mut text)?;
    if text.len() as u64 > MAX_BENCH_RELEASE_REPORT_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "{} exceeds {MAX_BENCH_RELEASE_REPORT_BYTES} byte release bench report read cap",
                path.display()
            ),
        ));
    }
    Ok(text)
}
