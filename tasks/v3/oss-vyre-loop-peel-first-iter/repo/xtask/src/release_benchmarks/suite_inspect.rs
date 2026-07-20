use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use serde_json::{json, Value};

use super::metrics::write_json;
use super::runner::run_command_status;
use super::types::{
    BackendSuiteArtifact, BackendSuiteArtifactInput, BackendSuiteEvidence, HardwareDigestField,
    HardwareUnavailableReason, MAX_RELEASE_BENCHMARK_TEXT_BYTES, MIN_CPU_SOTA_100X_RELEASE_CASES,
    MIN_CUDA_RELEASE_COMPUTE_CAPABILITY_MAJOR, MIN_CUDA_RELEASE_COMPUTE_CAPABILITY_MINOR,
    MIN_CUDA_RELEASE_MEMORY_MIB, REQUIRED_CPU_SOTA_100X_CASES,
};

pub(super) fn write_cpu_100x_proof(workspace_root: &Path, artifacts: &[String]) {
    let mut cases = Vec::new();
    let mut blockers = Vec::new();
    let mut contract_case_count = 0usize;
    let mut passing_contract_case_count = 0usize;
    let mut min_wall_samples = None::<u64>;
    let mut min_baseline_wall_samples = None::<u64>;
    let mut min_wall_p50 = None::<u64>;
    let mut min_wall_p95 = None::<u64>;
    let mut min_wall_p99 = None::<u64>;
    let mut min_baseline_wall_p50 = None::<u64>;
    let mut min_baseline_wall_p95 = None::<u64>;
    let mut min_baseline_wall_p99 = None::<u64>;
    let mut observed_required_cases = std::collections::BTreeSet::new();
    let mut environment = None::<Value>;
    let mut git = None::<Value>;
    let mut source_fingerprint = None::<String>;
    let mut source_tree_fingerprint = None::<String>;
    let mut unique_artifacts = BTreeSet::new();
    let mut component_rows = Vec::new();
    let mut component_blockers = Vec::new();
    for artifact in artifacts {
        if !unique_artifacts.insert(artifact.clone()) {
            blockers.push(format!(
                "100x proof source_artifact `{artifact}` is duplicated; aggregate proof counts must use distinct source artifacts"
            ));
        }
    }
    let artifacts = unique_artifacts.into_iter().collect::<Vec<_>>();
    for artifact in &artifacts {
        if let Some(issue) =
            crate::benchmark_evidence_semantics::benchmark_source_artifact_path_issue(
                workspace_root,
                artifact,
            )
        {
            blockers.push(format!(
                "100x {}",
                issue.describe("source_artifact", artifact)
            ));
            continue;
        }
        let path = workspace_root.join(artifact);
        let text = match read_text_bounded(&path, MAX_RELEASE_BENCHMARK_TEXT_BYTES) {
            Ok(text) => text,
            Err(error) => {
                blockers.push(format!(
                    "100x source artifact `{artifact}` is unreadable: {error}"
                ));
                continue;
            }
        };
        let Ok(report) = serde_json::from_str::<Value>(&text) else {
            blockers.push(format!("100x source artifact `{artifact}` is invalid JSON"));
            continue;
        };
        if report.get("selected_backend").and_then(Value::as_str) != Some("cuda") {
            blockers.push(format!(
                "100x source artifact `{artifact}` was not produced for cuda"
            ));
        }
        crate::benchmark_evidence_semantics::inspect_source_artifact_case_integrity(
            artifact,
            &report,
            "CPU-SOTA aggregate proof",
            &mut blockers,
        );
        if environment.is_none() {
            environment = report.get("environment").cloned();
        }
        if git.is_none() {
            git = report.get("git").cloned();
        }
        let report_source_fingerprint = report
            .get("source_fingerprint")
            .and_then(nonblank_str)
            .map(str::to_string);
        if let Some(fingerprint) = &report_source_fingerprint {
            if !crate::benchmark_evidence_semantics::source_fingerprint_issues(fingerprint)
                .is_empty()
            {
                blockers.push(format!(
                    "100x source artifact `{artifact}` source_fingerprint `{fingerprint}` is not release-grade provenance"
                ));
            }
        } else {
            blockers.push(format!(
                "100x source artifact `{artifact}` has no source_fingerprint"
            ));
        }
        if source_fingerprint.is_none() {
            if let Some(fingerprint) = &report_source_fingerprint {
                source_fingerprint = Some(fingerprint.clone());
            }
        }
        let report_source_tree_fingerprint = report
            .get("source_tree_fingerprint")
            .and_then(nonblank_str)
            .map(str::to_string);
        match (&source_tree_fingerprint, &report_source_tree_fingerprint) {
            (None, Some(fingerprint)) => source_tree_fingerprint = Some(fingerprint.clone()),
            (Some(expected), Some(actual)) if expected != actual => blockers.push(format!(
                "100x source artifact `{artifact}` source_tree_fingerprint `{actual}` does not match aggregate source tree `{expected}`"
            )),
            _ => {}
        }
        if report_source_tree_fingerprint.is_none() {
            blockers.push(format!(
                "100x source artifact `{artifact}` has no source_tree_fingerprint"
            ));
        }
        if let (Some((field, freshness_fingerprint)), Some(current_freshness_fingerprint)) = (
            crate::benchmark_evidence_semantics::report_freshness_fingerprint(&report),
            crate::benchmark_evidence_semantics::current_freshness_fingerprint_for_report(
                &path, &report,
            ),
        ) {
            for issue in crate::benchmark_evidence_semantics::source_fingerprint_freshness_issues(
                freshness_fingerprint,
                &current_freshness_fingerprint,
            ) {
                match issue {
                    crate::benchmark_evidence_semantics::SourceFingerprintFreshnessIssue::Mismatch {
                        source_fingerprint,
                        current_source_fingerprint,
                    } => blockers.push(format!(
                        "100x source artifact `{artifact}` {field} `{source_fingerprint}` does not match current workspace source `{current_source_fingerprint}`"
                    )),
                }
            }
        }
        let Some(report_cases) = report.get("cases").and_then(Value::as_array) else {
            blockers.push(format!(
                "100x source artifact `{artifact}` has no cases array"
            ));
            continue;
        };
        let (report_contract_case_count, report_passing_contract_case_count) =
            crate::benchmark_evidence_semantics::cpu_sota_100x_case_counts(&report);
        contract_case_count += report_contract_case_count as usize;
        passing_contract_case_count += report_passing_contract_case_count as usize;
        for case in report_cases {
            let case_id = case
                .get("id")
                .and_then(Value::as_str)
                .unwrap_or("<unknown>");
            let case_failure_reason =
                crate::benchmark_evidence_semantics::benchmark_case_failure_reason(case);
            if let Some(reason) = &case_failure_reason {
                blockers.push(format!(
                    "100x source artifact `{artifact}` case `{case_id}` failed: {reason}"
                ));
            }
            let metrics = case.get("metrics").and_then(Value::as_object);
            let case_backend = case
                .get("backend_id")
                .and_then(Value::as_str)
                .or_else(|| report.get("selected_backend").and_then(Value::as_str));
            if REQUIRED_CPU_SOTA_100X_CASES.contains(&case_id)
                && crate::benchmark_evidence_semantics::benchmark_case_proves_cpu_sota_100x(
                    case,
                    case_backend,
                )
            {
                observed_required_cases.insert(case_id.to_string());
                component_rows.push(cpu_sota_component_proof_case(
                    artifact,
                    case_id,
                    case,
                    metrics,
                    &mut component_blockers,
                ));
            }
            let wall_samples = metrics
                .and_then(|metrics| suite_metric_samples(metrics.get("wall_ns")))
                .unwrap_or(0);
            min_wall_samples =
                Some(min_wall_samples.map_or(wall_samples, |min| min.min(wall_samples)));
            if wall_samples < 30 {
                blockers.push(format!(
                    "100x source artifact `{artifact}` case `{case_id}` has {wall_samples} wall_ns sample(s), needs at least 30"
                ));
            }
            let baseline_wall_samples = metrics
                .and_then(|metrics| suite_metric_samples(metrics.get("baseline_wall_ns")))
                .unwrap_or(0);
            min_baseline_wall_samples = Some(
                min_baseline_wall_samples
                    .map_or(baseline_wall_samples, |min| min.min(baseline_wall_samples)),
            );
            if baseline_wall_samples < 30 {
                blockers.push(format!(
                    "100x source artifact `{artifact}` case `{case_id}` has {baseline_wall_samples} baseline_wall_ns sample(s), needs at least 30"
                ));
            }
            record_required_metric_percentile(
                &mut min_wall_p50,
                metrics,
                "wall_ns",
                "p50",
                &mut blockers,
                case_id,
            );
            record_required_metric_percentile(
                &mut min_wall_p95,
                metrics,
                "wall_ns",
                "p95",
                &mut blockers,
                case_id,
            );
            record_required_metric_percentile(
                &mut min_wall_p99,
                metrics,
                "wall_ns",
                "p99",
                &mut blockers,
                case_id,
            );
            record_required_metric_percentile(
                &mut min_baseline_wall_p50,
                metrics,
                "baseline_wall_ns",
                "p50",
                &mut blockers,
                case_id,
            );
            record_required_metric_percentile(
                &mut min_baseline_wall_p95,
                metrics,
                "baseline_wall_ns",
                "p95",
                &mut blockers,
                case_id,
            );
            record_required_metric_percentile(
                &mut min_baseline_wall_p99,
                metrics,
                "baseline_wall_ns",
                "p99",
                &mut blockers,
                case_id,
            );
        }
        cases.extend(report_cases.iter().cloned());
    }
    if artifacts.len() < MIN_CPU_SOTA_100X_RELEASE_CASES {
        blockers.push(format!(
            "100x proof has {} source artifact(s); release requires at least {} CPU-SOTA 100x workload families",
            artifacts.len(),
            MIN_CPU_SOTA_100X_RELEASE_CASES
        ));
    }
    if cases.len() < MIN_CPU_SOTA_100X_RELEASE_CASES {
        blockers.push(format!(
            "100x proof has {} benchmark case(s); release requires at least {}",
            cases.len(),
            MIN_CPU_SOTA_100X_RELEASE_CASES
        ));
    }
    if contract_case_count < MIN_CPU_SOTA_100X_RELEASE_CASES {
        blockers.push(format!(
            "100x proof has {contract_case_count} CPU-SOTA 100x contract case(s); release requires at least {MIN_CPU_SOTA_100X_RELEASE_CASES}"
        ));
    }
    if passing_contract_case_count < MIN_CPU_SOTA_100X_RELEASE_CASES {
        blockers.push(format!(
            "100x proof has {passing_contract_case_count} passing CPU-SOTA 100x case(s); release requires at least {MIN_CPU_SOTA_100X_RELEASE_CASES}"
        ));
    }
    let missing_required_cases = REQUIRED_CPU_SOTA_100X_CASES
        .iter()
        .copied()
        .filter(|required| !observed_required_cases.contains(*required))
        .collect::<Vec<_>>();
    for required in &missing_required_cases {
        blockers.push(format!(
            "100x proof is missing required release-defining case `{required}`"
        ));
    }
    let aggregate_failed = cases.len().saturating_sub(passing_contract_case_count);
    blockers.extend(component_blockers.iter().cloned());
    let evidence = json!({
        "schema_version": 1,
        "selected_backend": "cuda",
        "environment": environment,
        "git": git,
        "source_fingerprint": source_fingerprint,
        "source_tree_fingerprint": source_tree_fingerprint,
        "source_artifacts": &artifacts,
        "source_artifact_count": artifacts.len(),
        "required_cpu_sota_100x_cases": REQUIRED_CPU_SOTA_100X_CASES,
        "missing_required_cpu_sota_100x_cases": missing_required_cases,
        "cpu_sota_100x_contract_case_count": contract_case_count,
        "cpu_sota_100x_passing_case_count": passing_contract_case_count,
        "min_wall_samples": min_wall_samples,
        "min_wall_p50": min_wall_p50,
        "min_wall_p95": min_wall_p95,
        "min_wall_p99": min_wall_p99,
        "min_baseline_wall_samples": min_baseline_wall_samples,
        "min_baseline_wall_p50": min_baseline_wall_p50,
        "min_baseline_wall_p95": min_baseline_wall_p95,
        "min_baseline_wall_p99": min_baseline_wall_p99,
        "component_speedup_proof": {
            "schema_version": 1,
            "comparator_identity": "cpu-sota-component-speedup:v1",
            "collapsed_speedup_field_allowed": false,
            "required_components": [
                "scalar_cpu",
                "simd_cpu",
                "gpu_active",
                "transfer",
                "launch",
                "end_to_end"
            ],
            "parity_policy": "exact_cpu_gpu_digest",
            "case_count": component_rows.len(),
            "missing_component_count": component_blockers.len(),
            "missing_components": component_blockers,
            "cases": component_rows
        },
        "summary": {
            "total_cases": cases.len(),
            "passed": passing_contract_case_count,
            "failed": aggregate_failed,
            "total_time_ns": 0,
            "cache_hit_rate": null,
        },
        "cases": cases,
        "blockers": blockers,
    });
    write_json(
        &workspace_root.join("release/evidence/benchmarks/cpu-only-100x-proof.json"),
        &evidence,
    );
}

fn cpu_sota_component_proof_case(
    artifact: &str,
    case_id: &str,
    case: &Value,
    metrics: Option<&serde_json::Map<String, Value>>,
    blockers: &mut Vec<String>,
) -> Value {
    let scalar_cpu_wall_ns_p50 = first_metric_p50(
        metrics,
        &["scalar_cpu_wall_ns", "cpu_scalar_wall_ns"],
    );
    let simd_cpu_wall_ns_p50 =
        first_metric_p50(metrics, &["simd_cpu_wall_ns", "cpu_simd_wall_ns"]);
    let gpu_active_ns_p50 = first_metric_p50(
        metrics,
        &["active_time_ns", "gpu_active_ns", "kernel_execute_ns", "dispatch_ns"],
    );
    let transfer_bytes_p50 = first_metric_p50(metrics, &["transfer_bytes", "gpu_transfer_bytes"]);
    let launch_ns_p50 = first_metric_p50(metrics, &["launch_ns", "kernel_launch_ns"]);
    let end_to_end_wall_ns_p50 = first_metric_p50(metrics, &["wall_ns"]);
    let cpu_digest = first_metric_p50(metrics, &["cpu_digest"]);
    let gpu_digest = first_metric_p50(metrics, &["gpu_digest"]);
    for (field, value) in [
        ("scalar_cpu_wall_ns_p50", scalar_cpu_wall_ns_p50),
        ("simd_cpu_wall_ns_p50", simd_cpu_wall_ns_p50),
        ("gpu_active_ns_p50", gpu_active_ns_p50),
        ("transfer_bytes_p50", transfer_bytes_p50),
        ("launch_ns_p50", launch_ns_p50),
        ("end_to_end_wall_ns_p50", end_to_end_wall_ns_p50),
        ("cpu_digest", cpu_digest),
        ("gpu_digest", gpu_digest),
    ] {
        if value.is_none_or(|value| value == 0) {
            blockers.push(format!(
                "100x component proof source artifact `{artifact}` case `{case_id}` is missing {field}"
            ));
        }
    }
    let parity_passed = matches!((cpu_digest, gpu_digest), (Some(cpu), Some(gpu)) if cpu != 0 && cpu == gpu)
        && case.get("correctness").and_then(|correctness| correctness.get("Invalid")).is_none();
    if !parity_passed {
        blockers.push(format!(
            "100x component proof source artifact `{artifact}` case `{case_id}` must prove exact CPU/GPU digest parity"
        ));
    }
    json!({
        "artifact": artifact,
        "case_id": case_id,
        "scalar_cpu_wall_ns_p50": scalar_cpu_wall_ns_p50,
        "simd_cpu_wall_ns_p50": simd_cpu_wall_ns_p50,
        "gpu_active_ns_p50": gpu_active_ns_p50,
        "transfer_bytes_p50": transfer_bytes_p50,
        "launch_ns_p50": launch_ns_p50,
        "end_to_end_wall_ns_p50": end_to_end_wall_ns_p50,
        "cpu_digest": cpu_digest,
        "gpu_digest": gpu_digest,
        "parity_passed": parity_passed,
    })
}

