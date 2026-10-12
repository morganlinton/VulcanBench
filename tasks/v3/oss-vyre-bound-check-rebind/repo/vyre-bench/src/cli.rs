#![allow(missing_docs)]

use clap::{Parser, Subcommand};
#[cfg(test)]
use std::collections::BTreeMap;

use crate::api::suite::SuiteKind;
#[cfg(test)]
use crate::report::json::ReportSchema;
use crate::runner::{execute_suite, RunConfig};

#[path = "cli/bundle.rs"]
mod cli_bundle;
#[path = "cli/dashboard.rs"]
mod cli_dashboard;
#[path = "cli/compare.rs"]
mod cli_compare;
#[path = "cli/registry.rs"]
mod cli_registry;
#[path = "cli/report_io.rs"]
mod cli_report_io;
#[path = "cli/run.rs"]
mod cli_run;
#[cfg(not(test))]
use cli_bundle::validate_benchmark_bundle;
#[cfg(test)]
use cli_bundle::*;
use cli_dashboard::generate_dashboard;
#[cfg(test)]
use cli_dashboard::{generate_index_html, generate_scorecard_md};
use cli_compare::{compare_reports, load_comparison_artifact, validate_comparison_expectations};
#[cfg(test)]
use cli_compare::{build_comparison_artifact, write_comparison_artifact};
use cli_registry::{explain_case, list_cases};
use cli_report_io::{load_report, validate_report_expectations};
use cli_run::{execute_run_matrix, write_run_reports};

#[derive(Parser)]
#[command(name = "vyre-bench")]
#[command(about = "Canonical performance and evolution harness for Vyre", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Run {
        #[arg(long)]
        suite: String,
        #[arg(long, default_value = "table")]
        format: String,
        #[arg(long)]
        backend: Option<String>,
        #[arg(long)]
        enforce_budgets: bool,
        #[arg(long = "case")]
        case_ids: Vec<String>,
        #[arg(long, default_value_t = 3)]
        warmup_samples: usize,
        #[arg(long)]
        measured_samples: Option<usize>,
        #[arg(long, default_value_t = 30)]
        sample_timeout_secs: u64,
        #[arg(long)]
        snapshot_on_pass: bool,
        #[arg(long, default_value_t = 1)]
        determinism_runs: usize,
        #[arg(long)]
        workgroup_size: Option<u32>,
        #[arg(long)]
        roofline_only: bool,
        #[arg(long)]
        output: Option<String>,
    },
    Compare {
        #[arg(long)]
        baseline: String,
        #[arg(long)]
        candidate: String,
        #[arg(long)]
        output: Option<String>,
    },
    ValidateReport {
        #[arg(long)]
        path: String,
        #[arg(long)]
        backend: Option<String>,
        #[arg(long)]
        total_cases: Option<usize>,
        #[arg(long)]
        failed: Option<usize>,
    },
    ValidateComparison {
        #[arg(long)]
        path: String,
        #[arg(long)]
        baseline_backend: String,
        #[arg(long)]
        candidate_backend: String,
        #[arg(long = "case")]
        case_ids: Vec<String>,
    },
    ValidateBenchmarkBundle {
        #[arg(long)]
        dir: String,
        #[arg(long)]
        manifest_output: Option<String>,
        #[arg(long)]
        manifest_input: Option<String>,
    },
    SnapshotDiff {
        #[arg(long)]
        base: String,
    },
    List {
        #[arg(long, default_value = "table")]
        format: String,
    },
    Explain {
        id: String,
    },
    Dashboard {
        #[arg(long, default_value = "dashboard")]
        output: String,
    },
    ReleaseMatrix {
        #[arg(long, default_value = "table")]
        format: String,
        #[arg(long)]
        output: Option<String>,
        #[arg(long)]
        enforce: bool,
    },
    EvolveServer,
}

pub fn run_cli() -> anyhow::Result<()> {
    env_logger::init();
    run_cli_with(std::env::args_os())
}

