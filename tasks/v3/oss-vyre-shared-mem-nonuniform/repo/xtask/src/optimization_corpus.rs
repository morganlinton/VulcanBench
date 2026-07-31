//! Generate the release optimization corpus evidence artifact.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;

const MIN_OPTIMIZATION_FAMILIES: usize = REQUIRED_OPTIMIZATION_FAMILIES.len();
const MIN_CASES_PER_REQUIRED_OPTIMIZATION_FAMILY: usize = 128;
const REQUIRED_OPTIMIZATION_FAMILIES: &[&str] = &[
    "algebraic",
    "predicate",
    "egraph",
    "memory-layout",
    "control-flow",
    "vector-layout",
    "A13-coalesce-fixture",
    "A14-shared-mem-promote-fixture",
    "A15-bank-conflict-fixture",
    "A16-vec-pack-fixture",
    "weir-dataflow-dse",
    "weir-dataflow-loop-fusion",
    "weir-dataflow-loop-fission",
    "weir-dataflow-licm",
];

#[derive(Debug, Serialize)]
struct OptimizationCorpusContracts {
    schema_version: u32,
    required_min_cases: usize,
    generated_cases: usize,
    verified_cases: usize,
    optimized_cases: usize,
    dataflow_analysis_cases: usize,
    dataflow_analysis_optimized_cases: usize,
    non_converged_cases: usize,
    total_ops_before: usize,
    total_ops_after: usize,
    blockers: Vec<String>,
}

#[derive(Debug, Serialize)]
struct OptimizationFamilyManifest {
    schema_version: u32,
    required_family_count: usize,
    required_families: Vec<&'static str>,
    missing_required_families: Vec<String>,
    families: Vec<vyre_lower::optimization_corpus::OptimizationFamilyCount>,
    blockers: Vec<String>,
}

#[derive(Debug, Serialize)]
struct OptimizationCaseManifest {
    schema_version: u32,
    required_min_pass_instances: usize,
    pass_instance_count: usize,
    generated_cases: usize,
    unique_case_ids: usize,
    duplicate_case_ids: Vec<String>,
    family_count: usize,
    required_family_count: usize,
    dataflow_analysis_cases: usize,
    cases_with_child_bodies: usize,
    cases_with_bindings: usize,
    cases_with_literals: usize,
    entries: Vec<OptimizationCaseManifestEntry>,
    blockers: Vec<String>,
}

#[derive(Debug, Serialize)]
struct OptimizationCaseManifestEntry {
    id: String,
    family: String,
    top_level_ops: usize,
    total_ops: usize,
    child_body_count: usize,
    binding_count: usize,
    literal_count: usize,
}

#[derive(Debug, Serialize)]
struct OptimizationAnalysisFixtureManifest {
    schema_version: u32,
    required_min_cases_per_family: usize,
    required_families: Vec<&'static str>,
    missing_required_families: Vec<String>,
    total_fixture_cases: usize,
    total_triggered_cases: usize,
    families: Vec<AnalysisFixtureFamilyEvidence>,
    blockers: Vec<String>,
}

#[derive(Debug, Serialize)]
struct AnalysisFixtureFamilyEvidence {
    family: String,
    cases: usize,
    analysis_sites: usize,
    triggered_cases: usize,
    coalesced_unit_stride_sites: usize,
    strided_sites: usize,
    broadcast_sites: usize,
    shared_mem_candidates: usize,
    shared_mem_tile_bytes: u64,
    bank_conflict_sites: usize,
    bank_conflict_critical_sites: usize,
    vec_pack_chains: usize,
    vec_pack_ops_eliminated: u64,
}

