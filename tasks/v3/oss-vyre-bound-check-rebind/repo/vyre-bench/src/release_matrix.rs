//! Release workload matrix for the Vyre release plan.
//!
//! The release plan requires at least twelve proof workload families and at
//! at least ten formerly CPU-only workload families with 100x targets where the
//! workload exposes enough parallelism. This module makes those
//! requirements auditable from the benchmark registry.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use serde::Serialize;

use crate::api::case::{BaselineClass, BenchCase, PerformanceContract};
use crate::api::suite::SuiteKind;
use crate::registry::BenchRegistry;
use crate::report::json::{
    REQUIRED_BENCHMARK_CASE_FIELDS, REQUIRED_BENCHMARK_METRIC_FIELDS,
};

const REQUIRED_CLOSED_FAMILIES: usize = 12;
const MIN_CPU_SOTA_100X_FAMILIES: usize = 10;
const BENCH_TARGETS: &str = include_str!("../../docs/optimization/BENCH_TARGETS.toml");

#[derive(Debug, Serialize)]
pub struct ReleaseWorkloadMatrix {
    pub schema_version: u32,
    pub benchmark_evidence_schema_version: u32,
    pub required_benchmark_case_fields: Vec<&'static str>,
    pub required_benchmark_metric_fields: Vec<&'static str>,
    pub required_closed_families: usize,
    pub required_cpu_sota_100x_families: Vec<&'static str>,
    pub missing_required_cpu_sota_100x_families: Vec<&'static str>,
    pub matched_required_families: usize,
    pub release_suite_case_count: usize,
    pub cpu_sota_contract_count: usize,
    pub cpu_sota_100x_contract_count: usize,
    pub cpu_sota_100x_contract_cases: Vec<String>,
    pub cpu_sota_100x_family_count: usize,
    pub cpu_sota_100x_families: Vec<&'static str>,
    pub families: Vec<ReleaseWorkloadFamilyReport>,
    pub blockers: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct ReleaseWorkloadFamilyReport {
    pub id: &'static str,
    pub title: &'static str,
    pub release_plan_workload: u8,
    pub required: bool,
    pub dispatch_policy: &'static str,
    pub non_megakernel_justification: Option<&'static str>,
    pub matched_cases: Vec<String>,
    pub evidence_artifact: String,
    pub benchmark_command: Option<String>,
    pub bench_target_ids: Vec<&'static str>,
    pub cpu_sota_contracts: Vec<String>,
    pub cpu_sota_100x_cases: Vec<String>,
    pub cpu_sota_baseline_names: Vec<String>,
    pub cpu_sota_baseline_crates: Vec<String>,
    pub cpu_sota_backend_ids: Vec<String>,
    pub fair_cpu_sota_baseline_count: usize,
    pub reproducible_cuda_command: bool,
    pub max_cpu_sota_min_speedup_x: Option<f64>,
}

struct ReleaseWorkloadFamily {
    id: &'static str,
    title: &'static str,
    release_plan_workload: u8,
    required: bool,
    any_terms: &'static [&'static str],
    all_terms: &'static [&'static str],
    bench_target_id: &'static str,
    dispatch_policy: &'static str,
    non_megakernel_justification: Option<&'static str>,
}

#[derive(Clone, Debug, PartialEq)]
struct ReleaseBenchTarget {
    id: String,
    bench_case_id: String,
    min_speedup_over_cpu_sota: f64,
}