fn first_metric_p50(
    metrics: Option<&serde_json::Map<String, Value>>,
    names: &[&str],
) -> Option<u64> {
    names
        .iter()
        .find_map(|name| metrics.and_then(|metrics| suite_metric_percentile(metrics.get(*name), "p50")))
}

pub(super) fn read_text_bounded(path: &Path, max_bytes: u64) -> std::io::Result<String> {
    use std::io::Read as _;

    let mut file = fs::File::open(path)?;
    let metadata = file.metadata()?;
    if metadata.len() > max_bytes {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("release benchmark evidence exceeds {max_bytes} byte limit"),
        ));
    }
    let mut text = String::with_capacity(metadata.len() as usize);
    file.by_ref()
        .take(max_bytes + 1)
        .read_to_string(&mut text)?;
    if text.len() as u64 > max_bytes {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "release benchmark evidence exceeded bounded read limit",
        ));
    }
    Ok(text)
}

pub(super) fn run_workload_benchmark(
    workspace_root: &Path,
    case_id: &str,
    backend: &str,
    output: &str,
    measured_samples: Option<usize>,
    sample_timeout_secs: u64,
) -> Result<(), String> {
    let mut owned_args = vec![
        "run".to_string(),
        "-p".to_string(),
        "vyre-bench".to_string(),
        "--quiet".to_string(),
        "--".to_string(),
        "run".to_string(),
        "--suite".to_string(),
        "release".to_string(),
        "--case".to_string(),
        case_id.to_string(),
        "--backend".to_string(),
        backend.to_string(),
        "--enforce-budgets".to_string(),
        "--output".to_string(),
        output.to_string(),
        "--sample-timeout-secs".to_string(),
        sample_timeout_secs.to_string(),
    ];
    if let Some(samples) = measured_samples {
        owned_args.push("--measured-samples".to_string());
        owned_args.push(samples.to_string());
    }
    let borrowed = owned_args.iter().map(String::as_str).collect::<Vec<_>>();
    run_command_status(workspace_root, &borrowed)
}

pub(super) fn prefixed_benchmark_artifact(path: &str, prefix: &str) -> String {
    let path = Path::new(path);
    let Some(file_name) = path.file_name().and_then(|file| file.to_str()) else {
        return format!("{prefix}-{path}", path = path.display());
    };
    let file_name = format!("{prefix}-{file_name}");
    path.parent()
        .map(|parent| parent.join(&file_name).display().to_string())
        .unwrap_or(file_name)
}

pub(super) fn write_backend_suite(
    workspace_root: &Path,
    backend: &str,
    artifact_inputs: Vec<BackendSuiteArtifactInput>,
) {
    write_backend_suite_with_extra_blockers(workspace_root, backend, artifact_inputs, Vec::new());
}

pub(super) fn write_backend_suite_with_extra_blockers(
    workspace_root: &Path,
    backend: &str,
    artifact_inputs: Vec<BackendSuiteArtifactInput>,
    extra_blockers: Vec<String>,
) {
    let output = backend_suite_output_path(backend);
    let mut blockers = extra_blockers;
    if artifact_inputs.is_empty() {
        blockers.push(format!(
            "backend `{backend}` release suite has zero artifacts"
        ));
    }
    let path_counts = backend_suite_input_path_counts(&artifact_inputs);
    let family_counts = backend_suite_input_family_counts(&artifact_inputs);
    for artifact in artifact_inputs
        .iter()
        .filter(|artifact| artifact.family_id.trim().is_empty())
    {
        blockers.push(format!(
            "backend `{backend}` release suite artifact `{}` has blank family_id",
            artifact.path
        ));
    }
    for artifact in artifact_inputs
        .iter()
        .filter(|artifact| artifact.requested_case_id.trim().is_empty())
    {
        blockers.push(format!(
            "backend `{backend}` release suite artifact `{}` has blank requested_case_id",
            artifact.path
        ));
    }
    for (family_id, count) in &family_counts {
        if *count > 1 {
            blockers.push(format!(
                "backend `{backend}` release suite has {count} artifact input(s) for family `{family_id}`"
            ));
        }
    }
    for (path, count) in &path_counts {
        if *count > 1 {
            blockers.push(format!(
                "backend `{backend}` release suite has {count} artifact input(s) for path `{path}`"
            ));
        }
    }
    let artifacts = artifact_inputs
        .iter()
        .map(|artifact| artifact.path.clone())
        .collect::<Vec<_>>();
    let artifact_statuses = artifact_inputs
        .iter()
        .map(|artifact| inspect_backend_suite_artifact(workspace_root, backend, artifact))
        .inspect(|status| {
            blockers.extend(status.blockers.iter().map(|blocker| {
                format!(
                    "backend `{backend}` release suite artifact `{}`: {blocker}",
                    status.path
                )
            }));
        })
        .collect::<Vec<_>>();
    let (hardware_digest, hardware_digest_fields, hardware_unavailable_reasons, hardware_blockers) =
        backend_suite_hardware_digest(backend, &artifact_statuses);
    blockers.extend(hardware_blockers);
    let schema_digest_chain =
        backend_suite_schema_digest_chain(backend, &artifact_statuses, &hardware_digest);
    let evidence = BackendSuiteEvidence {
        schema_version: 3,
        backend: backend.to_string(),
        schema_digest_chain,
        hardware_digest,
        hardware_digest_fields,
        hardware_unavailable_reasons,
        family_count: family_counts.len(),
        artifacts,
        artifact_statuses,
        blockers,
    };
    let path = workspace_root.join(output);
    if let Some(parent) = path.parent() {
        if let Err(error) = fs::create_dir_all(parent) {
            eprintln!("Fix: failed to create `{}`: {error}", parent.display());
            std::process::exit(1);
        }
    }
    let json = match serde_json::to_string_pretty(&evidence) {
        Ok(json) => json,
        Err(error) => {
            eprintln!("Fix: failed to serialize backend suite evidence: {error}");
            std::process::exit(1);
        }
    };
    if let Err(error) = fs::write(&path, format!("{json}\n")) {
        eprintln!("Fix: failed to write `{}`: {error}", path.display());
        std::process::exit(1);
    }
}

fn backend_suite_schema_digest_chain(
    backend: &str,
    artifact_statuses: &[BackendSuiteArtifact],
    hardware_digest: &str,
) -> Value {
    let mut source_material = format!("benchmark-suite-source:v1\nbackend={backend}\n");
    let mut config_material = format!("benchmark-suite-config:v1\nbackend={backend}\n");
    let mut dataset_material = format!("benchmark-suite-dataset:v1\nbackend={backend}\n");
    for status in artifact_statuses {
        source_material.push_str("source_fingerprint=");
        source_material.push_str(status.source_fingerprint.as_deref().unwrap_or(""));
        source_material.push_str("\nsource_tree_fingerprint=");
        source_material.push_str(status.source_tree_fingerprint.as_deref().unwrap_or(""));
        source_material.push('\n');

        config_material.push_str("path=");
        config_material.push_str(&status.path);
        config_material.push_str("\nfamily_id=");
        config_material.push_str(&status.family_id);
        config_material.push_str("\nrequested_case_id=");
        config_material.push_str(&status.requested_case_id);
        config_material.push_str("\ncpu_sota_100x_required=");
        config_material.push_str(if status.cpu_sota_100x_required {
            "true"
        } else {
            "false"
        });
        config_material.push('\n');

        dataset_material.push_str("family_id=");
        dataset_material.push_str(&status.family_id);
        dataset_material.push_str("\nrequested_case_id=");
        dataset_material.push_str(&status.requested_case_id);
        dataset_material.push_str("\nartifact_path=");
        dataset_material.push_str(&status.path);
        dataset_material.push('\n');
    }
    let source_digest = format!(
        "benchmark-source-digest:v1:{}",
        crate::hash::sha256_hex(source_material.as_bytes())
    );
    let command_digest = format!(
        "benchmark-command-digest:v1:{}",
        crate::hash::sha256_hex(
            format!("release-benchmarks backend-suite:v1 backend={backend}\n").as_bytes()
        )
    );
    let config_digest = format!(
        "benchmark-config-digest:v1:{}",
        crate::hash::sha256_hex(config_material.as_bytes())
    );
    let dataset_digest = format!(
        "benchmark-dataset-digest:v1:{}",
        crate::hash::sha256_hex(dataset_material.as_bytes())
    );
    let comparator_version =
        "benchmark-suite-comparator:v1:case-integrity+summary+hardware".to_string();
    crate::benchmark_evidence_semantics::benchmark_schema_digest_chain_value(
        "backend-suite",
        3,
        &source_digest,
        &command_digest,
        &config_digest,
        hardware_digest,
        &dataset_digest,
        &comparator_version,
    )
}

fn backend_suite_hardware_digest(
    backend: &str,
    artifact_statuses: &[BackendSuiteArtifact],
) -> (
    String,
    Vec<HardwareDigestField>,
    Vec<HardwareUnavailableReason>,
    Vec<String>,
) {
    let mut fields = Vec::new();
    let mut unavailable = Vec::new();
    let mut blockers = Vec::new();
    record_string_hardware_field(
        &mut fields,
        &mut blockers,
        artifact_statuses,
        "host_cpu_model",
        "artifact.environment.host_cpu_model|cpu_model|host_cpu",
        |status| status.host_cpu_model.clone(),
    );
    if backend == "cuda" {
        record_string_hardware_field(
            &mut fields,
            &mut blockers,
            artifact_statuses,
            "gpu_model",
            "artifact.environment.gpu_devices[0].name",
            |status| status.gpu_model.clone(),
        );
        record_u64_hardware_field(
            &mut fields,
            &mut blockers,
            artifact_statuses,
            "gpu_memory_total_mib",
            "artifact.environment.gpu_devices[0].memory_total_mib",
            |status| status.gpu_memory_total_mib,
        );
        record_compute_capability_hardware_field(&mut fields, &mut blockers, artifact_statuses);
        record_string_hardware_field(
            &mut fields,
            &mut blockers,
            artifact_statuses,
            "nvidia_driver_version",
            "artifact.environment.nvidia_driver_version",
            |status| status.nvidia_driver_version.clone(),
        );
        record_string_hardware_field(
            &mut fields,
            &mut blockers,
            artifact_statuses,
            "cuda_toolkit_version",
            "artifact.environment.nvidia_cuda_version",
            |status| status.nvidia_cuda_version.clone(),
        );
        for (field, reason) in [
            (
                "host_ram_total_mib",
                "benchmark environment did not expose host RAM capacity",
            ),
            (
                "storage_filesystem_signature",
                "benchmark environment did not expose SSD or filesystem identity",
            ),
            (
                "os_kernel_version",
                "benchmark environment did not expose operating-system kernel version",
            ),
            (
                "gpu_ecc_state",
                "nvidia-smi benchmark capture did not expose visible ECC state",
            ),
            (
                "gpu_clock_mhz",
                "nvidia-smi benchmark capture did not expose graphics or memory clocks",
            ),
        ] {
            unavailable.push(HardwareUnavailableReason {
                field: field.to_string(),
                reason: reason.to_string(),
                fix: format!(
                    "Fix: extend benchmark environment capture to populate `{field}` before using hardware comparisons that depend on it."
                ),
            });
        }
    }
    let mut material = format!("benchmark-hardware-digest:v1\nbackend={backend}\n");
    for field in &fields {
        material.push_str("field=");
        material.push_str(&field.field);
        material.push_str("\nvalue=");
        material.push_str(&field.value);
        material.push_str("\nsource=");
        material.push_str(&field.source);
        material.push('\n');
    }
    for reason in &unavailable {
        material.push_str("unavailable=");
        material.push_str(&reason.field);
        material.push_str("\nreason=");
        material.push_str(&reason.reason);
        material.push_str("\nfix=");
        material.push_str(&reason.fix);
        material.push('\n');
    }
    let hardware_digest = format!(
        "benchmark-hardware-digest:v1:{}",
        crate::hash::sha256_hex(material.as_bytes())
    );
    (hardware_digest, fields, unavailable, blockers)
}

fn record_string_hardware_field(
    fields: &mut Vec<HardwareDigestField>,
    blockers: &mut Vec<String>,
    artifact_statuses: &[BackendSuiteArtifact],
    field: &str,
    source: &str,
    extractor: impl Fn(&BackendSuiteArtifact) -> Option<String>,
) {
    let values = artifact_statuses
        .iter()
        .filter_map(extractor)
        .collect::<BTreeSet<_>>();
    record_hardware_field_values(fields, blockers, field, source, values);
}

fn record_u64_hardware_field(
    fields: &mut Vec<HardwareDigestField>,
    blockers: &mut Vec<String>,
    artifact_statuses: &[BackendSuiteArtifact],
    field: &str,
    source: &str,
    extractor: impl Fn(&BackendSuiteArtifact) -> Option<u64>,
) {
    let values = artifact_statuses
        .iter()
        .filter_map(extractor)
        .map(|value| value.to_string())
        .collect::<BTreeSet<_>>();
    record_hardware_field_values(fields, blockers, field, source, values);
}

fn record_compute_capability_hardware_field(
    fields: &mut Vec<HardwareDigestField>,
    blockers: &mut Vec<String>,
    artifact_statuses: &[BackendSuiteArtifact],
) {
    let values = artifact_statuses
        .iter()
        .filter_map(|status| {
            Some(format!(
                "{}.{}",
                status.gpu_compute_capability_major?,
                status.gpu_compute_capability_minor?
            ))
        })
        .collect::<BTreeSet<_>>();
    record_hardware_field_values(
        fields,
        blockers,
        "gpu_compute_capability",
        "artifact.environment.gpu_devices[0].compute_capability_major/minor",
        values,
    );
}

fn record_hardware_field_values(
    fields: &mut Vec<HardwareDigestField>,
    blockers: &mut Vec<String>,
    field: &str,
    source: &str,
    values: BTreeSet<String>,
) {
    if values.is_empty() {
        blockers.push(format!(
            "hardware digest field `{field}` has no artifact-backed value"
        ));
        return;
    }
    if values.len() > 1 {
        blockers.push(format!(
            "hardware digest field `{field}` has {} distinct values; release benchmark suites must not merge incomparable hardware",
            values.len()
        ));
    }
    fields.push(HardwareDigestField {
        field: field.to_string(),
        value: values.into_iter().collect::<Vec<_>>().join(" | "),
        source: source.to_string(),
    });
}

fn backend_suite_input_family_counts(
    artifact_inputs: &[BackendSuiteArtifactInput],
) -> BTreeMap<String, usize> {
    artifact_inputs
        .iter()
        .filter_map(|artifact| {
            let family_id = artifact.family_id.trim();
            (!family_id.is_empty()).then(|| family_id.to_string())
        })
        .fold(BTreeMap::new(), |mut counts, family_id| {
            *counts.entry(family_id).or_default() += 1;
            counts
        })
}

fn backend_suite_input_path_counts(
    artifact_inputs: &[BackendSuiteArtifactInput],
) -> BTreeMap<String, usize> {
    artifact_inputs
        .iter()
        .filter_map(|artifact| {
            let path = artifact.path.trim();
            (!path.is_empty()).then(|| path.to_string())
        })
        .fold(BTreeMap::new(), |mut counts, path| {
            *counts.entry(path).or_default() += 1;
            counts
        })
}

pub(super) fn backend_suite_output_path(backend: &str) -> String {
    match backend {
        "cuda" => "release/evidence/benchmarks/cuda-release-suite.json".to_string(),
        "wgpu" => "release/evidence/benchmarks/wgpu-fallback-suite.json".to_string(),
        other => format!("release/evidence/benchmarks/{other}-release-suite.json"),
    }
}