pub(crate) fn run(args: &[String]) {
    let output = match parse_output(args) {
        Ok(output) => output,
        Err(message) => {
            eprintln!("{message}");
            std::process::exit(2);
        }
    };
    let cases = vyre_lower::optimization_corpus::generate_release_corpus();
    let manifest = vyre_lower::optimization_corpus::manifest_for(&cases);
    if manifest.generated_cases < manifest.required_min_cases {
        eprintln!(
            "optimization-corpus: generated {} cases, needs at least {}",
            manifest.generated_cases, manifest.required_min_cases
        );
        std::process::exit(1);
    }
    if manifest.verified_cases != manifest.generated_cases {
        eprintln!(
            "optimization-corpus: verified {} cases, generated {} cases",
            manifest.verified_cases, manifest.generated_cases
        );
        for blocker in manifest.blockers.iter().take(20) {
            eprintln!("  - {blocker}");
        }
        std::process::exit(1);
    }
    if !manifest.blockers.is_empty() {
        eprintln!(
            "optimization-corpus: validation reported {} blocker(s)",
            manifest.blockers.len()
        );
        for blocker in manifest.blockers.iter().take(20) {
            eprintln!("  - {blocker}");
        }
        std::process::exit(1);
    }
    let json = match serde_json::to_string_pretty(&manifest) {
        Ok(json) => json,
        Err(error) => {
            eprintln!("Fix: failed to serialize optimization corpus manifest: {error}");
            std::process::exit(1);
        }
    };
    if let Some(parent) = output.parent() {
        if let Err(error) = fs::create_dir_all(parent) {
            eprintln!("Fix: failed to create `{}`: {error}", parent.display());
            std::process::exit(1);
        }
    }
    if let Err(error) = fs::write(&output, format!("{json}\n")) {
        eprintln!("Fix: failed to write `{}`: {error}", output.display());
        std::process::exit(1);
    }
    write_sibling_artifacts(&output, &cases, &manifest);
    println!(
        "optimization-corpus: wrote {} generated cases to {}",
        manifest.generated_cases,
        output.display()
    );
}

fn write_sibling_artifacts(
    output: &Path,
    cases: &[vyre_lower::optimization_corpus::OptimizationCorpusCase],
    manifest: &vyre_lower::optimization_corpus::OptimizationCorpusManifest,
) {
    let Some(parent) = output.parent() else {
        eprintln!(
            "Fix: optimization corpus output `{}` has no parent directory.",
            output.display()
        );
        std::process::exit(1);
    };
    write_json(
        &parent.join("optimization-corpus-contracts.json"),
        &OptimizationCorpusContracts {
            schema_version: 1,
            required_min_cases: manifest.required_min_cases,
            generated_cases: manifest.generated_cases,
            verified_cases: manifest.verified_cases,
            optimized_cases: manifest.optimized_cases,
            dataflow_analysis_cases: manifest.dataflow_analysis_cases,
            dataflow_analysis_optimized_cases: manifest.dataflow_analysis_optimized_cases,
            non_converged_cases: manifest.non_converged_cases,
            total_ops_before: manifest.total_ops_before,
            total_ops_after: manifest.total_ops_after,
            blockers: manifest.blockers.clone(),
        },
    );
    let mut family_blockers = Vec::new();
    if manifest.families.len() < MIN_OPTIMIZATION_FAMILIES {
        family_blockers.push(format!(
            "optimization corpus has {} families, below release floor {MIN_OPTIMIZATION_FAMILIES}",
            manifest.families.len()
        ));
    }
    let family_case_sum = manifest
        .families
        .iter()
        .map(|family| family.cases)
        .sum::<usize>();
    if family_case_sum != manifest.generated_cases {
        family_blockers.push(format!(
            "optimization family manifest accounts for {family_case_sum} case(s), generated corpus reports {}",
            manifest.generated_cases
        ));
    }
    for family in &manifest.families {
        if family.cases == 0 {
            family_blockers.push(format!(
                "optimization family `{}` has zero generated cases",
                family.family
            ));
        }
    }
    let missing_required_families = REQUIRED_OPTIMIZATION_FAMILIES
        .iter()
        .filter_map(|required| {
            let required_cases = manifest
                .families
                .iter()
                .find(|family| family.family == **required)
                .map(|family| family.cases)
                .unwrap_or(0);
            (required_cases == 0).then(|| (*required).to_string())
        })
        .collect::<Vec<_>>();
    for required in REQUIRED_OPTIMIZATION_FAMILIES {
        let required_cases = manifest
            .families
            .iter()
            .find(|family| family.family == *required)
            .map(|family| family.cases)
            .unwrap_or(0);
        if required_cases == 0 {
            family_blockers.push(format!("missing required optimization family `{required}`"));
        } else if required_cases < MIN_CASES_PER_REQUIRED_OPTIMIZATION_FAMILY {
            family_blockers.push(format!(
                "required optimization family `{required}` has {required_cases} generated case(s), below release floor {MIN_CASES_PER_REQUIRED_OPTIMIZATION_FAMILY}"
            ));
        }
    }
    if manifest.dataflow_analysis_cases == 0 {
        family_blockers
            .push("optimization corpus has zero Weir dataflow-aware DSE cases".to_string());
    }
    if manifest.dataflow_analysis_optimized_cases < manifest.dataflow_analysis_cases {
        family_blockers.push(format!(
            "Weir dataflow-aware pipeline optimized {} of {} generated DSE case(s)",
            manifest.dataflow_analysis_optimized_cases, manifest.dataflow_analysis_cases
        ));
    }
    write_json(
        &parent.join("optimization-family-manifest.json"),
        &OptimizationFamilyManifest {
            schema_version: 1,
            required_family_count: REQUIRED_OPTIMIZATION_FAMILIES.len(),
            required_families: REQUIRED_OPTIMIZATION_FAMILIES.to_vec(),
            missing_required_families,
            families: manifest.families.clone(),
            blockers: family_blockers,
        },
    );
    write_analysis_fixture_manifest(parent, cases);
    write_case_manifest(parent, cases, manifest);
}