const RELEASE_WORKLOADS: &[ReleaseWorkloadFamily] = &[
    ReleaseWorkloadFamily {
        id: "condition-eval",
        title: "Bytecode-compatible condition evaluation",
        release_plan_workload: 1,
        required: true,
        any_terms: &["release.condition_eval", "conditions.yara_like"],
        all_terms: &["condition"],
        bench_target_id: "release.workload.condition_eval",
        dispatch_policy: "megakernel",
        non_megakernel_justification: None,
    },
    ReleaseWorkloadFamily {
        id: "string-bitmap-scatter",
        title: "String bitmap scatter",
        release_plan_workload: 2,
        required: true,
        any_terms: &["release.string_bitmap_scatter"],
        all_terms: &["string", "bitmap"],
        bench_target_id: "release.workload.string_bitmap_scatter",
        dispatch_policy: "megakernel",
        non_megakernel_justification: None,
    },
    ReleaseWorkloadFamily {
        id: "offset-count-aggregation",
        title: "Offset/count/length aggregation",
        release_plan_workload: 3,
        required: true,
        any_terms: &["release.offset_count_aggregation"],
        all_terms: &["release.offset_count_aggregation"],
        bench_target_id: "release.workload.offset_count_aggregation",
        dispatch_policy: "megakernel",
        non_megakernel_justification: None,
    },
    ReleaseWorkloadFamily {
        id: "metadata-conditions",
        title: "PE/header/file metadata condition evaluation",
        release_plan_workload: 4,
        required: true,
        any_terms: &["metadata.condition"],
        all_terms: &["metadata.condition"],
        bench_target_id: "release.workload.pe_metadata",
        dispatch_policy: "megakernel",
        non_megakernel_justification: None,
    },
    ReleaseWorkloadFamily {
        id: "entropy-window",
        title: "Entropy/window predicates",
        release_plan_workload: 5,
        required: true,
        any_terms: &["release.entropy_window"],
        all_terms: &["release.entropy_window"],
        bench_target_id: "release.workload.entropy_window",
        dispatch_policy: "megakernel",
        non_megakernel_justification: None,
    },
    ReleaseWorkloadFamily {
        id: "quantified-condition-loops",
        title: "Bounded quantified condition loops",
        release_plan_workload: 6,
        required: true,
        any_terms: &["release.quantified_condition_loops"],
        all_terms: &["quantifier", "condition"],
        bench_target_id: "release.workload.for_any_all_n",
        dispatch_policy: "megakernel",
        non_megakernel_justification: None,
    },
    ReleaseWorkloadFamily {
        id: "alias-reaching-def",
        title: "Alias-aware reaching-definition predicates",
        release_plan_workload: 7,
        required: true,
        any_terms: &["release.alias_reaching_def", "dataflow.reaching_def.bitset"],
        all_terms: &["alias"],
        bench_target_id: "release.workload.alias_reaching_def",
        dispatch_policy: "specialized-dataflow-kernel",
        non_megakernel_justification: Some(
            "architectural: alias-aware reaching-definition workloads use sparse relation kernels with fixpoint convergence rather than independent condition-slot dispatch",
        ),
    },
    ReleaseWorkloadFamily {
        id: "ifds-witness",
        title: "IFDS reachability and witness predicates",
        release_plan_workload: 8,
        required: true,
        any_terms: &["release.ifds_witness", "dataflow.ifds"],
        all_terms: &["ifds", "witness"],
        bench_target_id: "release.workload.ifds_witness",
        dispatch_policy: "specialized-dataflow-kernel",
        non_megakernel_justification: Some(
            "architectural: IFDS witness extraction uses frontier/fact-table scheduling and predecessor reconstruction that need dataflow-specific kernels",
        ),
    },
    ReleaseWorkloadFamily {
        id: "c-ast-traversal",
        title: "C AST traversal and motif predicates",
        release_plan_workload: 9,
        required: true,
        any_terms: &["release.c_ast_traversal"],
        all_terms: &["release.c_ast_traversal"],
        bench_target_id: "release.workload.c_ast_traversal",
        dispatch_policy: "specialized-parser-kernel",
        non_megakernel_justification: Some(
            "architectural: C AST traversal consumes parser-owned AST buffers with table/stream access patterns that remain outside the condition megakernel for this release",
        ),
    },
    ReleaseWorkloadFamily {
        id: "megakernel-queued-batches",
        title: "Persistent megakernel queued condition batches",
        release_plan_workload: 10,
        required: true,
        any_terms: &["release.megakernel_queue", "runtime.megakernel.condition"],
        all_terms: &["megakernel", "queue"],
        bench_target_id: "release.workload.megakernel_stream",
        dispatch_policy: "megakernel",
        non_megakernel_justification: None,
    },
    ReleaseWorkloadFamily {
        id: "egraph-saturation",
        title: "E-graph rewrite saturation and optimization impact",
        release_plan_workload: 11,
        required: true,
        any_terms: &["release.egraph_saturation", "egraph", "egglog", "lower.rewrites", "optimizer.impact"],
        all_terms: &[],
        bench_target_id: "release.workload.egraph_saturation",
        dispatch_policy: "bounded-saturation-kernel",
        non_megakernel_justification: Some(
            "architectural: e-graph saturation is a bounded rewrite worklist with fuel and equivalence-class state, so it uses saturation-specific kernels",
        ),
    },
    ReleaseWorkloadFamily {
        id: "sparse-output-compaction",
        title: "Sparse fired-rule readback and output compaction",
        release_plan_workload: 12,
        required: true,
        any_terms: &["sparse.compaction", "sparse_output"],
        all_terms: &["sparse"],
        bench_target_id: "release.workload.conformance_sparse_readback",
        dispatch_policy: "megakernel",
        non_megakernel_justification: None,
    },
    ReleaseWorkloadFamily {
        id: "callgraph-reachability",
        title: "Graph traversal and callgraph reachability",
        release_plan_workload: 13,
        required: true,
        any_terms: &["callgraph.reachability", "graph.reachability"],
        all_terms: &["reachability"],
        bench_target_id: "release.workload.callgraph_reachability",
        dispatch_policy: "specialized-graph-kernel",
        non_megakernel_justification: Some(
            "architectural: callgraph reachability is frontier graph traversal with convergence state, not independent rule-condition slot evaluation",
        ),
    },
    ReleaseWorkloadFamily {
        id: "compound-fused-filter",
        title: "Compound resident literal/dataflow/score filtering",
        release_plan_workload: 14,
        required: false,
        any_terms: &["compound.pipeline.fused_filter", "compound"],
        all_terms: &["resident", "dataflow"],
        bench_target_id: "release.workload.compound_fused_filter",
        dispatch_policy: "resident-fused-kernel",
        non_megakernel_justification: Some(
            "architectural: compound filtering fuses independent matching, dataflow, score, and taint-class predicates into one resident pass without condition-slot queue orchestration",
        ),
    },
    ReleaseWorkloadFamily {
        id: "adaptive-routing",
        title: "GPU-resident adaptive workload routing",
        release_plan_workload: 15,
        required: false,
        any_terms: &["runtime.adaptive_routing", "adaptive-routing"],
        all_terms: &["resident", "scheduler"],
        bench_target_id: "release.workload.adaptive_routing",
        dispatch_policy: "resident-routing-kernel",
        non_megakernel_justification: Some(
            "architectural: adaptive routing is GPU-side scheduling metadata generation rather than execution of a queued rule opcode stream",
        ),
    },
    ReleaseWorkloadFamily {
        id: "quantized-linear",
        title: "Fused grouped INT4 linear inference",
        release_plan_workload: 16,
        required: false,
        any_terms: &["nn.linear_4bit_affine_grouped", "quantized"],
        all_terms: &["resident", "inference"],
        bench_target_id: "release.workload.quantized_linear",
        dispatch_policy: "resident-fused-kernel",
        non_megakernel_justification: Some(
            "architectural: grouped INT4 linear fuses packed weight decode, scale/zero-point sidecars, and accumulation in one inference kernel instead of queueing scalar condition opcodes",
        ),
    },
];

