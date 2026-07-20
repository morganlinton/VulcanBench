use crate::report::json::ReportSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use super::cli_compare::{build_comparison_artifact, parse_comparison_artifact, validate_comparison_expectations, ComparisonArtifact, ComparisonCase};
use super::cli_report_io::{parse_report, read_report_bounded, validate_report_expectations};

pub(super) const BENCHMARK_BUNDLE_SCHEMA: &str = "vyre-bench.bundle.v1";
pub(super) const MAC_BENCHMARK_BUNDLE_CASE_ID: &str = "foundation.elementwise.add.1m";
pub(super) const MAC_BENCHMARK_BUNDLE_BASELINE_BACKEND: &str = "wgpu";
pub(super) const MAC_BENCHMARK_BUNDLE_CANDIDATE_BACKEND: &str = "metal";
const MAC_BENCHMARK_BUNDLE_COMPARISONS: &[(&str, &str, &str, &str)] = &[
    ("wgpu-vs-metal.json", "wgpu-vs-metal.txt", "wgpu", "metal"),
    (
        "cpu-ref-vs-metal.json",
        "cpu-ref-vs-metal.txt",
        "cpu-ref",
        "metal",
    ),
];
const BENCHMARK_BUNDLE_REQUIRED_ARTIFACTS: &[(&str, &str)] = &[
    ("cpu-ref.json", "backend_report"),
    ("wgpu.json", "backend_report"),
    ("metal.json", "backend_report"),
    ("wgpu-vs-metal.json", "comparison_json"),
    ("wgpu-vs-metal.txt", "comparison_text"),
    ("cpu-ref-vs-metal.json", "comparison_json"),
    ("cpu-ref-vs-metal.txt", "comparison_text"),
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct BenchmarkBundleManifest {
    pub(super) schema: String,
    pub(super) provenance: BenchmarkBundleProvenance,
    pub(super) artifact_count: usize,
    pub(super) bundle_blake3: String,
    pub(super) artifacts: Vec<BundleArtifact>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(super) struct BenchmarkBundleProvenance {
    pub(super) validator: String,
    pub(super) validator_version: String,
    pub(super) suite: String,
    pub(super) case_id: String,
    pub(super) report_backends: Vec<String>,
    pub(super) baseline_backend: String,
    pub(super) candidate_backend: String,
    pub(super) comparison_pairs: Vec<String>,
    pub(super) source_fingerprint: String,
    pub(super) source_tree_fingerprint: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct BundleArtifact {
    pub(super) path: String,
    pub(super) kind: String,
    pub(super) bytes: u64,
    pub(super) blake3: String,
}

#[derive(Serialize)]
pub(super) struct BundleHashMaterial<'a> {
    pub(super) schema: &'static str,
    pub(super) provenance: &'a BenchmarkBundleProvenance,
    pub(super) artifacts: &'a [BundleArtifact],
}

pub(super) fn validate_benchmark_bundle(
    dir: &str,
    manifest_output: Option<&str>,
    manifest_input: Option<&str>,
) -> anyhow::Result<BenchmarkBundleManifest> {
    let dir = std::path::Path::new(dir);
    if !dir.is_dir() {
        anyhow::bail!(
            "benchmark bundle dir `{}` is not a directory. Fix: pass the VYRE_MACBOOK_BENCH_OUTPUT_DIR directory produced by scripts/check_metal_macbook.sh benchmark.",
            dir.display()
        );
    }
    let mut artifacts = Vec::new();
    let mut reports = Vec::new();
    for backend in ["cpu-ref", "wgpu", "metal"] {
        let path = dir.join(format!("{backend}.json"));
        let bytes = read_report_bounded(&path).map_err(|error| {
            anyhow::anyhow!(
                "failed to load benchmark report `{}`: {error}. Fix: rerun the benchmark gate so {backend}.json is produced.",
                path.display()
            )
        })?;
        let report = parse_report(&bytes, &path.to_string_lossy())?;
        validate_report_expectations(&report, Some(backend), Some(1), Some(0))?;
        reports.push(report);
        artifacts.push(bundle_artifact(dir, &path, "backend_report", &bytes)?);
    }
    let mut comparisons = Vec::new();
    for (json_name, text_name, baseline_backend, candidate_backend) in
        MAC_BENCHMARK_BUNDLE_COMPARISONS
    {
        let comparison_json = dir.join(json_name);
        let comparison_bytes = read_report_bounded(&comparison_json).map_err(|error| {
            anyhow::anyhow!(
                "failed to load comparison artifact `{}`: {error}. Fix: rerun vyre-bench compare --output from the benchmark gate.",
                comparison_json.display()
            )
        })?;
        let comparison = parse_comparison_artifact(&comparison_bytes)?;
        validate_comparison_expectations(
            &comparison,
            baseline_backend,
            candidate_backend,
            &[MAC_BENCHMARK_BUNDLE_CASE_ID.to_string()],
        )?;
        validate_comparison_matches_bundle_reports(&comparison, &reports)?;
        artifacts.push(bundle_artifact(
            dir,
            &comparison_json,
            "comparison_json",
            &comparison_bytes,
        )?);

        let comparison_text_path = dir.join(text_name);
        let comparison_text_bytes =
            read_report_bounded(&comparison_text_path).map_err(|error| {
                anyhow::anyhow!(
                    "failed to load comparison text artifact `{}`: {error}. Fix: rerun vyre-bench compare from the benchmark gate.",
                    comparison_text_path.display()
                )
            })?;
        if comparison_text_bytes.is_empty() {
            anyhow::bail!(
                "comparison text artifact `{}` is empty. Fix: rerun vyre-bench compare from the MacBook benchmark gate.",
                comparison_text_path.display()
            );
        }
        let comparison_text = std::str::from_utf8(&comparison_text_bytes)?;
        validate_comparison_text_evidence(
            comparison_text,
            &comparison,
            baseline_backend,
            candidate_backend,
        )?;
        artifacts.push(bundle_artifact(
            dir,
            &comparison_text_path,
            "comparison_text",
            &comparison_text_bytes,
        )?);
        comparisons.push(comparison);
    }

    let provenance = derive_benchmark_bundle_provenance(&reports, &comparisons)?;
    let manifest = build_benchmark_bundle_manifest(artifacts, provenance)?;
    if let Some(path) = manifest_input {
        let expected = load_benchmark_bundle_manifest(std::path::Path::new(path))?;
        validate_benchmark_bundle_manifest_matches(&expected, &manifest, path)?;
    }
    if let Some(path) = manifest_output {
        write_benchmark_bundle_manifest(&manifest, std::path::Path::new(path))?;
    }
    Ok(manifest)
}

pub(super) fn bundle_artifact(
    dir: &std::path::Path,
    path: &std::path::Path,
    kind: &str,
    bytes: &[u8],
) -> anyhow::Result<BundleArtifact> {
    let relative = path.strip_prefix(dir).map_err(|error| {
        anyhow::anyhow!(
            "bundle artifact `{}` is not under bundle dir `{}`: {error}. Fix: validate artifacts from one benchmark output directory.",
            path.display(),
            dir.display()
        )
    })?;
    Ok(BundleArtifact {
        path: relative.to_string_lossy().replace('\\', "/"),
        kind: kind.to_string(),
        bytes: bytes.len() as u64,
        blake3: blake3::hash(bytes).to_hex().to_string(),
    })
}

pub(super) fn build_benchmark_bundle_manifest(
    mut artifacts: Vec<BundleArtifact>,
    provenance: BenchmarkBundleProvenance,
) -> anyhow::Result<BenchmarkBundleManifest> {
    artifacts.sort_by(|left, right| {
        left.path
            .cmp(&right.path)
            .then_with(|| left.kind.cmp(&right.kind))
    });
    let material = BundleHashMaterial {
        schema: BENCHMARK_BUNDLE_SCHEMA,
        provenance: &provenance,
        artifacts: &artifacts,
    };
    let canonical = serde_json::to_vec(&material)?;
    Ok(BenchmarkBundleManifest {
        schema: BENCHMARK_BUNDLE_SCHEMA.to_string(),
        provenance,
        artifact_count: artifacts.len(),
        bundle_blake3: blake3::hash(&canonical).to_hex().to_string(),
        artifacts,
    })
}

pub(super) fn derive_benchmark_bundle_provenance(
    reports: &[ReportSchema],
    comparisons: &[ComparisonArtifact],
) -> anyhow::Result<BenchmarkBundleProvenance> {
    if reports.is_empty() {
        anyhow::bail!(
            "benchmark bundle has no backend reports. Fix: rerun the benchmark gate so cpu-ref, wgpu, and metal reports are present."
        );
    }
    if comparisons.is_empty() {
        anyhow::bail!(
            "benchmark bundle has no comparison artifacts. Fix: rerun the benchmark gate so comparison JSON artifacts are present."
        );
    }
    let mut suites = reports
        .iter()
        .map(|report| report.suite.as_str())
        .collect::<Vec<_>>();
    suites.sort_unstable();
    suites.dedup();
    if suites.len() != 1 {
        anyhow::bail!(
            "benchmark bundle reports disagree on suite {:?}. Fix: regenerate the bundle from one vyre-bench run configuration.",
            suites
        );
    }
    let mut case_ids = comparisons
        .iter()
        .flat_map(|comparison| comparison.cases.iter())
        .map(|case| case.id.as_str())
        .collect::<Vec<_>>();
    case_ids.sort_unstable();
    case_ids.dedup();
    if case_ids.len() != 1 {
        anyhow::bail!(
            "benchmark bundle comparison must contain exactly one case for the Mac smoke bundle, got {:?}. Fix: rerun scripts/check_metal_macbook.sh benchmark.",
            case_ids
        );
    }
    let mut report_backends = reports
        .iter()
        .map(|report| {
            report
                .selected_backend
                .clone()
                .unwrap_or_else(|| "unknown".to_string())
        })
        .collect::<Vec<_>>();
    report_backends.sort();
    let mut source_fingerprints = reports
        .iter()
        .map(|report| report.source_fingerprint.clone())
        .collect::<Vec<_>>();
    for comparison in comparisons {
        source_fingerprints.push(comparison.baseline.source_fingerprint.clone());
        source_fingerprints.push(comparison.candidate.source_fingerprint.clone());
    }
    source_fingerprints.sort();
    source_fingerprints.dedup();
    if source_fingerprints.len() != 1 {
        anyhow::bail!(
            "benchmark bundle reports disagree on source_fingerprint {:?}. Fix: regenerate all benchmark artifacts from the same source checkout.",
            source_fingerprints
        );
    }
    let mut source_tree_fingerprints = reports
        .iter()
        .map(|report| report.source_tree_fingerprint.clone())
        .collect::<Vec<_>>();
    for comparison in comparisons {
        source_tree_fingerprints.push(comparison.baseline.source_tree_fingerprint.clone());
        source_tree_fingerprints.push(comparison.candidate.source_tree_fingerprint.clone());
    }
    source_tree_fingerprints.sort();
    source_tree_fingerprints.dedup();
    if source_tree_fingerprints.len() != 1 {
        anyhow::bail!(
            "benchmark bundle reports disagree on source_tree_fingerprint {:?}. Fix: regenerate all benchmark artifacts from the same source checkout.",
            source_tree_fingerprints
        );
    }
    let mut comparison_pairs = comparisons
        .iter()
        .map(|comparison| {
            format!(
                "{}->{}",
                comparison.baseline.profile_backend, comparison.candidate.profile_backend
            )
        })
        .collect::<Vec<_>>();
    comparison_pairs.sort();
    comparison_pairs.dedup();
    Ok(BenchmarkBundleProvenance {
        validator: "vyre-bench validate-benchmark-bundle".to_string(),
        validator_version: env!("CARGO_PKG_VERSION").to_string(),
        suite: suites[0].to_string(),
        case_id: case_ids[0].to_string(),
        report_backends,
        baseline_backend: comparisons[0].baseline.profile_backend.clone(),
        candidate_backend: comparisons[0].candidate.profile_backend.clone(),
        comparison_pairs,
        source_fingerprint: source_fingerprints[0].clone(),
        source_tree_fingerprint: source_tree_fingerprints[0].clone(),
    })
}

pub(super) fn write_benchmark_bundle_manifest(
    manifest: &BenchmarkBundleManifest,
    path: &std::path::Path,
) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(manifest)?;
    std::fs::write(path, format!("{json}\n"))?;
    Ok(())
}

pub(super) fn load_benchmark_bundle_manifest(
    path: &std::path::Path,
) -> anyhow::Result<BenchmarkBundleManifest> {
    let bytes = read_report_bounded(path).map_err(|error| {
        anyhow::anyhow!(
            "failed to load benchmark bundle manifest `{}`: {error}. Fix: pass the bundle-manifest.json produced by validate-benchmark-bundle --manifest-output.",
            path.display()
        )
    })?;
    let manifest: BenchmarkBundleManifest = serde_json::from_slice(&bytes).map_err(|error| {
        anyhow::anyhow!(
            "benchmark bundle manifest `{}` is invalid JSON: {error}. Fix: regenerate it with validate-benchmark-bundle --manifest-output.",
            path.display()
        )
    })?;
    validate_benchmark_bundle_manifest_integrity(&manifest, &path.display().to_string())?;
    Ok(manifest)
}

pub(super) fn validate_benchmark_bundle_manifest_integrity(
    manifest: &BenchmarkBundleManifest,
    label: &str,
) -> anyhow::Result<()> {
    if manifest.schema != BENCHMARK_BUNDLE_SCHEMA {
        anyhow::bail!(
            "benchmark bundle manifest `{label}` schema `{}` is not `{BENCHMARK_BUNDLE_SCHEMA}`. Fix: regenerate the manifest with current vyre-bench.",
            manifest.schema
        );
    }
    validate_benchmark_bundle_provenance_shape(&manifest.provenance, label)?;
    if manifest.artifact_count != manifest.artifacts.len() {
        anyhow::bail!(
            "benchmark bundle manifest `{label}` artifact_count={} contradicts artifacts.len()={}. Fix: regenerate the manifest from the benchmark bundle directory.",
            manifest.artifact_count,
            manifest.artifacts.len()
        );
    }
    for artifact in &manifest.artifacts {
        if artifact.path.is_empty()
            || artifact.path.starts_with('/')
            || artifact.path.contains("..")
            || artifact.path.contains('\\')
        {
            anyhow::bail!(
                "benchmark bundle manifest `{label}` has invalid relative artifact path `{}`. Fix: regenerate the manifest from one benchmark output directory.",
                artifact.path
            );
        }
        if artifact.kind.is_empty() {
            anyhow::bail!(
                "benchmark bundle manifest `{label}` artifact `{}` has an empty kind. Fix: regenerate the manifest with current vyre-bench.",
                artifact.path
            );
        }
        if !is_hex_64(&artifact.blake3) {
            anyhow::bail!(
                "benchmark bundle manifest `{label}` artifact `{}` has invalid blake3 `{}`. Fix: regenerate the manifest from the benchmark artifacts.",
                artifact.path,
                artifact.blake3
            );
        }
    }
    validate_benchmark_bundle_manifest_artifact_set(manifest, label)?;
    let normalized =
        build_benchmark_bundle_manifest(manifest.artifacts.clone(), manifest.provenance.clone())?;
    if normalized.artifact_count != manifest.artifact_count {
        anyhow::bail!(
            "benchmark bundle manifest `{label}` normalized artifact_count={} contradicts recorded artifact_count={}. Fix: regenerate the manifest from the benchmark bundle directory.",
            normalized.artifact_count,
            manifest.artifact_count
        );
    }
    if normalized.bundle_blake3 != manifest.bundle_blake3 {
        anyhow::bail!(
            "benchmark bundle manifest `{label}` bundle_blake3 `{}` does not match normalized artifact metadata hash `{}`. Fix: regenerate the manifest from the benchmark bundle directory.",
            manifest.bundle_blake3,
            normalized.bundle_blake3
        );
    }
    Ok(())
}

pub(super) fn validate_benchmark_bundle_manifest_artifact_set(
    manifest: &BenchmarkBundleManifest,
    label: &str,
) -> anyhow::Result<()> {
    let mut observed = BTreeMap::<(String, String), usize>::new();
    for artifact in &manifest.artifacts {
        *observed
            .entry((artifact.path.clone(), artifact.kind.clone()))
            .or_default() += 1;
    }
    for ((path, kind), count) in &observed {
        if *count != 1 {
            anyhow::bail!(
                "benchmark bundle manifest `{label}` repeats artifact path `{path}` kind `{kind}` {count} times. Fix: regenerate the manifest from the benchmark bundle directory."
            );
        }
        if !BENCHMARK_BUNDLE_REQUIRED_ARTIFACTS
            .iter()
            .any(|(expected_path, expected_kind)| expected_path == path && expected_kind == kind)
        {
            anyhow::bail!(
                "benchmark bundle manifest `{label}` has unexpected artifact path `{path}` kind `{kind}`. Fix: regenerate the manifest with current vyre-bench."
            );
        }
    }
    for (path, kind) in BENCHMARK_BUNDLE_REQUIRED_ARTIFACTS {
        if !observed.contains_key(&((*path).to_string(), (*kind).to_string())) {
            anyhow::bail!(
                "benchmark bundle manifest `{label}` is missing required artifact path `{path}` kind `{kind}`. Fix: regenerate the manifest from the benchmark bundle directory."
            );
        }
    }
    Ok(())
}

pub(super) fn validate_comparison_text_evidence(
    comparison_text: &str,
    comparison: &ComparisonArtifact,
    baseline_backend: &str,
    candidate_backend: &str,
) -> anyhow::Result<()> {
    for required in [
        format!("baseline_backend={baseline_backend}"),
        format!("candidate_backend={candidate_backend}"),
        format!("baseline_profile_backend={baseline_backend}"),
        format!("candidate_profile_backend={candidate_backend}"),
        "baseline_timing_quality=".to_string(),
        "candidate_timing_quality=".to_string(),
        "compare_exit_code=".to_string(),
        MAC_BENCHMARK_BUNDLE_CASE_ID.to_string(),
    ] {
        if !comparison_text.contains(&required) {
            anyhow::bail!(
                "comparison text artifact lacks `{required}`. Fix: regenerate the text comparison with current vyre-bench compare output."
            );
        }
    }
    validate_comparison_text_exit_code(comparison_text, comparison)
}

pub(super) fn validate_comparison_text_exit_code(
    comparison_text: &str,
    comparison: &ComparisonArtifact,
) -> anyhow::Result<()> {
    let exit_code = comparison_text
        .lines()
        .find_map(|line| line.strip_prefix("compare_exit_code="))
        .ok_or_else(|| {
            anyhow::anyhow!(
                "comparison text artifact lacks `compare_exit_code=`. Fix: regenerate the comparison text with scripts/check_metal_macbook.sh benchmark."
            )
        })?;
    let exit_code: i32 = exit_code.parse().map_err(|error| {
        anyhow::anyhow!(
            "comparison text artifact has invalid compare_exit_code `{exit_code}`: {error}. Fix: regenerate the comparison text with scripts/check_metal_macbook.sh benchmark."
        )
    })?;
    if exit_code < 0 {
        anyhow::bail!(
            "comparison text artifact has negative compare_exit_code={exit_code}. Fix: regenerate the comparison text with scripts/check_metal_macbook.sh benchmark."
        );
    }
    if comparison.regressed && exit_code == 0 {
        anyhow::bail!(
            "comparison text compare_exit_code=0 contradicts structured comparison regressed=true. Fix: rerun vyre-bench compare and capture its exit code."
        );
    }
    if !comparison.regressed && exit_code != 0 {
        anyhow::bail!(
            "comparison text compare_exit_code={exit_code} contradicts structured comparison regressed=false. Fix: rerun vyre-bench compare and capture its exit code."
        );
    }
    Ok(())
}

pub(super) fn validate_comparison_matches_bundle_reports(
    comparison: &ComparisonArtifact,
    reports: &[ReportSchema],
) -> anyhow::Result<()> {
    let baseline_report =
        report_for_profile_backend(reports, &comparison.baseline.profile_backend)?;
    let candidate_report =
        report_for_profile_backend(reports, &comparison.candidate.profile_backend)?;
    let expected = build_comparison_artifact(baseline_report, candidate_report)?;
    if expected.schema != comparison.schema {
        anyhow::bail!(
            "comparison artifact schema `{}` does not match recomputed schema `{}`. Fix: rerun vyre-bench compare against the reports in the same benchmark bundle directory.",
            comparison.schema,
            expected.schema
        );
    }
    if expected.baseline != comparison.baseline {
        anyhow::bail!(
            "comparison artifact does not match bundled baseline `{}` report. Fix: rerun vyre-bench compare against the reports in the same benchmark bundle directory.",
            comparison.baseline.profile_backend
        );
    }
    if expected.candidate != comparison.candidate {
        anyhow::bail!(
            "comparison artifact does not match bundled candidate `{}` report. Fix: rerun vyre-bench compare against the reports in the same benchmark bundle directory.",
            comparison.candidate.profile_backend
        );
    }
    if expected.regressed != comparison.regressed {
        anyhow::bail!(
            "comparison artifact regressed={} does not match recomputed regressed={}. Fix: rerun vyre-bench compare against the reports in the same benchmark bundle directory.",
            comparison.regressed,
            expected.regressed
        );
    }
    if expected.cases.len() != comparison.cases.len() {
        anyhow::bail!(
            "comparison artifact case count {} does not match recomputed case count {}. Fix: rerun vyre-bench compare against the reports in the same benchmark bundle directory.",
            comparison.cases.len(),
            expected.cases.len()
        );
    }
    for (actual, expected) in comparison.cases.iter().zip(expected.cases.iter()) {
        validate_comparison_case_matches(actual, expected)?;
    }
    Ok(())
}

pub(super) fn validate_comparison_case_matches(
    actual: &ComparisonCase,
    expected: &ComparisonCase,
) -> anyhow::Result<()> {
    if actual.id != expected.id
        || actual.baseline_p50_ns != expected.baseline_p50_ns
        || actual.candidate_p50_ns != expected.candidate_p50_ns
        || actual.verdict != expected.verdict
        || actual.regressed != expected.regressed
    {
        anyhow::bail!(
            "comparison artifact case `{}` does not match recomputed case evidence. Fix: rerun vyre-bench compare against the reports in the same benchmark bundle directory.",
            actual.id
        );
    }
    for (label, actual_value, expected_value) in [
        (
            "baseline_mean_ns",
            Some(actual.baseline_mean_ns),
            Some(expected.baseline_mean_ns),
        ),
        (
            "candidate_mean_ns",
            Some(actual.candidate_mean_ns),
            Some(expected.candidate_mean_ns),
        ),
        (
            "delta_fraction",
            actual.delta_fraction,
            expected.delta_fraction,
        ),
        (
            "delta_percent",
            actual.delta_percent,
            expected.delta_percent,
        ),
        ("p_value", actual.p_value, expected.p_value),
    ] {
        if !float_option_close(actual_value, expected_value) {
            anyhow::bail!(
                "comparison artifact case `{}` field `{label}` does not match recomputed floating evidence. Fix: rerun vyre-bench compare against the reports in the same benchmark bundle directory.",
                actual.id
            );
        }
    }
    Ok(())
}

pub(super) fn float_option_close(actual: Option<f64>, expected: Option<f64>) -> bool {
    match (actual, expected) {
        (None, None) => true,
        (Some(actual), Some(expected)) => {
            if actual == expected {
                true
            } else {
                let scale = actual.abs().max(expected.abs()).max(1.0);
                (actual - expected).abs() <= scale * 1.0e-9
            }
        }
        _ => false,
    }
}

pub(super) fn report_for_profile_backend<'a>(
    reports: &'a [ReportSchema],
    backend: &str,
) -> anyhow::Result<&'a ReportSchema> {
    reports
        .iter()
        .find(|report| {
            report
                .backend_profile
                .as_ref()
                .is_some_and(|profile| profile.backend == backend)
        })
        .ok_or_else(|| {
            anyhow::anyhow!(
                "comparison references backend `{backend}` but the benchmark bundle has no matching backend report. Fix: rerun the benchmark gate so the comparison and reports come from one bundle."
            )
        })
}

