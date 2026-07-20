use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;

use super::args::{parse_args, Config};
use super::frontier_leaderboard::write_frontier_leaderboard;
use super::optimization::{write_optimization_benchmark_manifest, write_release_axes};
use super::runner::{
    benchmark_artifact_is_reusable, copy_artifact, run_command, run_named_benchmark_if_needed,
};
use super::suite_inspect::{
    backend_suite_output_path, prefixed_benchmark_artifact, read_text_bounded,
    run_workload_benchmark, write_backend_suite_with_extra_blockers, write_cpu_100x_proof,
};
use super::types::{
    BackendSuiteArtifactInput, ReleaseWorkloadFamily, ReleaseWorkloadMatrix,
    MAX_RELEASE_BENCHMARK_TEXT_BYTES, REQUIRED_CPU_SOTA_100X_CASES,
};

pub(crate) fn run(args: &[String]) {
    let config = match parse_args(args) {
        Ok(config) => config,
        Err(message) => {
            eprintln!("{message}");
            std::process::exit(2);
        }
    };
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    let matrix_path =
        workspace_root.join("release/evidence/benchmarks/release-workload-matrix.json");
    if let Some(parent) = matrix_path.parent() {
        if let Err(error) = fs::create_dir_all(parent) {
            eprintln!("Fix: failed to create `{}`: {error}", parent.display());
            std::process::exit(1);
        }
    }
    run_command(
        &workspace_root,
        &[
            "run",
            "-p",
            "vyre-bench",
            "--quiet",
            "--",
            "release-matrix",
            "--format",
            "json",
            "--output",
            "release/evidence/benchmarks/release-workload-matrix.json",
            "--enforce",
        ],
    );
    let matrix_text = match read_text_bounded(&matrix_path, MAX_RELEASE_BENCHMARK_TEXT_BYTES) {
        Ok(text) => text,
        Err(error) => {
            eprintln!("Fix: failed to read `{}`: {error}", matrix_path.display());
            std::process::exit(1);
        }
    };
    let matrix = match serde_json::from_str::<ReleaseWorkloadMatrix>(&matrix_text) {
        Ok(matrix) => matrix,
        Err(error) => {
            eprintln!("Fix: release workload matrix JSON is invalid: {error}");
            std::process::exit(1);
        }
    };

    let mut suite_artifacts = Vec::new();
    let mut cpu_100x_artifacts = Vec::new();
    let mut workload_failures = Vec::new();
    let mut primary_suite_failures = Vec::new();
    let mut ran = 0usize;
    for family in benchmark_suite_families(&matrix, config.only.as_deref()) {
        let cpu_100x_family = requires_release_cpu_sota_100x_proof(&config.backend, family);
        let required_reuse_speedup = reusable_cpu_sota_min_speedup(&config.backend, family);
        let prefer_cpu_sota_case = cpu_100x_family || config.backend == "wgpu";
        let Some(case_id) = select_release_benchmark_case(family, prefer_cpu_sota_case) else {
            eprintln!(
                "Fix: release workload `{}` has no matched benchmark case.",
                family.id
            );
            std::process::exit(1);
        };
        let evidence_artifact =
            backend_workload_artifact(&config.backend, &family.evidence_artifact);
        let mut workload_ok = true;
        if !config.refresh_suites_only
            && (!config.reuse_existing
                || !benchmark_artifact_is_reusable(
                    &workspace_root,
                    &config.backend,
                    &family.id,
                    case_id,
                    &evidence_artifact,
                    required_reuse_speedup,
                ))
        {
            if let Err(error) = run_workload_benchmark(
                &workspace_root,
                case_id,
                &config.backend,
                &evidence_artifact,
                config.measured_samples,
                config.sample_timeout_secs,
            ) {
                workload_ok = false;
                let failure = format!(
                    "backend `{}` family `{}` case `{}` artifact `{}`: {error}",
                    config.backend, family.id, case_id, evidence_artifact
                );
                workload_failures.push(failure.clone());
                primary_suite_failures.push(failure);
            }
        }
        if !config.refresh_suites_only
            && workload_ok
            && config.backend == "cuda"
            && family.id == "megakernel-queued-batches"
        {
            copy_artifact(
                &workspace_root,
                &evidence_artifact,
                "release/evidence/benchmarks/megakernel-condition-cuda.json",
            );
            copy_artifact(
                &workspace_root,
                &evidence_artifact,
                "release/evidence/benchmarks/megakernel-condition-100x-proof.json",
            );
        }
        if cpu_100x_family {
            cpu_100x_artifacts.push(evidence_artifact.clone());
        }
        if !config.refresh_suites_only
            && workload_ok
            && config.backend == "cuda"
            && family.id == "megakernel-queued-batches"
        {
            copy_artifact(
                &workspace_root,
                &evidence_artifact,
                "release/evidence/benchmarks/megakernel-latency-cuda.json",
            );
        }
        if !config.refresh_suites_only
            && workload_ok
            && config.backend == "cuda"
            && family.id == "alias-reaching-def"
        {
            copy_artifact(
                &workspace_root,
                &evidence_artifact,
                "release/evidence/benchmarks/dataflow-analysis-release.json",
            );
        }
        suite_artifacts.push(BackendSuiteArtifactInput {
            path: evidence_artifact,
            family_id: family.id.clone(),
            requested_case_id: case_id.clone(),
            cpu_sota_100x_required: cpu_100x_family,
        });
        ran += 1;
    }
    if config.only.is_none() && config.backend == "cuda" && config.include_wgpu_comparison {
        let mut wgpu_artifacts = Vec::new();
        let mut wgpu_suite_failures = Vec::new();
        for family in benchmark_suite_families(&matrix, None) {
            let Some(case_id) = select_release_benchmark_case(family, true) else {
                eprintln!(
                    "Fix: release workload `{}` has no matched benchmark case.",
                    family.id
                );
                std::process::exit(1);
            };
            let output = prefixed_benchmark_artifact(&family.evidence_artifact, "wgpu");
            if !config.refresh_suites_only
                && (!config.reuse_existing
                    || !benchmark_artifact_is_reusable(
                        &workspace_root,
                        "wgpu",
                        &family.id,
                        case_id,
                        &output,
                        reusable_cpu_sota_min_speedup("wgpu", family),
                    ))
            {
                if let Err(error) = run_workload_benchmark(
                    &workspace_root,
                    case_id,
                    "wgpu",
                    &output,
                    config.measured_samples,
                    config.sample_timeout_secs,
                ) {
                    let failure = format!(
                        "backend `wgpu` comparison family `{}` case `{}` artifact `{}`: {error}",
                        family.id, case_id, output
                    );
                    workload_failures.push(failure.clone());
                    wgpu_suite_failures.push(failure);
                }
            }
            wgpu_artifacts.push(BackendSuiteArtifactInput {
                path: output,
                family_id: family.id.clone(),
                requested_case_id: case_id.clone(),
                cpu_sota_100x_required: false,
            });
        }
        write_backend_suite_with_extra_blockers(
            &workspace_root,
            "wgpu",
            wgpu_artifacts,
            wgpu_suite_failures,
        );
    }
    let wrote_optimization_manifest = should_write_optimization_manifest(&config, workload_failures.is_empty());
    if wrote_optimization_manifest {
        run_named_benchmark_if_needed(
            &workspace_root,
            "lower.rewrites.impact.corpus",
            &config.backend,
            "release/evidence/optimization/lower-rewrite-impact-before-after.json",
            config.measured_samples,
            config.sample_timeout_secs,
            config.reuse_existing,
        );
        copy_artifact(
            &workspace_root,
            "release/evidence/optimization/lower-rewrite-impact-before-after.json",
            "release/evidence/optimization/pass-family-benchmarks.json",
        );
        run_named_benchmark_if_needed(
            &workspace_root,
            "foundation.optimizer.impact",
            &config.backend,
            "release/evidence/optimization/optimizer-impact-cuda.json",
            config.measured_samples,
            config.sample_timeout_secs,
            config.reuse_existing,
        );
        run_named_benchmark_if_needed(
            &workspace_root,
            "cuda.ptx.patterns.release.corpus",
            &config.backend,
            "release/evidence/benchmarks/cuda-ptx-patterns.json",
            config.measured_samples,
            config.sample_timeout_secs,
            config.reuse_existing,
        );
        run_named_benchmark_if_needed(
            &workspace_root,
            "lower.egraph_saturation",
            &config.backend,
            "release/evidence/optimization/egraph-before-after.json",
            config.measured_samples,
            config.sample_timeout_secs,
            config.reuse_existing,
        );
        copy_artifact(
            &workspace_root,
            "release/evidence/optimization/egraph-before-after.json",
            "release/evidence/benchmarks/egraph-before-after.json",
        );
        run_named_benchmark_if_needed(
            &workspace_root,
            "lower.alias_aware_optimizations",
            &config.backend,
            "release/evidence/benchmarks/alias-aware-before-after.json",
            config.measured_samples,
            config.sample_timeout_secs,
            config.reuse_existing,
        );
        run_command(
            &workspace_root,
            &[
                "run",
                "--bin",
                "xtask",
                "--quiet",
                "--",
                "optimization-matrix",
                "--output",
                "release/evidence/optimization/optimization-integration-matrix.json",
            ],
        );
        write_optimization_benchmark_manifest(&workspace_root, &config.backend);
    }
    if ran == 0 {
        eprintln!("Fix: release-benchmarks selected zero benchmark families.");
        std::process::exit(1);
    }
    if config.backend == "cuda" {
        write_cpu_100x_proof(&workspace_root, &cpu_100x_artifacts);
    }
    write_backend_suite_with_extra_blockers(
        &workspace_root,
        &config.backend,
        suite_artifacts,
        primary_suite_failures,
    );
    if config.backend == "cuda" {
        write_frontier_leaderboard(&workspace_root);
        write_release_axes(&workspace_root);
    }
    let generated_evidence_paths = generated_release_benchmark_evidence_paths(
        &config.backend,
        config.backend == "cuda",
        config.backend == "cuda",
        config.include_wgpu_comparison && config.only.is_none() && config.backend == "cuda",
        wrote_optimization_manifest,
    );
    let generated_blockers =
        generated_benchmark_evidence_blockers(&workspace_root, &generated_evidence_paths);
    for failure in &workload_failures {
        eprintln!("Fix: release workload benchmark failed: {failure}");
    }
    for blocker in &generated_blockers {
        eprintln!("Fix: generated release benchmark evidence blocker: {blocker}");
    }
    if !workload_failures.is_empty() || !generated_blockers.is_empty() {
        std::process::exit(1);
    }
    if config.refresh_suites_only {
        println!("release-benchmarks: refreshed suite evidence for {ran} benchmark artifact(s)");
    } else {
        println!("release-benchmarks: wrote {ran} benchmark artifact(s)");
    }
}