pub fn build_release_matrix(registry: &BenchRegistry) -> ReleaseWorkloadMatrix {
    let (release_targets, mut blockers) = match release_bench_targets_from_manifest(BENCH_TARGETS) {
        Ok(targets) => (targets, Vec::new()),
        Err(error) => (Vec::new(), vec![error]),
    };
    let target_by_id = release_bench_target_by_id(&release_targets);
    let release_cases: Vec<_> = registry
        .iter()
        .filter(|case| case.active_in_suite(SuiteKind::Release))
        .collect();

    let mut cpu_sota_contract_ids = BTreeSet::new();
    let mut cpu_sota_100x_contract_ids = BTreeSet::new();
    for case in &release_cases {
        if let Some(contract) = case.performance_contract() {
            for baseline in &contract.baselines {
                if matches!(baseline.class, BaselineClass::CpuSota) {
                    let id = case.id().0;
                    cpu_sota_contract_ids.insert(id.clone());
                    if baseline.min_speedup_x >= 100.0 {
                        cpu_sota_100x_contract_ids.insert(id);
                    }
                }
            }
        }
    }

    let mut families = Vec::new();
    for family in RELEASE_WORKLOADS {
        families.push(build_family_report(family, &release_cases, &target_by_id));
    }
    let required_cpu_sota_100x_families =
        required_cpu_sota_100x_family_ids(RELEASE_WORKLOADS, &target_by_id);

    let required_closed_families = families.iter().filter(|family| family.required).count();
    let matched_required_families = families
        .iter()
        .filter(|family| family.required && !family.matched_cases.is_empty())
        .count();
    let required_matched_release_cases = families
        .iter()
        .filter(|family| family.required)
        .flat_map(|family| family.matched_cases.iter().cloned())
        .collect::<BTreeSet<_>>();
    let mut cpu_sota_100x_families = families
        .iter()
        .filter(|family| family.required)
        .filter(|family| {
            family
                .max_cpu_sota_min_speedup_x
                .is_some_and(|speedup| speedup >= 100.0)
                && !family.cpu_sota_100x_cases.is_empty()
        })
        .map(|family| family.id)
        .collect::<Vec<_>>();
    cpu_sota_100x_families.sort_unstable();
    let cpu_sota_100x_family_count = cpu_sota_100x_families.len();
    let missing_required_cpu_sota_100x_families = required_cpu_sota_100x_families
        .iter()
        .copied()
        .filter(|required| {
            !cpu_sota_100x_families
                .iter()
                .any(|family| *family == *required)
        })
        .collect::<Vec<_>>();
    let cpu_sota_100x_contract_cases = cpu_sota_100x_contract_ids.iter().cloned().collect();
    if matched_required_families < REQUIRED_CLOSED_FAMILIES {
        blockers.push(format!(
            "release suite covers {matched_required_families} required workload families; needs at least {REQUIRED_CLOSED_FAMILIES}"
        ));
    }
    for family in &families {
        if family.required && family.matched_cases.is_empty() {
            blockers.push(format!(
                "release workload {} `{}` has no active release benchmark case",
                family.release_plan_workload, family.id
            ));
        }
        if family.required && family.bench_target_ids.is_empty() {
            blockers.push(format!(
                "release workload {} `{}` has no canonical BENCH_TARGETS.toml target id",
                family.release_plan_workload, family.id
            ));
        }
        if family.required
            && !family.matched_cases.is_empty()
            && family.cpu_sota_contracts.is_empty()
        {
            blockers.push(format!(
                "release workload {} `{}` has active cases but no CPU-SOTA baseline contract",
                family.release_plan_workload, family.id
            ));
        }
        if family.required
            && !family.matched_cases.is_empty()
            && family.fair_cpu_sota_baseline_count == 0
        {
            blockers.push(format!(
                "release workload {} `{}` has no fair CPU-SOTA baseline crate with CUDA backend binding",
                family.release_plan_workload, family.id
            ));
        }
        if family.required && !family.matched_cases.is_empty() && !family.reproducible_cuda_command
        {
            blockers.push(format!(
                "release workload {} `{}` has no reproducible cargo_full CUDA benchmark command",
                family.release_plan_workload, family.id
            ));
        }
        if family.required
            && family.dispatch_policy != "megakernel"
            && family
                .non_megakernel_justification
                .is_none_or(|justification| justification.len() < 48)
        {
            blockers.push(format!(
                "release workload {} `{}` uses non-megakernel dispatch policy `{}` without a concrete architectural or measured justification",
                family.release_plan_workload, family.id, family.dispatch_policy
            ));
        }
    }
    if cpu_sota_100x_contract_ids.len() < MIN_CPU_SOTA_100X_FAMILIES {
        blockers.push(format!(
            "release suite declares {} CPU-SOTA 100x performance contract(s); needs at least {MIN_CPU_SOTA_100X_FAMILIES}",
            cpu_sota_100x_contract_ids.len()
        ));
    }
    if cpu_sota_100x_family_count < MIN_CPU_SOTA_100X_FAMILIES {
        blockers.push(format!(
            "release suite covers {cpu_sota_100x_family_count} CPU-SOTA 100x workload family/families; needs at least {MIN_CPU_SOTA_100X_FAMILIES}"
        ));
    }
    if cpu_sota_100x_family_count < required_cpu_sota_100x_families.len() {
        blockers.push(format!(
            "release suite covers {cpu_sota_100x_family_count} BENCH_TARGETS-derived CPU-SOTA 100x workload family/families; needs {}",
            required_cpu_sota_100x_families.len()
        ));
    }
    for family in &missing_required_cpu_sota_100x_families {
        blockers.push(format!(
            "release suite must prove CPU-SOTA 100x for required family `{family}`"
        ));
    }

    ReleaseWorkloadMatrix {
        schema_version: 1,
        benchmark_evidence_schema_version: 1,
        required_benchmark_case_fields: REQUIRED_BENCHMARK_CASE_FIELDS.to_vec(),
        required_benchmark_metric_fields: REQUIRED_BENCHMARK_METRIC_FIELDS.to_vec(),
        required_closed_families,
        required_cpu_sota_100x_families,
        missing_required_cpu_sota_100x_families,
        matched_required_families,
        release_suite_case_count: required_matched_release_cases.len(),
        cpu_sota_contract_count: cpu_sota_contract_ids.len(),
        cpu_sota_100x_contract_count: cpu_sota_100x_contract_ids.len(),
        cpu_sota_100x_contract_cases,
        cpu_sota_100x_family_count,
        cpu_sota_100x_families,
        families,
        blockers,
    }
}