pub(super) fn inspect_backend_suite_artifact(
    workspace_root: &Path,
    backend: &str,
    artifact: &BackendSuiteArtifactInput,
) -> BackendSuiteArtifact {
    let path = workspace_root.join(&artifact.path);
    let (exists, bytes, read_error) = match fs::metadata(&path) {
        Ok(metadata) => (metadata.is_file(), metadata.len(), None),
        Err(error) => {
            let label = if error.kind() == std::io::ErrorKind::NotFound {
                "missing".to_string()
            } else {
                format!("unreadable metadata: {error}")
            };
            (false, 0, Some(label))
        }
    };
    let mut blockers = Vec::new();
    if let Some(error) = &read_error {
        blockers.push(error.clone());
    }
    if !exists {
        if read_error.is_none() {
            blockers.push("not a file".to_string());
        }
        return BackendSuiteArtifact {
            path: artifact.path.clone(),
            family_id: artifact.family_id.clone(),
            requested_case_id: artifact.requested_case_id.clone(),
            exists,
            bytes,
            read_error,
            source_fingerprint: None,
            source_tree_fingerprint: None,
            selected_backend: None,
            host_cpu_model: None,
            gpu_model: None,
            gpu_memory_total_mib: None,
            gpu_compute_capability_major: None,
            gpu_compute_capability_minor: None,
            nvidia_driver_version: None,
            nvidia_cuda_version: None,
            min_cuda_ptx_source_cache_entries: None,
            min_cuda_ptx_source_cache_hits: None,
            min_cuda_ptx_source_cache_misses: None,
            min_kernel_launches: None,
            fused_execution_dag_contract: None,
            fused_execution_dag_node_count: None,
            fused_execution_dag_memory_edge_count: None,
            fused_execution_dag_host_sync_points: None,
            case_count: 0,
            failed_count: None,
            nonmatching_case_backend_count: 0,
            min_wall_samples: None,
            min_wall_p50: None,
            min_wall_p95: None,
            min_wall_p99: None,
            min_baseline_wall_samples: None,
            min_baseline_wall_p50: None,
            min_baseline_wall_p95: None,
            min_baseline_wall_p99: None,
            cpu_sota_100x_required: artifact.cpu_sota_100x_required,
            cpu_sota_100x_contract_cases: 0,
            cpu_sota_100x_passing_cases: 0,
            blockers,
        };
    }
    if bytes == 0 {
        blockers.push("empty".to_string());
    }
    let text = match read_text_bounded(&path, MAX_RELEASE_BENCHMARK_TEXT_BYTES) {
        Ok(text) => text,
        Err(error) => {
            blockers.push(format!("unreadable JSON: {error}"));
            String::new()
        }
    };
    let report = if text.is_empty() {
        Value::Null
    } else {
        match serde_json::from_str::<Value>(&text) {
            Ok(report) => report,
            Err(error) => {
                blockers.push(format!("invalid JSON: {error}"));
                Value::Null
            }
        }
    };
    let selected_backend = report
        .get("selected_backend")
        .and_then(Value::as_str)
        .map(str::to_string);
    let source_fingerprint = report
        .get("source_fingerprint")
        .and_then(nonblank_str)
        .map(str::to_string);
    let source_tree_fingerprint = report
        .get("source_tree_fingerprint")
        .and_then(nonblank_str)
        .map(str::to_string);
    if source_tree_fingerprint.is_none() {
        blockers.push("artifact has no source_tree_fingerprint provenance".to_string());
    }
    match &source_fingerprint {
        Some(fingerprint)
            if !crate::benchmark_evidence_semantics::source_fingerprint_issues(fingerprint)
                .is_empty() =>
        {
            blockers.push(format!(
                "source_fingerprint `{fingerprint}` is not release-grade provenance"
            ));
        }
        None => blockers.push("artifact has no source_fingerprint provenance".to_string()),
        Some(_) => {}
    }
    if let (Some((field, fingerprint)), Some(current_fingerprint)) = (
        crate::benchmark_evidence_semantics::report_freshness_fingerprint(&report),
        crate::benchmark_evidence_semantics::current_freshness_fingerprint_for_report(
            &path, &report,
        ),
    ) {
        for issue in crate::benchmark_evidence_semantics::source_fingerprint_freshness_issues(
            fingerprint,
            &current_fingerprint,
        ) {
            match issue {
                crate::benchmark_evidence_semantics::SourceFingerprintFreshnessIssue::Mismatch {
                    source_fingerprint,
                    current_source_fingerprint,
                } => blockers.push(format!(
                    "{field} `{source_fingerprint}` does not match current workspace source `{current_source_fingerprint}`"
                )),
            }
        }
    }
    if selected_backend.as_deref() != Some(backend) {
        blockers.push(format!(
            "selected_backend `{:?}` does not match requested backend `{backend}`",
            selected_backend
        ));
    }
    let requires_fused_execution_dag =
        artifact.family_id == "compound-fused-filter"
            || artifact.requested_case_id == "compound.pipeline.fused_filter.1m";
    if requires_fused_execution_dag {
        blockers.extend(
            crate::benchmark_evidence_semantics::benchmark_fused_execution_dag_issues(
                &artifact.path,
                &report,
            ),
        );
    }
    let fused_execution_dag = report.get("fused_execution_dag");
    let fused_execution_dag_contract = fused_execution_dag
        .and_then(|dag| dag.get("contract"))
        .and_then(nonblank_str)
        .map(str::to_string)
        .or_else(|| requires_fused_execution_dag.then(|| "fused-execution-dag:v1".to_string()));
    let fused_execution_dag_node_count = fused_execution_dag
        .and_then(|dag| dag.get("graph_nodes"))
        .and_then(Value::as_array)
        .map(Vec::len);
    let fused_execution_dag_memory_edge_count = fused_execution_dag
        .and_then(|dag| dag.get("memory_edges"))
        .and_then(Value::as_array)
        .map(Vec::len);
    let fused_execution_dag_host_sync_points = fused_execution_dag
        .and_then(|dag| dag.get("host_sync_points"))
        .and_then(Value::as_u64);
    let environment = report.get("environment");
    let first_gpu = environment
        .and_then(|environment| environment.get("gpu_devices"))
        .and_then(Value::as_array)
        .and_then(|devices| devices.first());
    let gpu_model = first_gpu
        .and_then(|device| device.get("name"))
        .and_then(nonblank_str)
        .map(str::to_string);
    let gpu_memory_total_mib = first_gpu
        .and_then(|device| device.get("memory_total_mib"))
        .and_then(Value::as_u64);
    let gpu_compute_capability_major = first_gpu
        .and_then(|device| device.get("compute_capability_major"))
        .and_then(Value::as_u64);
    let gpu_compute_capability_minor = first_gpu
        .and_then(|device| device.get("compute_capability_minor"))
        .and_then(Value::as_u64);
    let host_cpu_model = environment
        .and_then(|environment| {
            environment
                .get("host_cpu_model")
                .or_else(|| environment.get("cpu_model"))
                .or_else(|| environment.get("host_cpu"))
        })
        .and_then(nonblank_str)
        .map(str::to_string);
    let nvidia_driver_version = environment
        .and_then(|environment| environment.get("nvidia_driver_version"))
        .and_then(nonblank_str)
        .map(str::to_string);
    let nvidia_cuda_version = environment
        .and_then(|environment| environment.get("nvidia_cuda_version"))
        .and_then(nonblank_str)
        .map(str::to_string);
    if backend == "cuda" {
        if gpu_model.is_none() {
            blockers.push("CUDA artifact has no nvidia-smi GPU model provenance".to_string());
        }
        if nvidia_driver_version.is_none() {
            blockers.push(
                "CUDA artifact has no nvidia-smi NVIDIA driver version provenance".to_string(),
            );
        }
        if nvidia_cuda_version.is_none() {
            blockers.push(
                "CUDA artifact has no nvidia-smi CUDA runtime version provenance".to_string(),
            );
        }
        match gpu_memory_total_mib {
            Some(mib) if mib >= MIN_CUDA_RELEASE_MEMORY_MIB => {}
            Some(mib) => blockers.push(format!(
                "CUDA artifact GPU memory is {mib} MiB, below release floor {MIN_CUDA_RELEASE_MEMORY_MIB} MiB"
            )),
            None => blockers.push("CUDA artifact has no nvidia-smi GPU memory provenance".to_string()),
        }
        match (gpu_compute_capability_major, gpu_compute_capability_minor) {
            (Some(major), Some(minor))
                if (major, minor)
                    >= (
                        MIN_CUDA_RELEASE_COMPUTE_CAPABILITY_MAJOR,
                        MIN_CUDA_RELEASE_COMPUTE_CAPABILITY_MINOR,
                    ) => {}
            (Some(major), Some(minor)) => blockers.push(format!(
                "CUDA artifact compute capability is {major}.{minor}, below release floor {MIN_CUDA_RELEASE_COMPUTE_CAPABILITY_MAJOR}.{MIN_CUDA_RELEASE_COMPUTE_CAPABILITY_MINOR}"
            )),
            _ => blockers.push(
                "CUDA artifact has no nvidia-smi compute capability provenance".to_string(),
            ),
        }
    }
    let summary_failed_count = report
        .get("summary")
        .and_then(|summary| summary.get("failed"))
        .and_then(Value::as_u64);
    if summary_failed_count != Some(0) {
        blockers.push(format!(
            "summary.failed is `{:?}`, expected 0",
            summary_failed_count
        ));
    }
    let cases = report
        .get("cases")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    if cases.is_empty() {
        blockers.push("cases array is empty or missing".to_string());
    }
    if let Some(mismatch) =
        crate::benchmark_evidence_semantics::benchmark_report_summary_case_evidence_mismatch(
            &report,
        )
    {
        blockers.push(format!("benchmark summary is invalid: {mismatch}"));
    }
    crate::benchmark_evidence_semantics::inspect_source_artifact_case_integrity(
        &artifact.path,
        &report,
        &format!("backend `{backend}` release suite artifact"),
        &mut blockers,
    );
    let mut case_failed_count = 0u64;
    let mut nonmatching_case_backend_count = 0usize;
    let mut min_wall_samples = None::<u64>;
    let mut min_baseline_wall_samples = None::<u64>;
    let mut min_wall_p50 = None::<u64>;
    let mut min_wall_p95 = None::<u64>;
    let mut min_wall_p99 = None::<u64>;
    let mut min_baseline_wall_p50 = None::<u64>;
    let mut min_baseline_wall_p95 = None::<u64>;
    let mut min_baseline_wall_p99 = None::<u64>;
    let mut min_cuda_ptx_source_cache_entries = None::<u64>;
    let mut min_cuda_ptx_source_cache_hits = None::<u64>;
    let mut min_cuda_ptx_source_cache_misses = None::<u64>;
    let mut min_kernel_launches = None::<u64>;
    let mut requested_case_count = 0usize;
    for case in &cases {
        let case_id = case
            .get("id")
            .and_then(Value::as_str)
            .unwrap_or("<unknown>");
        let case_failure_reason =
            crate::benchmark_evidence_semantics::benchmark_case_failure_reason(case);
        if let Some(reason) = &case_failure_reason {
            case_failed_count += 1;
            blockers.push(format!("case `{case_id}` failed: {reason}"));
        }
        if case_id == artifact.requested_case_id {
            requested_case_count += 1;
        }
        if case.get("backend_id").and_then(Value::as_str) != Some(backend) {
            nonmatching_case_backend_count += 1;
        }
        let metrics = case.get("metrics").and_then(Value::as_object);
        let wall_samples = metrics
            .and_then(|metrics| suite_metric_samples(metrics.get("wall_ns")))
            .unwrap_or(0);
        min_wall_samples = Some(min_wall_samples.map_or(wall_samples, |min| min.min(wall_samples)));
        if wall_samples < 30 {
            blockers.push(format!(
                "case `{case_id}` has {wall_samples} wall_ns sample(s), needs at least 30"
            ));
        }
        let baseline_wall_samples = metrics
            .and_then(|metrics| suite_metric_samples(metrics.get("baseline_wall_ns")))
            .unwrap_or(0);
        min_baseline_wall_samples = Some(
            min_baseline_wall_samples
                .map_or(baseline_wall_samples, |min| min.min(baseline_wall_samples)),
        );
        if baseline_wall_samples < 30 {
            blockers.push(format!(
                "case `{case_id}` has {baseline_wall_samples} baseline_wall_ns sample(s), needs at least 30"
            ));
        }
        record_required_metric_percentile(
            &mut min_wall_p50,
            metrics,
            "wall_ns",
            "p50",
            &mut blockers,
            case_id,
        );
        record_required_metric_percentile(
            &mut min_wall_p95,
            metrics,
            "wall_ns",
            "p95",
            &mut blockers,
            case_id,
        );
        record_required_metric_percentile(
            &mut min_wall_p99,
            metrics,
            "wall_ns",
            "p99",
            &mut blockers,
            case_id,
        );
        record_required_metric_percentile(
            &mut min_baseline_wall_p50,
            metrics,
            "baseline_wall_ns",
            "p50",
            &mut blockers,
            case_id,
        );
        record_required_metric_percentile(
            &mut min_baseline_wall_p95,
            metrics,
            "baseline_wall_ns",
            "p95",
            &mut blockers,
            case_id,
        );
        record_required_metric_percentile(
            &mut min_baseline_wall_p99,
            metrics,
            "baseline_wall_ns",
            "p99",
            &mut blockers,
            case_id,
        );
        if matches!(backend, "cuda" | "wgpu") {
            record_required_metric_percentile(
                &mut min_kernel_launches,
                metrics,
                "kernel_launches",
                "p50",
                &mut blockers,
                case_id,
            );
        }
        if backend == "cuda" {
            record_required_metric_percentile(
                &mut min_cuda_ptx_source_cache_entries,
                metrics,
                "cuda_ptx_source_cache_entries",
                "p50",
                &mut blockers,
                case_id,
            );
            record_observed_metric_percentile(
                &mut min_cuda_ptx_source_cache_hits,
                metrics,
                "cuda_ptx_source_cache_hits",
                "p50",
                &mut blockers,
                case_id,
            );
            record_observed_metric_percentile(
                &mut min_cuda_ptx_source_cache_misses,
                metrics,
                "cuda_ptx_source_cache_misses",
                "p50",
                &mut blockers,
                case_id,
            );
        }
    }
    let (cpu_sota_100x_contract_cases, cpu_sota_100x_passing_cases) =
        crate::benchmark_evidence_semantics::cpu_sota_100x_case_counts(&report);
    let cpu_sota_100x_contract_cases = cpu_sota_100x_contract_cases as usize;
    let cpu_sota_100x_passing_cases = cpu_sota_100x_passing_cases as usize;
    if !cases.is_empty() && summary_failed_count != Some(case_failed_count) {
        blockers.push(format!(
            "summary.failed is `{:?}` but case evidence reports {case_failed_count} failed case(s)",
            summary_failed_count
        ));
    }
    let failed_count = (!cases.is_empty())
        .then_some(case_failed_count)
        .or(summary_failed_count);
    if nonmatching_case_backend_count > 0 {
        blockers.push(format!(
            "{nonmatching_case_backend_count} case(s) do not match requested backend `{backend}`"
        ));
    }
    if requested_case_count == 0 {
        blockers.push(format!(
            "requested case `{}` is absent from artifact cases",
            artifact.requested_case_id
        ));
    } else if requested_case_count > 1 {
        blockers.push(format!(
            "requested case `{}` appears {requested_case_count} times in artifact cases",
            artifact.requested_case_id
        ));
    }
    if artifact.cpu_sota_100x_required && cpu_sota_100x_contract_cases == 0 {
        blockers.push("CPU-SOTA 100x workload artifact has no 100x contract case".to_string());
    }
    if artifact.cpu_sota_100x_required && cpu_sota_100x_passing_cases == 0 {
        blockers.push("CPU-SOTA 100x workload artifact has no passing 100x case".to_string());
    }
    BackendSuiteArtifact {
        path: artifact.path.clone(),
        family_id: artifact.family_id.clone(),
        requested_case_id: artifact.requested_case_id.clone(),
        exists,
        bytes,
        read_error,
        source_fingerprint,
        source_tree_fingerprint,
        selected_backend,
        host_cpu_model,
        gpu_model,
        gpu_memory_total_mib,
        gpu_compute_capability_major,
        gpu_compute_capability_minor,
        nvidia_driver_version,
        nvidia_cuda_version,
        min_cuda_ptx_source_cache_entries,
        min_cuda_ptx_source_cache_hits,
        min_cuda_ptx_source_cache_misses,
        min_kernel_launches,
        fused_execution_dag_contract,
        fused_execution_dag_node_count,
        fused_execution_dag_memory_edge_count,
        fused_execution_dag_host_sync_points,
        case_count: cases.len(),
        failed_count,
        nonmatching_case_backend_count,
        min_wall_samples,
        min_wall_p50,
        min_wall_p95,
        min_wall_p99,
        min_baseline_wall_samples,
        min_baseline_wall_p50,
        min_baseline_wall_p95,
        min_baseline_wall_p99,
        cpu_sota_100x_required: artifact.cpu_sota_100x_required,
        cpu_sota_100x_contract_cases,
        cpu_sota_100x_passing_cases,
        blockers,
    }
}