fn write_analysis_fixture_manifest(
    parent: &Path,
    cases: &[vyre_lower::optimization_corpus::OptimizationCorpusCase],
) {
    let required_families = vec![
        "A13-coalesce-fixture",
        "A14-shared-mem-promote-fixture",
        "A15-bank-conflict-fixture",
        "A16-vec-pack-fixture",
    ];
    let families = required_families
        .iter()
        .map(|family| analysis_fixture_family_evidence(family, cases))
        .collect::<Vec<_>>();
    let missing_required_families = families
        .iter()
        .filter(|family| family.cases == 0)
        .map(|family| family.family.clone())
        .collect::<Vec<_>>();
    let total_fixture_cases = families.iter().map(|family| family.cases).sum::<usize>();
    let total_triggered_cases = families
        .iter()
        .map(|family| family.triggered_cases)
        .sum::<usize>();
    let mut blockers = Vec::new();
    for family in &families {
        if family.cases < MIN_CASES_PER_REQUIRED_OPTIMIZATION_FAMILY {
            blockers.push(format!(
                "analysis fixture family `{}` has {} case(s), below release floor {MIN_CASES_PER_REQUIRED_OPTIMIZATION_FAMILY}",
                family.family, family.cases
            ));
        }
        if family.triggered_cases != family.cases {
            blockers.push(format!(
                "analysis fixture family `{}` triggered {}/{} generated case(s); every hand-built A13-A16 fixture case must exercise its analysis",
                family.family, family.triggered_cases, family.cases
            ));
        }
        if family.analysis_sites < family.cases {
            blockers.push(format!(
                "analysis fixture family `{}` produced {} analysis site(s) for {} case(s); release fixtures need at least one site per case",
                family.family, family.analysis_sites, family.cases
            ));
        }
        match family.family.as_str() {
            "A13-coalesce-fixture" => {
                if family.coalesced_unit_stride_sites == 0
                    || family.strided_sites == 0
                    || family.broadcast_sites == 0
                {
                    blockers.push(format!(
                        "A13 coalesce fixture must prove unit-stride, strided, and broadcast classifications; saw unit_stride={}, strided={}, broadcast={}",
                        family.coalesced_unit_stride_sites, family.strided_sites, family.broadcast_sites
                    ));
                }
            }
            "A14-shared-mem-promote-fixture" => {
                if family.shared_mem_candidates == 0 || family.shared_mem_tile_bytes == 0 {
                    blockers.push(
                        "A14 shared-memory promotion fixture produced no promotion candidates"
                            .to_string(),
                    );
                }
            }
            "A15-bank-conflict-fixture" => {
                if family.bank_conflict_sites == 0 || family.bank_conflict_critical_sites == 0 {
                    blockers.push(
                        "A15 bank-conflict fixture produced no critical conflict sites".to_string(),
                    );
                }
            }
            "A16-vec-pack-fixture" => {
                if family.vec_pack_chains == 0 || family.vec_pack_ops_eliminated == 0 {
                    blockers.push(
                        "A16 vec-pack fixture produced no adjacent-load packing chains".to_string(),
                    );
                }
            }
            _ => {}
        }
    }
    write_json(
        &parent.join("optimization-analysis-fixtures.json"),
        &OptimizationAnalysisFixtureManifest {
            schema_version: 1,
            required_min_cases_per_family: MIN_CASES_PER_REQUIRED_OPTIMIZATION_FAMILY,
            required_families,
            missing_required_families,
            total_fixture_cases,
            total_triggered_cases,
            families,
            blockers,
        },
    );
}