pub fn emit_release_matrix(
    matrix: &ReleaseWorkloadMatrix,
    format: &str,
    output: Option<&str>,
) -> anyhow::Result<()> {
    let rendered = render_release_matrix(matrix, format)?;
    if let Some(output) = output {
        let output = Path::new(output);
        if let Some(parent) = output.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(output, rendered)?;
        return Ok(());
    }
    print!("{rendered}");
    Ok(())
}

fn render_release_matrix(matrix: &ReleaseWorkloadMatrix, format: &str) -> anyhow::Result<String> {
    if format == "json" {
        return Ok(format!("{}\n", serde_json::to_string_pretty(matrix)?));
    }

    let mut out = String::new();
    out.push_str(&format!(
        "release workload families: {}/{} required, {} release cases, {} CPU-SOTA contracts, {} CPU-SOTA 100x contracts",
        matrix.matched_required_families,
        matrix.required_closed_families,
        matrix.release_suite_case_count,
        matrix.cpu_sota_contract_count,
        matrix.cpu_sota_100x_contract_count
    ));
    out.push('\n');
    if !matrix.cpu_sota_100x_families.is_empty() {
        out.push_str(&format!(
            "CPU-SOTA 100x release families: {}\n",
            matrix.cpu_sota_100x_families.join(", ")
        ));
    }
    for family in &matrix.families {
        let status = if family.matched_cases.is_empty() {
            "open"
        } else {
            "covered"
        };
        out.push_str(&format!(
            "W{} {:<28} {:<7} {}",
            family.release_plan_workload, family.id, status, family.title
        ));
        out.push('\n');
        out.push_str(&format!("  dispatch-policy: {}\n", family.dispatch_policy));
        if let Some(justification) = family.non_megakernel_justification {
            out.push_str(&format!(
                "  non-megakernel-justification: {justification}\n"
            ));
        }
        for case in &family.matched_cases {
            out.push_str(&format!("  case: {case}\n"));
        }
        for contract in &family.cpu_sota_contracts {
            out.push_str(&format!("  contract: {contract}\n"));
        }
        if !family.cpu_sota_baseline_crates.is_empty() {
            out.push_str(&format!(
                "  cpu-baseline-crates: {}\n",
                family.cpu_sota_baseline_crates.join(", ")
            ));
        }
        if !family.cpu_sota_backend_ids.is_empty() {
            out.push_str(&format!(
                "  contract-backends: {}\n",
                family.cpu_sota_backend_ids.join(", ")
            ));
        }
        out.push_str(&format!("  artifact: {}\n", family.evidence_artifact));
        out.push_str(&format!(
            "  bench-targets: {}\n",
            family.bench_target_ids.join(", ")
        ));
        if let Some(command) = &family.benchmark_command {
            out.push_str(&format!("  command: {command}\n"));
        }
    }
    if !matrix.blockers.is_empty() {
        out.push_str("blockers:\n");
        for blocker in &matrix.blockers {
            out.push_str(&format!("  - {blocker}\n"));
        }
    }
    Ok(out)
}