pub(super) fn validate_benchmark_bundle_provenance_shape(
    provenance: &BenchmarkBundleProvenance,
    label: &str,
) -> anyhow::Result<()> {
    if provenance.validator != "vyre-bench validate-benchmark-bundle" {
        anyhow::bail!(
            "benchmark bundle manifest `{label}` validator `{}` is not `vyre-bench validate-benchmark-bundle`. Fix: regenerate the manifest with current vyre-bench.",
            provenance.validator
        );
    }
    if provenance.validator_version != env!("CARGO_PKG_VERSION") {
        anyhow::bail!(
            "benchmark bundle manifest `{label}` validator_version `{}` does not match current vyre-bench `{}`. Fix: regenerate the manifest with the same validator binary used for replay.",
            provenance.validator_version,
            env!("CARGO_PKG_VERSION")
        );
    }
    for (field, value) in [
        ("suite", provenance.suite.as_str()),
        ("case_id", provenance.case_id.as_str()),
        ("baseline_backend", provenance.baseline_backend.as_str()),
        ("candidate_backend", provenance.candidate_backend.as_str()),
        ("source_fingerprint", provenance.source_fingerprint.as_str()),
        (
            "source_tree_fingerprint",
            provenance.source_tree_fingerprint.as_str(),
        ),
    ] {
        if value.is_empty() {
            anyhow::bail!(
                "benchmark bundle manifest `{label}` provenance field `{field}` is empty. Fix: regenerate the manifest from validated benchmark reports."
            );
        }
    }
    if provenance.report_backends.is_empty() {
        anyhow::bail!(
            "benchmark bundle manifest `{label}` provenance has no report_backends. Fix: regenerate the manifest from benchmark reports."
        );
    }
    if provenance.comparison_pairs.is_empty() {
        anyhow::bail!(
            "benchmark bundle manifest `{label}` provenance has no comparison_pairs. Fix: regenerate the manifest from comparison artifacts."
        );
    }
    let mut sorted = provenance.report_backends.clone();
    sorted.sort();
    if sorted != provenance.report_backends {
        anyhow::bail!(
            "benchmark bundle manifest `{label}` report_backends are not sorted. Fix: regenerate the manifest with current vyre-bench."
        );
    }
    if provenance
        .report_backends
        .iter()
        .any(|backend| backend.is_empty())
    {
        anyhow::bail!(
            "benchmark bundle manifest `{label}` report_backends contains an empty backend id. Fix: regenerate the manifest from validated benchmark reports."
        );
    }
    let mut sorted_pairs = provenance.comparison_pairs.clone();
    sorted_pairs.sort();
    if sorted_pairs != provenance.comparison_pairs {
        anyhow::bail!(
            "benchmark bundle manifest `{label}` comparison_pairs are not sorted. Fix: regenerate the manifest with current vyre-bench."
        );
    }
    if provenance
        .comparison_pairs
        .iter()
        .any(|pair| !pair.contains("->"))
    {
        anyhow::bail!(
            "benchmark bundle manifest `{label}` comparison_pairs contains an invalid pair. Fix: regenerate the manifest from comparison artifacts."
        );
    }
    Ok(())
}

pub(super) fn validate_benchmark_bundle_manifest_matches(
    expected: &BenchmarkBundleManifest,
    observed: &BenchmarkBundleManifest,
    label: &str,
) -> anyhow::Result<()> {
    validate_benchmark_bundle_manifest_integrity(observed, "fresh benchmark bundle")?;
    if expected.bundle_blake3 != observed.bundle_blake3 {
        anyhow::bail!(
            "benchmark bundle manifest `{label}` bundle_blake3 `{}` does not match current artifacts `{}`. Fix: rerun the benchmark gate or investigate artifact drift.",
            expected.bundle_blake3,
            observed.bundle_blake3
        );
    }
    let expected_json = serde_json::to_value(expected)?;
    let observed_json = serde_json::to_value(observed)?;
    if expected_json != observed_json {
        anyhow::bail!(
            "benchmark bundle manifest `{label}` metadata does not match current artifacts. Fix: rerun validate-benchmark-bundle --manifest-output after checking for artifact drift."
        );
    }
    Ok(())
}

pub(super) fn is_hex_64(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}