fn analysis_fixture_family_evidence(
    family: &str,
    cases: &[vyre_lower::optimization_corpus::OptimizationCorpusCase],
) -> AnalysisFixtureFamilyEvidence {
    let mut evidence = AnalysisFixtureFamilyEvidence {
        family: family.to_string(),
        cases: 0,
        analysis_sites: 0,
        triggered_cases: 0,
        coalesced_unit_stride_sites: 0,
        strided_sites: 0,
        broadcast_sites: 0,
        shared_mem_candidates: 0,
        shared_mem_tile_bytes: 0,
        bank_conflict_sites: 0,
        bank_conflict_critical_sites: 0,
        vec_pack_chains: 0,
        vec_pack_ops_eliminated: 0,
    };
    for case in cases.iter().filter(|case| case.family == family) {
        evidence.cases += 1;
        match family {
            "A13-coalesce-fixture" => {
                let report = vyre_lower::analyses::analyze_coalesce(&case.descriptor);
                let coalesced = report
                    .sites
                    .iter()
                    .filter(|site| {
                        matches!(
                            &site.pattern,
                            vyre_lower::analyses::coalesce::AccessPattern::CoalescedUnitStride
                        )
                    })
                    .count();
                let strided = report
                    .sites
                    .iter()
                    .filter(|site| {
                        matches!(
                            &site.pattern,
                            vyre_lower::analyses::coalesce::AccessPattern::Strided { .. }
                        )
                    })
                    .count();
                let broadcast = report
                    .sites
                    .iter()
                    .filter(|site| {
                        matches!(
                            &site.pattern,
                            vyre_lower::analyses::coalesce::AccessPattern::Broadcast
                        )
                    })
                    .count();
                evidence.analysis_sites += report.sites.len();
                evidence.coalesced_unit_stride_sites += coalesced;
                evidence.strided_sites += strided;
                evidence.broadcast_sites += broadcast;
                if coalesced != 0 || strided != 0 || broadcast != 0 {
                    evidence.triggered_cases += 1;
                }
            }
            "A14-shared-mem-promote-fixture" => {
                let plan = vyre_lower::analyses::analyze_shared_mem_promote(&case.descriptor);
                evidence.analysis_sites += plan.candidates.len();
                evidence.shared_mem_candidates += plan.candidates.len();
                evidence.shared_mem_tile_bytes = evidence
                    .shared_mem_tile_bytes
                    .saturating_add(u64::from(plan.total_tile_bytes));
                if !plan.candidates.is_empty() {
                    evidence.triggered_cases += 1;
                }
            }
            "A15-bank-conflict-fixture" => {
                let report = vyre_lower::analyses::analyze_bank_conflict(&case.descriptor);
                let critical = report
                    .sites
                    .iter()
                    .filter(|site| {
                        matches!(
                            &site.conflict,
                            vyre_lower::analyses::bank_conflict::BankConflictKind::Conflict { .. }
                        )
                    })
                    .count();
                evidence.analysis_sites += report.sites.len();
                evidence.bank_conflict_sites += report.sites.len();
                evidence.bank_conflict_critical_sites += critical;
                if critical != 0 {
                    evidence.triggered_cases += 1;
                }
            }
            "A16-vec-pack-fixture" => {
                let report = vyre_lower::analyses::vec_pack::analyze(&case.descriptor);
                evidence.analysis_sites += report.chains.len();
                evidence.vec_pack_chains += report.chains.len();
                evidence.vec_pack_ops_eliminated = evidence
                    .vec_pack_ops_eliminated
                    .saturating_add(u64::from(report.total_ops_eliminated));
                if report.has_chains() {
                    evidence.triggered_cases += 1;
                }
            }
            _ => {}
        }
    }
    evidence
}

