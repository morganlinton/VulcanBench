//! Test architecture evidence for the Vyre/Weir release.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use walkdir::WalkDir;

#[derive(Debug, Serialize)]
struct TestMatrix {
    schema_version: u32,
    test_files: usize,
    vyre_test_files: usize,
    weir_test_files: usize,
    vyrec_test_files: usize,
    layers: Vec<String>,
    surface_coverages: Vec<SurfaceCoverage>,
    modular_directories: Vec<ModularDirectory>,
    oversized_files: Vec<OversizedFile>,
    god_test_candidates: Vec<String>,
    modularity_finding_count: usize,
    modularity_summary: Vec<ModularitySummary>,
    modularity_findings: Vec<ModularityFinding>,
    risk_dimension_coverages: Vec<RiskDimensionCoverage>,
    risk_family_coverages: Vec<RiskFamilyCoverage>,
    regex_adversarial_coverages: Vec<RegexAdversarialCoverage>,
    regex_adversarial_findings: Vec<RegexAdversarialFinding>,
    blockers: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
struct SurfaceCoverage {
    surface: &'static str,
    file_count: usize,
    assertion_count: usize,
    entrypoint_count: usize,
    layers: Vec<String>,
    required_layers: Vec<&'static str>,
    missing_layers: Vec<&'static str>,
    case_roles: Vec<String>,
    required_case_roles: Vec<&'static str>,
    missing_case_roles: Vec<&'static str>,
    blockers: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
struct OversizedFile {
    path: String,
    lines: usize,
    lines_over_threshold: usize,
    recommended_split: Vec<String>,
    release_blocker: bool,
}

#[derive(Debug, Clone, Serialize)]
struct ModularityFinding {
    path: String,
    surface: &'static str,
    primary_layer: String,
    finding_kind: &'static str,
    lines: usize,
    lines_over_threshold: usize,
    recommended_split: Vec<String>,
    release_blocker: bool,
}

#[derive(Debug, Clone, Serialize)]
struct ModularitySummary {
    surface: &'static str,
    primary_layer: String,
    finding_kind: &'static str,
    recommended_split: String,
    finding_count: usize,
    release_blocker_count: usize,
    max_lines_over_threshold: usize,
}

#[derive(Debug, Clone, Serialize)]
struct ModularDirectory {
    surface: &'static str,
    layer: &'static str,
    path: String,
    exists: bool,
}

#[derive(Debug, Clone, Serialize)]
struct TestFileRecord {
    path: String,
    layers: Vec<String>,
    lines: usize,
    dedicated_test_file: bool,
    inline_test_module_file: bool,
    line_threshold_exceeded: bool,
    has_test_entrypoint: bool,
    assertion_count: usize,
    oversized: bool,
    god_test_candidate: bool,
    recommended_split: Vec<String>,
    case_roles: Vec<String>,
    op_families: Vec<String>,
    backend_families: Vec<String>,
    feature_families: Vec<String>,
    error_path_families: Vec<String>,
    corpus_families: Vec<String>,
    weir_flow_families: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TestFileKind {
    dedicated_test_file: bool,
    inline_test_module_file: bool,
    has_test_entrypoint: bool,
    is_test_file: bool,
}

#[derive(Debug, Serialize)]
struct ModularizationMap {
    schema_version: u32,
    directories: Vec<ModularDirectory>,
    blockers: Vec<String>,
}

#[derive(Debug, Serialize)]
struct OversizedTestClosure {
    schema_version: u32,
    threshold_lines: usize,
    closed: bool,
    total_oversized_files: usize,
    total_god_test_candidates: usize,
    oversized_files: Vec<OversizedFile>,
    god_test_candidates: Vec<String>,
    required_split_count: usize,
    blockers: Vec<String>,
}

#[derive(Debug, Serialize)]
struct ModularityFindingsArtifact {
    schema_version: u32,
    threshold_lines: usize,
    finding_count: usize,
    summary: Vec<ModularitySummary>,
    findings: Vec<ModularityFinding>,
    blockers: Vec<String>,
}

#[derive(Debug, Serialize)]
struct SuiteEvidence {
    schema_version: u32,
    suite: String,
    file_count: usize,
    vyre_file_count: usize,
    dataflow_consumer_file_count: usize,
    vyrec_file_count: usize,
    files: Vec<TestFileRecord>,
    blockers: Vec<String>,
}

#[derive(Debug, Serialize)]
struct SurfaceCoverageArtifact {
    schema_version: u32,
    surfaces: Vec<SurfaceCoverage>,
    blockers: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
struct RiskFamilyCoverage {
    surface: &'static str,
    dimension: &'static str,
    family: String,
    risk_weight: u8,
    file_count: usize,
    assertion_count: usize,
    case_roles: Vec<String>,
    required_case_roles: Vec<&'static str>,
    missing_case_roles: Vec<&'static str>,
}

#[derive(Debug, Clone, Serialize)]
struct RiskDimensionCoverage {
    surface: &'static str,
    dimension: &'static str,
    family_count: usize,
    file_count: usize,
    assertion_count: usize,
    case_roles: Vec<String>,
    required_case_roles: Vec<&'static str>,
    missing_case_roles: Vec<&'static str>,
    blockers: Vec<String>,
}

#[derive(Debug, Serialize)]
struct RiskCoverageArtifact {
    schema_version: u32,
    required_case_roles: Vec<&'static str>,
    dimension_coverages: Vec<RiskDimensionCoverage>,
    surface_coverages: Vec<SurfaceCoverage>,
    family_coverages: Vec<RiskFamilyCoverage>,
    regex_adversarial_coverages: Vec<RegexAdversarialCoverage>,
    regex_adversarial_findings: Vec<RegexAdversarialFinding>,
    blockers: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
struct RegexAdversarialCoverage {
    class_id: &'static str,
    roles: Vec<String>,
    required_roles: Vec<&'static str>,
    missing_roles: Vec<&'static str>,
    evidence_paths: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
struct RegexAdversarialFinding {
    class_id: String,
    role: Option<String>,
    issue: String,
}

#[derive(Debug, Deserialize)]
struct RegexAdversarialCatalog {
    schema_version: u32,
    case: Vec<RegexAdversarialCatalogCase>,
}

#[derive(Debug, Deserialize)]
struct RegexAdversarialCatalogCase {
    class_id: String,
    role: String,
    evidence_path: String,
}

#[derive(Debug, Clone)]
struct RiskFamilies {
    op_families: Vec<String>,
    backend_families: Vec<String>,
    feature_families: Vec<String>,
    error_path_families: Vec<String>,
    corpus_families: Vec<String>,
    weir_flow_families: Vec<String>,
}

const REQUIRED_LAYERS: &[&str] = &[
    "unit",
    "integration",
    "property",
    "adversarial",
    "corpus",
    "benchmark",
    "conformance",
    "gap",
    "fuzz",
];

const REQUIRED_CASE_ROLES: &[&str] = &[
    "positive",
    "negative",
    "boundary",
    "adversarial",
    "property",
    "fuzz",
    "benchmark",
    "conformance",
    "e2e",
];

const REQUIRED_REGEX_ADVERSARIAL_CLASSES: &[&str] = &[
    "quadratic_rescan",
    "empty_match",
    "overlapping_suffix",
    "utf8_boundary",
    "weak_literal",
    "nested_repeats",
    "nullable_loops",
    "anchors",
    "lookarounds",
    "backreferences",
    "unicode_classes",
    "huge_alternations",
];

const REQUIRED_REGEX_ADVERSARIAL_ROLES: &[&str] = &[
    "positive",
    "negative",
    "boundary",
    "adversarial",
    "baseline",
    "evasion",
    "resource_budget",
];

const REGEX_ADVERSARIAL_CLASS_CATALOG: &str =
    "docs/optimization/REGEX_ADVERSARIAL_CLASSES.toml";

const REQUIRED_RISK_DIMENSIONS: &[&str] = &[
    "op",
    "backend",
    "feature",
    "error_path",
    "corpus",
    "weir_flow",
];

const REQUIRED_MODULAR_DIRS: &[(&str, &str)] = &[
    ("fixtures", "tests/fixtures"),
    ("contracts", "tests/contracts"),
    ("properties", "tests/properties"),
    ("backends", "tests/backends"),
    ("corpus", "tests/corpus"),
    ("bench", "benches"),
    ("regression", "tests/regression"),
];

const MAX_TEST_SOURCE_BYTES: u64 = 2_097_152;

const RELEASE_SURFACES: &[(&str, &[&str])] = &[
    ("vyre", REQUIRED_LAYERS),
    ("weir", REQUIRED_LAYERS),
    ("vyrec", REQUIRED_LAYERS),
];

// A test file crossing this line count is a genuine god-file (the same
// `TEST_MAX_LINES` ceiling `scripts/check_max_file_size.sh` enforces). The
// 500-line figure is a split-by-responsibility *guideline* surfaced as a
// non-blocking advisory by `xtask lego-audit`, not a release gate — so this
// closure only blocks on real god-files, not on cohesive corpora that
// legitimately run long.
const OVERSIZED_TEST_THRESHOLD_LINES: usize = 8000;

pub(crate) fn run(args: &[String]) {
    let output = match parse_output(args) {
        Ok(output) => output,
        Err(message) => {
            eprintln!("{message}");
            std::process::exit(2);
        }
    };
    let vyre_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    let santh_root = vyre_root
        .parent()
        .and_then(Path::parent)
        .and_then(Path::parent)
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .unwrap_or_else(|| vyre_root.clone());
    let modular_roots = [
        ("vyre", vyre_root.clone()),
        ("weir", santh_root.join("libs/dataflow/weir")),
        ("vyrec", santh_root.join("tools/vyrec")),
    ];
    let test_roots = [
        vyre_root.clone(),
        santh_root.join("libs/dataflow/weir"),
        santh_root.join("tools/vyrec"),
    ];
    let mut test_files = 0usize;
    let mut layers = BTreeSet::new();
    let mut oversized_files = Vec::new();
    let mut modular_directories = Vec::new();
    let mut file_records = Vec::new();
    let mut scan_blockers = Vec::new();
    for root in &test_roots {
        scan_tests(
            root,
            &mut test_files,
            &mut layers,
            &mut oversized_files,
            &mut file_records,
            &mut scan_blockers,
        );
    }
    for (surface, root) in &modular_roots {
        collect_modular_dirs(surface, root, &mut modular_directories);
    }
    let mut blockers = Vec::new();
    blockers.extend(scan_blockers);
    for required in REQUIRED_LAYERS {
        if !layers.contains(*required) {
            blockers.push(format!("missing required test layer `{required}`"));
        }
    }
    if !oversized_files.is_empty() {
        blockers.push(format!(
            "{} test file(s) exceed the {OVERSIZED_TEST_THRESHOLD_LINES}-line modularity threshold",
            oversized_files.len()
        ));
    }
    let god_test_candidates = file_records
        .iter()
        .filter(|file| file.god_test_candidate)
        .map(|file| file.path.clone())
        .collect::<Vec<_>>();
    if !god_test_candidates.is_empty() {
        blockers.push(format!(
            "{} monolithic test file(s) must be split into modular test layers",
            god_test_candidates.len()
        ));
    }
    for directory in &modular_directories {
        if !directory.exists {
            blockers.push(format!(
                "missing modular test directory `{}` for `{}`",
                directory.path, directory.layer
            ));
        }
    }
    let vyre_test_files = file_records
        .iter()
        .filter(|file| {
            !file.path.contains("/libs/dataflow/weir/") && !file.path.contains("/tools/vyrec/")
        })
        .count();
    let weir_test_files = file_records
        .iter()
        .filter(|file| file.path.contains("/libs/dataflow/weir/"))
        .count();
    let vyrec_test_files = file_records
        .iter()
        .filter(|file| file.path.contains("/tools/vyrec/"))
        .count();
    if vyre_test_files == 0 {
        blockers.push("test matrix has zero Vyre release-surface test files".to_string());
    }
    if weir_test_files == 0 {
        blockers.push("test matrix has zero Weir release-surface test files".to_string());
    }
    if vyrec_test_files == 0 {
        blockers.push("test matrix has zero tools/vyrec release-surface test files".to_string());
    }
    let surface_coverages = release_surface_coverages(&file_records);
    for surface in &surface_coverages {
        for blocker in &surface.blockers {
            blockers.push(blocker.clone());
        }
    }
    let risk_family_coverages = risk_family_coverages(&file_records);
    let risk_dimension_coverages = risk_dimension_coverages(&risk_family_coverages);
    for coverage in &risk_dimension_coverages {
        for blocker in &coverage.blockers {
            blockers.push(blocker.clone());
        }
    }
    let (regex_adversarial_coverages, regex_adversarial_findings) =
        regex_adversarial_coverages(&vyre_root);
    for finding in &regex_adversarial_findings {
        blockers.push(format!(
            "regex adversarial class `{}` role {:?} is invalid: {}",
            finding.class_id, finding.role, finding.issue
        ));
    }
    let modularity_findings = modularity_findings(&file_records);
    let modularity_summary = modularity_summary(&modularity_findings);
    let modularity_finding_count = modularity_findings.len();
    let matrix = TestMatrix {
        schema_version: 5,
        test_files,
        vyre_test_files,
        weir_test_files,
        vyrec_test_files,
        layers: layers.into_iter().map(String::from).collect(),
        surface_coverages,
        modular_directories,
        oversized_files,
        god_test_candidates,
        modularity_finding_count,
        modularity_summary,
        modularity_findings,
        risk_dimension_coverages,
        risk_family_coverages,
        regex_adversarial_coverages,
        regex_adversarial_findings,
        blockers,
    };
    let json = match serde_json::to_string_pretty(&matrix) {
        Ok(json) => json,
        Err(error) => {
            eprintln!("Fix: failed to serialize test matrix: {error}");
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
    write_sibling_artifacts(&output, &matrix, &file_records);
    println!("test-matrix: wrote {}", output.display());
    if !matrix.blockers.is_empty() {
        std::process::exit(1);
    }
}

fn write_sibling_artifacts(output: &Path, matrix: &TestMatrix, files: &[TestFileRecord]) {
    let Some(parent) = output.parent() else {
        eprintln!(
            "Fix: test matrix output `{}` has no parent directory.",
            output.display()
        );
        std::process::exit(1);
    };
    let modular_blockers = matrix
        .modular_directories
        .iter()
        .filter(|directory| !directory.exists)
        .map(|directory| {
            format!(
                "missing modular test directory `{}` for `{}`",
                directory.path, directory.layer
            )
        })
        .collect::<Vec<_>>();
    write_json(
        &parent.join("modularization-map.json"),
        &ModularizationMap {
            schema_version: 1,
            directories: matrix.modular_directories.clone(),
            blockers: modular_blockers,
        },
    );
    let oversized_blockers =
        if matrix.oversized_files.is_empty() && matrix.god_test_candidates.is_empty() {
            Vec::new()
        } else {
            let mut blockers = vec![format!(
            "{} test file(s) exceed the {OVERSIZED_TEST_THRESHOLD_LINES}-line modularity threshold",
            matrix.oversized_files.len()
        )];
            if !matrix.god_test_candidates.is_empty() {
                blockers.push(format!(
                    "{} monolithic tests.rs file(s) still need modularization",
                    matrix.god_test_candidates.len()
                ));
            }
            blockers
        };
    write_json(
        &parent.join("oversized-test-closure.json"),
        &OversizedTestClosure {
            schema_version: 1,
            threshold_lines: OVERSIZED_TEST_THRESHOLD_LINES,
            closed: matrix.oversized_files.is_empty() && matrix.god_test_candidates.is_empty(),
            total_oversized_files: matrix.oversized_files.len(),
            total_god_test_candidates: matrix.god_test_candidates.len(),
            required_split_count: matrix
                .oversized_files
                .iter()
                .map(|file| file.recommended_split.len())
                .sum(),
            oversized_files: matrix.oversized_files.clone(),
            god_test_candidates: matrix.god_test_candidates.clone(),
            blockers: oversized_blockers,
        },
    );
    let modularity_blockers = if matrix.modularity_findings.is_empty() {
        Vec::new()
    } else {
        vec![format!(
            "{} modularity finding(s) remain; split by surface, layer, and target in modularity-findings.json",
            matrix.modularity_finding_count
        )]
    };
    write_json(
        &parent.join("modularity-findings.json"),
        &ModularityFindingsArtifact {
            schema_version: 1,
            threshold_lines: OVERSIZED_TEST_THRESHOLD_LINES,
            finding_count: matrix.modularity_finding_count,
            summary: matrix.modularity_summary.clone(),
            findings: matrix.modularity_findings.clone(),
            blockers: modularity_blockers,
        },
    );
    let mut risk_blockers = matrix
        .surface_coverages
        .iter()
        .flat_map(|surface| {
            surface.missing_case_roles.iter().map(|role| {
                format!(
                    "release surface `{}` is missing required `{role}` case-role evidence",
                    surface.surface
                )
            })
        })
        .chain(
            matrix
                .risk_dimension_coverages
                .iter()
                .flat_map(|coverage| coverage.blockers.iter().cloned()),
        )
        .collect::<Vec<_>>();
    for finding in &matrix.regex_adversarial_findings {
        risk_blockers.push(format!(
            "regex adversarial class `{}` role {:?} is invalid: {}",
            finding.class_id, finding.role, finding.issue
        ));
    }
    write_json(
        &parent.join("risk-coverage.json"),
        &RiskCoverageArtifact {
            schema_version: 3,
            required_case_roles: REQUIRED_CASE_ROLES.to_vec(),
            dimension_coverages: matrix.risk_dimension_coverages.clone(),
            surface_coverages: matrix.surface_coverages.clone(),
            family_coverages: matrix.risk_family_coverages.clone(),
            regex_adversarial_coverages: matrix.regex_adversarial_coverages.clone(),
            regex_adversarial_findings: matrix.regex_adversarial_findings.clone(),
            blockers: risk_blockers,
        },
    );
    for (suite, artifact) in [
        ("unit", "unit-suite.json"),
        ("adversarial", "adversarial-suite.json"),
        ("property", "property-suite.json"),
        ("conformance", "conformance-suite.json"),
        ("corpus", "corpus-suite.json"),
        ("benchmark", "benchmark-suite.json"),
        ("gap", "gap-suite.json"),
        ("fuzz", "fuzz-suite.json"),
    ] {
        write_suite_artifact(parent, suite, artifact, files);
    }
    write_json(
        &parent.join("release-surface-suite-coverage.json"),
        &SurfaceCoverageArtifact {
            schema_version: 1,
            surfaces: matrix.surface_coverages.clone(),
            blockers: matrix
                .surface_coverages
                .iter()
                .flat_map(|surface| surface.blockers.iter().cloned())
                .collect(),
        },
    );
}

fn write_suite_artifact(parent: &Path, suite: &str, artifact: &str, files: &[TestFileRecord]) {
    let suite_files = files
        .iter()
        .filter(|file| file.layers.iter().any(|layer| layer == suite))
        .cloned()
        .collect::<Vec<_>>();
    let blockers = if suite_files.is_empty() {
        vec![format!("test suite `{suite}` has zero files")]
    } else {
        let mut blockers = Vec::new();
        let vyre_file_count = suite_files
            .iter()
            .filter(|file| {
                !file.path.contains("/libs/dataflow/weir/") && !file.path.contains("/tools/vyrec/")
            })
            .count();
        let dataflow_consumer_file_count = suite_files
            .iter()
            .filter(|file| file.path.contains("/libs/dataflow/weir/"))
            .count();
        let vyrec_file_count = suite_files
            .iter()
            .filter(|file| file.path.contains("/tools/vyrec/"))
            .count();
        if vyre_file_count == 0 {
            blockers.push(format!("test suite `{suite}` has zero Vyre-side files"));
        }
        if dataflow_consumer_file_count == 0 {
            blockers.push(format!("test suite `{suite}` has zero Weir-side files"));
        }
        if vyrec_file_count == 0 {
            blockers.push(format!(
                "test suite `{suite}` has zero tools/vyrec-side files"
            ));
        }
        let asserted_files = suite_files
            .iter()
            .filter(|file| {
                file.assertion_count > 0 || file.layers.iter().any(|layer| layer == "benchmark")
            })
            .count();
        if asserted_files == 0 {
            blockers.push(format!(
                "test suite `{suite}` has no files with assertions or benchmark bodies"
            ));
        }
        let entrypoint_files = suite_files
            .iter()
            .filter(|file| {
                file.has_test_entrypoint || file.layers.iter().any(|layer| layer == "benchmark")
            })
            .count();
        if entrypoint_files == 0 {
            blockers.push(format!(
                "test suite `{suite}` has no #[test], proptest!, criterion, or bench entrypoints"
            ));
        }
        blockers
    };
    let vyre_file_count = suite_files
        .iter()
        .filter(|file| {
            !file.path.contains("/libs/dataflow/weir/") && !file.path.contains("/tools/vyrec/")
        })
        .count();
    let dataflow_consumer_file_count = suite_files
        .iter()
        .filter(|file| file.path.contains("/libs/dataflow/weir/"))
        .count();
    let vyrec_file_count = suite_files
        .iter()
        .filter(|file| file.path.contains("/tools/vyrec/"))
        .count();
    write_json(
        &parent.join(artifact),
        &SuiteEvidence {
            schema_version: 1,
            suite: suite.to_string(),
            file_count: suite_files.len(),
            vyre_file_count,
            dataflow_consumer_file_count,
            vyrec_file_count,
            files: suite_files,
            blockers,
        },
    );
}

fn regex_adversarial_coverages(
    vyre_root: &Path,
) -> (Vec<RegexAdversarialCoverage>, Vec<RegexAdversarialFinding>) {
    let mut findings = Vec::new();
    let catalog_path = vyre_root.join(REGEX_ADVERSARIAL_CLASS_CATALOG);
    let text = match read_text_bounded(&catalog_path) {
        Ok(text) => text,
        Err(error) => {
            findings.push(RegexAdversarialFinding {
                class_id: "<catalog>".to_string(),
                role: None,
                issue: format!(
                    "could not read `{REGEX_ADVERSARIAL_CLASS_CATALOG}`: {error}. Fix: keep regex adversarial classes in the canonical catalog."
                ),
            });
            return (empty_regex_adversarial_coverages(), findings);
        }
    };
    let catalog = match toml::from_str::<RegexAdversarialCatalog>(&text) {
        Ok(catalog) => catalog,
        Err(error) => {
            findings.push(RegexAdversarialFinding {
                class_id: "<catalog>".to_string(),
                role: None,
                issue: format!(
                    "could not parse `{REGEX_ADVERSARIAL_CLASS_CATALOG}`: {error}. Fix: use [[case]] rows with class_id, role, and evidence_path."
                ),
            });
            return (empty_regex_adversarial_coverages(), findings);
        }
    };
    if catalog.schema_version != 1 {
        findings.push(RegexAdversarialFinding {
            class_id: "<catalog>".to_string(),
            role: None,
            issue: format!(
                "schema_version {} is unsupported; expected 1",
                catalog.schema_version
            ),
        });
    }

    let mut roles_by_class: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut paths_by_class: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut seen_cases = BTreeSet::new();
    for case in &catalog.case {
        let class_id = case.class_id.trim();
        let role = case.role.trim();
        let evidence_path = case.evidence_path.trim();
        if !REQUIRED_REGEX_ADVERSARIAL_CLASSES.contains(&class_id) {
            findings.push(RegexAdversarialFinding {
                class_id: case.class_id.clone(),
                role: Some(case.role.clone()),
                issue: "unknown regex adversarial class. Fix: use the canonical class ids."
                    .to_string(),
            });
            continue;
        }
        if !REQUIRED_REGEX_ADVERSARIAL_ROLES.contains(&role) {
            findings.push(RegexAdversarialFinding {
                class_id: case.class_id.clone(),
                role: Some(case.role.clone()),
                issue: "unknown regex adversarial role. Fix: use positive, negative, boundary, adversarial, baseline, evasion, or resource_budget."
                    .to_string(),
            });
            continue;
        }
        if evidence_path.is_empty() {
            findings.push(RegexAdversarialFinding {
                class_id: case.class_id.clone(),
                role: Some(case.role.clone()),
                issue: "missing evidence_path. Fix: point at the test file carrying this class/role."
                    .to_string(),
            });
            continue;
        }
        if !vyre_root.join(evidence_path).is_file() {
            findings.push(RegexAdversarialFinding {
                class_id: case.class_id.clone(),
                role: Some(case.role.clone()),
                issue: format!(
                    "evidence_path `{evidence_path}` does not exist. Fix: point at a committed scan test file."
                ),
            });
        }
        let key = format!("{class_id}\n{role}");
        if !seen_cases.insert(key) {
            findings.push(RegexAdversarialFinding {
                class_id: case.class_id.clone(),
                role: Some(case.role.clone()),
                issue: "duplicate class/role row. Fix: keep one canonical row per regex class role."
                    .to_string(),
            });
        }
        roles_by_class
            .entry(class_id.to_string())
            .or_default()
            .insert(role.to_string());
        paths_by_class
            .entry(class_id.to_string())
            .or_default()
            .insert(evidence_path.to_string());
    }

    let mut coverages = Vec::new();
    for class_id in REQUIRED_REGEX_ADVERSARIAL_CLASSES {
        let roles = roles_by_class
            .get(*class_id)
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .collect::<Vec<_>>();
        let missing_roles = REQUIRED_REGEX_ADVERSARIAL_ROLES
            .iter()
            .copied()
            .filter(|role| !roles.iter().any(|candidate| candidate.as_str() == *role))
            .collect::<Vec<_>>();
        for role in &missing_roles {
            findings.push(RegexAdversarialFinding {
                class_id: (*class_id).to_string(),
                role: Some((*role).to_string()),
                issue: "missing required regex adversarial role coverage".to_string(),
            });
        }
        coverages.push(RegexAdversarialCoverage {
            class_id: *class_id,
            roles,
            required_roles: REQUIRED_REGEX_ADVERSARIAL_ROLES.to_vec(),
            missing_roles,
            evidence_paths: paths_by_class
                .get(*class_id)
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .collect(),
        });
    }
    (coverages, findings)
}

fn empty_regex_adversarial_coverages() -> Vec<RegexAdversarialCoverage> {
    REQUIRED_REGEX_ADVERSARIAL_CLASSES
        .iter()
        .map(|class_id| RegexAdversarialCoverage {
            class_id: *class_id,
            roles: Vec::new(),
            required_roles: REQUIRED_REGEX_ADVERSARIAL_ROLES.to_vec(),
            missing_roles: REQUIRED_REGEX_ADVERSARIAL_ROLES.to_vec(),
            evidence_paths: Vec::new(),
        })
        .collect()
}

fn release_surface_coverages(files: &[TestFileRecord]) -> Vec<SurfaceCoverage> {
    RELEASE_SURFACES
        .iter()
        .map(|&(surface, required_layers)| {
            let surface_files = files
                .iter()
                .filter(|file| file_belongs_to_surface(&file.path, surface))
                .collect::<Vec<_>>();
            let mut layers = BTreeSet::new();
            let mut case_roles = BTreeSet::new();
            let mut assertion_count = 0usize;
            let mut entrypoint_count = 0usize;
            for file in &surface_files {
                assertion_count += file.assertion_count;
                if file.has_test_entrypoint
                    || file.layers.iter().any(|layer| layer == "benchmark")
                {
                    entrypoint_count += 1;
                }
                for layer in &file.layers {
                    layers.insert(layer.as_str());
                }
                for role in &file.case_roles {
                    case_roles.insert(role.as_str());
                }
            }
            let missing_layers = required_layers
                .iter()
                .copied()
                .filter(|layer| !layers.contains(layer))
                .collect::<Vec<_>>();
            let missing_case_roles = REQUIRED_CASE_ROLES
                .iter()
                .copied()
                .filter(|role| !case_roles.contains(role))
                .collect::<Vec<_>>();
            let mut blockers = Vec::new();
            if surface_files.is_empty() {
                blockers.push(format!("release surface `{surface}` has zero test files"));
            }
            if assertion_count == 0 {
                blockers.push(format!(
                    "release surface `{surface}` has no assertions across its test files"
                ));
            }
            if entrypoint_count == 0 {
                blockers.push(format!(
                    "release surface `{surface}` has no executable test, proptest, fuzz, or benchmark entrypoints"
                ));
            }
            for layer in &missing_layers {
                blockers.push(format!(
                    "release surface `{surface}` is missing required `{layer}` test coverage"
                ));
            }
            for role in &missing_case_roles {
                blockers.push(format!(
                    "release surface `{surface}` is missing required `{role}` case-role evidence"
                ));
            }
            SurfaceCoverage {
                surface,
                file_count: surface_files.len(),
                assertion_count,
                entrypoint_count,
                layers: layers.into_iter().map(String::from).collect(),
                required_layers: required_layers.to_vec(),
                missing_layers,
                case_roles: case_roles.into_iter().map(String::from).collect(),
                required_case_roles: REQUIRED_CASE_ROLES.to_vec(),
                missing_case_roles,
                blockers,
            }
        })
        .collect()
}

fn file_belongs_to_surface(path: &str, surface: &str) -> bool {
    match surface {
        "weir" => path.contains("/libs/dataflow/weir/"),
        "vyrec" => path.contains("/tools/vyrec/"),
        "vyre" => !path.contains("/libs/dataflow/weir/") && !path.contains("/tools/vyrec/"),
        _ => false,
    }
}

fn modularity_findings(files: &[TestFileRecord]) -> Vec<ModularityFinding> {
    let mut findings = Vec::new();
    for file in files {
        if file.oversized {
            findings.push(ModularityFinding {
                path: file.path.clone(),
                surface: surface_for_path(&file.path),
                primary_layer: primary_modularity_layer(&file.layers),
                finding_kind: "oversized_file",
                lines: file.lines,
                lines_over_threshold: file.lines.saturating_sub(OVERSIZED_TEST_THRESHOLD_LINES),
                recommended_split: modularity_split_targets(file),
                release_blocker: true,
            });
        }
        if file.god_test_candidate {
            findings.push(ModularityFinding {
                path: file.path.clone(),
                surface: surface_for_path(&file.path),
                primary_layer: primary_modularity_layer(&file.layers),
                finding_kind: "monolithic_split_required",
                lines: file.lines,
                lines_over_threshold: file.lines.saturating_sub(OVERSIZED_TEST_THRESHOLD_LINES),
                recommended_split: modularity_split_targets(file),
                release_blocker: true,
            });
        }
    }
    findings.sort_by(|left, right| {
        left.surface
            .cmp(right.surface)
            .then_with(|| left.primary_layer.cmp(&right.primary_layer))
            .then_with(|| left.finding_kind.cmp(right.finding_kind))
            .then_with(|| right.lines_over_threshold.cmp(&left.lines_over_threshold))
            .then_with(|| left.path.cmp(&right.path))
    });
    findings
}

fn modularity_summary(findings: &[ModularityFinding]) -> Vec<ModularitySummary> {
    let mut groups: BTreeMap<(&'static str, String, &'static str, String), (usize, usize, usize)> =
        BTreeMap::new();
    for finding in findings {
        for split in &finding.recommended_split {
            let entry = groups
                .entry((
                    finding.surface,
                    finding.primary_layer.clone(),
                    finding.finding_kind,
                    split.clone(),
                ))
                .or_insert((0, 0, 0));
            entry.0 += 1;
            if finding.release_blocker {
                entry.1 += 1;
            }
            entry.2 = entry.2.max(finding.lines_over_threshold);
        }
    }
    groups
        .into_iter()
        .map(
            |(
                (surface, primary_layer, finding_kind, recommended_split),
                (finding_count, release_blocker_count, max_lines_over_threshold),
            )| ModularitySummary {
                surface,
                primary_layer,
                finding_kind,
                recommended_split,
                finding_count,
                release_blocker_count,
                max_lines_over_threshold,
            },
        )
        .collect()
}

fn surface_for_path(path: &str) -> &'static str {
    RELEASE_SURFACES
        .iter()
        .map(|&(surface, _)| surface)
        .find(|surface| file_belongs_to_surface(path, surface))
        .unwrap_or("unknown")
}

fn primary_modularity_layer(layers: &[String]) -> String {
    for priority in [
        "conformance",
        "property",
        "adversarial",
        "corpus",
        "fuzz",
        "benchmark",
        "gap",
        "integration",
        "unit",
    ] {
        if layers.iter().any(|layer| layer == priority) {
            return priority.to_string();
        }
    }
    layers
        .first()
        .cloned()
        .unwrap_or_else(|| "unknown".to_string())
}

fn modularity_split_targets(file: &TestFileRecord) -> Vec<String> {
    if !file.recommended_split.is_empty() {
        return file.recommended_split.clone();
    }
    vec!["split by API contract into tests/contracts/ and tests/regression/".to_string()]
}

fn risk_family_coverages(files: &[TestFileRecord]) -> Vec<RiskFamilyCoverage> {
    #[derive(Default)]
    struct Bucket {
        file_count: usize,
        assertion_count: usize,
        case_roles: BTreeSet<String>,
    }

    let mut buckets: BTreeMap<(&'static str, &'static str, String), Bucket> = BTreeMap::new();
    for file in files {
        let surface = surface_for_path(&file.path);
        for (dimension, families) in [
            ("op", &file.op_families),
            ("backend", &file.backend_families),
            ("feature", &file.feature_families),
            ("error_path", &file.error_path_families),
            ("corpus", &file.corpus_families),
            ("weir_flow", &file.weir_flow_families),
        ] {
            for family in families {
                let bucket = buckets
                    .entry((surface, dimension, family.clone()))
                    .or_default();
                bucket.file_count += 1;
                bucket.assertion_count += file.assertion_count;
                for role in &file.case_roles {
                    bucket.case_roles.insert(role.clone());
                }
            }
        }
    }
    buckets
        .into_iter()
        .map(
            |((surface, dimension, family), bucket)| RiskFamilyCoverage {
                surface,
                dimension,
                risk_weight: risk_family_weight(dimension, &family),
                family,
                file_count: bucket.file_count,
                assertion_count: bucket.assertion_count,
                case_roles: {
                    let roles = bucket.case_roles.into_iter().collect::<Vec<_>>();
                    roles
                },
                required_case_roles: REQUIRED_CASE_ROLES.to_vec(),
                missing_case_roles: Vec::new(),
            },
        )
        .map(|mut coverage| {
            coverage.missing_case_roles = missing_case_roles(&coverage.case_roles);
            coverage
        })
        .collect()
}

fn risk_dimension_coverages(families: &[RiskFamilyCoverage]) -> Vec<RiskDimensionCoverage> {
    #[derive(Default)]
    struct Bucket {
        families: BTreeSet<String>,
        file_count: usize,
        assertion_count: usize,
        case_roles: BTreeSet<String>,
    }

    let mut buckets: BTreeMap<(&'static str, &'static str), Bucket> = BTreeMap::new();
    for family in families {
        let bucket = buckets
            .entry((family.surface, family.dimension))
            .or_default();
        bucket.families.insert(family.family.clone());
        bucket.file_count += family.file_count;
        bucket.assertion_count += family.assertion_count;
        for role in &family.case_roles {
            bucket.case_roles.insert(role.clone());
        }
    }
    for &(surface, _) in RELEASE_SURFACES {
        for dimension in REQUIRED_RISK_DIMENSIONS {
            buckets.entry((surface, *dimension)).or_default();
        }
    }
    buckets
        .into_iter()
        .map(|((surface, dimension), bucket)| {
            let case_roles = bucket.case_roles.into_iter().collect::<Vec<_>>();
            let missing_case_roles = missing_case_roles(&case_roles);
            let mut blockers = Vec::new();
            if bucket.families.is_empty() {
                blockers.push(format!(
                    "release surface `{surface}` risk dimension `{dimension}` has zero family evidence"
                ));
            }
            if bucket.assertion_count == 0 {
                blockers.push(format!(
                    "release surface `{surface}` risk dimension `{dimension}` has no assertion evidence"
                ));
            }
            for role in &missing_case_roles {
                blockers.push(format!(
                    "release surface `{surface}` risk dimension `{dimension}` is missing required `{role}` case-role evidence"
                ));
            }
            RiskDimensionCoverage {
                surface,
                dimension,
                family_count: bucket.families.len(),
                file_count: bucket.file_count,
                assertion_count: bucket.assertion_count,
                case_roles,
                required_case_roles: REQUIRED_CASE_ROLES.to_vec(),
                missing_case_roles,
                blockers,
            }
        })
        .collect()
}

fn missing_case_roles(case_roles: &[String]) -> Vec<&'static str> {
    REQUIRED_CASE_ROLES
        .iter()
        .copied()
        .filter(|role| !case_roles.iter().any(|candidate| candidate == role))
        .collect()
}

fn risk_family_weight(dimension: &str, family: &str) -> u8 {
    match (dimension, family) {
        ("backend", "cuda_ptx" | "metal" | "wgpu" | "resident") => 5,
        ("op", "flow" | "scan_static" | "parser_frontend" | "nn_math") => 5,
        ("feature", "validation" | "soundness" | "capability") => 5,
        ("error_path", "overflow" | "malformed_input" | "unsupported_capability") => 5,
        ("corpus", "release" | "fuzz" | "differential") => 5,
        ("weir_flow", family) if family != "non_weir" => 5,
        (_, family) if family.contains("uncategorized") => 1,
        _ => 3,
    }
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

fn collect_modular_dirs(
    surface: &'static str,
    root: &Path,
    modular_directories: &mut Vec<ModularDirectory>,
) {
    for &(layer, relative) in REQUIRED_MODULAR_DIRS {
        let path = root.join(relative);
        modular_directories.push(ModularDirectory {
            surface,
            layer,
            path: path.display().to_string(),
            exists: path.is_dir(),
        });
    }
}

fn scan_tests(
    root: &Path,
    test_files: &mut usize,
    layers: &mut BTreeSet<&'static str>,
    oversized_files: &mut Vec<OversizedFile>,
    file_records: &mut Vec<TestFileRecord>,
    blockers: &mut Vec<String>,
) {
    for entry in WalkDir::new(root).into_iter().filter_entry(|entry| {
        let name = entry.file_name().to_string_lossy();
        !matches!(
            name.as_ref(),
            "target" | "target-codex" | "target_tests" | ".git" | ".cargo-target" | "release"
        )
    }) {
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) => {
                blockers.push(format!(
                    "failed to walk test evidence root `{}`: {error}",
                    error
                        .path()
                        .map(|path| path.display().to_string())
                        .unwrap_or_else(|| root.display().to_string())
                ));
                continue;
            }
        };
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("rs") {
            continue;
        }
        let path_string = path.display().to_string();
        let text = match read_text_bounded(path) {
            Ok(text) => text,
            Err(error) => {
                blockers.push(format!(
                    "failed to read test evidence file `{}`: {error}",
                    path.display()
                ));
                continue;
            }
        };
        let test_file_kind = classify_test_file_kind(&path_string, &text);
        if !test_file_kind.is_test_file {
            continue;
        }
        *test_files += 1;
        let lines = text.lines().count();
        let file_layers = classify_file_layers(&path_string, &text);
        let case_roles = classify_case_roles(&path_string, &text, &file_layers);
        let risk_families = classify_risk_families(&path_string, &text);
        for layer in &file_layers {
            layers.insert(*layer);
        }
        let line_threshold_exceeded = lines > OVERSIZED_TEST_THRESHOLD_LINES;
        let oversized = test_file_kind.dedicated_test_file && line_threshold_exceeded;
        let recommended_split = recommended_split(&path_string, &file_layers, lines);
        let god_test_candidate =
            test_file_kind.dedicated_test_file && (oversized || path_string.ends_with("/tests.rs"));
        if oversized {
            oversized_files.push(OversizedFile {
                path: path_string.clone(),
                lines,
                lines_over_threshold: lines - OVERSIZED_TEST_THRESHOLD_LINES,
                recommended_split: recommended_split.clone(),
                release_blocker: true,
            });
        }
        file_records.push(TestFileRecord {
            path: path_string,
            layers: file_layers.into_iter().map(String::from).collect(),
            lines,
            dedicated_test_file: test_file_kind.dedicated_test_file,
            inline_test_module_file: test_file_kind.inline_test_module_file,
            line_threshold_exceeded,
            has_test_entrypoint: test_file_kind.has_test_entrypoint,
            assertion_count: assertion_count(&text),
            oversized,
            god_test_candidate,
            recommended_split,
            case_roles,
            op_families: risk_families.op_families,
            backend_families: risk_families.backend_families,
            feature_families: risk_families.feature_families,
            error_path_families: risk_families.error_path_families,
            corpus_families: risk_families.corpus_families,
            weir_flow_families: risk_families.weir_flow_families,
        });
    }
}

fn classify_test_file_kind(path: &str, text: &str) -> TestFileKind {
    let has_test_entrypoint = has_test_entrypoint(text);
    let support_only_module = is_test_support_module(path) && !has_test_entrypoint;
    let dedicated_test_file = is_dedicated_test_evidence_file(path) && !support_only_module;
    let inline_test_module_file = !dedicated_test_file && has_test_entrypoint;
    let is_test_file = dedicated_test_file || inline_test_module_file;
    TestFileKind {
        dedicated_test_file,
        inline_test_module_file,
        has_test_entrypoint,
        is_test_file,
    }
}

fn is_dedicated_test_evidence_file(path: &str) -> bool {
    path.contains("/tests/")
        || path.contains("/benches/")
        || path.contains("/fuzz/fuzz_targets/")
        || path.ends_with("/tests.rs")
        || path.ends_with("_tests.rs")
        || path.ends_with("_test.rs")
        || path.contains("_tests_")
        || path.contains("_test_")
}

fn is_test_support_module(path: &str) -> bool {
    path.contains("/tests/common/")
        || path.contains("/tests/support/")
        || path.ends_with("/tests/common.rs")
        || path.ends_with("/tests/support.rs")
}

fn recommended_split(path: &str, layers: &BTreeSet<&'static str>, lines: usize) -> Vec<String> {
    if lines <= OVERSIZED_TEST_THRESHOLD_LINES && !path.ends_with("/tests.rs") {
        return Vec::new();
    }
    let mut splits = Vec::new();
    if path.ends_with("/tests.rs") {
        splits.push(
            "move monolithic src/tests.rs coverage into focused tests/<domain>/ files".to_string(),
        );
    }
    for layer in layers {
        match *layer {
            "property" => {
                splits.push("extract property invariants into tests/properties/".to_string())
            }
            "adversarial" => {
                splits.push("extract hostile-input cases into tests/adversarial/".to_string())
            }
            "corpus" => splits.push("extract fixture-driven cases into tests/corpus/".to_string()),
            "conformance" => {
                splits.push("extract backend/op parity cases into tests/conformance/".to_string())
            }
            "benchmark" => splits.push(
                "move timing-only checks into benches/ or release benchmark cases".to_string(),
            ),
            "gap" => splits.push("extract expected-failure coverage into tests/gap/".to_string()),
            _ => {}
        }
    }
    if splits.is_empty() {
        splits
            .push("split by API contract into tests/contracts/ and tests/regression/".to_string());
    }
    splits.sort();
    splits.dedup();
    splits
}

fn has_test_entrypoint(text: &str) -> bool {
    text.contains("#[test]")
        || text.contains("#[tokio::test]")
        || text.contains("proptest!")
        || text.contains("criterion_group!")
        || text.contains("fuzz_target!")
        || text.contains("#[bench]")
}

fn assertion_count(text: &str) -> usize {
    [
        "assert!(",
        "assert_eq!(",
        "assert_ne!(",
        "prop_assert!(",
        "prop_assert_eq!(",
    ]
    .iter()
    .map(|needle| text.matches(needle).count())
    .sum()
}

fn classify_file_layers(path: &str, text: &str) -> BTreeSet<&'static str> {
    let mut layers = BTreeSet::new();
    let lowered = text.to_ascii_lowercase();
    layers.insert("unit");
    if path.contains("/tests/") {
        layers.insert("integration");
    }
    if path.contains("/benches/")
        || path.contains("bench")
        || path.contains("perf")
        || lowered.contains("criterion_group!")
        || lowered.contains("#[bench]")
        || lowered.contains("benchmark")
    {
        layers.insert("benchmark");
    }
    if path.contains("property") || path.contains("proptest") || lowered.contains("proptest!") {
        layers.insert("property");
    }
    if path.contains("adversarial")
        || path.contains("malformed")
        || path.contains("hostile")
        || lowered.contains("hostile")
        || lowered.contains("malformed")
        || lowered.contains("fail closed")
    {
        layers.insert("adversarial");
    }
    if path.contains("corpus")
        || path.contains("linux")
        || lowered.contains("corpus")
        || lowered.contains("linux subsystem")
    {
        layers.insert("corpus");
    }
    if path.contains("conform")
        || path.contains("parity")
        || path.contains("cross_backend")
        || lowered.contains("conformance")
        || lowered.contains("parity")
        || lowered.contains("frontend api handoff")
    {
        layers.insert("conformance");
    }
    if path.contains("gap")
        || path.contains("blocker")
        || lowered.contains("gap contract")
        || lowered.contains("missing source")
    {
        layers.insert("gap");
    }
    if path.contains("fuzz") || lowered.contains("fuzz") || lowered.contains("hostile_arg") {
        layers.insert("fuzz");
    }
    layers
}

fn classify_case_roles(path: &str, text: &str, layers: &BTreeSet<&'static str>) -> Vec<String> {
    let mut roles = BTreeSet::new();
    let lowered = lower_path_text(path, text);
    if text.contains("assert!(")
        || text.contains("assert_eq!(")
        || text.contains("assert_ne!(")
        || lowered.contains(" ok")
        || lowered.contains("success")
        || lowered.contains("valid")
        || lowered.contains("accept")
        || lowered.contains("parity")
        || lowered.contains("golden")
    {
        roles.insert("positive");
    }
    if lowered.contains("err")
        || lowered.contains("error")
        || lowered.contains("reject")
        || lowered.contains("invalid")
        || lowered.contains("unsupported")
        || lowered.contains("fail")
        || lowered.contains("panic")
        || lowered.contains("malformed")
    {
        roles.insert("negative");
    }
    if lowered.contains("boundary")
        || lowered.contains("overflow")
        || lowered.contains("underflow")
        || lowered.contains("zero")
        || lowered.contains("empty")
        || lowered.contains("max")
        || lowered.contains("min")
        || lowered.contains("limit")
        || lowered.contains("cap")
    {
        roles.insert("boundary");
    }
    for layer_role in [
        "adversarial",
        "property",
        "fuzz",
        "benchmark",
        "conformance",
    ] {
        if layers.contains(layer_role) {
            roles.insert(layer_role);
        }
    }
    if path.contains("/tests/e2e/")
        || path.contains("e2e")
        || path.contains("end_to_end")
        || path.contains("end-to-end")
        || lowered.contains("end_to_end")
        || lowered.contains("end-to-end")
        || lowered.contains("e2e")
    {
        roles.insert("e2e");
    }
    if roles.is_empty() {
        roles.insert("uncategorized");
    }
    roles.into_iter().map(String::from).collect()
}

fn classify_risk_families(path: &str, text: &str) -> RiskFamilies {
    let lowered = lower_path_text(path, text);
    RiskFamilies {
        op_families: classify_op_families(&lowered),
        backend_families: classify_backend_families(&lowered),
        feature_families: classify_feature_families(&lowered),
        error_path_families: classify_error_path_families(&lowered),
        corpus_families: classify_corpus_families(&lowered),
        weir_flow_families: classify_weir_flow_families(path, &lowered),
    }
}

fn lower_path_text(path: &str, text: &str) -> String {
    format!("{path}\n{text}").to_ascii_lowercase()
}

fn classify_op_families(lowered: &str) -> Vec<String> {
    let mut families = BTreeSet::new();
    push_family(
        &mut families,
        lowered,
        &["scan", "regex", "literal", "aho", "hyperscan"],
        "scan_static",
    );
    push_family(
        &mut families,
        lowered,
        &["parser", "parse", "preprocess", "vast", "tree_sitter"],
        "parser_frontend",
    );
    push_family(
        &mut families,
        lowered,
        &["lower", "emit", "naga", "ptx", "spirv", "wgsl", "msl"],
        "lower_emit",
    );
    push_family(
        &mut families,
        lowered,
        &["driver", "cuda", "wgpu", "metal", "resident", "dispatch"],
        "driver_runtime",
    );
    push_family(
        &mut families,
        lowered,
        &["optimizer", "eqsat", "loop", "const_fold", "rewrite"],
        "optimizer",
    );
    push_family(
        &mut families,
        lowered,
        &["dataflow", "ifds", "fixed_point", "reachability", "witness"],
        "flow",
    );
    push_family(
        &mut families,
        lowered,
        &["matmul", "attention", "linear", "tensor", "nn", "softmax"],
        "nn_math",
    );
    if families.is_empty() {
        families.insert("uncategorized_op");
    }
    families.into_iter().map(String::from).collect()
}

fn classify_backend_families(lowered: &str) -> Vec<String> {
    let mut families = BTreeSet::new();
    push_family(&mut families, lowered, &["cuda", "ptx"], "cuda_ptx");
    push_family(&mut families, lowered, &["metal", "msl"], "metal");
    push_family(&mut families, lowered, &["wgpu", "wgsl"], "wgpu");
    push_family(&mut families, lowered, &["spirv", "spv"], "spirv");
    push_family(&mut families, lowered, &["naga"], "naga");
    push_family(
        &mut families,
        lowered,
        &["cpu", "reference", "oracle", "host"],
        "cpu_oracle",
    );
    push_family(&mut families, lowered, &["resident"], "resident");
    push_family(&mut families, lowered, &["direct"], "direct");
    if families.is_empty() {
        families.insert("backend_agnostic");
    }
    families.into_iter().map(String::from).collect()
}

fn classify_feature_families(lowered: &str) -> Vec<String> {
    let mut families = BTreeSet::new();
    push_family(&mut families, lowered, &["serde", "json"], "serde");
    push_family(
        &mut families,
        lowered,
        &["simd", "vector", "avx", "neon"],
        "simd",
    );
    push_family(
        &mut families,
        lowered,
        &["zero_copy", "borrowed", "scratch", "allocation"],
        "allocation_scratch",
    );
    push_family(
        &mut families,
        lowered,
        &["graph_capture", "capture", "replay"],
        "graph_capture",
    );
    push_family(&mut families, lowered, &["cache", "cached"], "cache");
    push_family(
        &mut families,
        lowered,
        &["validate", "validation", "certificate"],
        "validation",
    );
    push_family(
        &mut families,
        lowered,
        &["soundness", "mayover", "mustunder", "exact"],
        "soundness",
    );
    push_family(
        &mut families,
        lowered,
        &["capability", "unsupported", "feature"],
        "capability",
    );
    if families.is_empty() {
        families.insert("core_feature");
    }
    families.into_iter().map(String::from).collect()
}

fn classify_error_path_families(lowered: &str) -> Vec<String> {
    let mut families = BTreeSet::new();
    push_family(
        &mut families,
        lowered,
        &["overflow", "underflow"],
        "overflow",
    );
    push_family(
        &mut families,
        lowered,
        &["malformed", "invalid", "corrupt"],
        "malformed_input",
    );
    push_family(
        &mut families,
        lowered,
        &["unsupported", "capability"],
        "unsupported_capability",
    );
    push_family(&mut families, lowered, &["panic"], "panic_path");
    push_family(&mut families, lowered, &["timeout", "deadline"], "timeout");
    push_family(
        &mut families,
        lowered,
        &["oom", "resource", "budget", "limit"],
        "resource_budget",
    );
    push_family(&mut families, lowered, &["empty", "zero"], "empty_zero");
    push_family(
        &mut families,
        lowered,
        &["missing", "absent"],
        "missing_input",
    );
    if families.is_empty() {
        families.insert("nominal");
    }
    families.into_iter().map(String::from).collect()
}

fn classify_corpus_families(lowered: &str) -> Vec<String> {
    let mut families = BTreeSet::new();
    push_family(&mut families, lowered, &["linux"], "linux");
    push_family(&mut families, lowered, &["r2", "radare"], "r2");
    push_family(&mut families, lowered, &["csmith"], "csmith");
    push_family(&mut families, lowered, &["fixture", "fixtures"], "fixture");
    push_family(
        &mut families,
        lowered,
        &["generated", "generator"],
        "generated",
    );
    push_family(&mut families, lowered, &["release"], "release");
    push_family(&mut families, lowered, &["fuzz"], "fuzz");
    push_family(&mut families, lowered, &["golden"], "golden");
    push_family(
        &mut families,
        lowered,
        &["differential", "parity"],
        "differential",
    );
    if families.is_empty() {
        families.insert("unit_fixture");
    }
    families.into_iter().map(String::from).collect()
}

fn classify_weir_flow_families(path: &str, lowered: &str) -> Vec<String> {
    let mut families = BTreeSet::new();
    push_family(
        &mut families,
        lowered,
        &["fixed_point", "fixed-point", "closure"],
        "fixed_point",
    );
    push_family(&mut families, lowered, &["ifds"], "ifds");
    push_family(&mut families, lowered, &["witness"], "witness");
    push_family(
        &mut families,
        lowered,
        &["cross_language", "cross-language"],
        "cross_language",
    );
    push_family(
        &mut families,
        lowered,
        &["points_to", "points-to"],
        "points_to",
    );
    push_family(&mut families, lowered, &["live", "liveness"], "liveness");
    push_family(&mut families, lowered, &["reaching"], "reaching");
    push_family(&mut families, lowered, &["slice"], "slice");
    push_family(
        &mut families,
        lowered,
        &["dispatch_decode", "dispatch-decode"],
        "dispatch_decode",
    );
    push_family(&mut families, lowered, &["resident"], "resident_flow");
    if families.is_empty() {
        if path.contains("/libs/dataflow/weir/") {
            families.insert("weir_uncategorized");
        } else {
            families.insert("non_weir");
        }
    }
    families.into_iter().map(String::from).collect()
}

fn push_family(
    families: &mut BTreeSet<&'static str>,
    haystack: &str,
    needles: &[&str],
    family: &'static str,
) {
    if needles.iter().any(|needle| haystack.contains(needle)) {
        families.insert(family);
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
                    "USAGE:\n  cargo_full run --bin xtask -- test-matrix [--output PATH]\n\n\
                     Writes Vyre/Weir test architecture evidence."
                );
                std::process::exit(0);
            }
            other => return Err(format!("Fix: unknown test-matrix option `{other}`.")),
        }
    }
    Ok(output.unwrap_or_else(default_output))
}

fn default_output() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(|path| path.join("release/evidence/tests/test-matrix.json"))
        .unwrap_or_else(|| PathBuf::from("release/evidence/tests/test-matrix.json"))
}