pub fn enforce_release_matrix(matrix: &ReleaseWorkloadMatrix) -> anyhow::Result<()> {
    if matrix.blockers.is_empty() {
        return Ok(());
    }
    anyhow::bail!(
        "release workload matrix has {} blocker(s): {}",
        matrix.blockers.len(),
        matrix.blockers.join("; ")
    )
}

fn build_family_report(
    family: &ReleaseWorkloadFamily,
    release_cases: &[&'static dyn BenchCase],
    target_by_id: &BTreeMap<&str, &ReleaseBenchTarget>,
) -> ReleaseWorkloadFamilyReport {
    let mut matched_cases = Vec::new();
    let mut cpu_sota_contracts = Vec::new();
    let mut cpu_sota_100x_cases = Vec::new();
    let mut cpu_sota_baseline_names = BTreeSet::new();
    let mut cpu_sota_baseline_crates = BTreeSet::new();
    let mut cpu_sota_backend_ids = BTreeSet::new();
    let mut max_cpu_sota_min_speedup_x: Option<f64> = None;

    for case in release_cases {
        if !case_matches_family(*case, family) {
            continue;
        }
        let id = case.id().0;
        matched_cases.push(id.clone());
        if has_cpu_sota_100x_contract(case.performance_contract().as_ref()) {
            cpu_sota_100x_cases.push(id.clone());
        }
        collect_cpu_sota_contracts(
            &id,
            case.performance_contract().as_ref(),
            &mut cpu_sota_contracts,
            &mut cpu_sota_baseline_names,
            &mut cpu_sota_baseline_crates,
            &mut cpu_sota_backend_ids,
            &mut max_cpu_sota_min_speedup_x,
        );
    }

    matched_cases.sort();
    cpu_sota_100x_cases.sort();
    let evidence_artifact = format!(
        "release/evidence/benchmarks/workload-{:02}-{}.json",
        family.release_plan_workload, family.id
    );
    let bench_target = target_by_id.get(family.bench_target_id).copied();
    let requires_release_defining_cpu_sota = family.required
        && bench_target.is_some_and(|target| target.min_speedup_over_cpu_sota >= 100.0);
    let benchmark_case = preferred_release_case(
        &matched_cases,
        &cpu_sota_100x_cases,
        bench_target.map(|target| target.bench_case_id.as_str()),
        requires_release_defining_cpu_sota,
    );
    let benchmark_command = benchmark_case.map(|case_id| {
        format!(
            "cargo_full run -p vyre-bench -- run --suite release --case {case_id} --backend cuda --enforce-budgets --output {evidence_artifact}"
        )
    });
    cpu_sota_contracts.sort();
    let cpu_sota_baseline_names = cpu_sota_baseline_names.into_iter().collect::<Vec<_>>();
    let cpu_sota_baseline_crates = cpu_sota_baseline_crates.into_iter().collect::<Vec<_>>();
    let cpu_sota_backend_ids = cpu_sota_backend_ids.into_iter().collect::<Vec<_>>();
    let fair_cpu_sota_baseline_count = if cpu_sota_baseline_crates.is_empty()
        || cpu_sota_baseline_names.is_empty()
        || !cpu_sota_backend_ids.iter().any(|backend| backend == "cuda")
    {
        0
    } else {
        cpu_sota_baseline_crates.len()
    };
    let reproducible_cuda_command = benchmark_command.as_ref().is_some_and(|command| {
        command.contains("cargo_full")
            && command.contains("--backend cuda")
            && command.contains("--enforce-budgets")
            && command.contains(&evidence_artifact)
    });
    ReleaseWorkloadFamilyReport {
        id: family.id,
        title: family.title,
        release_plan_workload: family.release_plan_workload,
        required: family.required,
        dispatch_policy: family.dispatch_policy,
        non_megakernel_justification: family.non_megakernel_justification,
        matched_cases,
        evidence_artifact,
        benchmark_command,
        bench_target_ids: if bench_target.is_some() {
            vec![family.bench_target_id]
        } else {
            Vec::new()
        },
        cpu_sota_contracts,
        cpu_sota_100x_cases,
        cpu_sota_baseline_names,
        cpu_sota_baseline_crates,
        cpu_sota_backend_ids,
        fair_cpu_sota_baseline_count,
        reproducible_cuda_command,
        max_cpu_sota_min_speedup_x,
    }
}

fn preferred_release_case<'a>(
    matched_cases: &'a [String],
    cpu_sota_100x_cases: &'a [String],
    release_defining_case_id: Option<&str>,
    require_release_defining_cpu_sota: bool,
) -> Option<&'a str> {
    if require_release_defining_cpu_sota {
        let release_defining_case_id = release_defining_case_id?;
        return cpu_sota_100x_cases
            .iter()
            .find(|case_id| case_id.as_str() == release_defining_case_id)
            .map(String::as_str);
    }
    if let Some(release_defining_case_id) = release_defining_case_id {
        if let Some(case_id) = matched_cases
            .iter()
            .find(|case_id| case_id.as_str() == release_defining_case_id)
        {
            return Some(case_id.as_str());
        }
    }
    matched_cases.first().map(String::as_str)
}