pub(super) fn suite_metric_samples(value: Option<&Value>) -> Option<u64> {
    value
        .and_then(|metric| metric.get("samples"))
        .and_then(Value::as_u64)
}

pub(super) fn suite_metric_percentile(value: Option<&Value>, percentile: &str) -> Option<u64> {
    value
        .and_then(|metric| metric.get(percentile))
        .and_then(Value::as_u64)
        .or_else(|| {
            value
                .and_then(|metric| metric.get(percentile))
                .and_then(Value::as_f64)
                .filter(|value| *value >= 0.0)
                .map(|value| value as u64)
        })
}

fn nonblank_str(value: &Value) -> Option<&str> {
    value.as_str().filter(|value| !value.trim().is_empty())
}

pub(super) fn record_required_metric_percentile(
    current_min: &mut Option<u64>,
    metrics: Option<&serde_json::Map<String, Value>>,
    metric_name: &str,
    percentile: &str,
    blockers: &mut Vec<String>,
    case_id: &str,
) {
    match metrics.and_then(|metrics| suite_metric_percentile(metrics.get(metric_name), percentile))
    {
        Some(value) if value > 0 => {
            *current_min = Some(current_min.map_or(value, |min| min.min(value)));
        }
        _ => blockers.push(format!(
            "case `{case_id}` must include positive {percentile} {metric_name}"
        )),
    }
}

