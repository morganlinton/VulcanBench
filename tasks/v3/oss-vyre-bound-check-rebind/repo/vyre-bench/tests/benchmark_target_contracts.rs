//! BENCH_TARGETS release benchmark contracts.
//!
//! VX-005 requires canonical target rows to be the source for release
//! benchmark case ids, baseline class, metric, and timing-quality
//! requirements. These tests keep that contract executable instead of relying
//! on release-matrix conventions alone.

use std::collections::{BTreeMap, BTreeSet};

use vyre_bench::api::suite::SuiteKind;

const BENCH_TARGETS: &str = include_str!("../../docs/optimization/BENCH_TARGETS.toml");

#[derive(Debug, Clone)]
struct ReleaseTargetRow {
    id: String,
    bench_case_id: String,
    backend_focus: String,
    metric: String,
    timing_quality: String,
    shape_skew: String,
    fact_width: String,
    frontier_density: String,
    transfer_pressure: String,
    primitive: String,
    cpu_baseline: String,
    min_speedup_over_cpu_sota: f64,
    dataset_id: String,
    metric_schema: String,
    threshold_source: String,
    hardware_digest_source: String,
}

fn benchmark_target_contracts_validate_release_targets(
    manifest: &toml::Value,
    expected_release_case_ids: &BTreeSet<String>,
    active_release_case_ids: &BTreeSet<String>,
) -> Result<Vec<ReleaseTargetRow>, Vec<String>> {
    let Some(targets) = manifest.get("target").and_then(toml::Value::as_array) else {
        return Err(vec![
            "Fix: BENCH_TARGETS.toml must contain [[target]] rows.".to_string(),
        ]);
    };
    let baseline_classes = manifest
        .get("baseline_class_values")
        .and_then(toml::Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(toml::Value::as_str)
                .collect::<BTreeSet<_>>()
        })
        .unwrap_or_default();
    let mut failures = Vec::new();
    if !baseline_classes.contains("cpu_sota") {
        failures.push(
            "Fix: BENCH_TARGETS.toml baseline_class_values must include cpu_sota.".to_string(),
        );
    }
    let identity = manifest.get("release_workload_identity");
    let dataset_id_source = identity
        .and_then(|identity| identity.get("dataset_id_source"))
        .and_then(toml::Value::as_str)
        .unwrap_or("");
    if dataset_id_source != "bench_case_id" {
        failures.push(
            "Fix: BENCH_TARGETS.toml [release_workload_identity].dataset_id_source must be `bench_case_id`."
                .to_string(),
        );
    }
    let metric_schema = identity
        .and_then(|identity| identity.get("metric_schema"))
        .and_then(toml::Value::as_str)
        .unwrap_or("")
        .to_string();
    if metric_schema != "bench-target-release-workload:v1" {
        failures.push(
            "Fix: BENCH_TARGETS.toml [release_workload_identity].metric_schema must be `bench-target-release-workload:v1`."
                .to_string(),
        );
    }
    let threshold_source = identity
        .and_then(|identity| identity.get("threshold_source"))
        .and_then(toml::Value::as_str)
        .unwrap_or("")
        .to_string();
    if threshold_source != "docs/optimization/BENCH_TARGETS.toml" {
        failures.push(
            "Fix: BENCH_TARGETS.toml [release_workload_identity].threshold_source must be `docs/optimization/BENCH_TARGETS.toml`."
                .to_string(),
        );
    }
    let hardware_digest_source = identity
        .and_then(|identity| identity.get("hardware_digest_source"))
        .and_then(toml::Value::as_str)
        .unwrap_or("")
        .to_string();
    if hardware_digest_source
        != "release/evidence/benchmarks/cuda-release-suite.json#hardware_digest"
    {
        failures.push(
            "Fix: BENCH_TARGETS.toml [release_workload_identity].hardware_digest_source must point at cuda-release-suite.json#hardware_digest."
                .to_string(),
        );
    }
    let mut ids = BTreeSet::new();
    let mut by_case = BTreeMap::<String, Vec<String>>::new();
    let mut rows = Vec::new();
    for target in targets.iter().filter(|target| {
        target.get("suite").and_then(toml::Value::as_str) == Some("release-workload")
    }) {
        let id = required_string(target, "id", &mut failures);
        let bench_case_id = required_string(target, "bench_case_id", &mut failures);
        let backend_focus = required_string(target, "backend_focus", &mut failures);
        let metric = required_string(target, "metric", &mut failures);
        let timing_quality = required_string(target, "timing_quality", &mut failures);
        let shape_skew = required_string(target, "shape_skew", &mut failures);
        let fact_width = required_string(target, "fact_width", &mut failures);
        let frontier_density = required_string(target, "frontier_density", &mut failures);
        let transfer_pressure = required_string(target, "transfer_pressure", &mut failures);
        let primitive = required_string(target, "primitive", &mut failures);
        let cpu_baseline = required_string(target, "cpu_baseline", &mut failures);
        let dataset_id = bench_case_id.clone();
        let min_speedup_over_cpu_sota = target
            .get("min_speedup_over_cpu_sota")
            .and_then(toml::Value::as_float)
            .or_else(|| {
                target
                    .get("min_speedup_over_cpu_sota")
                    .and_then(toml::Value::as_integer)
                    .map(|value| value as f64)
            })
            .unwrap_or_else(|| {
                failures.push(format!(
                    "Fix: release-workload BENCH_TARGETS row `{id}` must declare numeric min_speedup_over_cpu_sota."
                ));
                0.0
            });
        if !id.is_empty() && !ids.insert(id.clone()) {
            failures.push(format!(
                "Fix: duplicate release-workload BENCH_TARGETS target id `{id}`."
            ));
        }
        if !bench_case_id.is_empty() {
            by_case
                .entry(bench_case_id.clone())
                .or_default()
                .push(id.clone());
            if !active_release_case_ids.contains(&bench_case_id) {
                failures.push(format!(
                    "Fix: BENCH_TARGETS target `{id}` references stale or inactive release benchmark case `{bench_case_id}`."
                ));
            }
        }
        if !matches!(
            backend_focus.as_str(),
            "cuda" | "wgpu" | "metal" | "spirv" | "reference" | "all"
        ) {
            failures.push(format!(
                "Fix: BENCH_TARGETS target `{id}` has unsupported backend_focus `{backend_focus}`."
            ));
        }
        if !metric.ends_with("_p50")
            && !metric.ends_with("_delta_p50")
            && !matches!(
                metric.as_str(),
                "bytes_per_second_p50" | "programs_per_second_p50"
            )
        {
            failures.push(format!(
                "Fix: BENCH_TARGETS target `{id}` metric `{metric}` must name a p50 release metric."
            ));
        }
        if !matches!(
            timing_quality.as_str(),
            "host_enqueue_wait" | "device_timestamps" | "hardware_counters"
        ) {
            failures.push(format!(
                "Fix: BENCH_TARGETS target `{id}` timing_quality `{timing_quality}` must be host_enqueue_wait, device_timestamps, or hardware_counters."
            ));
        }
        if min_speedup_over_cpu_sota <= 0.0 {
            failures.push(format!(
                "Fix: BENCH_TARGETS target `{id}` min_speedup_over_cpu_sota must be positive."
            ));
        }
        if !shape_skew.is_empty()
            && !shape_skew
                .chars()
                .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '_')
        {
            failures.push(format!(
                "Fix: BENCH_TARGETS target `{id}` shape_skew `{shape_skew}` must be lowercase snake-case data."
            ));
        }
        if !fact_width.is_empty()
            && !fact_width
                .chars()
                .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '_')
        {
            failures.push(format!(
                "Fix: BENCH_TARGETS target `{id}` fact_width `{fact_width}` must be lowercase snake-case data."
            ));
        }
        if !frontier_density.is_empty()
            && !frontier_density
                .chars()
                .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '_')
        {
            failures.push(format!(
                "Fix: BENCH_TARGETS target `{id}` frontier_density `{frontier_density}` must be lowercase snake-case data."
            ));
        }
        if !transfer_pressure.is_empty()
            && !transfer_pressure
                .chars()
                .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '_')
        {
            failures.push(format!(
                "Fix: BENCH_TARGETS target `{id}` transfer_pressure `{transfer_pressure}` must be lowercase snake-case data."
            ));
        }
        if !primitive.is_empty() && primitive == cpu_baseline {
            failures.push(format!(
                "Fix: BENCH_TARGETS target `{id}` primitive and cpu_baseline must describe different concepts."
            ));
        }
        rows.push(ReleaseTargetRow {
            id,
            bench_case_id,
            backend_focus,
            metric,
            timing_quality,
            shape_skew,
            fact_width,
            frontier_density,
            transfer_pressure,
            primitive,
            cpu_baseline,
            min_speedup_over_cpu_sota,
            dataset_id,
            metric_schema: metric_schema.clone(),
            threshold_source: threshold_source.clone(),
            hardware_digest_source: hardware_digest_source.clone(),
        });
    }
    if rows.is_empty() {
        failures.push("Fix: BENCH_TARGETS.toml has no release-workload target rows.".to_string());
    }
    for expected in expected_release_case_ids {
        if !by_case.contains_key(expected) {
            failures.push(format!(
                "Fix: active release benchmark case `{expected}` has no canonical release-workload BENCH_TARGETS target."
            ));
        }
    }
    if failures.is_empty() {
        Ok(rows)
    } else {
        Err(failures)
    }
}