fn release_bench_targets_from_manifest(text: &str) -> Result<Vec<ReleaseBenchTarget>, String> {
    let value = toml::from_str::<toml::Value>(text)
        .map_err(|error| format!("Fix: BENCH_TARGETS.toml must parse as TOML: {error}"))?;
    let targets = value
        .get("target")
        .and_then(toml::Value::as_array)
        .ok_or_else(|| "Fix: BENCH_TARGETS.toml must contain [[target]] rows.".to_string())?;
    let mut seen = BTreeSet::new();
    let mut rows = Vec::new();
    for target in targets.iter().filter(|target| {
        target.get("suite").and_then(toml::Value::as_str) == Some("release-workload")
    }) {
        let id = release_target_string(target, "id")?;
        if !seen.insert(id.clone()) {
            return Err(format!(
                "Fix: BENCH_TARGETS.toml contains duplicate release-workload target id `{id}`."
            ));
        }
        rows.push(ReleaseBenchTarget {
            id,
            bench_case_id: release_target_string(target, "bench_case_id")?,
            min_speedup_over_cpu_sota: release_target_number(target, "min_speedup_over_cpu_sota")?,
        });
    }
    if rows.is_empty() {
        return Err(
            "Fix: BENCH_TARGETS.toml must define at least one suite=release-workload target."
                .to_string(),
        );
    }
    Ok(rows)
}