fn read_text_bounded(path: &Path) -> io::Result<String> {
    let mut reader = fs::File::open(path)?.take(MAX_TEST_SOURCE_BYTES.saturating_add(1));
    let mut text = String::new();
    reader.read_to_string(&mut text)?;
    if text.len() as u64 > MAX_TEST_SOURCE_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "{} exceeds {MAX_TEST_SOURCE_BYTES} byte release test-source read cap",
                path.display()
            ),
        ));
    }
    Ok(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn support_helper_without_entrypoint_is_not_test_evidence() {
        let kind = classify_test_file_kind(
            "/repo/vyre-driver-cuda/tests/common/mod.rs",
            "pub fn make_fixture() -> usize { 1 }",
        );

        assert_eq!(
            kind,
            TestFileKind {
                dedicated_test_file: false,
                inline_test_module_file: false,
                has_test_entrypoint: false,
                is_test_file: false,
            }
        );
    }

    #[test]
    fn support_helper_with_entrypoint_remains_test_evidence() {
        let kind = classify_test_file_kind(
            "/repo/vyre-frontend-c/tests/support/ast_oracle.rs",
            "#[test]\nfn parses_fixture() { assert!(true); }",
        );

        assert_eq!(
            kind,
            TestFileKind {
                dedicated_test_file: true,
                inline_test_module_file: false,
                has_test_entrypoint: true,
                is_test_file: true,
            }
        );
    }

    #[test]
    fn inline_source_test_module_is_counted_without_dedicated_path() {
        let kind = classify_test_file_kind(
            "/repo/vyre-core/src/lib.rs",
            "#[cfg(test)] mod tests { #[test] fn accepts_contract() {} }",
        );

        assert_eq!(
            kind,
            TestFileKind {
                dedicated_test_file: false,
                inline_test_module_file: true,
                has_test_entrypoint: true,
                is_test_file: true,
            }
        );
    }

    #[test]
    fn modularity_findings_group_by_surface_layer_and_split_target() {
        let files = vec![
            TestFileRecord {
                path: "/repo/libs/dataflow/weir/tests/properties/huge.rs".to_string(),
                layers: vec!["unit".to_string(), "property".to_string()],
                lines: OVERSIZED_TEST_THRESHOLD_LINES + 17,
                dedicated_test_file: true,
                inline_test_module_file: false,
                line_threshold_exceeded: true,
                has_test_entrypoint: true,
                assertion_count: 3,
                oversized: true,
                god_test_candidate: true,
                recommended_split: vec![
                    "extract property invariants into tests/properties/".to_string()
                ],
                case_roles: vec!["positive".to_string(), "property".to_string()],
                op_families: vec!["flow".to_string()],
                backend_families: vec!["backend_agnostic".to_string()],
                feature_families: vec!["core_feature".to_string()],
                error_path_families: vec!["nominal".to_string()],
                corpus_families: vec!["unit_fixture".to_string()],
                weir_flow_families: vec!["fixed_point".to_string()],
            },
            TestFileRecord {
                path: "/repo/tools/vyrec/src/tests.rs".to_string(),
                layers: vec!["unit".to_string()],
                lines: 80,
                dedicated_test_file: true,
                inline_test_module_file: false,
                line_threshold_exceeded: false,
                has_test_entrypoint: true,
                assertion_count: 1,
                oversized: false,
                god_test_candidate: true,
                recommended_split: vec![
                    "move monolithic src/tests.rs coverage into focused tests/<domain>/ files"
                        .to_string(),
                ],
                case_roles: vec!["positive".to_string()],
                op_families: vec!["parser_frontend".to_string()],
                backend_families: vec!["backend_agnostic".to_string()],
                feature_families: vec!["core_feature".to_string()],
                error_path_families: vec!["nominal".to_string()],
                corpus_families: vec!["unit_fixture".to_string()],
                weir_flow_families: vec!["non_weir".to_string()],
            },
        ];

        let findings = modularity_findings(&files);
        let summary = modularity_summary(&findings);

        assert_eq!(findings.len(), 3);
        assert!(summary.iter().any(|row| {
            row.surface == "weir"
                && row.primary_layer == "property"
                && row.recommended_split == "extract property invariants into tests/properties/"
                && row.finding_count == 1
                && row.release_blocker_count == 1
                && row.max_lines_over_threshold == 17
        }));
        assert!(summary.iter().any(|row| {
            row.surface == "vyrec"
                && row.primary_layer == "unit"
                && row.recommended_split
                    == "move monolithic src/tests.rs coverage into focused tests/<domain>/ files"
                && row.finding_count == 1
        }));
    }

    #[test]
    fn risk_dimension_gate_reports_missing_roles_per_surface_dimension() {
        let files = vec![
            TestFileRecord {
                path: "/repo/libs/dataflow/weir/tests/contracts/ifds.rs".to_string(),
                layers: vec!["unit".to_string(), "integration".to_string()],
                lines: 120,
                dedicated_test_file: true,
                inline_test_module_file: false,
                line_threshold_exceeded: false,
                has_test_entrypoint: true,
                assertion_count: 2,
                oversized: false,
                god_test_candidate: false,
                recommended_split: Vec::new(),
                case_roles: vec!["positive".to_string(), "negative".to_string()],
                op_families: vec!["flow".to_string()],
                backend_families: vec!["backend_agnostic".to_string()],
                feature_families: vec!["validation".to_string()],
                error_path_families: vec!["malformed_input".to_string()],
                corpus_families: vec!["fixture".to_string()],
                weir_flow_families: vec!["ifds".to_string()],
            },
            TestFileRecord {
                path: "/repo/libs/dataflow/weir/tests/e2e/ifds_release.rs".to_string(),
                layers: vec![
                    "benchmark".to_string(),
                    "conformance".to_string(),
                    "property".to_string(),
                    "adversarial".to_string(),
                    "fuzz".to_string(),
                ],
                lines: 140,
                dedicated_test_file: true,
                inline_test_module_file: false,
                line_threshold_exceeded: false,
                has_test_entrypoint: true,
                assertion_count: 4,
                oversized: false,
                god_test_candidate: false,
                recommended_split: Vec::new(),
                case_roles: vec![
                    "boundary".to_string(),
                    "adversarial".to_string(),
                    "property".to_string(),
                    "fuzz".to_string(),
                    "benchmark".to_string(),
                    "conformance".to_string(),
                    "e2e".to_string(),
                ],
                op_families: vec!["flow".to_string()],
                backend_families: vec!["backend_agnostic".to_string()],
                feature_families: vec!["validation".to_string()],
                error_path_families: vec!["malformed_input".to_string()],
                corpus_families: vec!["release".to_string()],
                weir_flow_families: vec!["ifds".to_string()],
            },
        ];

        let families = risk_family_coverages(&files);
        let dimensions = risk_dimension_coverages(&families);
        let weir_flow = dimensions
            .iter()
            .find(|coverage| coverage.surface == "weir" && coverage.dimension == "weir_flow")
            .expect("Fix: risk dimension coverage must include Weir flow.");

        assert!(
            weir_flow.missing_case_roles.is_empty(),
            "Fix: dimension aggregation must prove every required case role; missing={:?}",
            weir_flow.missing_case_roles
        );
        assert!(
            families.iter().any(|coverage| {
                coverage.surface == "weir"
                    && coverage.dimension == "weir_flow"
                    && coverage.family == "ifds"
                    && coverage.risk_weight == 5
            }),
            "Fix: Weir flow families must receive high risk weight."
        );
    }
}