fn should_write_optimization_manifest(config: &Config, workload_failures_empty: bool) -> bool {
    workload_failures_empty
        && config.backend == "cuda"
        && config.only.is_none()
        && !config.refresh_suites_only
        && !config.workload_suite_only
}

fn benchmark_suite_families<'a>(
    matrix: &'a ReleaseWorkloadMatrix,
    only: Option<&str>,
) -> Vec<&'a ReleaseWorkloadFamily> {
    matrix
        .families
        .iter()
        .filter(|family| only.is_none_or(|only| only == family.id))
        .collect()
}

fn select_release_benchmark_case<'a>(
    family: &'a ReleaseWorkloadFamily,
    prefer_cpu_sota_100x: bool,
) -> Option<&'a String> {
    if prefer_cpu_sota_100x && !family.cpu_sota_100x_cases.is_empty() {
        let required_case = family
            .cpu_sota_100x_cases
            .iter()
            .find(|case_id| REQUIRED_CPU_SOTA_100X_CASES.contains(&case_id.as_str()));
        return if family.required {
            required_case
        } else {
            required_case.or_else(|| family.cpu_sota_100x_cases.first())
        };
    }
    family
        .matched_cases
        .iter()
        .find(|case_id| REQUIRED_CPU_SOTA_100X_CASES.contains(&case_id.as_str()))
        .or_else(|| family.matched_cases.first())
}