fn release_target_string(target: &toml::Value, key: &'static str) -> Result<String, String> {
    let id = target
        .get("id")
        .and_then(toml::Value::as_str)
        .unwrap_or("<missing id>");
    let value = target
        .get(key)
        .and_then(toml::Value::as_str)
        .unwrap_or("")
        .trim();
    if value.is_empty() {
        return Err(format!(
            "Fix: release-workload BENCH_TARGETS target `{id}` must declare non-empty `{key}`."
        ));
    }
    Ok(value.to_string())
}

fn release_target_number(target: &toml::Value, key: &'static str) -> Result<f64, String> {
    let id = target
        .get("id")
        .and_then(toml::Value::as_str)
        .unwrap_or("<missing id>");
    let value = target
        .get(key)
        .and_then(toml::Value::as_float)
        .or_else(|| {
            target
                .get(key)
                .and_then(toml::Value::as_integer)
                .map(|value| value as f64)
        })
        .ok_or_else(|| {
            format!(
                "Fix: release-workload BENCH_TARGETS target `{id}` must declare numeric `{key}`."
            )
        })?;
    if value <= 0.0 {
        return Err(format!(
            "Fix: release-workload BENCH_TARGETS target `{id}` numeric `{key}` must be positive."
        ));
    }
    Ok(value)
}

fn release_bench_target_by_id(
    targets: &[ReleaseBenchTarget],
) -> BTreeMap<&str, &ReleaseBenchTarget> {
    targets
        .iter()
        .map(|target| (target.id.as_str(), target))
        .collect()
}

fn required_cpu_sota_100x_family_ids(
    families: &'static [ReleaseWorkloadFamily],
    target_by_id: &BTreeMap<&str, &ReleaseBenchTarget>,
) -> Vec<&'static str> {
    families
        .iter()
        .filter(|family| family.required)
        .filter(|family| {
            target_by_id
                .get(family.bench_target_id)
                .is_some_and(|target| target.min_speedup_over_cpu_sota >= 100.0)
        })
        .map(|family| family.id)
        .collect()
}

fn has_cpu_sota_100x_contract(contract: Option<&PerformanceContract>) -> bool {
    contract.is_some_and(|contract| {
        contract.baselines.iter().any(|baseline| {
            matches!(baseline.class, BaselineClass::CpuSota) && baseline.min_speedup_x >= 100.0
        })
    })
}

fn case_matches_family(case: &'static dyn BenchCase, family: &ReleaseWorkloadFamily) -> bool {
    let metadata = case.metadata();
    let id = metadata.id.0.to_ascii_lowercase();
    let name = metadata.name.to_ascii_lowercase();
    let description = metadata.description.to_ascii_lowercase();
    let tags: Vec<String> = metadata
        .tags
        .iter()
        .map(|tag| tag.to_ascii_lowercase())
        .collect();
    let any_match = family.any_terms.iter().any(|term| {
        let term = term.to_ascii_lowercase();
        id.contains(&term)
            || name.contains(&term)
            || description.contains(&term)
            || tags.iter().any(|tag| tag.contains(&term))
    });
    let all_match = !family.all_terms.is_empty()
        && family.all_terms.iter().all(|term| {
            let term = term.to_ascii_lowercase();
            id.contains(&term)
                || name.contains(&term)
                || description.contains(&term)
                || tags.iter().any(|tag| tag.contains(&term))
        });
    any_match || all_match
}