fn required_string(target: &toml::Value, key: &'static str, failures: &mut Vec<String>) -> String {
    let value = target
        .get(key)
        .and_then(toml::Value::as_str)
        .unwrap_or("")
        .trim()
        .to_string();
    if value.is_empty() {
        let id = target
            .get("id")
            .and_then(toml::Value::as_str)
            .unwrap_or("<missing id>");
        failures.push(format!(
            "Fix: release-workload BENCH_TARGETS row `{id}` must declare non-empty `{key}`."
        ));
    }
    value
}

#[test]
fn benchmark_target_contracts_cover_release_workload_case_ids() {
    let manifest = toml::from_str::<toml::Value>(BENCH_TARGETS)
        .expect("Fix: BENCH_TARGETS.toml must parse as TOML.");
    let registry = vyre_bench::registry::collect_all();
    let active_release_case_ids = registry
        .iter()
        .filter(|case| case.active_in_suite(SuiteKind::Release))
        .map(|case| case.id().0)
        .collect::<BTreeSet<_>>();
    let matrix = vyre_bench::release_matrix::build_release_matrix(&registry);
    let expected_release_case_ids = matrix
        .families
        .iter()
        .filter_map(|family| family.benchmark_command.as_deref())
        .filter_map(case_id_from_command)
        .collect::<BTreeSet<_>>();
    assert!(
        !expected_release_case_ids.is_empty(),
        "Fix: release matrix must expose canonical release benchmark case ids."
    );
    let rows = benchmark_target_contracts_validate_release_targets(
        &manifest,
        &expected_release_case_ids,
        &active_release_case_ids,
    )
    .unwrap_or_else(|failures| panic!("{}", failures.join("\n")));
    assert!(
        rows.len() >= expected_release_case_ids.len(),
        "Fix: release BENCH_TARGETS rows ({}) must cover every canonical release benchmark case id ({}).",
        rows.len(),
        expected_release_case_ids.len()
    );
    for row in rows {
        assert!(
            !row.id.is_empty()
                && !row.bench_case_id.is_empty()
                && !row.backend_focus.is_empty()
                && !row.metric.is_empty()
                && !row.timing_quality.is_empty()
                && !row.shape_skew.is_empty()
                && !row.fact_width.is_empty()
                && !row.frontier_density.is_empty()
                && !row.transfer_pressure.is_empty()
                && !row.primitive.is_empty()
                && !row.cpu_baseline.is_empty()
                && row.dataset_id == row.bench_case_id
                && row.metric_schema == "bench-target-release-workload:v1"
                && row.threshold_source == "docs/optimization/BENCH_TARGETS.toml"
                && row.hardware_digest_source
                    == "release/evidence/benchmarks/cuda-release-suite.json#hardware_digest"
                && row.min_speedup_over_cpu_sota > 0.0,
            "Fix: release target row must retain non-empty canonical metadata: {:?}",
            row
        );
    }
}