fn requires_release_cpu_sota_100x_proof(backend: &str, family: &ReleaseWorkloadFamily) -> bool {
    backend == "cuda"
        && family.required
        && family
            .max_cpu_sota_min_speedup_x
            .is_some_and(|speedup| speedup >= 100.0)
}

fn reusable_cpu_sota_min_speedup(backend: &str, family: &ReleaseWorkloadFamily) -> Option<f64> {
    if backend != "cuda" && backend != "wgpu" {
        return None;
    }
    family
        .max_cpu_sota_min_speedup_x
        .filter(|speedup| *speedup > 1.0)
}

fn backend_workload_artifact(backend: &str, matrix_artifact: &str) -> String {
    if backend == "wgpu" {
        prefixed_benchmark_artifact(matrix_artifact, "wgpu")
    } else {
        matrix_artifact.to_string()
    }
}

fn generated_release_benchmark_evidence_paths(
    backend: &str,
    include_release_axes: bool,
    include_cpu_100x_proof: bool,
    include_wgpu_comparison: bool,
    include_optimization_manifest: bool,
) -> Vec<String> {
    let mut paths = vec![backend_suite_output_path(backend)];
    if include_release_axes {
        paths.push("release/evidence/benchmarks/bench-release-axes.json".to_string());
        paths.push(crate::artifact_paths::FRONTIER_LEADERBOARD_ARTIFACT.to_string());
    }
    if include_cpu_100x_proof {
        paths.push("release/evidence/benchmarks/cpu-only-100x-proof.json".to_string());
    }
    if include_wgpu_comparison {
        paths.push(backend_suite_output_path("wgpu"));
    }
    if include_optimization_manifest {
        paths.push("release/evidence/optimization/pass-family-benchmark-manifest.json".to_string());
    }
    paths
}