pub(super) fn record_observed_metric_percentile(
    current_min: &mut Option<u64>,
    metrics: Option<&serde_json::Map<String, Value>>,
    metric_name: &str,
    percentile: &str,
    blockers: &mut Vec<String>,
    case_id: &str,
) {
    match metrics.and_then(|metrics| suite_metric_percentile(metrics.get(metric_name), percentile))
    {
        Some(value) => {
            *current_min = Some(current_min.map_or(value, |min| min.min(value)));
        }
        None => blockers.push(format!(
            "case `{case_id}` must include {percentile} {metric_name}"
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use tempfile::TempDir;

    #[test]
    fn wgpu_suite_output_matches_release_gate_contract() {
        assert_eq!(
            backend_suite_output_path("wgpu"),
            "release/evidence/benchmarks/wgpu-fallback-suite.json",
            "Fix: release-benchmarks must regenerate the WGPU suite artifact consumed by the release gate and completion audit."
        );
    }

    #[test]
    fn cuda_suite_output_matches_release_gate_contract() {
        assert_eq!(
            backend_suite_output_path("cuda"),
            "release/evidence/benchmarks/cuda-release-suite.json",
            "Fix: release-benchmarks must regenerate the CUDA suite artifact consumed by the release gate and completion audit."
        );
    }

    #[test]
    fn write_wgpu_suite_regenerates_gated_fallback_artifact() {
        let dir = TempDir::new().expect("Fix: create a temporary workspace for suite output test.");

        write_backend_suite(dir.path(), "wgpu", Vec::new());

        let fallback = dir
            .path()
            .join("release/evidence/benchmarks/wgpu-fallback-suite.json");
        let comparison = dir
            .path()
            .join("release/evidence/benchmarks/wgpu-comparison-suite.json");
        assert!(
            fallback.exists(),
            "Fix: WGPU release benchmark generation must write the suite artifact consumed by the release gate."
        );
        assert!(
            !comparison.exists(),
            "Fix: WGPU release benchmark generation must not write the stale comparison suite path instead of the gated fallback suite."
        );
        let text = fs::read_to_string(&fallback)
            .expect("Fix: read generated WGPU fallback suite JSON for contract assertions.");
        let suite = serde_json::from_str::<Value>(&text)
            .expect("Fix: generated WGPU fallback suite JSON must be parseable.");
        assert_eq!(
            suite.get("backend").and_then(Value::as_str),
            Some("wgpu"),
            "Fix: generated WGPU fallback suite must retain backend provenance."
        );
    }

    #[test]
    fn write_backend_suite_records_workload_run_failures() {
        let dir = TempDir::new()
            .expect("Fix: create a temporary workspace for suite run-failure blocker test.");

        write_backend_suite_with_extra_blockers(
            dir.path(),
            "wgpu",
            Vec::new(),
            vec![
                "backend `wgpu` comparison family `string-bitmap-scatter` case `release.string_bitmap_scatter.1m` artifact `release/evidence/benchmarks/wgpu-workload-02-string-bitmap-scatter.json`: Fix: benchmark command failed with exit status 1"
                    .to_string(),
            ],
        );

        let suite_path = dir
            .path()
            .join("release/evidence/benchmarks/wgpu-fallback-suite.json");
        let text = fs::read_to_string(&suite_path)
            .expect("Fix: read generated WGPU fallback suite JSON for run-failure assertions.");
        let suite = serde_json::from_str::<Value>(&text)
            .expect("Fix: generated WGPU fallback suite JSON must be parseable.");
        let blockers = suite
            .get("blockers")
            .and_then(Value::as_array)
            .expect("Fix: generated suite must carry blockers array.");

        assert!(
            blockers.iter().any(|blocker| {
                blocker.as_str().is_some_and(|blocker| {
                    blocker.contains("comparison family `string-bitmap-scatter`")
                        && blocker.contains("benchmark command failed")
                })
            }),
            "Fix: backend suite evidence must record benchmark run failures instead of leaving them only on stderr; blockers={blockers:?}"
        );
    }

    #[test]
    fn write_backend_suite_rejects_duplicate_family_input_coverage() {
        let dir = TempDir::new()
            .expect("Fix: create a temporary workspace for suite duplicate family test.");

        write_backend_suite(
            dir.path(),
            "wgpu",
            vec![
                BackendSuiteArtifactInput {
                    path: "release/evidence/benchmarks/wgpu-condition-fast.json".to_string(),
                    family_id: "condition-eval".to_string(),
                    requested_case_id: "release.condition_eval.1m".to_string(),
                    cpu_sota_100x_required: false,
                },
                BackendSuiteArtifactInput {
                    path: "release/evidence/benchmarks/wgpu-condition-slow.json".to_string(),
                    family_id: "condition-eval".to_string(),
                    requested_case_id: "release.condition_eval.10m".to_string(),
                    cpu_sota_100x_required: false,
                },
            ],
        );

        let suite_path = dir
            .path()
            .join("release/evidence/benchmarks/wgpu-fallback-suite.json");
        let text = fs::read_to_string(&suite_path)
            .expect("Fix: read generated WGPU fallback suite JSON for duplicate family test.");
        let suite = serde_json::from_str::<Value>(&text)
            .expect("Fix: generated WGPU fallback suite JSON must be parseable.");
        let blockers = suite
            .get("blockers")
            .and_then(Value::as_array)
            .expect("Fix: generated suite must carry blockers array.");

        assert_eq!(
            suite.get("family_count").and_then(Value::as_u64),
            Some(1),
            "Fix: generated backend suite family_count must count unique workload families, not raw artifact inputs."
        );
        assert!(
            blockers.iter().any(|blocker| {
                blocker.as_str().is_some_and(|blocker| {
                    blocker.contains("has 2 artifact input(s) for family `condition-eval`")
                })
            }),
            "Fix: generated backend suite evidence must preserve duplicate family input blockers; blockers={blockers:?}"
        );
    }

    #[test]
    fn write_backend_suite_rejects_duplicate_artifact_input_paths() {
        let dir = TempDir::new()
            .expect("Fix: create a temporary workspace for suite duplicate path test.");
        let artifact_rel = "release/evidence/benchmarks/wgpu-shared-path.json";

        write_backend_suite(
            dir.path(),
            "wgpu",
            vec![
                BackendSuiteArtifactInput {
                    path: artifact_rel.to_string(),
                    family_id: "condition-eval".to_string(),
                    requested_case_id: "release.condition_eval.1m".to_string(),
                    cpu_sota_100x_required: false,
                },
                BackendSuiteArtifactInput {
                    path: artifact_rel.to_string(),
                    family_id: "entropy-window".to_string(),
                    requested_case_id: "release.entropy_window.1m".to_string(),
                    cpu_sota_100x_required: false,
                },
            ],
        );

        let suite_path = dir
            .path()
            .join("release/evidence/benchmarks/wgpu-fallback-suite.json");
        let text = fs::read_to_string(&suite_path)
            .expect("Fix: read generated WGPU fallback suite JSON for duplicate path test.");
        let suite = serde_json::from_str::<Value>(&text)
            .expect("Fix: generated WGPU fallback suite JSON must be parseable.");
        let blockers = suite
            .get("blockers")
            .and_then(Value::as_array)
            .expect("Fix: generated suite must carry blockers array.");

        assert!(
            blockers.iter().any(|blocker| {
                blocker.as_str().is_some_and(|blocker| {
                    blocker.contains(
                        "backend `wgpu` release suite has 2 artifact input(s) for path `release/evidence/benchmarks/wgpu-shared-path.json`",
                    )
                })
            }),
            "Fix: generated backend suite evidence must reject duplicate artifact input paths; blockers={blockers:?}"
        );
    }

    #[test]
    fn write_backend_suite_rejects_blank_requested_case_input() {
        let dir = TempDir::new()
            .expect("Fix: create a temporary workspace for suite blank requested-case test.");
        let artifact_rel = "release/evidence/benchmarks/wgpu-blank-requested-case.json";
        let artifact_path = dir.path().join(artifact_rel);
        fs::create_dir_all(
            artifact_path
                .parent()
                .expect("Fix: blank requested-case artifact path must have a parent directory."),
        )
        .expect("Fix: create blank requested-case artifact parent directory.");
        fs::write(
            &artifact_path,
            serde_json::to_string_pretty(&json!({
                "selected_backend": "wgpu",
                "source_fingerprint": "git:abc:dirty=false",
                "source_tree_fingerprint": "source-tree-v1:abc",
                "summary": {"total_cases": 1, "passed": 1, "failed": 0},
                "cases": [
                    {
                        "id": "release.condition_eval.1m",
                        "backend_id": "wgpu",
                        "status": "pass",
                        "metrics": {
                            "wall_ns": {"samples": 30, "p50": 10, "p95": 11, "p99": 12},
                            "baseline_wall_ns": {"samples": 30, "p50": 1000, "p95": 1001, "p99": 1002}
                        }
                    }
                ]
            }))
            .expect("Fix: serialize blank requested-case benchmark artifact JSON."),
        )
        .expect("Fix: write blank requested-case benchmark artifact JSON.");

        write_backend_suite(
            dir.path(),
            "wgpu",
            vec![BackendSuiteArtifactInput {
                path: artifact_rel.to_string(),
                family_id: "condition-eval".to_string(),
                requested_case_id: " \t ".to_string(),
                cpu_sota_100x_required: false,
            }],
        );

        let suite_path = dir
            .path()
            .join("release/evidence/benchmarks/wgpu-fallback-suite.json");
        let text = fs::read_to_string(&suite_path)
            .expect("Fix: read generated WGPU fallback suite JSON for blank requested-case test.");
        let suite = serde_json::from_str::<Value>(&text)
            .expect("Fix: generated WGPU fallback suite JSON must be parseable.");
        let blockers = suite
            .get("blockers")
            .and_then(Value::as_array)
            .expect("Fix: generated suite must carry blockers array.");

        assert!(
            blockers.iter().any(|blocker| {
                blocker.as_str().is_some_and(|blocker| {
                    blocker.contains(
                        "backend `wgpu` release suite artifact `release/evidence/benchmarks/wgpu-blank-requested-case.json` has blank requested_case_id",
                    )
                })
            }),
            "Fix: generated backend suite evidence must reject blank requested_case_id inputs; blockers={blockers:?}"
        );
    }

    #[test]
    fn suite_artifact_status_rejects_whitespace_only_provenance() {
        let dir = TempDir::new()
            .expect("Fix: create a temporary workspace for suite blank provenance test.");
        let artifact_rel = "release/evidence/benchmarks/cuda-blank-provenance.json";
        let artifact_path = dir.path().join(artifact_rel);
        fs::create_dir_all(
            artifact_path
                .parent()
                .expect("Fix: suite artifact must have parent directory."),
        )
        .expect("Fix: create blank provenance suite artifact parent directory.");
        fs::write(
            &artifact_path,
            serde_json::to_string_pretty(&json!({
                "schema_version": 2,
                "selected_backend": "cuda",
                "source_fingerprint": "   ",
                "source_tree_fingerprint": "\t",
                "summary": {"total_cases": 1, "passed": 1, "failed": 0},
                "environment": {
                    "host_cpu_model": " ",
                    "gpu_devices": [
                        {
                            "name": " ",
                            "memory_total_mib": 24576,
                            "compute_capability_major": 8,
                            "compute_capability_minor": 9
                        }
                    ],
                    "nvidia_driver_version": "\t",
                    "nvidia_cuda_version": "\n"
                },
                "cases": [
                    {
                        "id": "release.condition_eval.1m",
                        "backend_id": "cuda",
                        "status": "pass",
                        "metrics": {
                            "wall_ns": {"samples": 30, "p50": 10, "p95": 11, "p99": 12},
                            "baseline_wall_ns": {"samples": 30, "p50": 1000, "p95": 1001, "p99": 1002},
                            "kernel_launches": {"samples": 1, "p50": 1}
                        },
                        "performance": {"contract_passed": true, "speedup_x": 120.0}
                    }
                ]
            }))
            .expect("Fix: serialize blank provenance benchmark artifact JSON."),
        )
        .expect("Fix: write blank provenance benchmark artifact JSON.");

        let status = inspect_backend_suite_artifact(
            dir.path(),
            "cuda",
            &BackendSuiteArtifactInput {
                path: artifact_rel.to_string(),
                family_id: "condition-eval".to_string(),
                requested_case_id: "release.condition_eval.1m".to_string(),
                cpu_sota_100x_required: false,
            },
        );

        assert_eq!(
            status.source_fingerprint, None,
            "Fix: whitespace-only source_fingerprint must not be serialized as suite provenance."
        );
        assert_eq!(
            status.source_tree_fingerprint, None,
            "Fix: whitespace-only source_tree_fingerprint must not be serialized as suite provenance."
        );
        assert_eq!(
            status.host_cpu_model, None,
            "Fix: whitespace-only host_cpu_model must not be serialized as suite provenance."
        );
        for expected in [
            "artifact has no source_fingerprint provenance",
            "artifact has no source_tree_fingerprint provenance",
            "CUDA artifact has no nvidia-smi GPU model provenance",
            "CUDA artifact has no nvidia-smi NVIDIA driver version provenance",
            "CUDA artifact has no nvidia-smi CUDA runtime version provenance",
        ] {
            assert!(
                status
                    .blockers
                    .iter()
                    .any(|blocker| blocker.contains(expected)),
                "Fix: suite artifact inspection must reject whitespace-only CUDA provenance `{expected}`; blockers={:?}",
                status.blockers
            );
        }
    }

    #[test]
    fn suite_artifact_status_rejects_missing_and_weak_source_fingerprint() {
        let dir = TempDir::new()
            .expect("Fix: create a temporary workspace for suite source provenance test.");
        let artifacts = [
            (
                "release/evidence/benchmarks/cuda-missing-source-fingerprint.json",
                None,
                None,
                "artifact has no source_fingerprint provenance",
            ),
            (
                "release/evidence/benchmarks/cuda-git-commit-only-source.json",
                None,
                Some(json!({"commit": "abc123", "dirty": false})),
                "artifact has no source_fingerprint provenance",
            ),
            (
                "release/evidence/benchmarks/cuda-legacy-dirty-source-fingerprint.json",
                Some("git:abc123:dirty=true"),
                None,
                "source_fingerprint `git:abc123:dirty=true` is not release-grade provenance",
            ),
        ];

        for (artifact_rel, source_fingerprint, git, expected_blocker) in artifacts {
            let artifact_path = dir.path().join(artifact_rel);
            fs::create_dir_all(
                artifact_path
                    .parent()
                    .expect("Fix: suite artifact must have parent directory."),
            )
            .expect("Fix: create source provenance suite artifact parent directory.");
            let mut artifact = json!({
                "schema_version": 2,
                "selected_backend": "cuda",
                "source_tree_fingerprint": "source-tree-v1:abc",
                "summary": {"total_cases": 1, "passed": 1, "failed": 0},
                "environment": {
                    "host_cpu_model": "test CPU",
                    "gpu_devices": [
                        {
                            "name": "RTX 5090",
                            "memory_total_mib": 24576,
                            "compute_capability_major": 8,
                            "compute_capability_minor": 9
                        }
                    ],
                    "nvidia_driver_version": "580.0",
                    "nvidia_cuda_version": "13.0"
                },
                "cases": [
                    {
                        "id": "release.condition_eval.1m",
                        "backend_id": "cuda",
                        "status": "pass",
                        "metrics": {
                            "wall_ns": {"samples": 30, "p50": 10, "p95": 11, "p99": 12},
                            "baseline_wall_ns": {"samples": 30, "p50": 1000, "p95": 1001, "p99": 1002},
                            "kernel_launches": {"samples": 30, "p50": 1},
                            "cuda_ptx_source_cache_entries": {"samples": 30, "p50": 1},
                            "cuda_ptx_source_cache_hits": {"samples": 30, "p50": 1},
                            "cuda_ptx_source_cache_misses": {"samples": 30, "p50": 0}
                        },
                        "performance": {"contract_passed": true, "speedup_x": 120.0}
                    }
                ]
            });
            if let Some(source_fingerprint) = source_fingerprint {
                artifact["source_fingerprint"] = Value::String(source_fingerprint.to_string());
            }
            if let Some(git) = git {
                artifact["git"] = git;
            }
            fs::write(
                &artifact_path,
                serde_json::to_string_pretty(&artifact)
                    .expect("Fix: serialize source provenance benchmark artifact JSON."),
            )
            .expect("Fix: write source provenance benchmark artifact JSON.");

            let status = inspect_backend_suite_artifact(
                dir.path(),
                "cuda",
                &BackendSuiteArtifactInput {
                    path: artifact_rel.to_string(),
                    family_id: "condition-eval".to_string(),
                    requested_case_id: "release.condition_eval.1m".to_string(),
                    cpu_sota_100x_required: false,
                },
            );

            assert!(
                status
                    .blockers
                    .iter()
                    .any(|blocker| blocker.contains(expected_blocker)),
                "Fix: generated CUDA suite evidence must reject weak source fingerprint provenance `{expected_blocker}`; blockers={:?}",
                status.blockers
            );
        }
    }

    #[test]
    fn suite_artifact_status_rejects_stale_source_tree_fingerprint() {
        let dir = TempDir::new()
            .expect("Fix: create a temporary workspace for stale suite source-tree test.");
        fs::write(dir.path().join("Cargo.toml"), "[workspace]\n")
            .expect("Fix: create temp workspace Cargo.toml for source-tree freshness test.");
        let artifact_rel = "release/evidence/benchmarks/cuda-stale-source-tree.json";
        let artifact_path = dir.path().join(artifact_rel);
        fs::create_dir_all(
            artifact_path
                .parent()
                .expect("Fix: suite artifact must have parent directory."),
        )
        .expect("Fix: create stale source-tree suite artifact parent directory.");
        fs::write(
            &artifact_path,
            serde_json::to_string_pretty(&json!({
                "schema_version": 2,
                "selected_backend": "cuda",
                "source_fingerprint": "git:abc:dirty=false",
                "source_tree_fingerprint": "source-tree-v1:stale",
                "summary": {"total_cases": 1, "passed": 1, "failed": 0},
                "environment": {
                    "host_cpu_model": "test CPU",
                    "gpu_devices": [
                        {
                            "name": "RTX 5090",
                            "memory_total_mib": 24576,
                            "compute_capability_major": 8,
                            "compute_capability_minor": 9
                        }
                    ],
                    "nvidia_driver_version": "580.0",
                    "nvidia_cuda_version": "13.0"
                },
                "cases": [
                    {
                        "id": "release.condition_eval.1m",
                        "backend_id": "cuda",
                        "status": "pass",
                        "metrics": {
                            "wall_ns": {"samples": 30, "p50": 10, "p95": 11, "p99": 12},
                            "baseline_wall_ns": {"samples": 30, "p50": 1000, "p95": 1001, "p99": 1002},
                            "kernel_launches": {"samples": 30, "p50": 1},
                            "cuda_ptx_source_cache_entries": {"samples": 30, "p50": 1},
                            "cuda_ptx_source_cache_hits": {"samples": 30, "p50": 1},
                            "cuda_ptx_source_cache_misses": {"samples": 30, "p50": 0}
                        },
                        "performance": {"contract_passed": true, "speedup_x": 120.0}
                    }
                ]
            }))
            .expect("Fix: serialize stale source-tree benchmark artifact JSON."),
        )
        .expect("Fix: write stale source-tree benchmark artifact JSON.");

        let status = inspect_backend_suite_artifact(
            dir.path(),
            "cuda",
            &BackendSuiteArtifactInput {
                path: artifact_rel.to_string(),
                family_id: "condition-eval".to_string(),
                requested_case_id: "release.condition_eval.1m".to_string(),
                cpu_sota_100x_required: false,
            },
        );

        assert!(
            status.blockers.iter().any(|blocker| {
                blocker.contains("source_tree_fingerprint `source-tree-v1:stale`")
                    && blocker.contains("does not match current workspace source")
            }),
            "Fix: generated CUDA suite evidence must reject stale source-tree benchmark artifacts; blockers={:?}",
            status.blockers
        );
    }

    #[test]
    fn suite_artifact_status_rejects_duplicate_requested_case_rows() {
        let dir = TempDir::new()
            .expect("Fix: create a temporary workspace for duplicate requested-case suite test.");
        let artifact_rel = "release/evidence/benchmarks/cuda-duplicate-requested-case.json";
        let artifact_path = dir.path().join(artifact_rel);
        fs::create_dir_all(
            artifact_path
                .parent()
                .expect("Fix: suite artifact must have parent directory."),
        )
        .expect("Fix: create duplicate requested-case suite artifact parent directory.");
        fs::write(
            &artifact_path,
            serde_json::to_string_pretty(&json!({
                "schema_version": 2,
                "selected_backend": "cuda",
                "source_fingerprint": "git:abc:dirty=false",
                "source_tree_fingerprint": "source-tree-v1:abc",
                "summary": {"total_cases": 2, "passed": 2, "failed": 0},
                "environment": {
                    "host_cpu_model": "test CPU",
                    "gpu_devices": [
                        {
                            "name": "RTX 5090",
                            "memory_total_mib": 24576,
                            "compute_capability_major": 8,
                            "compute_capability_minor": 9
                        }
                    ],
                    "nvidia_driver_version": "580.0",
                    "nvidia_cuda_version": "13.0"
                },
                "cases": [
                    {
                        "id": "release.condition_eval.1m",
                        "backend_id": "cuda",
                        "status": "pass",
                        "metrics": {
                            "wall_ns": {"samples": 30, "p50": 10, "p95": 11, "p99": 12},
                            "baseline_wall_ns": {"samples": 30, "p50": 1000, "p95": 1001, "p99": 1002},
                            "kernel_launches": {"samples": 30, "p50": 1},
                            "cuda_ptx_source_cache_entries": {"samples": 30, "p50": 1},
                            "cuda_ptx_source_cache_hits": {"samples": 30, "p50": 1},
                            "cuda_ptx_source_cache_misses": {"samples": 30, "p50": 0}
                        },
                        "performance": {"contract_passed": true, "speedup_x": 120.0}
                    },
                    {
                        "id": "release.condition_eval.1m",
                        "backend_id": "cuda",
                        "status": "pass",
                        "metrics": {
                            "wall_ns": {"samples": 30, "p50": 12, "p95": 13, "p99": 14},
                            "baseline_wall_ns": {"samples": 30, "p50": 1200, "p95": 1201, "p99": 1202},
                            "kernel_launches": {"samples": 30, "p50": 1},
                            "cuda_ptx_source_cache_entries": {"samples": 30, "p50": 1},
                            "cuda_ptx_source_cache_hits": {"samples": 30, "p50": 1},
                            "cuda_ptx_source_cache_misses": {"samples": 30, "p50": 0}
                        },
                        "performance": {"contract_passed": true, "speedup_x": 100.0}
                    }
                ]
            }))
            .expect("Fix: serialize duplicate requested-case benchmark artifact JSON."),
        )
        .expect("Fix: write duplicate requested-case benchmark artifact JSON.");

        let status = inspect_backend_suite_artifact(
            dir.path(),
            "cuda",
            &BackendSuiteArtifactInput {
                path: artifact_rel.to_string(),
                family_id: "condition-eval".to_string(),
                requested_case_id: "release.condition_eval.1m".to_string(),
                cpu_sota_100x_required: false,
            },
        );

        assert!(
            status.blockers.iter().any(|blocker| blocker.contains(
                "requested case `release.condition_eval.1m` appears 2 times in artifact cases"
            )),
            "Fix: generated CUDA suite evidence must reject artifacts where the requested_case_id resolves to multiple benchmark rows; blockers={:?}",
            status.blockers
        );
    }

    #[test]
    fn suite_artifact_status_rejects_backend_mismatched_cpu_sota_counts() {
        let dir = TempDir::new()
            .expect("Fix: create a temporary workspace for backend-mismatch CPU-SOTA test.");
        let artifact_rel = "release/evidence/benchmarks/cuda-backend-mismatch-cpu-sota.json";
        let artifact_path = dir.path().join(artifact_rel);
        fs::create_dir_all(
            artifact_path
                .parent()
                .expect("Fix: suite artifact must have parent directory."),
        )
        .expect("Fix: create backend-mismatch CPU-SOTA suite artifact parent directory.");
        fs::write(
            &artifact_path,
            serde_json::to_string_pretty(&json!({
                "schema_version": 2,
                "selected_backend": "cuda",
                "source_fingerprint": "git:abc:dirty=false",
                "source_tree_fingerprint": "source-tree-v1:abc",
                "summary": {"total_cases": 1, "passed": 1, "failed": 0},
                "environment": {
                    "host_cpu_model": "test CPU",
                    "gpu_devices": [
                        {
                            "name": "RTX 5090",
                            "memory_total_mib": 24576,
                            "compute_capability_major": 8,
                            "compute_capability_minor": 9
                        }
                    ],
                    "nvidia_driver_version": "580.0",
                    "nvidia_cuda_version": "13.0"
                },
                "cases": [
                    {
                        "id": "release.condition_eval.1m",
                        "backend_id": "wgpu",
                        "status": "pass",
                        "metrics": {
                            "wall_ns": {"samples": 30, "p50": 10, "p95": 11, "p99": 12},
                            "baseline_wall_ns": {"samples": 30, "p50": 1000, "p95": 1001, "p99": 1002},
                            "kernel_launches": {"samples": 30, "p50": 1},
                            "cuda_ptx_source_cache_entries": {"samples": 30, "p50": 1},
                            "cuda_ptx_source_cache_hits": {"samples": 30, "p50": 1},
                            "cuda_ptx_source_cache_misses": {"samples": 30, "p50": 0}
                        },
                        "contract": {
                            "primitive": "release condition eval",
                            "baselines": [
                                {
                                    "name": "CPU-SOTA",
                                    "crate_name": "vyre-runtime",
                                    "class": "CpuSota",
                                    "min_speedup_x": 100.0,
                                    "backend_ids": ["cuda"]
                                }
                            ]
                        },
                        "performance": {"contract_passed": true, "speedup_x": 200.0}
                    }
                ]
            }))
            .expect("Fix: serialize backend-mismatch CPU-SOTA benchmark artifact JSON."),
        )
        .expect("Fix: write backend-mismatch CPU-SOTA benchmark artifact JSON.");

        let status = inspect_backend_suite_artifact(
            dir.path(),
            "cuda",
            &BackendSuiteArtifactInput {
                path: artifact_rel.to_string(),
                family_id: "condition-eval".to_string(),
                requested_case_id: "release.condition_eval.1m".to_string(),
                cpu_sota_100x_required: true,
            },
        );

        assert_eq!(
            status.nonmatching_case_backend_count, 1,
            "Fix: backend-mismatched suite artifacts must remain visible in generated status rows."
        );
        assert_eq!(
            status.cpu_sota_100x_contract_cases, 0,
            "Fix: generated CUDA suite status must not count WGPU case rows as CUDA CPU-SOTA proof."
        );
        assert_eq!(
            status.cpu_sota_100x_passing_cases, 0,
            "Fix: generated CUDA suite status must not count backend-mismatched rows as passing CPU-SOTA proof."
        );
        for expected in [
            "1 case(s) do not match requested backend `cuda`",
            "CPU-SOTA 100x workload artifact has no 100x contract case",
            "CPU-SOTA 100x workload artifact has no passing 100x case",
        ] {
            assert!(
                status
                    .blockers
                    .iter()
                    .any(|blocker| blocker.contains(expected)),
                "Fix: generated CUDA suite evidence must expose backend-mismatched CPU-SOTA proof drift `{expected}`; blockers={:?}",
                status.blockers
            );
        }
    }

    #[test]
    fn suite_artifact_status_surfaces_source_integrity_blockers() {
        let dir = TempDir::new()
            .expect("Fix: create a temporary workspace for CUDA suite integrity test.");
        let artifact_rel = "release/evidence/benchmarks/cuda-suite-integrity-drift.json";
        let artifact_path = dir.path().join(artifact_rel);
        fs::create_dir_all(
            artifact_path
                .parent()
                .expect("Fix: suite artifact must have parent directory."),
        )
        .expect("Fix: create CUDA suite integrity artifact parent directory.");
        fs::write(
            &artifact_path,
            serde_json::to_string_pretty(&json!({
                "schema_version": 2,
                "selected_backend": "cuda",
                "source_fingerprint": "git:abc:dirty=false",
                "source_tree_fingerprint": "source-tree-v1:abc",
                "summary": {"total_cases": 1, "passed": 1, "failed": 0},
                "environment": {
                    "host_cpu_model": "test CPU",
                    "gpu_devices": [
                        {
                            "name": "RTX 5090",
                            "memory_total_mib": 24576,
                            "compute_capability_major": 8,
                            "compute_capability_minor": 9
                        }
                    ],
                    "nvidia_driver_version": "580.0",
                    "nvidia_cuda_version": "13.0"
                },
                "cases": [
                    {
                        "id": "release.condition_eval.1m",
                        "backend_id": "cuda",
                        "status": "pass",
                        "optimization_passes_applied": ["cuda-resident-borrowed-escape-hatch"],
                        "metrics": {
                            "wall_ns": {"samples": 30, "p50": 10, "p95": 11, "p99": 12},
                            "baseline_wall_ns": {"samples": 30, "p50": 2000, "p95": 2001, "p99": 2002},
                            "kernel_launches": {"samples": 30, "p50": 1},
                            "cuda_ptx_source_cache_entries": {"samples": 30, "p50": 1},
                            "cuda_ptx_source_cache_hits": {"samples": 30, "p50": 1},
                            "cuda_ptx_source_cache_misses": {"samples": 30, "p50": 0},
                            "cuda_resident_borrowed_fallback_dispatches": {"p50": 2.0}
                        },
                        "contract": {
                            "primitive": "release condition eval",
                            "baselines": [
                                {
                                    "name": "CPU-SOTA",
                                    "crate_name": "vyre-runtime",
                                    "class": "CpuSota",
                                    "min_speedup_x": 100.0,
                                    "backend_ids": ["wgpu"]
                                }
                            ]
                        },
                        "performance": {"contract_passed": true, "speedup_x": 200.0}
                    }
                ]
            }))
            .expect("Fix: serialize CUDA suite integrity benchmark artifact JSON."),
        )
        .expect("Fix: write CUDA suite integrity benchmark artifact JSON.");

        let status = inspect_backend_suite_artifact(
            dir.path(),
            "cuda",
            &BackendSuiteArtifactInput {
                path: artifact_rel.to_string(),
                family_id: "condition-eval".to_string(),
                requested_case_id: "release.condition_eval.1m".to_string(),
                cpu_sota_100x_required: true,
            },
        );

        assert!(
            status.blockers.iter().any(|blocker| blocker.contains(
                "source_artifact `release/evidence/benchmarks/cuda-suite-integrity-drift.json` case `release.condition_eval.1m` backend `cuda` has no applicable performance contract baseline"
            )),
            "Fix: generated CUDA suite status must expose wrong-backend source artifact contracts; blockers={:?}",
            status.blockers
        );
        assert!(
            status.blockers.iter().any(|blocker| {
                blocker.contains(
                    "source_artifact `release/evidence/benchmarks/cuda-suite-integrity-drift.json` case `release.condition_eval.1m` has cuda_resident_borrowed_fallback_dispatches p50=2",
                ) && blocker.contains("backend `cuda` release suite artifact must use native resident dispatch")
            }),
            "Fix: generated CUDA suite status must expose borrowed resident dispatch telemetry; blockers={:?}",
            status.blockers
        );
    }

    #[test]
    fn failed_suite_artifact_blocker_preserves_case_failure_reason() {
        let dir = TempDir::new()
            .expect("Fix: create a temporary workspace for failed suite artifact test.");
        let artifact_rel = "release/evidence/benchmarks/wgpu-workload-failed.json";
        let artifact_path = dir.path().join(artifact_rel);
        fs::create_dir_all(
            artifact_path
                .parent()
                .expect("Fix: suite artifact must have parent directory."),
        )
        .expect("Fix: create failed suite artifact parent directory.");
        fs::write(
            &artifact_path,
            serde_json::to_string_pretty(&json!({
                "schema_version": 2,
                "selected_backend": "wgpu",
                "summary": {
                    "total_cases": 1,
                    "passed": 0,
                    "failed": 1,
                    "total_time_ns": 0,
                    "cache_hit_rate": null
                },
                "cases": [
                    {
                        "id": "sparse.compaction.count.1m",
                        "backend_id": "wgpu",
                        "status": "failed",
                        "correctness": {
                            "Invalid": {
                                "reason": "Performance contract failed: sparse output compaction count requires 100.00x over optimized CPU fired-rule collection over predicate masks, observed 91.75x"
                            }
                        },
                        "metrics": {
                            "wall_ns": {"samples": 30, "p50": 10, "p95": 11, "p99": 12},
                            "baseline_wall_ns": {"samples": 30, "p50": 1000, "p95": 1001, "p99": 1002},
                            "kernel_launches": {"samples": 1, "p50": 1}
                        },
                        "contract": {
                            "primitive": "sparse output compaction count",
                            "baselines": [
                                {
                                    "name": "optimized CPU fired-rule collection over predicate masks",
                                    "crate_name": "vyre-runtime",
                                    "class": "CpuSota",
                                    "min_speedup_x": 100.0,
                                    "backend_ids": ["cuda", "wgpu"]
                                }
                            ]
                        },
                        "performance": null,
                        "optimization_passes_applied": ["wgpu-release-path"]
                    }
                ]
            }))
            .expect("Fix: serialize failed benchmark artifact JSON."),
        )
        .expect("Fix: write failed benchmark artifact JSON.");

        let status = inspect_backend_suite_artifact(
            dir.path(),
            "wgpu",
            &BackendSuiteArtifactInput {
                path: artifact_rel.to_string(),
                family_id: "sparse-output-compaction".to_string(),
                requested_case_id: "sparse.compaction.count.1m".to_string(),
                cpu_sota_100x_required: false,
            },
        );

        assert!(
            status.blockers.iter().any(|blocker| blocker.contains(
                "case `sparse.compaction.count.1m` failed: Performance contract failed"
            ) && blocker.contains("observed 91.75x")),
            "Fix: WGPU suite blockers must preserve the benchmark case failure reason instead of exposing only missing metric fallout: {:?}",
            status.blockers
        );
    }

    #[test]
    fn suite_artifact_status_recomputes_hidden_case_failures() {
        let dir = TempDir::new()
            .expect("Fix: create a temporary workspace for hidden suite failure test.");
        let artifact_rel = "release/evidence/benchmarks/wgpu-hidden-invalid.json";
        let artifact_path = dir.path().join(artifact_rel);
        fs::create_dir_all(
            artifact_path
                .parent()
                .expect("Fix: suite artifact must have parent directory."),
        )
        .expect("Fix: create hidden suite artifact parent directory.");
        fs::write(
            &artifact_path,
            serde_json::to_string_pretty(&json!({
                "schema_version": 2,
                "selected_backend": "wgpu",
                "summary": {
                    "total_cases": 1,
                    "passed": 1,
                    "failed": 0,
                    "total_time_ns": 0,
                    "cache_hit_rate": null
                },
                "cases": [
                    {
                        "id": "release.condition_eval.1m",
                        "backend_id": "wgpu",
                        "status": "pass",
                        "correctness": {
                            "Invalid": {
                                "reason": "CUDA/WGPU output mismatch at row 17"
                            }
                        },
                        "metrics": {
                            "wall_ns": {"samples": 30, "p50": 10, "p95": 11, "p99": 12},
                            "baseline_wall_ns": {"samples": 30, "p50": 2000, "p95": 2001, "p99": 2002},
                            "kernel_launches": {"samples": 1, "p50": 1}
                        },
                        "contract": {
                            "primitive": "release condition eval",
                            "baselines": [
                                {
                                    "name": "CPU-SOTA",
                                    "crate_name": "vyre-runtime",
                                    "class": "CpuSota",
                                    "min_speedup_x": 100.0,
                                    "backend_ids": ["wgpu"]
                                }
                            ]
                        },
                        "performance": {"contract_passed": true, "speedup_x": 200.0}
                    }
                ]
            }))
            .expect("Fix: serialize hidden-invalid WGPU benchmark artifact JSON."),
        )
        .expect("Fix: write hidden-invalid WGPU benchmark artifact JSON.");

        let status = inspect_backend_suite_artifact(
            dir.path(),
            "wgpu",
            &BackendSuiteArtifactInput {
                path: artifact_rel.to_string(),
                family_id: "condition-eval".to_string(),
                requested_case_id: "release.condition_eval.1m".to_string(),
                cpu_sota_100x_required: true,
            },
        );

        assert_eq!(
            status.failed_count,
            Some(1),
            "Fix: backend suite status rows must derive failed_count from case evidence, not stale summary.failed."
        );
        assert_eq!(
            status.cpu_sota_100x_contract_cases, 1,
            "Fix: hidden invalid correctness must not erase the applicable CPU-SOTA contract count."
        );
        assert_eq!(
            status.cpu_sota_100x_passing_cases, 0,
            "Fix: hidden invalid correctness must disqualify a case from passing CPU-SOTA status proof."
        );
        assert!(
            status.blockers.iter().any(|blocker| blocker.contains(
                "case `release.condition_eval.1m` failed: CUDA/WGPU output mismatch at row 17"
            )),
            "Fix: backend suite blockers must preserve hidden case failure reasons; blockers={:?}",
            status.blockers
        );
        assert!(
            status.blockers.iter().any(|blocker| blocker.contains(
                "summary.failed is `Some(0)` but case evidence reports 1 failed case(s)"
            )),
            "Fix: backend suite blockers must expose stale summary.failed drift; blockers={:?}",
            status.blockers
        );
    }

    #[test]
    fn suite_artifact_status_rejects_stale_summary_passed_count() {
        let dir = TempDir::new()
            .expect("Fix: create a temporary workspace for stale suite summary test.");
        let artifact_rel = "release/evidence/benchmarks/wgpu-stale-passed.json";
        let artifact_path = dir.path().join(artifact_rel);
        fs::create_dir_all(
            artifact_path
                .parent()
                .expect("Fix: suite artifact must have parent directory."),
        )
        .expect("Fix: create stale summary suite artifact parent directory.");
        fs::write(
            &artifact_path,
            serde_json::to_string_pretty(&json!({
                "schema_version": 2,
                "selected_backend": "wgpu",
                "summary": {
                    "total_cases": 1,
                    "passed": 0,
                    "failed": 0,
                    "total_time_ns": 0,
                    "cache_hit_rate": null
                },
                "cases": [
                    {
                        "id": "release.condition_eval.1m",
                        "backend_id": "wgpu",
                        "status": "pass",
                        "metrics": {
                            "wall_ns": {"samples": 30, "p50": 10, "p95": 11, "p99": 12},
                            "baseline_wall_ns": {"samples": 30, "p50": 2000, "p95": 2001, "p99": 2002},
                            "kernel_launches": {"samples": 1, "p50": 1}
                        },
                        "contract": {
                            "primitive": "release condition eval",
                            "baselines": [
                                {
                                    "name": "CPU-SOTA",
                                    "crate_name": "vyre-runtime",
                                    "class": "CpuSota",
                                    "min_speedup_x": 100.0,
                                    "backend_ids": ["wgpu"]
                                }
                            ]
                        },
                        "performance": {"contract_passed": true, "speedup_x": 200.0}
                    }
                ]
            }))
            .expect("Fix: serialize stale-passed WGPU benchmark artifact JSON."),
        )
        .expect("Fix: write stale-passed WGPU benchmark artifact JSON.");

        let status = inspect_backend_suite_artifact(
            dir.path(),
            "wgpu",
            &BackendSuiteArtifactInput {
                path: artifact_rel.to_string(),
                family_id: "condition-eval".to_string(),
                requested_case_id: "release.condition_eval.1m".to_string(),
                cpu_sota_100x_required: true,
            },
        );

        assert!(
            status.blockers.iter().any(|blocker| blocker.contains(
                "benchmark summary is invalid: summary total/pass/fail (Some(1)/Some(0)/Some(0)) contradicts case evidence (1/1/0)"
            )),
            "Fix: backend suite inspector must reject stale summary.passed drift before suite rows prove release evidence; blockers={:?}",
            status.blockers
        );
    }

    #[test]
    fn suite_artifact_status_rejects_unproven_cpu_sota_pass_status() {
        let dir = TempDir::new()
            .expect("Fix: create a temporary workspace for unproven CPU-SOTA suite test.");
        let artifact_rel = "release/evidence/benchmarks/wgpu-unproven-pass.json";
        let artifact_path = dir.path().join(artifact_rel);
        fs::create_dir_all(
            artifact_path
                .parent()
                .expect("Fix: suite artifact must have parent directory."),
        )
        .expect("Fix: create unproven CPU-SOTA suite artifact parent directory.");
        fs::write(
            &artifact_path,
            serde_json::to_string_pretty(&json!({
                "schema_version": 2,
                "selected_backend": "wgpu",
                "summary": {
                    "total_cases": 1,
                    "passed": 0,
                    "failed": 1,
                    "total_time_ns": 0,
                    "cache_hit_rate": null
                },
                "cases": [
                    {
                        "id": "release.condition_eval.1m",
                        "backend_id": "wgpu",
                        "metrics": {
                            "wall_ns": {"samples": 30, "p50": 10, "p95": 11, "p99": 12},
                            "baseline_wall_ns": {"samples": 30, "p50": 2000, "p95": 2001, "p99": 2002},
                            "kernel_launches": {"samples": 1, "p50": 1}
                        },
                        "contract": {
                            "primitive": "release condition eval",
                            "baselines": [
                                {
                                    "name": "CPU-SOTA",
                                    "crate_name": "vyre-runtime",
                                    "class": "CpuSota",
                                    "min_speedup_x": 100.0,
                                    "backend_ids": ["wgpu"]
                                }
                            ]
                        },
                        "performance": {"contract_passed": true, "speedup_x": 200.0}
                    }
                ]
            }))
            .expect("Fix: serialize unproven CPU-SOTA WGPU benchmark artifact JSON."),
        )
        .expect("Fix: write unproven CPU-SOTA WGPU benchmark artifact JSON.");

        let status = inspect_backend_suite_artifact(
            dir.path(),
            "wgpu",
            &BackendSuiteArtifactInput {
                path: artifact_rel.to_string(),
                family_id: "condition-eval".to_string(),
                requested_case_id: "release.condition_eval.1m".to_string(),
                cpu_sota_100x_required: true,
            },
        );

        assert_eq!(
            status.failed_count,
            Some(1),
            "Fix: missing pass status must count as a failed suite artifact case."
        );
        assert_eq!(
            status.cpu_sota_100x_contract_cases, 1,
            "Fix: missing pass status must not erase the applicable CPU-SOTA contract count."
        );
        assert_eq!(
            status.cpu_sota_100x_passing_cases, 0,
            "Fix: CPU-SOTA passing suite rows must require explicit pass status evidence."
        );
        assert!(
            status.blockers.iter().any(|blocker| blocker.contains(
                "case `release.condition_eval.1m` failed: missing pass status"
            )),
            "Fix: unproven CPU-SOTA suite rows must expose the missing pass status reason; blockers={:?}",
            status.blockers
        );
    }

    #[test]
    fn cpu_100x_proof_rejects_case_failure_hidden_by_passing_contract() {
        let dir = TempDir::new()
            .expect("Fix: create a temporary workspace for CPU-SOTA proof regression test.");
        let artifact_rel = "release/evidence/benchmarks/cuda-hidden-invalid.json";
        let artifact_path = dir.path().join(artifact_rel);
        fs::create_dir_all(
            artifact_path
                .parent()
                .expect("Fix: CPU-SOTA proof artifact path must have a parent directory."),
        )
        .expect("Fix: create CPU-SOTA proof artifact parent directory.");
        fs::write(
            &artifact_path,
            serde_json::to_string_pretty(&json!({
                "schema_version": 2,
                "selected_backend": "cuda",
                "summary": {
                    "total_cases": 1,
                    "passed": 1,
                    "failed": 0,
                    "total_time_ns": 0,
                    "cache_hit_rate": null
                },
                "cases": [
                    {
                        "id": "release.condition_eval.1m",
                        "backend_id": "cuda",
                        "status": "pass",
                        "correctness": {
                            "Invalid": {
                                "reason": "CUDA/WGPU output mismatch at row 17"
                            }
                        },
                        "metrics": {
                            "wall_ns": {"samples": 30, "p50": 10, "p95": 11, "p99": 12},
                            "baseline_wall_ns": {"samples": 30, "p50": 2000, "p95": 2001, "p99": 2002}
                        },
                        "contract": {
                            "primitive": "release condition eval",
                            "baselines": [
                                {
                                    "name": "CPU-SOTA",
                                    "crate_name": "vyre-runtime",
                                    "class": "CpuSota",
                                    "min_speedup_x": 100.0,
                                    "backend_ids": ["cuda"]
                                }
                            ]
                        },
                        "performance": {"contract_passed": true, "speedup_x": 200.0}
                    }
                ]
            }))
            .expect("Fix: serialize hidden-invalid CUDA benchmark artifact JSON."),
        )
        .expect("Fix: write hidden-invalid CUDA benchmark artifact JSON.");

        write_cpu_100x_proof(dir.path(), &[artifact_rel.to_string()]);

        let proof_path = dir
            .path()
            .join("release/evidence/benchmarks/cpu-only-100x-proof.json");
        let proof_text = fs::read_to_string(&proof_path)
            .expect("Fix: read generated CPU-SOTA 100x proof artifact.");
        let proof = serde_json::from_str::<Value>(&proof_text)
            .expect("Fix: generated CPU-SOTA 100x proof must be valid JSON.");

        assert_eq!(
            proof
                .get("cpu_sota_100x_passing_case_count")
                .and_then(Value::as_u64),
            Some(0),
            "Fix: invalid correctness evidence must disqualify a case from passing CPU-SOTA proof even when performance says contract_passed=true."
        );
        assert_eq!(
            proof
                .get("summary")
                .and_then(|summary| summary.get("failed"))
                .and_then(Value::as_u64),
            Some(1),
            "Fix: aggregate CPU-SOTA proof summary must count hidden invalid cases as failed."
        );
        let blockers = proof
            .get("blockers")
            .and_then(Value::as_array)
            .expect("Fix: generated CPU-SOTA proof must include blockers array.");
        assert!(
            blockers
                .iter()
                .filter_map(Value::as_str)
                .any(|blocker| blocker.contains(
                    "100x source artifact `release/evidence/benchmarks/cuda-hidden-invalid.json` case `release.condition_eval.1m` failed: CUDA/WGPU output mismatch at row 17"
                )),
            "Fix: aggregate CPU-SOTA proof blockers must preserve hidden case failure reasons; blockers={blockers:?}"
        );
    }

    #[test]
    fn cpu_100x_proof_rejects_missing_pass_status_with_passing_contract() {
        let dir = TempDir::new()
            .expect("Fix: create a temporary workspace for missing-status CPU-SOTA proof test.");
        let artifact_rel = "release/evidence/benchmarks/cuda-missing-status.json";
        let artifact_path = dir.path().join(artifact_rel);
        fs::create_dir_all(
            artifact_path
                .parent()
                .expect("Fix: CPU-SOTA proof artifact path must have a parent directory."),
        )
        .expect("Fix: create missing-status CPU-SOTA proof artifact parent directory.");
        fs::write(
            &artifact_path,
            serde_json::to_string_pretty(&json!({
                "schema_version": 2,
                "selected_backend": "cuda",
                "summary": {
                    "total_cases": 1,
                    "passed": 0,
                    "failed": 1,
                    "total_time_ns": 0,
                    "cache_hit_rate": null
                },
                "cases": [
                    {
                        "id": "release.condition_eval.1m",
                        "backend_id": "cuda",
                        "metrics": {
                            "wall_ns": {"samples": 30, "p50": 10, "p95": 11, "p99": 12},
                            "baseline_wall_ns": {"samples": 30, "p50": 2000, "p95": 2001, "p99": 2002}
                        },
                        "contract": {
                            "primitive": "release condition eval",
                            "baselines": [
                                {
                                    "name": "CPU-SOTA",
                                    "crate_name": "vyre-runtime",
                                    "class": "CpuSota",
                                    "min_speedup_x": 100.0,
                                    "backend_ids": ["cuda"]
                                }
                            ]
                        },
                        "performance": {"contract_passed": true, "speedup_x": 200.0}
                    }
                ]
            }))
            .expect("Fix: serialize missing-status CUDA benchmark artifact JSON."),
        )
        .expect("Fix: write missing-status CUDA benchmark artifact JSON.");

        write_cpu_100x_proof(dir.path(), &[artifact_rel.to_string()]);

        let proof_path = dir
            .path()
            .join("release/evidence/benchmarks/cpu-only-100x-proof.json");
        let proof_text = fs::read_to_string(&proof_path)
            .expect("Fix: read generated CPU-SOTA 100x proof artifact.");
        let proof = serde_json::from_str::<Value>(&proof_text)
            .expect("Fix: generated CPU-SOTA 100x proof must be valid JSON.");

        assert_eq!(
            proof
                .get("cpu_sota_100x_contract_case_count")
                .and_then(Value::as_u64),
            Some(1),
            "Fix: missing pass status must not erase applicable CPU-SOTA contracts from aggregate proof."
        );
        assert_eq!(
            proof
                .get("cpu_sota_100x_passing_case_count")
                .and_then(Value::as_u64),
            Some(0),
            "Fix: aggregate CPU-SOTA proof must require explicit pass status before counting a passing 100x case."
        );
        assert_eq!(
            proof
                .get("summary")
                .and_then(|summary| summary.get("failed"))
                .and_then(Value::as_u64),
            Some(1),
            "Fix: aggregate CPU-SOTA proof summary must count missing pass status cases as failed."
        );
        let blockers = proof
            .get("blockers")
            .and_then(Value::as_array)
            .expect("Fix: generated CPU-SOTA proof must include blockers array.");
        assert!(
            blockers
                .iter()
                .filter_map(Value::as_str)
                .any(|blocker| blocker.contains(
                    "100x source artifact `release/evidence/benchmarks/cuda-missing-status.json` case `release.condition_eval.1m` failed: missing pass status"
                )),
            "Fix: aggregate CPU-SOTA proof blockers must expose missing pass status; blockers={blockers:?}"
        );
    }

    #[test]
    fn cpu_100x_proof_requires_each_release_defining_case_to_pass_100x() {
        let dir = TempDir::new()
            .expect("Fix: create a temporary workspace for required CPU-SOTA proof test.");
        let artifact_rel = "release/evidence/benchmarks/cuda-required-case-failed.json";
        let artifact_path = dir.path().join(artifact_rel);
        fs::create_dir_all(
            artifact_path
                .parent()
                .expect("Fix: required CPU-SOTA artifact path must have a parent directory."),
        )
        .expect("Fix: create required CPU-SOTA proof artifact parent directory.");
        let cases = REQUIRED_CPU_SOTA_100X_CASES
            .iter()
            .enumerate()
            .map(|(index, case_id)| {
                json!({
                    "id": case_id,
                    "backend_id": "cuda",
                    "status": if index == 0 { "fail" } else { "pass" },
                    "metrics": {
                        "wall_ns": {"samples": 30, "p50": 10, "p95": 11, "p99": 12},
                        "baseline_wall_ns": {"samples": 30, "p50": 2000, "p95": 2001, "p99": 2002}
                    },
                    "contract": {
                        "primitive": "release CPU-SOTA required case",
                        "baselines": [
                            {
                                "name": "CPU-SOTA",
                                "crate_name": "vyre-runtime",
                                "class": "CpuSota",
                                "min_speedup_x": 100.0,
                                "backend_ids": ["cuda"]
                            }
                        ]
                    },
                    "performance": {"contract_passed": true, "speedup_x": 200.0}
                })
            })
            .collect::<Vec<_>>();
        fs::write(
            &artifact_path,
            serde_json::to_string_pretty(&json!({
                "schema_version": 2,
                "selected_backend": "cuda",
                "source_fingerprint": "git:source-a:dirty=false",
                "source_tree_fingerprint": "source-tree-v1:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                "summary": {
                    "total_cases": REQUIRED_CPU_SOTA_100X_CASES.len(),
                    "passed": REQUIRED_CPU_SOTA_100X_CASES.len() - 1,
                    "failed": 1,
                    "total_time_ns": 0,
                    "cache_hit_rate": null
                },
                "cases": cases
            }))
            .expect("Fix: serialize required CPU-SOTA benchmark artifact JSON."),
        )
        .expect("Fix: write required CPU-SOTA benchmark artifact JSON.");

        write_cpu_100x_proof(dir.path(), &[artifact_rel.to_string()]);

        let proof_path = dir
            .path()
            .join("release/evidence/benchmarks/cpu-only-100x-proof.json");
        let proof_text = fs::read_to_string(&proof_path)
            .expect("Fix: read generated CPU-SOTA 100x proof artifact.");
        let proof = serde_json::from_str::<Value>(&proof_text)
            .expect("Fix: generated CPU-SOTA 100x proof must be valid JSON.");
        let blockers = proof
            .get("blockers")
            .and_then(Value::as_array)
            .expect("Fix: generated CPU-SOTA proof must include blockers array.");

        assert!(
            blockers.iter().filter_map(Value::as_str).any(|blocker| {
                blocker.contains(
                    "100x proof is missing required release-defining case `release.condition_eval.1m`",
                )
            }),
            "Fix: required CPU-SOTA cases must be counted as present only when they prove a passing 100x CUDA win; blockers={blockers:?}"
        );
    }

    #[test]
    fn cpu_100x_proof_rejects_claimed_speedup_without_measured_100x() {
        let dir = TempDir::new()
            .expect("Fix: create a temporary workspace for CPU-SOTA measured speedup test.");
        let artifact_rel = "release/evidence/benchmarks/cuda-claimed-speedup.json";
        let artifact_path = dir.path().join(artifact_rel);
        fs::create_dir_all(
            artifact_path
                .parent()
                .expect("Fix: CPU-SOTA proof artifact path must have a parent directory."),
        )
        .expect("Fix: create measured-speedup CPU-SOTA proof artifact parent directory.");
        fs::write(
            &artifact_path,
            serde_json::to_string_pretty(&json!({
                "schema_version": 2,
                "selected_backend": "cuda",
                "summary": {
                    "total_cases": 1,
                    "passed": 1,
                    "failed": 0,
                    "total_time_ns": 0,
                    "cache_hit_rate": null
                },
                "cases": [
                    {
                        "id": "release.condition_eval.1m",
                        "backend_id": "cuda",
                        "status": "pass",
                        "metrics": {
                            "wall_ns": {"samples": 30, "p50": 100, "p95": 101, "p99": 102},
                            "baseline_wall_ns": {"samples": 30, "p50": 1000, "p95": 1001, "p99": 1002}
                        },
                        "contract": {
                            "primitive": "release condition eval",
                            "baselines": [
                                {
                                    "name": "CPU-SOTA",
                                    "crate_name": "vyre-runtime",
                                    "class": "CpuSota",
                                    "min_speedup_x": 100.0,
                                    "backend_ids": ["cuda"]
                                }
                            ]
                        },
                        "performance": {"contract_passed": true, "speedup_x": 200.0}
                    }
                ]
            }))
            .expect("Fix: serialize claimed-speedup CUDA benchmark artifact JSON."),
        )
        .expect("Fix: write claimed-speedup CUDA benchmark artifact JSON.");

        write_cpu_100x_proof(dir.path(), &[artifact_rel.to_string()]);

        let proof_path = dir
            .path()
            .join("release/evidence/benchmarks/cpu-only-100x-proof.json");
        let proof_text = fs::read_to_string(&proof_path)
            .expect("Fix: read generated CPU-SOTA measured-speedup proof artifact.");
        let proof = serde_json::from_str::<Value>(&proof_text)
            .expect("Fix: generated CPU-SOTA measured-speedup proof must be valid JSON.");

        assert_eq!(
            proof
                .get("cpu_sota_100x_contract_case_count")
                .and_then(Value::as_u64),
            Some(1),
            "Fix: measured-speedup failure must not erase applicable CPU-SOTA contracts from aggregate proof."
        );
        assert_eq!(
            proof
                .get("cpu_sota_100x_passing_case_count")
                .and_then(Value::as_u64),
            Some(0),
            "Fix: aggregate CPU-SOTA proof must not count claimed speedup_x without measured baseline_wall_ns / wall_ns >= 100x."
        );
        assert_eq!(
            proof
                .get("summary")
                .and_then(|summary| summary.get("failed"))
                .and_then(Value::as_u64),
            Some(1),
            "Fix: aggregate CPU-SOTA proof summary must count claimed-only speedup cases as failed."
        );
    }

    #[test]
    fn cpu_100x_proof_surfaces_source_artifact_integrity_blockers() {
        let dir = TempDir::new()
            .expect("Fix: create a temporary workspace for CPU-SOTA integrity blocker test.");
        let artifact_rel = "release/evidence/benchmarks/cuda-integrity-drift.json";
        let artifact_path = dir.path().join(artifact_rel);
        fs::create_dir_all(
            artifact_path
                .parent()
                .expect("Fix: CPU-SOTA integrity artifact path must have a parent directory."),
        )
        .expect("Fix: create CPU-SOTA integrity artifact parent directory.");
        fs::write(
            &artifact_path,
            serde_json::to_string_pretty(&json!({
                "schema_version": 2,
                "selected_backend": "cuda",
                "summary": {
                    "total_cases": 1,
                    "passed": 1,
                    "failed": 0,
                    "total_time_ns": 0,
                    "cache_hit_rate": null
                },
                "cases": [
                    {
                        "id": "release.condition_eval.1m",
                        "backend_id": "cuda",
                        "status": "pass",
                        "optimization_passes_applied": ["cuda-resident-borrowed-escape-hatch"],
                        "metrics": {
                            "wall_ns": {"samples": 30, "p50": 10, "p95": 11, "p99": 12},
                            "baseline_wall_ns": {"samples": 30, "p50": 2000, "p95": 2001, "p99": 2002},
                            "cuda_resident_borrowed_fallback_dispatches": {"p50": 2.0}
                        },
                        "contract": {
                            "primitive": "release condition eval",
                            "baselines": [
                                {
                                    "name": "CPU-SOTA",
                                    "crate_name": "vyre-runtime",
                                    "class": "CpuSota",
                                    "min_speedup_x": 100.0,
                                    "backend_ids": ["wgpu"]
                                }
                            ]
                        },
                        "performance": {"contract_passed": true, "speedup_x": 200.0}
                    }
                ]
            }))
            .expect("Fix: serialize CPU-SOTA integrity benchmark artifact JSON."),
        )
        .expect("Fix: write CPU-SOTA integrity benchmark artifact JSON.");

        write_cpu_100x_proof(dir.path(), &[artifact_rel.to_string()]);

        let proof_path = dir
            .path()
            .join("release/evidence/benchmarks/cpu-only-100x-proof.json");
        let proof_text = fs::read_to_string(&proof_path)
            .expect("Fix: read generated CPU-SOTA integrity proof artifact.");
        let proof = serde_json::from_str::<Value>(&proof_text)
            .expect("Fix: generated CPU-SOTA integrity proof must be valid JSON.");
        let blockers = proof
            .get("blockers")
            .and_then(Value::as_array)
            .expect("Fix: generated CPU-SOTA proof must include blockers array.");

        assert!(
            blockers.iter().filter_map(Value::as_str).any(|blocker| {
                blocker.contains(
                    "source_artifact `release/evidence/benchmarks/cuda-integrity-drift.json` case `release.condition_eval.1m` backend `cuda` has no applicable performance contract baseline",
                )
            }),
            "Fix: aggregate CPU-SOTA proof blockers must expose wrong-backend source artifact contracts; blockers={blockers:?}"
        );
        assert!(
            blockers.iter().filter_map(Value::as_str).any(|blocker| {
                blocker.contains(
                    "source_artifact `release/evidence/benchmarks/cuda-integrity-drift.json` case `release.condition_eval.1m` has cuda_resident_borrowed_fallback_dispatches p50=2",
                ) && blocker.contains("CPU-SOTA aggregate proof must use native resident dispatch")
            }),
            "Fix: aggregate CPU-SOTA proof blockers must expose borrowed resident CUDA dispatch telemetry; blockers={blockers:?}"
        );
    }

    #[test]
    fn cpu_100x_proof_rejects_missing_and_weak_source_fingerprint() {
        let dir = TempDir::new()
            .expect("Fix: create a temporary workspace for CPU-SOTA provenance proof test.");
        let artifacts = [
            (
                "release/evidence/benchmarks/cuda-no-source-fingerprint.json",
                None,
            ),
            (
                "release/evidence/benchmarks/cuda-legacy-dirty-source.json",
                Some("git:abc123:dirty=true"),
            ),
        ];
        for (artifact_rel, source_fingerprint) in artifacts {
            let artifact_path = dir.path().join(artifact_rel);
            fs::create_dir_all(
                artifact_path
                    .parent()
                    .expect("Fix: CPU-SOTA provenance artifact path must have a parent directory."),
            )
            .expect("Fix: create CPU-SOTA provenance proof artifact parent directory.");
            let mut artifact = json!({
                "schema_version": 2,
                "selected_backend": "cuda",
                "source_tree_fingerprint": "source-tree-v1:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                "summary": {
                    "total_cases": 1,
                    "passed": 1,
                    "failed": 0,
                    "total_time_ns": 0,
                    "cache_hit_rate": null
                },
                "cases": [
                    {
                        "id": "release.condition_eval.1m",
                        "backend_id": "cuda",
                        "status": "pass",
                        "metrics": {
                            "wall_ns": {"samples": 30, "p50": 10, "p95": 11, "p99": 12},
                            "baseline_wall_ns": {"samples": 30, "p50": 2000, "p95": 2001, "p99": 2002}
                        },
                        "contract": {
                            "primitive": "release condition eval",
                            "baselines": [
                                {
                                    "name": "CPU-SOTA",
                                    "crate_name": "vyre-runtime",
                                    "class": "CpuSota",
                                    "min_speedup_x": 100.0,
                                    "backend_ids": ["cuda"]
                                }
                            ]
                        },
                        "performance": {"contract_passed": true, "speedup_x": 200.0}
                    }
                ]
            });
            if let Some(source_fingerprint) = source_fingerprint {
                artifact["source_fingerprint"] = Value::String(source_fingerprint.to_string());
            }
            fs::write(
                &artifact_path,
                serde_json::to_string_pretty(&artifact)
                    .expect("Fix: serialize CPU-SOTA provenance benchmark artifact JSON."),
            )
            .expect("Fix: write CPU-SOTA provenance benchmark artifact JSON.");
        }

        write_cpu_100x_proof(
            dir.path(),
            &artifacts
                .iter()
                .map(|(artifact, _)| artifact.to_string())
                .collect::<Vec<_>>(),
        );

        let proof_path = dir
            .path()
            .join("release/evidence/benchmarks/cpu-only-100x-proof.json");
        let proof_text = fs::read_to_string(&proof_path)
            .expect("Fix: read generated CPU-SOTA 100x proof artifact.");
        let proof = serde_json::from_str::<Value>(&proof_text)
            .expect("Fix: generated CPU-SOTA 100x proof must be valid JSON.");
        let blockers = proof
            .get("blockers")
            .and_then(Value::as_array)
            .expect("Fix: generated CPU-SOTA proof must include blockers array.");

        assert!(
            blockers.iter().filter_map(Value::as_str).any(|blocker| {
                blocker.contains(
                    "100x source artifact `release/evidence/benchmarks/cuda-no-source-fingerprint.json` has no source_fingerprint",
                )
            }),
            "Fix: aggregate CPU-SOTA proof must reject source artifacts without explicit source_fingerprint; blockers={blockers:?}"
        );
        assert!(
            blockers.iter().filter_map(Value::as_str).any(|blocker| {
                blocker.contains(
                    "100x source artifact `release/evidence/benchmarks/cuda-legacy-dirty-source.json` source_fingerprint `git:abc123:dirty=true` is not release-grade provenance",
                )
            }),
            "Fix: aggregate CPU-SOTA proof must reject weak dirty source_fingerprint provenance; blockers={blockers:?}"
        );
    }

    #[test]
    fn cpu_100x_proof_rejects_whitespace_only_source_provenance() {
        let dir = TempDir::new()
            .expect("Fix: create a temporary workspace for blank CPU-SOTA provenance test.");
        let artifact_rel = "release/evidence/benchmarks/cuda-blank-source-provenance.json";
        let artifact_path = dir.path().join(artifact_rel);
        fs::create_dir_all(
            artifact_path
                .parent()
                .expect("Fix: blank provenance artifact path must have a parent directory."),
        )
        .expect("Fix: create blank provenance proof artifact parent directory.");
        fs::write(
            &artifact_path,
            serde_json::to_string_pretty(&json!({
                "schema_version": 2,
                "selected_backend": "cuda",
                "source_fingerprint": "   ",
                "source_tree_fingerprint": "\t",
                "summary": {
                    "total_cases": 1,
                    "passed": 1,
                    "failed": 0,
                    "total_time_ns": 0,
                    "cache_hit_rate": null
                },
                "cases": [
                    {
                        "id": "release.condition_eval.1m",
                        "backend_id": "cuda",
                        "status": "pass",
                        "metrics": {
                            "wall_ns": {"samples": 30, "p50": 10, "p95": 11, "p99": 12},
                            "baseline_wall_ns": {"samples": 30, "p50": 2000, "p95": 2001, "p99": 2002}
                        },
                        "contract": {
                            "primitive": "release condition eval",
                            "baselines": [
                                {
                                    "name": "CPU-SOTA",
                                    "crate_name": "vyre-runtime",
                                    "class": "CpuSota",
                                    "min_speedup_x": 100.0,
                                    "backend_ids": ["cuda"]
                                }
                            ]
                        },
                        "performance": {"contract_passed": true, "speedup_x": 200.0}
                    }
                ]
            }))
            .expect("Fix: serialize blank provenance CUDA benchmark artifact JSON."),
        )
        .expect("Fix: write blank provenance CUDA benchmark artifact JSON.");

        write_cpu_100x_proof(dir.path(), &[artifact_rel.to_string()]);

        let proof_path = dir
            .path()
            .join("release/evidence/benchmarks/cpu-only-100x-proof.json");
        let proof_text = fs::read_to_string(&proof_path)
            .expect("Fix: read generated CPU-SOTA 100x proof artifact.");
        let proof = serde_json::from_str::<Value>(&proof_text)
            .expect("Fix: generated CPU-SOTA 100x proof must be valid JSON.");
        let blockers = proof
            .get("blockers")
            .and_then(Value::as_array)
            .expect("Fix: generated CPU-SOTA proof must include blockers array.");

        assert!(
            blockers.iter().filter_map(Value::as_str).any(|blocker| {
                blocker.contains(
                    "100x source artifact `release/evidence/benchmarks/cuda-blank-source-provenance.json` has no source_fingerprint",
                )
            }),
            "Fix: aggregate CPU-SOTA proof must reject blank source_fingerprint provenance; blockers={blockers:?}"
        );
        assert!(
            blockers.iter().filter_map(Value::as_str).any(|blocker| {
                blocker.contains(
                    "100x source artifact `release/evidence/benchmarks/cuda-blank-source-provenance.json` has no source_tree_fingerprint",
                )
            }),
            "Fix: aggregate CPU-SOTA proof must reject blank source_tree_fingerprint provenance; blockers={blockers:?}"
        );
        assert_eq!(
            proof.get("source_fingerprint"),
            Some(&Value::Null),
            "Fix: blank source_fingerprint must not be serialized as aggregate CPU-SOTA provenance."
        );
        assert_eq!(
            proof.get("source_tree_fingerprint"),
            Some(&Value::Null),
            "Fix: blank source_tree_fingerprint must not be serialized as aggregate CPU-SOTA provenance."
        );
    }

    #[test]
    fn cpu_100x_proof_rejects_stale_source_tree_fingerprint() {
        let dir = TempDir::new()
            .expect("Fix: create a temporary workspace for stale CPU-SOTA source-tree test.");
        fs::write(dir.path().join("Cargo.toml"), "[workspace]\n")
            .expect("Fix: create temp workspace Cargo.toml for CPU-SOTA source-tree test.");
        let artifact_rel = "release/evidence/benchmarks/cuda-stale-source-tree.json";
        let artifact_path = dir.path().join(artifact_rel);
        fs::create_dir_all(
            artifact_path
                .parent()
                .expect("Fix: stale source-tree proof artifact path must have a parent directory."),
        )
        .expect("Fix: create stale source-tree proof artifact parent directory.");
        fs::write(
            &artifact_path,
            serde_json::to_string_pretty(&json!({
                "schema_version": 2,
                "selected_backend": "cuda",
                "source_fingerprint": "git:source-a:dirty=false",
                "source_tree_fingerprint": "source-tree-v1:stale",
                "summary": {
                    "total_cases": 1,
                    "passed": 1,
                    "failed": 0,
                    "total_time_ns": 0,
                    "cache_hit_rate": null
                },
                "cases": [
                    {
                        "id": "release.condition_eval.1m",
                        "backend_id": "cuda",
                        "status": "pass",
                        "metrics": {
                            "wall_ns": {"samples": 30, "p50": 10, "p95": 11, "p99": 12},
                            "baseline_wall_ns": {"samples": 30, "p50": 2000, "p95": 2001, "p99": 2002}
                        },
                        "contract": {
                            "primitive": "release condition eval",
                            "baselines": [
                                {
                                    "name": "CPU-SOTA",
                                    "crate_name": "vyre-runtime",
                                    "class": "CpuSota",
                                    "min_speedup_x": 100.0,
                                    "backend_ids": ["cuda"]
                                }
                            ]
                        },
                        "performance": {"contract_passed": true, "speedup_x": 200.0}
                    }
                ]
            }))
            .expect("Fix: serialize stale source-tree CUDA benchmark artifact JSON."),
        )
        .expect("Fix: write stale source-tree CUDA benchmark artifact JSON.");

        write_cpu_100x_proof(dir.path(), &[artifact_rel.to_string()]);

        let proof_path = dir
            .path()
            .join("release/evidence/benchmarks/cpu-only-100x-proof.json");
        let proof_text = fs::read_to_string(&proof_path)
            .expect("Fix: read generated CPU-SOTA 100x proof artifact.");
        let proof = serde_json::from_str::<Value>(&proof_text)
            .expect("Fix: generated CPU-SOTA 100x proof must be valid JSON.");
        let blockers = proof
            .get("blockers")
            .and_then(Value::as_array)
            .expect("Fix: generated CPU-SOTA proof must include blockers array.");

        assert!(
            blockers.iter().filter_map(Value::as_str).any(|blocker| {
                blocker.contains(
                    "100x source artifact `release/evidence/benchmarks/cuda-stale-source-tree.json` source_tree_fingerprint `source-tree-v1:stale`",
                ) && blocker.contains("does not match current workspace source")
            }),
            "Fix: aggregate CPU-SOTA proof must reject stale source-tree benchmark artifacts; blockers={blockers:?}"
        );
    }

    #[test]
    fn cpu_100x_proof_rejects_mixed_source_trees_not_clean_evidence_commit_drift() {
        let dir = TempDir::new()
            .expect("Fix: create a temporary workspace for mixed-source CPU-SOTA proof test.");
        let artifacts = [
            (
                "release/evidence/benchmarks/cuda-source-a.json",
                "git:source-a:dirty=false",
                "source-tree-v1:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            ),
            (
                "release/evidence/benchmarks/cuda-source-b.json",
                "git:source-b:dirty=false",
                "source-tree-v1:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
            ),
        ];
        for (artifact_rel, source_fingerprint, source_tree_fingerprint) in artifacts {
            let artifact_path = dir.path().join(artifact_rel);
            fs::create_dir_all(
                artifact_path
                    .parent()
                    .expect("Fix: mixed-source proof artifact path must have a parent directory."),
            )
            .expect("Fix: create mixed-source proof artifact parent directory.");
            fs::write(
                &artifact_path,
                serde_json::to_string_pretty(&json!({
                    "schema_version": 2,
                    "selected_backend": "cuda",
                    "source_fingerprint": source_fingerprint,
                    "source_tree_fingerprint": source_tree_fingerprint,
                    "summary": {
                        "total_cases": 1,
                        "passed": 1,
                        "failed": 0,
                        "total_time_ns": 0,
                        "cache_hit_rate": null
                    },
                    "cases": [
                        {
                            "id": "release.condition_eval.1m",
                            "backend_id": "cuda",
                            "status": "pass",
                            "metrics": {
                                "wall_ns": {"samples": 30, "p50": 10, "p95": 11, "p99": 12},
                                "baseline_wall_ns": {"samples": 30, "p50": 2000, "p95": 2001, "p99": 2002}
                            },
                            "contract": {
                                "primitive": "release condition eval",
                                "baselines": [
                                    {
                                        "name": "CPU-SOTA",
                                        "crate_name": "vyre-runtime",
                                        "class": "CpuSota",
                                        "min_speedup_x": 100.0,
                                        "backend_ids": ["cuda"]
                                    }
                                ]
                            },
                            "performance": {"contract_passed": true, "speedup_x": 200.0}
                        }
                    ]
                }))
                .expect("Fix: serialize mixed-source CUDA benchmark artifact JSON."),
            )
            .expect("Fix: write mixed-source CUDA benchmark artifact JSON.");
        }
        write_cpu_100x_proof(
            dir.path(),
            &artifacts
                .iter()
                .map(|(artifact, _, _)| artifact.to_string())
                .collect::<Vec<_>>(),
        );

        let proof_path = dir
            .path()
            .join("release/evidence/benchmarks/cpu-only-100x-proof.json");
        let proof_text = fs::read_to_string(&proof_path)
            .expect("Fix: read generated CPU-SOTA 100x proof artifact.");
        let proof = serde_json::from_str::<Value>(&proof_text)
            .expect("Fix: generated CPU-SOTA 100x proof must be valid JSON.");
        let blockers = proof
            .get("blockers")
            .and_then(Value::as_array)
            .expect("Fix: generated CPU-SOTA proof must include blockers array.");

        assert!(
            !blockers.iter().filter_map(Value::as_str).any(|blocker| {
                blocker.contains("source_fingerprint `git:source-b:dirty=false` does not match aggregate source")
            }),
            "Fix: aggregate CPU-SOTA proof must tolerate clean evidence commit drift when source_tree_fingerprint carries source identity; blockers={blockers:?}"
        );
        assert!(
            blockers.iter().filter_map(Value::as_str).any(|blocker| {
                blocker.contains(
                    "100x source artifact `release/evidence/benchmarks/cuda-source-b.json` source_tree_fingerprint `source-tree-v1:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb` does not match aggregate source tree `source-tree-v1:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa`",
                )
            }),
            "Fix: aggregate CPU-SOTA proof must reject mixed source_tree_fingerprint inputs; blockers={blockers:?}"
        );
    }

    #[test]
    fn cpu_100x_proof_does_not_count_duplicate_source_artifacts() {
        let dir = TempDir::new()
            .expect("Fix: create a temporary workspace for duplicate-source CPU-SOTA proof test.");
        let artifact_rel = "release/evidence/benchmarks/cuda-duplicate-source.json";
        let artifact_path = dir.path().join(artifact_rel);
        fs::create_dir_all(
            artifact_path
                .parent()
                .expect("Fix: duplicate-source proof artifact path must have a parent directory."),
        )
        .expect("Fix: create duplicate-source proof artifact parent directory.");
        fs::write(
            &artifact_path,
            serde_json::to_string_pretty(&json!({
                "schema_version": 2,
                "selected_backend": "cuda",
                "source_fingerprint": "git:source-a:dirty=false",
                "source_tree_fingerprint": "source-tree-v1:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                "summary": {
                    "total_cases": 1,
                    "passed": 1,
                    "failed": 0,
                    "total_time_ns": 0,
                    "cache_hit_rate": null
                },
                "cases": [
                    {
                        "id": "release.condition_eval.1m",
                        "backend_id": "cuda",
                        "status": "pass",
                        "metrics": {
                            "wall_ns": {"samples": 30, "p50": 10, "p95": 11, "p99": 12},
                            "baseline_wall_ns": {"samples": 30, "p50": 2000, "p95": 2001, "p99": 2002}
                        },
                        "contract": {
                            "primitive": "release condition eval",
                            "baselines": [
                                {
                                    "name": "CPU-SOTA",
                                    "crate_name": "vyre-runtime",
                                    "class": "CpuSota",
                                    "min_speedup_x": 100.0,
                                    "backend_ids": ["cuda"]
                                }
                            ]
                        },
                        "performance": {"contract_passed": true, "speedup_x": 200.0}
                    }
                ]
            }))
            .expect("Fix: serialize duplicate-source CUDA benchmark artifact JSON."),
        )
        .expect("Fix: write duplicate-source CUDA benchmark artifact JSON.");

        write_cpu_100x_proof(
            dir.path(),
            &[artifact_rel.to_string(), artifact_rel.to_string()],
        );

        let proof_path = dir
            .path()
            .join("release/evidence/benchmarks/cpu-only-100x-proof.json");
        let proof_text = fs::read_to_string(&proof_path)
            .expect("Fix: read generated CPU-SOTA 100x proof artifact.");
        let proof = serde_json::from_str::<Value>(&proof_text)
            .expect("Fix: generated CPU-SOTA 100x proof must be valid JSON.");
        let blockers = proof
            .get("blockers")
            .and_then(Value::as_array)
            .expect("Fix: generated CPU-SOTA proof must include blockers array.");

        assert_eq!(
            proof.get("source_artifact_count").and_then(Value::as_u64),
            Some(1),
            "Fix: duplicate source_artifacts must not inflate aggregate source_artifact_count."
        );
        assert_eq!(
            proof
                .get("cpu_sota_100x_contract_case_count")
                .and_then(Value::as_u64),
            Some(1),
            "Fix: duplicate source_artifacts must not duplicate cases into the aggregate proof."
        );
        assert!(
            blockers.iter().filter_map(Value::as_str).any(|blocker| {
                blocker.contains(
                    "100x proof source_artifact `release/evidence/benchmarks/cuda-duplicate-source.json` is duplicated"
                )
            }),
            "Fix: aggregate CPU-SOTA proof must report duplicated source_artifacts; blockers={blockers:?}"
        );
    }

    #[test]
    fn cpu_100x_proof_rejects_absolute_source_artifact_path() {
        let dir = TempDir::new()
            .expect("Fix: create a temporary workspace for absolute-source CPU-SOTA proof test.");
        let external_artifact = dir.path().join("external-cuda-source.json");
        fs::write(&external_artifact, "{}").expect("Fix: write external CUDA benchmark artifact.");

        write_cpu_100x_proof(dir.path(), &[external_artifact.display().to_string()]);

        let proof_path = dir
            .path()
            .join("release/evidence/benchmarks/cpu-only-100x-proof.json");
        let proof_text = fs::read_to_string(&proof_path)
            .expect("Fix: read generated CPU-SOTA 100x proof artifact.");
        let proof = serde_json::from_str::<Value>(&proof_text)
            .expect("Fix: generated CPU-SOTA 100x proof must be valid JSON.");
        let blockers = proof
            .get("blockers")
            .and_then(Value::as_array)
            .expect("Fix: generated CPU-SOTA proof must include blockers array.");

        assert!(
            blockers.iter().filter_map(Value::as_str).any(|blocker| {
                blocker.contains("100x source_artifact `")
                    && blocker.contains("must be a relative release path")
            }),
            "Fix: aggregate CPU-SOTA proof generation must reject existing absolute source_artifact paths before reading them; blockers={blockers:?}"
        );
    }
}