pub fn run_cli_with<I, T>(args: I) -> anyhow::Result<()>
where
    I: IntoIterator<Item = T>,
    T: Into<std::ffi::OsString> + Clone,
{
    crate::link_benchmark_backend_registrations();
    let cli = Cli::parse_from(args);
    match &cli.command {
        Commands::Run {
            suite,
            format,
            backend,
            enforce_budgets,
            case_ids,
            warmup_samples,
            measured_samples,
            sample_timeout_secs,
            snapshot_on_pass,
            determinism_runs,
            workgroup_size,
            roofline_only,
            output,
        } => {
            let suite_kind: SuiteKind = suite
                .parse()
                .map_err(|error: String| anyhow::anyhow!("{error}"))?;
            let registry = crate::registry::collect_all();
            let config = RunConfig {
                backend_id: backend.clone(),
                enforce_budgets: *enforce_budgets,
                case_ids: case_ids.clone(),
                warmup_samples: *warmup_samples,
                measured_samples: *measured_samples,
                sample_timeout: std::time::Duration::from_secs(*sample_timeout_secs),
                determinism_runs: *determinism_runs,
                workgroup_override: workgroup_size.map(|size| [size, 1, 1]),
                baseline_warmup_runs: 0,
                snapshot_on_pass: *snapshot_on_pass,
            };
            let reports = execute_run_matrix(&registry, suite_kind, &config)?;
            if let Some(output) = output {
                write_run_reports(&reports, output)?;
            }
            for report in &reports {
                crate::runner::print_report(report, format, *roofline_only)?;
            }

            let failed: usize = reports.iter().map(|report| report.summary.failed).sum();
            if failed > 0 {
                anyhow::bail!("{failed} benchmark case(s) failed");
            }
        }
        Commands::Compare {
            baseline,
            candidate,
            output,
        } => {
            let baseline_report = load_report(baseline)?;
            let candidate_report = load_report(candidate)?;
            compare_reports(&baseline_report, &candidate_report, output.as_deref())?;
        }
        Commands::ValidateReport {
            path,
            backend,
            total_cases,
            failed,
        } => {
            let report = load_report(path)?;
            validate_report_expectations(&report, backend.as_deref(), *total_cases, *failed)?;
            let selected = report.selected_backend.as_deref().unwrap_or("unknown");
            let timing_quality = report
                .backend_profile
                .as_ref()
                .map(|profile| profile.timing_quality.as_str())
                .unwrap_or("unknown");
            println!(
                "report_valid path={} selected_backend={} timing_quality={}",
                path, selected, timing_quality
            );
        }
        Commands::ValidateComparison {
            path,
            baseline_backend,
            candidate_backend,
            case_ids,
        } => {
            let comparison = load_comparison_artifact(path)?;
            validate_comparison_expectations(
                &comparison,
                baseline_backend,
                candidate_backend,
                case_ids,
            )?;
            println!(
                "comparison_valid path={} baseline_backend={} candidate_backend={} cases={}",
                path,
                comparison.baseline.profile_backend,
                comparison.candidate.profile_backend,
                comparison.cases.len()
            );
        }
        Commands::ValidateBenchmarkBundle {
            dir,
            manifest_output,
            manifest_input,
        } => {
            let manifest = validate_benchmark_bundle(
                dir,
                manifest_output.as_deref(),
                manifest_input.as_deref(),
            )?;
            println!(
                "benchmark_bundle_valid dir={} artifacts={} bundle_blake3={}",
                dir, manifest.artifact_count, manifest.bundle_blake3
            );
        }
        Commands::SnapshotDiff { base } => {
            let snapshots_dir = std::path::Path::new("snapshots");
            let path = snapshots_dir.join(format!("{}.json", base));
            if !path.exists() {
                anyhow::bail!("snapshot for commit `{}` not found in snapshots/", base);
            }
            let baseline_report = load_report(&path.to_string_lossy())?;
            let registry = crate::registry::collect_all();
            let config = RunConfig::default();
            let current_report = execute_suite(&registry, SuiteKind::Release, &config);
            compare_reports(&baseline_report, &current_report, None)?;
        }
        Commands::List { format } => list_cases(format)?,
        Commands::Explain { id } => explain_case(id)?,
        Commands::Dashboard { output } => generate_dashboard(output)?,
        Commands::ReleaseMatrix {
            format,
            output,
            enforce,
        } => {
            let registry = crate::registry::collect_all();
            let matrix = crate::release_matrix::build_release_matrix(&registry);
            crate::release_matrix::emit_release_matrix(&matrix, format, output.as_deref())?;
            if *enforce {
                crate::release_matrix::enforce_release_matrix(&matrix)?;
            }
        }
        Commands::EvolveServer => crate::evolve::server::run_evolve_server()?,
    }
    Ok(())
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::case::{Correctness, PerformanceEvaluation};
    use crate::api::metric::MetricStats;
    use crate::probes::environment::EnvironmentData;
    use crate::report::json::{CaseReport, ReportBackendProfile, ReportSummary};

    fn report(cases: Vec<CaseReport>, passed: usize, failed: usize) -> ReportSchema {
        ReportSchema {
            schema: "vyre-bench.result.v1".to_string(),
            run_id: "vyre-bench.release".to_string(),
            suite: "release".to_string(),
            selected_backend: Some("cuda".to_string()),
            backend_profile: None,
            git: BTreeMap::new(),
            source_fingerprint: "source:unit".to_string(),
            source_tree_fingerprint: "tree:unit".to_string(),
            environment: EnvironmentData {
                os: "linux".to_string(),
                architecture: "x86_64".to_string(),
                cpu_model: Some("unit".to_string()),
                cpu_cores: 1,
                has_gpu: true,
                gpu_devices: Vec::new(),
                nvidia_driver_version: Some("unit".to_string()),
                nvidia_cuda_version: Some("unit".to_string()),
                features: vec!["backend.usable.cuda".to_string()],
            },
            features: vec!["backend:cuda".to_string()],
            summary: ReportSummary {
                total_cases: cases.len(),
                passed,
                failed,
                total_time_ns: 1,
                cache_hit_rate: None,
            },
            cases,
            blockers: Vec::new(),
        }
    }

    fn backend_profile(backend: &str, timing_quality: &str) -> ReportBackendProfile {
        ReportBackendProfile {
            backend: backend.to_string(),
            timing_quality: timing_quality.to_string(),
            supports_device_timestamps: timing_quality == "device_timestamps",
            supports_hardware_counters: timing_quality == "hardware_counters",
            supports_subgroup_ops: false,
            supports_indirect_dispatch: false,
            max_workgroup_size: [1, 1, 1],
            max_invocations_per_workgroup: 1,
            max_shared_memory_bytes: 0,
            max_storage_buffer_binding_size: 0,
            subgroup_size: 0,
            compute_units: 0,
            mem_bw_gbps: 0,
        }
    }

    fn case_report(id: &str, status: &str, contract_passed: bool) -> CaseReport {
        CaseReport {
            id: id.to_string(),
            workload_fingerprint: format!("bench-case:{id}"),
            name: id.to_string(),
            owner_crate: "vyre-bench".to_string(),
            workload_class: "Release".to_string(),
            tags: Vec::new(),
            backend_id: Some("cuda".to_string()),
            device_signature: Some("device-profile-v1:test".to_string()),
            held_out_corpus_id: Some(format!("heldout:bench-case:{id}")),
            needs_gpu: true,
            min_vram_bytes: None,
            min_input_bytes: None,
            required_features: Vec::new(),
            status: status.to_string(),
            wall_ns: Some(1.0),
            correctness: Correctness::Exact,
            contract: None,
            performance: Some(PerformanceEvaluation {
                speedup_x: Some(100.0),
                contract_passed,
                violations: if contract_passed {
                    Vec::new()
                } else {
                    vec!["speedup below release floor".to_string()]
                },
            }),
            metrics: benchmark_evidence_metrics(),
            optimization_passes_applied: Vec::new(),
            artifacts: Vec::new(),
        }
    }

    fn benchmark_evidence_metrics() -> BTreeMap<String, MetricStats> {
        let mut metrics = BTreeMap::new();
        for (name, value) in [
            ("cpu_digest", 1),
            ("gpu_digest", 1),
            ("active_time_ns", 10),
            ("transfer_bytes", 64),
        ] {
            metrics.insert(name.to_string(), wall_stats(value, value as f64, 0.0, 1));
        }
        metrics
    }

    fn wall_stats(p50: u64, mean: f64, stddev: f64, samples: u32) -> MetricStats {
        MetricStats {
            min: p50,
            p50,
            p90: p50,
            p95: p50,
            p99: p50,
            p999: p50,
            p9999: p50,
            max: p50,
            mean,
            stddev,
            samples,
            determinism_cv: None,
        }
    }

    fn case_report_with_wall(id: &str, p50: u64, mean: f64) -> CaseReport {
        let mut case = case_report(id, "pass", true);
        case.metrics
            .insert("wall_ns".to_string(), wall_stats(p50, mean, 1.0, 3));
        case
    }

    fn comparison_reports() -> (ReportSchema, ReportSchema) {
        let case_id = "foundation.elementwise.add.1m";
        let mut baseline = report(vec![case_report_with_wall(case_id, 100, 100.0)], 1, 0);
        baseline.suite = "smoke".to_string();
        baseline.selected_backend = Some("wgpu".to_string());
        baseline.backend_profile = Some(backend_profile("wgpu", "host_enqueue_wait"));
        let mut candidate = report(vec![case_report_with_wall(case_id, 90, 90.0)], 1, 0);
        candidate.suite = "smoke".to_string();
        candidate.selected_backend = Some("metal".to_string());
        candidate.backend_profile = Some(backend_profile("metal", "host_enqueue_wait"));
        (baseline, candidate)
    }

    fn unique_temp_dir(label: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "vyre-bench-{label}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system clock must be after UNIX_EPOCH")
                .as_nanos()
        ))
    }

    fn write_report(path: &std::path::Path, report: &ReportSchema) {
        std::fs::write(
            path,
            serde_json::to_vec(report).expect("test report should serialize"),
        )
        .expect("test report should be writable");
    }

    fn write_complete_benchmark_bundle(dir: &std::path::Path) {
        std::fs::create_dir_all(dir).expect("test bundle dir should be creatable");
        let case_id = "foundation.elementwise.add.1m";
        let mut cpu_ref = report(vec![case_report_with_wall(case_id, 120, 120.0)], 1, 0);
        cpu_ref.suite = "smoke".to_string();
        cpu_ref.selected_backend = Some("cpu-ref".to_string());
        cpu_ref.backend_profile = Some(backend_profile("cpu-ref", "host_only"));
        write_report(&dir.join("cpu-ref.json"), &cpu_ref);

        let (wgpu, metal) = comparison_reports();
        write_report(&dir.join("wgpu.json"), &wgpu);
        write_report(&dir.join("metal.json"), &metal);

        let comparison = build_comparison_artifact(&wgpu, &metal)
            .expect("test comparison artifact should build");
        write_comparison_artifact(
            &comparison,
            &dir.join("wgpu-vs-metal.json").to_string_lossy(),
        )
        .expect("test comparison artifact should be writable");
        std::fs::write(
            dir.join("wgpu-vs-metal.txt"),
            "baseline_backend=wgpu\ncandidate_backend=metal\nbaseline_selected_backend=wgpu baseline_profile_backend=wgpu baseline_timing_quality=host_enqueue_wait\ncandidate_selected_backend=metal candidate_profile_backend=metal candidate_timing_quality=host_enqueue_wait\nfoundation.elementwise.add.1m\ncompare_exit_code=0\n",
        )
        .expect("test comparison text should be writable");
        let ref_comparison = build_comparison_artifact(&cpu_ref, &metal)
            .expect("test reference comparison artifact should build");
        write_comparison_artifact(
            &ref_comparison,
            &dir.join("cpu-ref-vs-metal.json").to_string_lossy(),
        )
        .expect("test reference comparison artifact should be writable");
        std::fs::write(
            dir.join("cpu-ref-vs-metal.txt"),
            "baseline_backend=cpu-ref\ncandidate_backend=metal\nbaseline_selected_backend=cpu-ref baseline_profile_backend=cpu-ref baseline_timing_quality=host_only\ncandidate_selected_backend=metal candidate_profile_backend=metal candidate_timing_quality=host_enqueue_wait\nfoundation.elementwise.add.1m\ncompare_exit_code=0\n",
        )
        .expect("test reference comparison text should be writable");
    }

    fn write_manifest_variant<F>(
        dir: &std::path::Path,
        label: &str,
        mutate: F,
    ) -> std::path::PathBuf
    where
        F: FnOnce(&mut BenchmarkBundleManifest),
    {
        let mut manifest = validate_benchmark_bundle(&dir.to_string_lossy(), None, None)
            .expect("Fix: complete benchmark bundle should produce a manifest model.");
        mutate(&mut manifest);
        let manifest = build_benchmark_bundle_manifest(
            manifest.artifacts.clone(),
            manifest.provenance.clone(),
        )
        .expect("Fix: mutated manifest should be hashable for schema-negative tests.");
        let path = dir.join(format!("{label}.json"));
        std::fs::write(
            &path,
            serde_json::to_vec_pretty(&manifest).expect("Fix: manifest should serialize"),
        )
        .expect("Fix: manifest variant should be writable");
        path
    }

    fn expect_manifest_artifact_set_error<F>(label: &str, mutate: F, expected: &str)
    where
        F: FnOnce(&mut BenchmarkBundleManifest),
    {
        let dir = unique_temp_dir(label);
        write_complete_benchmark_bundle(&dir);
        let manifest_path = write_manifest_variant(&dir, "bundle-manifest-mutated", mutate);
        let error = validate_benchmark_bundle(
            &dir.to_string_lossy(),
            None,
            Some(&manifest_path.to_string_lossy()),
        )
        .expect_err("Fix: mutated manifest artifact set should be rejected.");
        let _ = std::fs::remove_dir_all(&dir);
        let error = error.to_string();
        assert!(
            error.contains(expected),
            "Fix: manifest artifact-set error should contain `{expected}`: {error}"
        );
    }

    #[test]
    fn compare_writes_structured_profile_artifact() {
        let (baseline, candidate) = comparison_reports();
        let path = std::env::temp_dir().join(format!(
            "vyre-bench-compare-{}-{}.json",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system clock must be after UNIX_EPOCH")
                .as_nanos()
        ));
        let path_arg = path.to_string_lossy().into_owned();

        compare_reports(&baseline, &candidate, Some(&path_arg))
            .expect("Fix: non-regressed comparison must write an artifact and succeed.");

        let artifact = load_comparison_artifact(&path_arg)
            .expect("Fix: comparison artifact must deserialize through the benchmark loader.");
        let _ = std::fs::remove_file(&path);
        validate_comparison_expectations(
            &artifact,
            "wgpu",
            "metal",
            &["foundation.elementwise.add.1m".to_string()],
        )
        .expect("Fix: comparison artifact must validate profile backend and case evidence.");
        assert_eq!(artifact.schema, "vyre-bench.compare.v1");
        assert_eq!(artifact.baseline.profile_backend, "wgpu");
        assert_eq!(artifact.candidate.profile_backend, "metal");
        assert_eq!(artifact.cases[0].baseline_p50_ns, 100);
        assert_eq!(artifact.cases[0].candidate_p50_ns, 90);
        assert!(!artifact.regressed);
    }

    #[test]
    fn validate_benchmark_bundle_accepts_complete_mac_gate_artifacts() {
        let dir = unique_temp_dir("bundle-ok");
        write_complete_benchmark_bundle(&dir);
        let manifest_path = dir.join("bundle-manifest.json");

        let manifest = validate_benchmark_bundle(
            &dir.to_string_lossy(),
            Some(&manifest_path.to_string_lossy()),
            None,
        )
        .expect("Fix: complete benchmark bundle should validate as one artifact set.");
        validate_benchmark_bundle(
            &dir.to_string_lossy(),
            None,
            Some(&manifest_path.to_string_lossy()),
        )
        .expect("Fix: freshly written bundle manifest should replay against current artifacts.");
        let manifest_bytes =
            std::fs::read(&manifest_path).expect("Fix: bundle manifest should be written.");
        let manifest_from_disk: BenchmarkBundleManifest = serde_json::from_slice(&manifest_bytes)
            .expect("Fix: bundle manifest should deserialize.");
        let _ = std::fs::remove_dir_all(&dir);

        assert_eq!(
            manifest.artifact_count, 7,
            "Fix: bundle validation should cover three backend reports and two comparison JSON/text pairs."
        );
        assert_eq!(manifest.schema, BENCHMARK_BUNDLE_SCHEMA);
        assert_eq!(
            manifest.provenance.validator,
            "vyre-bench validate-benchmark-bundle"
        );
        assert_eq!(
            manifest.provenance.validator_version,
            env!("CARGO_PKG_VERSION")
        );
        assert_eq!(manifest.provenance.suite, "smoke");
        assert_eq!(manifest.provenance.case_id, MAC_BENCHMARK_BUNDLE_CASE_ID);
        assert_eq!(
            manifest.provenance.report_backends,
            vec![
                "cpu-ref".to_string(),
                "metal".to_string(),
                "wgpu".to_string()
            ]
        );
        assert_eq!(
            manifest.provenance.baseline_backend,
            MAC_BENCHMARK_BUNDLE_BASELINE_BACKEND
        );
        assert_eq!(
            manifest.provenance.candidate_backend,
            MAC_BENCHMARK_BUNDLE_CANDIDATE_BACKEND
        );
        assert_eq!(
            manifest.provenance.comparison_pairs,
            vec!["cpu-ref->metal".to_string(), "wgpu->metal".to_string()]
        );
        assert_eq!(manifest.provenance.source_fingerprint, "source:unit");
        assert_eq!(manifest.provenance.source_tree_fingerprint, "tree:unit");
        assert_eq!(manifest.bundle_blake3.len(), 64);
        assert_eq!(manifest_from_disk.bundle_blake3, manifest.bundle_blake3);
        assert!(
            manifest
                .artifacts
                .iter()
                .any(|artifact| artifact.path == "metal.json"
                    && artifact.kind == "backend_report"
                    && artifact.blake3.len() == 64),
            "Fix: bundle manifest must content-address the Metal backend report."
        );
    }

    #[test]
    fn validate_benchmark_bundle_cli_writes_and_replays_manifest() {
        let dir = unique_temp_dir("bundle-cli-ok");
        write_complete_benchmark_bundle(&dir);
        let manifest_path = dir.join("bundle-manifest.json");
        let dir_arg = dir.to_string_lossy().into_owned();
        let manifest_arg = manifest_path.to_string_lossy().into_owned();

        run_cli_with(vec![
            "vyre-bench".to_string(),
            "validate-benchmark-bundle".to_string(),
            "--dir".to_string(),
            dir_arg.clone(),
            "--manifest-output".to_string(),
            manifest_arg.clone(),
        ])
        .expect("Fix: CLI should write a manifest for a complete benchmark bundle.");
        assert!(
            manifest_path.exists(),
            "Fix: CLI --manifest-output must create the bundle manifest."
        );
        run_cli_with(vec![
            "vyre-bench".to_string(),
            "validate-benchmark-bundle".to_string(),
            "--dir".to_string(),
            dir_arg,
            "--manifest-input".to_string(),
            manifest_arg,
        ])
        .expect("Fix: CLI should replay a freshly written benchmark bundle manifest.");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn validate_benchmark_bundle_cli_rejects_compare_exit_code_drift() {
        let dir = unique_temp_dir("bundle-cli-exit-code-drift");
        write_complete_benchmark_bundle(&dir);
        std::fs::write(
            dir.join("wgpu-vs-metal.txt"),
            "baseline_backend=wgpu\ncandidate_backend=metal\nbaseline_selected_backend=wgpu baseline_profile_backend=wgpu baseline_timing_quality=host_enqueue_wait\ncandidate_selected_backend=metal candidate_profile_backend=metal candidate_timing_quality=host_enqueue_wait\nfoundation.elementwise.add.1m\ncompare_exit_code=9\n",
        )
        .expect("test comparison text should be writable");
        let error = run_cli_with(vec![
            "vyre-bench".to_string(),
            "validate-benchmark-bundle".to_string(),
            "--dir".to_string(),
            dir.to_string_lossy().into_owned(),
        ])
        .expect_err("Fix: CLI must reject contradictory compare exit-code evidence.");
        let _ = std::fs::remove_dir_all(&dir);
        let error = error.to_string();
        assert!(
            error.contains("compare_exit_code=9") && error.contains("regressed=false"),
            "Fix: CLI error should surface compare exit-code contradiction: {error}"
        );
    }

    #[test]
    fn validate_benchmark_bundle_rejects_manifest_provenance_drift() {
        let dir = unique_temp_dir("bundle-provenance-drift");
        write_complete_benchmark_bundle(&dir);
        let manifest_path = dir.join("bundle-manifest.json");
        validate_benchmark_bundle(
            &dir.to_string_lossy(),
            Some(&manifest_path.to_string_lossy()),
            None,
        )
        .expect("Fix: complete benchmark bundle should write a replay manifest.");
        let manifest_bytes =
            std::fs::read(&manifest_path).expect("Fix: bundle manifest should be readable.");
        let mut manifest: BenchmarkBundleManifest = serde_json::from_slice(&manifest_bytes)
            .expect("Fix: bundle manifest should deserialize.");
        manifest.provenance.case_id = "wrong.case".to_string();
        std::fs::write(
            &manifest_path,
            serde_json::to_vec_pretty(&manifest).expect("Fix: mutated manifest should serialize"),
        )
        .expect("Fix: mutated manifest should be writable.");

        let error = validate_benchmark_bundle(
            &dir.to_string_lossy(),
            None,
            Some(&manifest_path.to_string_lossy()),
        )
        .expect_err("Fix: bundle validation must reject edited manifest provenance.");
        let _ = std::fs::remove_dir_all(&dir);
        let error = error.to_string();
        assert!(
            error.contains("bundle_blake3") && error.contains("normalized artifact metadata hash"),
            "Fix: manifest provenance drift should invalidate the normalized bundle hash: {error}"
        );
    }

    #[test]
    fn validate_benchmark_bundle_rejects_manifest_artifact_set_drift() {
        expect_manifest_artifact_set_error(
            "bundle-manifest-missing-artifact",
            |manifest| {
                manifest
                    .artifacts
                    .retain(|artifact| artifact.path != "metal.json");
            },
            "missing required artifact",
        );
        expect_manifest_artifact_set_error(
            "bundle-manifest-duplicate-artifact",
            |manifest| {
                let duplicate = manifest
                    .artifacts
                    .iter()
                    .find(|artifact| artifact.path == "metal.json")
                    .expect("Fix: base manifest should contain metal.json")
                    .clone();
                manifest.artifacts.push(duplicate);
            },
            "repeats artifact path",
        );
        expect_manifest_artifact_set_error(
            "bundle-manifest-unknown-artifact",
            |manifest| {
                let mut extra = manifest
                    .artifacts
                    .iter()
                    .find(|artifact| artifact.path == "metal.json")
                    .expect("Fix: base manifest should contain metal.json")
                    .clone();
                extra.path = "extra.json".to_string();
                manifest.artifacts.push(extra);
            },
            "unexpected artifact path",
        );
        expect_manifest_artifact_set_error(
            "bundle-manifest-mislabeled-artifact",
            |manifest| {
                let artifact = manifest
                    .artifacts
                    .iter_mut()
                    .find(|artifact| artifact.path == "metal.json")
                    .expect("Fix: base manifest should contain metal.json");
                artifact.kind = "comparison_json".to_string();
            },
            "unexpected artifact path",
        );
    }

    #[test]
    fn benchmark_bundle_provenance_is_derived_from_report_evidence() {
        let case_id = "custom.case";
        let mut cpu_ref = report(vec![case_report(case_id, "pass", true)], 1, 0);
        cpu_ref.suite = "custom-suite".to_string();
        cpu_ref.selected_backend = Some("cpu-ref".to_string());
        cpu_ref.backend_profile = Some(backend_profile("cpu-ref", "host_only"));
        let mut baseline = report(vec![case_report_with_wall(case_id, 100, 100.0)], 1, 0);
        baseline.suite = "custom-suite".to_string();
        baseline.selected_backend = Some("alpha".to_string());
        baseline.backend_profile = Some(backend_profile("alpha", "host_enqueue_wait"));
        let mut candidate = report(vec![case_report_with_wall(case_id, 90, 90.0)], 1, 0);
        candidate.suite = "custom-suite".to_string();
        candidate.selected_backend = Some("beta".to_string());
        candidate.backend_profile = Some(backend_profile("beta", "host_enqueue_wait"));
        let comparison = build_comparison_artifact(&baseline, &candidate)
            .expect("Fix: comparison artifact should build from matching custom cases.");

        let provenance =
            derive_benchmark_bundle_provenance(&[cpu_ref, baseline, candidate], &[comparison])
                .expect("Fix: provenance should derive from valid report and comparison evidence.");

        assert_eq!(provenance.suite, "custom-suite");
        assert_eq!(provenance.case_id, case_id);
        assert_eq!(
            provenance.report_backends,
            vec![
                "alpha".to_string(),
                "beta".to_string(),
                "cpu-ref".to_string()
            ]
        );
        assert_eq!(provenance.baseline_backend, "alpha");
        assert_eq!(provenance.candidate_backend, "beta");
        assert_eq!(provenance.comparison_pairs, vec!["alpha->beta".to_string()]);
        assert_eq!(provenance.source_fingerprint, "source:unit");
        assert_eq!(provenance.source_tree_fingerprint, "tree:unit");
    }

    #[test]
    fn benchmark_bundle_provenance_rejects_mixed_source_reports() {
        let case_id = "custom.case";
        let mut cpu_ref = report(vec![case_report(case_id, "pass", true)], 1, 0);
        cpu_ref.suite = "custom-suite".to_string();
        cpu_ref.selected_backend = Some("cpu-ref".to_string());
        cpu_ref.backend_profile = Some(backend_profile("cpu-ref", "host_only"));
        let mut baseline = report(vec![case_report_with_wall(case_id, 100, 100.0)], 1, 0);
        baseline.suite = "custom-suite".to_string();
        baseline.selected_backend = Some("alpha".to_string());
        baseline.backend_profile = Some(backend_profile("alpha", "host_enqueue_wait"));
        let mut candidate = report(vec![case_report_with_wall(case_id, 90, 90.0)], 1, 0);
        candidate.suite = "custom-suite".to_string();
        candidate.source_tree_fingerprint = "tree:other".to_string();
        candidate.selected_backend = Some("beta".to_string());
        candidate.backend_profile = Some(backend_profile("beta", "host_enqueue_wait"));
        let comparison = build_comparison_artifact(&baseline, &candidate)
            .expect("Fix: comparison artifact should build from matching custom cases.");

        let error =
            derive_benchmark_bundle_provenance(&[cpu_ref, baseline, candidate], &[comparison])
                .expect_err("Fix: bundle provenance must reject mixed source-tree evidence.");
        let error = error.to_string();
        assert!(
            error.contains("source_tree_fingerprint"),
            "Fix: mixed source-tree rejection must name the drifting field: {error}"
        );
    }

    #[test]
    fn validate_benchmark_bundle_rejects_comparison_report_drift() {
        let dir = unique_temp_dir("bundle-comparison-drift");
        write_complete_benchmark_bundle(&dir);
        let wgpu_path = dir.join("wgpu.json");
        let mut wgpu_report = load_report(&wgpu_path.to_string_lossy())
            .expect("Fix: synthetic WGPU report should load before mutation.");
        wgpu_report.run_id = "mutated-after-comparison".to_string();
        write_report(&wgpu_path, &wgpu_report);

        let error = validate_benchmark_bundle(&dir.to_string_lossy(), None, None)
            .expect_err("Fix: bundle validation must reject stale comparison JSON.");
        let _ = std::fs::remove_dir_all(&dir);
        let error = error.to_string();
        assert!(
            error.contains("comparison artifact does not match bundled"),
            "Fix: comparison/report drift must explain the stale comparison artifact: {error}"
        );
    }

    #[test]
    fn validate_benchmark_bundle_rejects_compare_exit_code_drift() {
        let dir = unique_temp_dir("bundle-exit-code-drift");
        write_complete_benchmark_bundle(&dir);
        std::fs::write(
            dir.join("wgpu-vs-metal.txt"),
            "baseline_backend=wgpu\ncandidate_backend=metal\nbaseline_selected_backend=wgpu baseline_profile_backend=wgpu baseline_timing_quality=host_enqueue_wait\ncandidate_selected_backend=metal candidate_profile_backend=metal candidate_timing_quality=host_enqueue_wait\nfoundation.elementwise.add.1m\ncompare_exit_code=7\n",
        )
        .expect("test comparison text should be writable");

        let error = validate_benchmark_bundle(&dir.to_string_lossy(), None, None)
            .expect_err("Fix: bundle validation must reject contradictory compare exit code.");
        let _ = std::fs::remove_dir_all(&dir);
        let error = error.to_string();
        assert!(
            error.contains("compare_exit_code=7") && error.contains("regressed=false"),
            "Fix: compare exit-code drift must explain the JSON/text contradiction: {error}"
        );
    }

    #[test]
    fn validate_benchmark_bundle_rejects_missing_comparison_json() {
        let dir = unique_temp_dir("bundle-missing-comparison");
        write_complete_benchmark_bundle(&dir);
        std::fs::remove_file(dir.join("wgpu-vs-metal.json"))
            .expect("test comparison JSON should be removable");

        let error = validate_benchmark_bundle(&dir.to_string_lossy(), None, None)
            .expect_err("Fix: bundle validation must reject missing comparison JSON.");
        let _ = std::fs::remove_dir_all(&dir);
        let error = error.to_string();
        assert!(
            error.contains("wgpu-vs-metal.json"),
            "Fix: missing comparison JSON should be named in the validation error: {error}"
        );
    }

    #[test]
    fn validate_benchmark_bundle_rejects_manifest_artifact_drift() {
        let dir = unique_temp_dir("bundle-manifest-drift");
        write_complete_benchmark_bundle(&dir);
        let manifest_path = dir.join("bundle-manifest.json");
        validate_benchmark_bundle(
            &dir.to_string_lossy(),
            Some(&manifest_path.to_string_lossy()),
            None,
        )
        .expect("Fix: complete benchmark bundle should write a replay manifest.");
        std::fs::write(
            dir.join("wgpu-vs-metal.txt"),
            "baseline_backend=wgpu\ncandidate_backend=metal\nbaseline_selected_backend=wgpu baseline_profile_backend=wgpu baseline_timing_quality=host_enqueue_wait\ncandidate_selected_backend=metal candidate_profile_backend=metal candidate_timing_quality=host_enqueue_wait\nfoundation.elementwise.add.1m\ncompare_exit_code=0\nmutated_after_manifest=1\n",
        )
        .expect("test comparison text should be writable");

        let error = validate_benchmark_bundle(
            &dir.to_string_lossy(),
            None,
            Some(&manifest_path.to_string_lossy()),
        )
        .expect_err(
            "Fix: bundle validation must reject artifacts drifted after manifest creation.",
        );
        let _ = std::fs::remove_dir_all(&dir);
        let error = error.to_string();
        assert!(
            error.contains("bundle_blake3") && error.contains("does not match current artifacts"),
            "Fix: manifest drift error should name the bundle hash mismatch: {error}"
        );
    }

    #[test]
    fn validate_comparison_rejects_candidate_backend_drift() {
        let (baseline, candidate) = comparison_reports();
        let artifact = build_comparison_artifact(&baseline, &candidate)
            .expect("Fix: comparison artifact should build from matching cases.");
        let error = validate_comparison_expectations(
            &artifact,
            "wgpu",
            "cuda",
            &["foundation.elementwise.add.1m".to_string()],
        )
        .expect_err("Fix: comparison validation must reject wrong candidate backend.");
        let error = error.to_string();
        assert!(
            error.contains("candidate profile backend"),
            "Fix: candidate backend drift must be explained: {error}"
        );
    }

    #[test]
    fn validate_report_command_accepts_backend_profile_contract() {
        let mut valid = report(
            vec![case_report("foundation.elementwise.add.1m", "pass", true)],
            1,
            0,
        );
        valid.backend_profile = Some(backend_profile("cuda", "device_timestamps"));
        let path = std::env::temp_dir().join(format!(
            "vyre-bench-valid-report-{}-{}.json",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system clock must be after UNIX_EPOCH")
                .as_nanos()
        ));
        std::fs::write(
            &path,
            serde_json::to_vec(&valid).expect("test report should serialize"),
        )
        .expect("test report should be writable");
        let path_arg = path.to_string_lossy().into_owned();

        let result = run_cli_with(vec![
            "vyre-bench".to_string(),
            "validate-report".to_string(),
            "--path".to_string(),
            path_arg,
            "--backend".to_string(),
            "cuda".to_string(),
            "--total-cases".to_string(),
            "1".to_string(),
            "--failed".to_string(),
            "0".to_string(),
        ]);
        let _ = std::fs::remove_file(&path);
        assert_eq!(
            result.is_ok(),
            true,
            "Fix: validate-report must accept matching backend profile evidence: {result:?}"
        );
    }

    #[test]
    fn validate_report_expectations_rejects_missing_backend_profile() {
        let forged = report(
            vec![case_report("foundation.elementwise.add.1m", "pass", true)],
            1,
            0,
        );
        let error = validate_report_expectations(&forged, Some("cuda"), Some(1), Some(0))
            .expect_err("Fix: expected-backend validation must reject missing backend_profile");
        let error = error.to_string();
        assert!(
            error.contains("lacks backend_profile"),
            "Fix: missing backend_profile errors should explain the report must be regenerated: {error}"
        );
    }

    #[test]
    fn validate_report_expectations_rejects_profile_backend_drift() {
        let mut forged = report(
            vec![case_report("foundation.elementwise.add.1m", "pass", true)],
            1,
            0,
        );
        forged.backend_profile = Some(backend_profile("wgpu", "host_enqueue_wait"));
        let error = validate_report_expectations(&forged, Some("cuda"), Some(1), Some(0))
            .expect_err("Fix: expected-backend validation must reject mismatched backend_profile");
        let error = error.to_string();
        assert!(
            error.contains("contradicts expected backend"),
            "Fix: backend drift errors should name the profile mismatch: {error}"
        );
    }

    #[test]
    fn load_report_rejects_summary_that_hides_contract_failed_case() {
        let forged = report(
            vec![case_report("release.condition_eval.1m", "pass", false)],
            1,
            0,
        );
        let path = std::env::temp_dir().join(format!(
            "vyre-bench-forged-summary-{}-{}.json",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system clock must be after UNIX_EPOCH")
                .as_nanos()
        ));
        std::fs::write(
            &path,
            serde_json::to_vec(&forged).expect("test report should serialize"),
        )
        .expect("test report should be writable");

        let error = load_report(&path.to_string_lossy())
            .expect_err("Fix: loaded benchmark evidence must reject hidden contract failures");
        let _ = std::fs::remove_file(&path);
        let error = error.to_string();
        assert!(
            error.contains("invalid benchmark report") && error.contains("contradicts case evidence"),
            "Fix: report loader should explain that summary counts disagree with case evidence: {error}"
        );
    }

    #[test]
    fn load_report_rejects_blockers_that_hide_contract_failed_case() {
        let mut forged = report(
            vec![case_report("release.condition_eval.1m", "pass", false)],
            0,
            1,
        );
        forged.blockers.clear();
        let path = std::env::temp_dir().join(format!(
            "vyre-bench-forged-blockers-{}-{}.json",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system clock must be after UNIX_EPOCH")
                .as_nanos()
        ));
        std::fs::write(
            &path,
            serde_json::to_vec(&forged).expect("test report should serialize"),
        )
        .expect("test report should be writable");

        let error = load_report(&path.to_string_lossy())
            .expect_err("Fix: loaded benchmark evidence must reject hidden blockers");
        let _ = std::fs::remove_file(&path);
        let error = error.to_string();
        assert!(
            error.contains("invalid benchmark report")
                && error.contains("top-level blockers")
                && error.contains("contradict case-derived blockers"),
            "Fix: report loader should explain that top-level blockers disagree with case evidence: {error}"
        );
    }

    #[test]
    fn dashboard_counts_pass_status_evidence_not_legacy_passed_string() {
        let report = report(
            vec![
                case_report("release.condition_eval.1m", "pass", true),
                case_report("release.scan_ac_irregular.1m", "failed", true),
            ],
            1,
            1,
        );

        let scorecard = generate_scorecard_md(&report);
        assert!(
            scorecard.contains("Cases: 1/2 passed"),
            "Fix: dashboard scorecard must count generated `pass` status as pass evidence: {scorecard}"
        );

        let html = generate_index_html(&report, &scorecard);
        assert!(
            html.contains("<div class=\"stat-value\">1</div>")
                && html.contains("<td class=\"status-pass\">pass</td>"),
            "Fix: dashboard HTML must render generated `pass` status with pass styling: {html}"
        );
    }
}