fn case_id_from_command(command: &str) -> Option<String> {
    let mut parts = command.split_whitespace();
    while let Some(part) = parts.next() {
        if part == "--case" {
            return parts.next().map(ToString::to_string);
        }
    }
    None
}

#[test]
fn benchmark_target_contracts_negative_fixtures_reject_missing_target_and_stale_case() {
    let active = BTreeSet::from(["release.fixture.1m".to_string()]);
    let expected = active.clone();
    let missing_manifest = toml::from_str::<toml::Value>(
        r#"
schema = 1
baseline_class_values = ["cpu_sota"]

[release_workload_identity]
dataset_id_source = "bench_case_id"
metric_schema = "bench-target-release-workload:v1"
threshold_source = "docs/optimization/BENCH_TARGETS.toml"
hardware_digest_source = "release/evidence/benchmarks/cuda-release-suite.json#hardware_digest"

[[target]]
id = "release.workload.other"
bench_case_id = "release.other.1m"
primitive = "other primitive"
suite = "release-workload"
backend_focus = "cuda"
metric = "items_per_second_p50"
timing_quality = "device_timestamps"
min_speedup_over_cpu_sota = 10.0
cpu_baseline = "other CPU baseline"
gpu_competitor = "none"
"#,
    )
    .expect("fixture TOML parses");
    let missing =
        benchmark_target_contracts_validate_release_targets(&missing_manifest, &expected, &active)
            .expect_err("Fix: missing target fixture must fail");
    assert!(
        missing
            .iter()
            .any(|failure| failure.contains("has no canonical release-workload")),
        "Fix: missing target fixture produced weak failures: {missing:?}"
    );

    let stale_manifest = toml::from_str::<toml::Value>(
        r#"
schema = 1
baseline_class_values = ["cpu_sota"]

[release_workload_identity]
dataset_id_source = "bench_case_id"
metric_schema = "bench-target-release-workload:v1"
threshold_source = "docs/optimization/BENCH_TARGETS.toml"
hardware_digest_source = "release/evidence/benchmarks/cuda-release-suite.json#hardware_digest"

[[target]]
id = "release.workload.fixture"
bench_case_id = "release.stale.1m"
primitive = "fixture primitive"
suite = "release-workload"
backend_focus = "cuda"
metric = "items_per_second_p50"
timing_quality = "device_timestamps"
min_speedup_over_cpu_sota = 10.0
cpu_baseline = "fixture CPU baseline"
gpu_competitor = "none"
"#,
    )
    .expect("fixture TOML parses");
    let stale =
        benchmark_target_contracts_validate_release_targets(&stale_manifest, &expected, &active)
            .expect_err("Fix: stale target fixture must fail");
    assert!(
        stale
            .iter()
            .any(|failure| failure.contains("stale or inactive release benchmark case")),
        "Fix: stale target fixture produced weak failures: {stale:?}"
    );
}