fn collect_cpu_sota_contracts(
    case_id: &str,
    contract: Option<&PerformanceContract>,
    cpu_sota_contracts: &mut Vec<String>,
    cpu_sota_baseline_names: &mut BTreeSet<String>,
    cpu_sota_baseline_crates: &mut BTreeSet<String>,
    cpu_sota_backend_ids: &mut BTreeSet<String>,
    max_cpu_sota_min_speedup_x: &mut Option<f64>,
) {
    let Some(contract) = contract else {
        return;
    };
    for baseline in &contract.baselines {
        if !matches!(baseline.class, BaselineClass::CpuSota) {
            continue;
        }
        cpu_sota_contracts.push(format!(
            "{} => {} {}x",
            case_id, baseline.name, baseline.min_speedup_x
        ));
        if !baseline.crate_name.trim().is_empty() {
            cpu_sota_baseline_crates.insert(baseline.crate_name.clone());
        }
        if !baseline.name.trim().is_empty() {
            cpu_sota_baseline_names.insert(baseline.name.clone());
        }
        for backend in &baseline.backend_ids {
            cpu_sota_backend_ids.insert(backend.clone());
        }
        *max_cpu_sota_min_speedup_x = Some(
            max_cpu_sota_min_speedup_x
                .unwrap_or(0.0)
                .max(baseline.min_speedup_x),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preferred_release_case_rejects_non_release_defining_cpu_sota_cases() {
        let matched_cases = vec![
            "conditions.yara_like.batch.16x64k".to_string(),
            "release.condition_eval.1m".to_string(),
        ];
        let cpu_sota_100x_cases = vec!["conditions.yara_like.eval.1m".to_string()];

        assert_eq!(
            preferred_release_case(
                &matched_cases,
                &cpu_sota_100x_cases,
                Some("release.condition_eval.1m"),
                true
            ),
            None,
            "Fix: release matrix commands must not publish a broad CPU-SOTA case when the 100x case list does not include a release-defining case."
        );
    }

    #[test]
    fn preferred_release_case_falls_back_to_matched_cases_without_cpu_sota_contracts() {
        let matched_cases = vec![
            "conditions.yara_like.batch.16x64k".to_string(),
            "release.condition_eval.1m".to_string(),
        ];
        let cpu_sota_100x_cases = Vec::new();

        assert_eq!(
            preferred_release_case(
                &matched_cases,
                &cpu_sota_100x_cases,
                Some("release.condition_eval.1m"),
                false
            ),
            Some("release.condition_eval.1m"),
            "Fix: non-100x release workload commands should still prefer release-defining matched cases."
        );
    }

    #[test]
    fn preferred_release_case_keeps_non_proof_cpu_sota_workload_commands() {
        let matched_cases = vec!["nn.linear_4bit_affine_grouped.1m".to_string()];
        let cpu_sota_100x_cases = vec!["nn.linear_4bit_affine_grouped.1m".to_string()];

        assert_eq!(
            preferred_release_case(
                &matched_cases,
                &cpu_sota_100x_cases,
                Some("nn.linear_4bit_affine_grouped.1m"),
                false
            ),
            Some("nn.linear_4bit_affine_grouped.1m"),
            "Fix: optional or non-proof 100x workloads should keep reproducible CUDA commands while CPU-SOTA proof workloads require release-defining case ids."
        );
    }

    #[test]
    fn required_cpu_sota_100x_families_follow_bench_target_data() {
        let low_manifest = r#"
schema = 1

[[target]]
id = "release.workload.condition_eval"
bench_case_id = "release.condition_eval.1m"
suite = "release-workload"
min_speedup_over_cpu_sota = 50.0
"#;
        let high_manifest = r#"
schema = 1

[[target]]
id = "release.workload.condition_eval"
bench_case_id = "release.condition_eval.1m"
suite = "release-workload"
min_speedup_over_cpu_sota = 100.0
"#;
        let low_targets = release_bench_targets_from_manifest(low_manifest)
            .expect("Fix: low-speedup target fixture must parse.");
        let high_targets = release_bench_targets_from_manifest(high_manifest)
            .expect("Fix: high-speedup target fixture must parse.");
        let low_by_id = release_bench_target_by_id(&low_targets);
        let high_by_id = release_bench_target_by_id(&high_targets);

        assert!(
            !required_cpu_sota_100x_family_ids(RELEASE_WORKLOADS, &low_by_id)
                .contains(&"condition-eval"),
            "Fix: release matrix must not require CPU-SOTA 100x for a target whose target data sets a lower speedup."
        );
        assert!(
            required_cpu_sota_100x_family_ids(RELEASE_WORKLOADS, &high_by_id)
                .contains(&"condition-eval"),
            "Fix: release matrix required CPU-SOTA 100x families must follow BENCH_TARGETS.toml target data."
        );
    }
}