fn write_case_manifest(
    parent: &Path,
    cases: &[vyre_lower::optimization_corpus::OptimizationCorpusCase],
    manifest: &vyre_lower::optimization_corpus::OptimizationCorpusManifest,
) {
    let mut seen = BTreeSet::new();
    let mut duplicate_case_ids = BTreeSet::new();
    let mut entries = Vec::with_capacity(cases.len());
    for case in cases {
        if !seen.insert(case.id.clone()) {
            duplicate_case_ids.insert(case.id.clone());
        }
        entries.push(OptimizationCaseManifestEntry {
            id: case.id.clone(),
            family: case.family.clone(),
            top_level_ops: case.descriptor.body.ops.len(),
            total_ops: total_op_count(&case.descriptor.body),
            child_body_count: child_body_count(&case.descriptor.body),
            binding_count: case.descriptor.bindings.slots.len(),
            literal_count: case.descriptor.body.literals.len(),
        });
    }
    let mut blockers = Vec::new();
    if cases.len() < manifest.required_min_cases {
        blockers.push(format!(
            "case manifest has {} pass instance(s), below release floor {}",
            cases.len(),
            manifest.required_min_cases
        ));
    }
    if seen.len() != cases.len() {
        blockers.push(format!(
            "case manifest has {} unique id(s) for {} generated case(s)",
            seen.len(),
            cases.len()
        ));
    }
    if manifest.families.len() < MIN_OPTIMIZATION_FAMILIES {
        blockers.push(format!(
            "case manifest covers {} family/families, below release floor {MIN_OPTIMIZATION_FAMILIES}",
            manifest.families.len()
        ));
    }
    if entries.iter().any(|entry| entry.total_ops == 0) {
        blockers.push(
            "case manifest contains a generated pass instance with zero total ops".to_string(),
        );
    }
    let cases_with_child_bodies = entries
        .iter()
        .filter(|entry| entry.child_body_count != 0)
        .count();
    let cases_with_bindings = entries
        .iter()
        .filter(|entry| entry.binding_count != 0)
        .count();
    let cases_with_literals = entries
        .iter()
        .filter(|entry| entry.literal_count != 0)
        .count();
    if cases_with_child_bodies == 0 {
        blockers.push(
            "case manifest contains no generated pass instance with child bodies".to_string(),
        );
    }
    if cases_with_bindings == 0 {
        blockers
            .push("case manifest contains no generated pass instance with bindings".to_string());
    }
    if cases_with_literals == 0 {
        blockers
            .push("case manifest contains no generated pass instance with literals".to_string());
    }
    write_json(
        &parent.join("optimization-case-manifest.json"),
        &OptimizationCaseManifest {
            schema_version: 1,
            required_min_pass_instances: manifest.required_min_cases,
            pass_instance_count: cases.len(),
            generated_cases: manifest.generated_cases,
            unique_case_ids: seen.len(),
            duplicate_case_ids: duplicate_case_ids.into_iter().collect(),
            family_count: manifest.families.len(),
            required_family_count: REQUIRED_OPTIMIZATION_FAMILIES.len(),
            dataflow_analysis_cases: manifest.dataflow_analysis_cases,
            cases_with_child_bodies,
            cases_with_bindings,
            cases_with_literals,
            entries,
            blockers,
        },
    );
}

fn total_op_count(body: &vyre_lower::KernelBody) -> usize {
    body.ops.len() + body.child_bodies.iter().map(total_op_count).sum::<usize>()
}

fn child_body_count(body: &vyre_lower::KernelBody) -> usize {
    body.child_bodies.len()
        + body
            .child_bodies
            .iter()
            .map(child_body_count)
            .sum::<usize>()
}

fn write_json(path: &Path, value: &impl Serialize) {
    let json = match serde_json::to_string_pretty(value) {
        Ok(json) => json,
        Err(error) => {
            eprintln!("Fix: failed to serialize `{}`: {error}", path.display());
            std::process::exit(1);
        }
    };
    if let Err(error) = fs::write(path, format!("{json}\n")) {
        eprintln!("Fix: failed to write `{}`: {error}", path.display());
        std::process::exit(1);
    }
}

fn parse_output(args: &[String]) -> Result<PathBuf, String> {
    let mut output = None;
    let mut index = 2;
    while index < args.len() {
        match args[index].as_str() {
            "--output" => {
                let Some(path) = args.get(index + 1) else {
                    return Err("Fix: --output requires a path.".to_string());
                };
                output = Some(PathBuf::from(path));
                index += 2;
            }
            "--help" | "-h" => {
                println!(
                    "USAGE:\n  cargo_full run --bin xtask -- optimization-corpus [--output PATH]\n\n\
                     Generates the release optimization corpus manifest evidence artifact."
                );
                std::process::exit(0);
            }
            other => {
                return Err(format!(
                    "Fix: unknown optimization-corpus option `{other}`."
                ))
            }
        }
    }
    Ok(output.unwrap_or_else(default_output))
}

fn default_output() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(|path| path.join("release/evidence/optimization/optimization-corpus.json"))
        .unwrap_or_else(|| PathBuf::from("release/evidence/optimization/optimization-corpus.json"))
}