fn generated_benchmark_evidence_blockers(workspace_root: &Path, paths: &[String]) -> Vec<String> {
    let mut blockers = Vec::new();
    for path in paths {
        let artifact_path = workspace_root.join(path);
        let text = match read_text_bounded(&artifact_path, MAX_RELEASE_BENCHMARK_TEXT_BYTES) {
            Ok(text) => text,
            Err(error) => {
                blockers.push(format!("`{path}` is unreadable: {error}"));
                continue;
            }
        };
        let value = match serde_json::from_str::<Value>(&text) {
            Ok(value) => value,
            Err(error) => {
                blockers.push(format!("`{path}` is invalid JSON: {error}"));
                continue;
            }
        };
        blockers.extend(
            crate::benchmark_evidence_semantics::benchmark_evidence_blocker_issues(path, &value),
        );
    }
    blockers
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wgpu_comparison_prefers_release_defining_cpu_sota_case() {
        let family = ReleaseWorkloadFamily {
            id: "condition-eval".to_string(),
            required: true,
            matched_cases: vec![
                "conditions.yara_like.batch.16x64k".to_string(),
                "release.condition_eval.1m".to_string(),
            ],
            evidence_artifact: "release/evidence/benchmarks/workload-01-condition-eval.json"
                .to_string(),
            max_cpu_sota_min_speedup_x: Some(100.0),
            cpu_sota_100x_cases: vec![
                "conditions.yara_like.eval.1m".to_string(),
                "release.condition_eval.1m".to_string(),
            ],
        };

        assert_eq!(
            select_release_benchmark_case(&family, true).map(String::as_str),
            Some("release.condition_eval.1m"),
            "Fix: WGPU comparison suite generation must not drift to a broad matched case when a release-defining CPU-SOTA case exists."
        );
    }

    #[test]
    fn cpu_sota_selection_rejects_non_release_defining_cpu_sota_cases() {
        let family = ReleaseWorkloadFamily {
            id: "condition-eval".to_string(),
            required: true,
            matched_cases: vec![
                "conditions.yara_like.batch.16x64k".to_string(),
                "release.condition_eval.1m".to_string(),
            ],
            evidence_artifact: "release/evidence/benchmarks/workload-01-condition-eval.json"
                .to_string(),
            max_cpu_sota_min_speedup_x: Some(100.0),
            cpu_sota_100x_cases: vec!["conditions.yara_like.eval.1m".to_string()],
        };

        assert_eq!(
            select_release_benchmark_case(&family, true),
            None,
            "Fix: release-benchmarks must fail before dispatch when the CPU-SOTA case list cannot prove a required release-defining 100x case."
        );
    }

    #[test]
    fn optional_cpu_sota_family_selects_its_proof_case() {
        let family = ReleaseWorkloadFamily {
            id: "quantized-linear".to_string(),
            required: false,
            matched_cases: vec!["nn.linear_4bit_affine_grouped.1m".to_string()],
            evidence_artifact: "release/evidence/benchmarks/workload-16-quantized-linear.json"
                .to_string(),
            max_cpu_sota_min_speedup_x: Some(100.0),
            cpu_sota_100x_cases: vec!["nn.linear_4bit_affine_grouped.1m".to_string()],
        };

        assert_eq!(
            select_release_benchmark_case(&family, true).map(String::as_str),
            Some("nn.linear_4bit_affine_grouped.1m"),
            "Fix: optional CPU-SOTA 100x families must still generate suite evidence for their proof case."
        );
    }

    #[test]
    fn required_cpu_sota_proof_scope_excludes_optional_workloads() {
        let required_family = ReleaseWorkloadFamily {
            id: "condition-eval".to_string(),
            required: true,
            matched_cases: vec!["release.condition_eval.1m".to_string()],
            evidence_artifact: "release/evidence/benchmarks/workload-01-condition-eval.json"
                .to_string(),
            max_cpu_sota_min_speedup_x: Some(100.0),
            cpu_sota_100x_cases: vec!["release.condition_eval.1m".to_string()],
        };
        let optional_family = ReleaseWorkloadFamily {
            id: "quantized-linear".to_string(),
            required: false,
            matched_cases: vec!["nn.linear_4bit_affine_grouped.1m".to_string()],
            evidence_artifact: "release/evidence/benchmarks/workload-16-quantized-linear.json"
                .to_string(),
            max_cpu_sota_min_speedup_x: Some(100.0),
            cpu_sota_100x_cases: vec!["nn.linear_4bit_affine_grouped.1m".to_string()],
        };

        assert!(
            requires_release_cpu_sota_100x_proof("cuda", &required_family),
            "Fix: required CUDA 100x families must remain in the release-defining proof aggregate."
        );
        assert!(
            !requires_release_cpu_sota_100x_proof("cuda", &optional_family),
            "Fix: optional CUDA 100x families must not be counted as required release-defining proof cases."
        );
        assert!(
            !requires_release_cpu_sota_100x_proof("wgpu", &required_family),
            "Fix: WGPU comparison evidence must not rewrite the CUDA-only 100x proof aggregate."
        );
    }

    #[test]
    fn optional_cpu_sota_family_requires_reusable_contract_evidence() {
        let family = ReleaseWorkloadFamily {
            id: "quantized-linear".to_string(),
            required: false,
            matched_cases: vec!["nn.linear_4bit_affine_grouped.1m".to_string()],
            evidence_artifact: "release/evidence/benchmarks/workload-16-quantized-linear.json"
                .to_string(),
            max_cpu_sota_min_speedup_x: Some(100.0),
            cpu_sota_100x_cases: vec!["nn.linear_4bit_affine_grouped.1m".to_string()],
        };

        assert_eq!(
            reusable_cpu_sota_min_speedup("cuda", &family),
            Some(100.0),
            "Fix: CUDA optional CPU-SOTA workloads must prove their own contract before reuse."
        );
        assert_eq!(
            reusable_cpu_sota_min_speedup("wgpu", &family),
            Some(100.0),
            "Fix: WGPU comparison artifacts must prove the same CPU-SOTA contract before reuse."
        );
        assert_eq!(
            reusable_cpu_sota_min_speedup("cpu-ref", &family),
            None,
            "Fix: reusable GPU release contract checks should stay scoped to release GPU backends."
        );
    }

    #[test]
    fn wgpu_primary_backend_uses_prefixed_workload_artifacts() {
        assert_eq!(
            backend_workload_artifact(
                "wgpu",
                "release/evidence/benchmarks/workload-01-condition-eval.json"
            ),
            "release/evidence/benchmarks/wgpu-workload-01-condition-eval.json",
            "Fix: running release-benchmarks --backend wgpu must not overwrite CUDA workload evidence."
        );
        assert_eq!(
            backend_workload_artifact(
                "cuda",
                "release/evidence/benchmarks/workload-01-condition-eval.json"
            ),
            "release/evidence/benchmarks/workload-01-condition-eval.json"
        );
    }

    #[test]
    fn benchmark_suite_families_include_optional_release_workloads() {
        let matrix = ReleaseWorkloadMatrix {
            families: vec![
                ReleaseWorkloadFamily {
                    id: "condition-eval".to_string(),
                    required: true,
                    matched_cases: vec!["release.condition_eval.1m".to_string()],
                    evidence_artifact:
                        "release/evidence/benchmarks/workload-01-condition-eval.json".to_string(),
                    max_cpu_sota_min_speedup_x: Some(100.0),
                    cpu_sota_100x_cases: vec!["release.condition_eval.1m".to_string()],
                },
                ReleaseWorkloadFamily {
                    id: "adaptive-routing".to_string(),
                    required: false,
                    matched_cases: vec!["runtime.adaptive_routing.gpu_resident.1m".to_string()],
                    evidence_artifact:
                        "release/evidence/benchmarks/workload-15-adaptive-routing.json".to_string(),
                    max_cpu_sota_min_speedup_x: Some(10.0),
                    cpu_sota_100x_cases: Vec::new(),
                },
            ],
        };

        let families = benchmark_suite_families(&matrix, None)
            .into_iter()
            .map(|family| family.id.as_str())
            .collect::<Vec<_>>();

        assert_eq!(
            families,
            vec!["condition-eval", "adaptive-routing"],
            "Fix: release-benchmarks must generate suite evidence for every release matrix family, including non-required CUDA acceleration workloads."
        );
    }

    #[test]
    fn benchmark_suite_only_filter_can_select_optional_workload() {
        let matrix = ReleaseWorkloadMatrix {
            families: vec![
                ReleaseWorkloadFamily {
                    id: "condition-eval".to_string(),
                    required: true,
                    matched_cases: vec!["release.condition_eval.1m".to_string()],
                    evidence_artifact:
                        "release/evidence/benchmarks/workload-01-condition-eval.json".to_string(),
                    max_cpu_sota_min_speedup_x: Some(100.0),
                    cpu_sota_100x_cases: vec!["release.condition_eval.1m".to_string()],
                },
                ReleaseWorkloadFamily {
                    id: "compound-fused-filter".to_string(),
                    required: false,
                    matched_cases: vec!["compound.pipeline.fused_filter.1m".to_string()],
                    evidence_artifact:
                        "release/evidence/benchmarks/workload-14-compound-fused-filter.json"
                            .to_string(),
                    max_cpu_sota_min_speedup_x: Some(10.0),
                    cpu_sota_100x_cases: Vec::new(),
                },
            ],
        };

        let families = benchmark_suite_families(&matrix, Some("compound-fused-filter"))
            .into_iter()
            .map(|family| family.id.as_str())
            .collect::<Vec<_>>();

        assert_eq!(
            families,
            vec!["compound-fused-filter"],
            "Fix: --only must allow dogfooding optional release workload families instead of limiting selection to required rows."
        );
    }

    #[test]
    fn generated_evidence_blockers_surface_written_suite_blockers() {
        let dir = tempfile::TempDir::new()
            .expect("Fix: create temporary workspace for generated blocker test.");
        let artifact = "release/evidence/benchmarks/cuda-release-suite.json".to_string();
        let artifact_path = dir.path().join(&artifact);
        fs::create_dir_all(
            artifact_path
                .parent()
                .expect("Fix: temporary artifact has a parent directory."),
        )
        .expect("Fix: create temporary generated evidence directory.");
        fs::write(
            &artifact_path,
            serde_json::to_string_pretty(&serde_json::json!({
                "blockers": ["stale source fingerprint"]
            }))
            .expect("Fix: serialize temporary generated evidence."),
        )
        .expect("Fix: write temporary generated evidence.");

        let blockers = generated_benchmark_evidence_blockers(dir.path(), &[artifact]);

        assert_eq!(
            blockers,
            vec![
                "`release/evidence/benchmarks/cuda-release-suite.json` blocker[0]: stale source fingerprint"
                    .to_string(),
                "`release/evidence/benchmarks/cuda-release-suite.json` is missing artifact_statuses array"
                    .to_string()
            ],
            "Fix: release-benchmarks must fail closed when generated suite evidence carries blockers."
        );
    }

    #[test]
    fn generated_evidence_blockers_reject_suite_missing_artifact_statuses() {
        let dir = tempfile::TempDir::new()
            .expect("Fix: create temporary workspace for generated suite inventory test.");
        let artifact = "release/evidence/benchmarks/cuda-release-suite.json".to_string();
        let artifact_path = dir.path().join(&artifact);
        fs::create_dir_all(
            artifact_path
                .parent()
                .expect("Fix: temporary artifact has a parent directory."),
        )
        .expect("Fix: create temporary generated evidence directory.");
        fs::write(
            &artifact_path,
            serde_json::to_string_pretty(&serde_json::json!({
                "blockers": []
            }))
            .expect("Fix: serialize suite evidence without artifact_statuses."),
        )
        .expect("Fix: write suite evidence without artifact_statuses.");

        let blockers = generated_benchmark_evidence_blockers(dir.path(), &[artifact]);

        assert_eq!(
            blockers,
            vec![
                "`release/evidence/benchmarks/cuda-release-suite.json` is missing artifact_statuses array"
                    .to_string()
            ],
            "Fix: release-benchmarks must fail closed when generated suite evidence omits artifact_statuses."
        );
    }

    #[test]
    fn generated_evidence_blockers_surface_suite_status_blockers() {
        let dir = tempfile::TempDir::new()
            .expect("Fix: create temporary workspace for generated suite status blocker test.");
        let artifact = "release/evidence/benchmarks/cuda-release-suite.json".to_string();
        let artifact_path = dir.path().join(&artifact);
        fs::create_dir_all(
            artifact_path
                .parent()
                .expect("Fix: temporary artifact has a parent directory."),
        )
        .expect("Fix: create temporary generated evidence directory.");
        fs::write(
            &artifact_path,
            serde_json::to_string_pretty(&serde_json::json!({
                "blockers": [],
                "artifact_statuses": [
                    {
                        "path": "release/evidence/benchmarks/workload-01-condition-eval.json",
                        "blockers": ["case `release.condition_eval.1m` failed: wrong answer"]
                    }
                ]
            }))
            .expect("Fix: serialize suite evidence with nested status blocker."),
        )
        .expect("Fix: write suite evidence with nested status blocker.");

        let blockers = generated_benchmark_evidence_blockers(dir.path(), &[artifact]);

        assert_eq!(
            blockers,
            vec![
                "`release/evidence/benchmarks/cuda-release-suite.json` artifact_statuses[0] `release/evidence/benchmarks/workload-01-condition-eval.json` blocker[0]: case `release.condition_eval.1m` failed: wrong answer"
                    .to_string()
            ],
            "Fix: release-benchmarks must fail closed when generated suite status rows carry blockers."
        );
    }

    #[test]
    fn generated_evidence_blockers_reject_suite_status_missing_blockers_array() {
        let dir = tempfile::TempDir::new()
            .expect("Fix: create temporary workspace for generated suite status blocker test.");
        let artifact = "release/evidence/benchmarks/cuda-release-suite.json".to_string();
        let artifact_path = dir.path().join(&artifact);
        fs::create_dir_all(
            artifact_path
                .parent()
                .expect("Fix: temporary artifact has a parent directory."),
        )
        .expect("Fix: create temporary generated evidence directory.");
        fs::write(
            &artifact_path,
            serde_json::to_string_pretty(&serde_json::json!({
                "blockers": [],
                "artifact_statuses": [
                    {
                        "path": "release/evidence/benchmarks/workload-01-condition-eval.json"
                    }
                ]
            }))
            .expect("Fix: serialize suite evidence with nested status blocker."),
        )
        .expect("Fix: write suite evidence with nested status blocker.");

        let blockers = generated_benchmark_evidence_blockers(dir.path(), &[artifact]);

        assert_eq!(
            blockers,
            vec![
                "`release/evidence/benchmarks/cuda-release-suite.json` artifact_statuses[0] `release/evidence/benchmarks/workload-01-condition-eval.json` is missing blockers array"
                    .to_string()
            ],
            "Fix: release-benchmarks must fail closed when generated suite status rows omit blockers."
        );
    }

    #[test]
    fn generated_evidence_blockers_reject_missing_blockers_array() {
        let dir = tempfile::TempDir::new()
            .expect("Fix: create temporary workspace for missing blockers test.");
        let artifact = "release/evidence/benchmarks/bench-release-axes.json".to_string();
        let artifact_path = dir.path().join(&artifact);
        fs::create_dir_all(
            artifact_path
                .parent()
                .expect("Fix: temporary artifact has a parent directory."),
        )
        .expect("Fix: create temporary generated evidence directory.");
        fs::write(
            &artifact_path,
            serde_json::to_string_pretty(&serde_json::json!({
                "schema_version": 1,
                "source_artifacts": []
            }))
            .expect("Fix: serialize generated evidence without blockers."),
        )
        .expect("Fix: write generated evidence without blockers.");

        let blockers = generated_benchmark_evidence_blockers(dir.path(), &[artifact]);

        assert_eq!(
            blockers,
            vec![
                "`release/evidence/benchmarks/bench-release-axes.json` is missing blockers array"
                    .to_string()
            ],
            "Fix: release-benchmarks must fail closed when generated evidence omits blockers."
        );
    }

    #[test]
    fn generated_evidence_paths_include_release_proof_surfaces() {
        assert_eq!(
            generated_release_benchmark_evidence_paths("cuda", true, true, true, true),
            vec![
                "release/evidence/benchmarks/cuda-release-suite.json",
                "release/evidence/benchmarks/bench-release-axes.json",
                crate::artifact_paths::FRONTIER_LEADERBOARD_ARTIFACT,
                "release/evidence/benchmarks/cpu-only-100x-proof.json",
                "release/evidence/benchmarks/wgpu-fallback-suite.json",
                "release/evidence/optimization/pass-family-benchmark-manifest.json"
            ],
            "Fix: command-level blocker checks must cover suite, axes, frontier leaderboard, CPU-SOTA proof, WGPU comparison, and optimization proof artifacts generated by release-benchmarks."
        );
    }

    #[test]
    fn wgpu_generated_evidence_paths_do_not_include_cuda_release_axes() {
        assert_eq!(
            generated_release_benchmark_evidence_paths("wgpu", false, false, false, false),
            vec!["release/evidence/benchmarks/wgpu-fallback-suite.json"],
            "Fix: release-benchmarks --backend wgpu must not rewrite or gate CUDA-only bench-release axes as a fallback-suite side effect."
        );
    }

    #[test]
    fn wgpu_backend_does_not_write_cuda_owned_optimization_manifest() {
        let mut config = Config {
            backend: "wgpu".to_string(),
            only: None,
            measured_samples: Some(30),
            sample_timeout_secs: 120,
            include_wgpu_comparison: false,
            reuse_existing: false,
            refresh_suites_only: false,
            workload_suite_only: false,
        };

        let wrote_optimization_manifest = should_write_optimization_manifest(&config, true);
        assert!(
            !wrote_optimization_manifest,
            "Fix: WGPU fallback refresh must not overwrite CUDA-owned optimization benchmark evidence."
        );

        config.backend = "cuda".to_string();
        let wrote_optimization_manifest = should_write_optimization_manifest(&config, true);
        assert!(
            wrote_optimization_manifest,
            "Fix: CUDA release refresh must still regenerate optimization benchmark evidence."
        );
        assert!(
            !should_write_optimization_manifest(&config, false),
            "Fix: auxiliary optimization evidence must not refresh after workload benchmark failures."
        );
    }
}
