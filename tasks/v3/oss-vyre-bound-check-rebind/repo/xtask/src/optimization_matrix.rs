//! Release optimization integration evidence.

use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use serde::Serialize;

const MAX_OPTIMIZATION_EVIDENCE_TEXT_BYTES: u64 = 4_194_304;

#[derive(Debug, Serialize)]
struct OptimizationMatrix {
    schema_version: u32,
    required_passes: Vec<OptimizationRequirement>,
    required_analyses: Vec<OptimizationRequirement>,
    analysis_fixture_corpuses: Vec<AnalysisFixtureRequirement>,
    integration_markers: Vec<AnalysisFixtureRequirement>,
    blockers: Vec<String>,
}

#[derive(Debug, Serialize)]
struct OptimizationRequirement {
    id: &'static str,
    path: String,
    exists: bool,
    read_error: Option<String>,
    role: &'static str,
    source_bytes: usize,
    has_transform_entrypoint: bool,
    has_local_tests: bool,
    unresolved_markers: Vec<&'static str>,
}

#[derive(Debug, Serialize)]
struct AnalysisFixtureRequirement {
    id: &'static str,
    path: String,
    marker: &'static str,
    exists: bool,
    read_error: Option<String>,
    marker_present: bool,
    marker_on_code_line: bool,
    unresolved_markers: Vec<&'static str>,
    role: &'static str,
}

#[derive(Debug, Serialize)]
struct DerivedEvidence<'a> {
    schema_version: u32,
    id: &'a str,
    source_matrix: String,
    markers: Vec<&'a AnalysisFixtureRequirement>,
    blockers: Vec<String>,
}

const REQUIRED_PASSES: &[(&str, &str, &str)] = &[
    (
        "dse",
        "src/rewrites/dead_store/mod.rs",
        "Dead-store elimination",
    ),
    (
        "store-to-load-forwarding",
        "src/rewrites/load_forwarding/mod.rs",
        "Store/load forwarding",
    ),
    (
        "licm",
        "src/rewrites/licm/mod.rs",
        "Loop-invariant code motion",
    ),
    (
        "loop-fusion",
        "src/rewrites/loop_fusion/mod.rs",
        "Loop fusion legality and transform",
    ),
    (
        "loop-fission",
        "src/rewrites/loop_fission/mod.rs",
        "Loop fission legality and transform",
    ),
    (
        "loop-unroll",
        "src/rewrites/loop_unroll/mod.rs",
        "Loop transform substrate",
    ),
    (
        "descriptor-cse",
        "src/rewrites/descriptor_cse/mod.rs",
        "Common subexpression elimination",
    ),
    (
        "descriptor-dce",
        "src/rewrites/descriptor_dce/mod.rs",
        "Dead-code elimination",
    ),
    (
        "strength-reduce",
        "src/rewrites/strength_reduce/mod.rs",
        "Strength reduction",
    ),
    (
        "shared-mem-promote",
        "src/rewrites/shared_mem_promote/mod.rs",
        "Shared-memory promotion",
    ),
    (
        "bank-conflict-pad",
        "src/rewrites/bank_conflict_pad/mod.rs",
        "Bank-conflict padding",
    ),
    (
        "egraph-saturation",
        "src/rewrites/egraph_saturation/mod.rs",
        "Bounded e-graph saturation",
    ),
];

const REQUIRED_ANALYSES: &[(&str, &str, &str)] = &[
    (
        "def-use",
        "src/analyses/def_use/mod.rs",
        "Definition/use facts",
    ),
    (
        "value-range",
        "src/analyses/value_range/mod.rs",
        "Range facts",
    ),
    (
        "coalesce",
        "src/analyses/coalesce/mod.rs",
        "Memory coalescing analysis",
    ),
    (
        "shared-mem-promote",
        "src/analyses/shared_mem_promote/mod.rs",
        "Shared promotion analysis",
    ),
    (
        "bank-conflict",
        "src/analyses/bank_conflict/mod.rs",
        "Bank-conflict analysis",
    ),
    (
        "vec-pack",
        "src/analyses/vec_pack/mod.rs",
        "Vector packing analysis",
    ),
    (
        "weir-alias",
        "src/analyses/weir_alias/mod.rs",
        "Weir alias/points-to facts",
    ),
    (
        "weir-reaching-def",
        "src/analyses/weir_reaching_def/mod.rs",
        "Weir reaching-def facts",
    ),
];

const REQUIRED_ANALYSIS_FIXTURES: &[(&str, &str, &str, &str)] = &[
    (
        "a13-coalesce-corpus",
        "tests/analysis_fixture_corpuses.rs",
        "a13_coalesce_corpus_classifies_unit_stride_strided_and_broadcast",
        "Hand-built coalesce KernelDescriptor corpus",
    ),
    (
        "a14-shared-mem-promote-corpus",
        "tests/analysis_fixture_corpuses.rs",
        "a14_shared_mem_promote_corpus_finds_reused_global_tile",
        "Hand-built shared-memory promotion KernelDescriptor corpus",
    ),
    (
        "a15-bank-conflict-corpus",
        "tests/analysis_fixture_corpuses.rs",
        "a15_bank_conflict_corpus_detects_full_warp_serialization",
        "Hand-built bank-conflict KernelDescriptor corpus",
    ),
    (
        "a16-vec-pack-corpus",
        "tests/analysis_fixture_corpuses.rs",
        "a16_vec_pack_corpus_detects_adjacent_load_chain",
        "Hand-built vector-pack KernelDescriptor corpus",
    ),
];

const REQUIRED_INTEGRATION_MARKERS: &[(&str, &str, &str, &str)] = &[
    (
        "alias-aware-dse-entrypoint",
        "src/rewrites/dead_store/mod.rs",
        "dead_store_with_weir_alias_facts",
        "DSE consumes Weir alias facts",
    ),
    (
        "reaching-def-dse-entrypoint",
        "src/rewrites/dead_store/mod.rs",
        "dead_store_with_dataflow_analysis_facts",
        "DSE consumes Weir reaching-definition facts",
    ),
    (
        "alias-aware-stlf-entrypoint",
        "src/rewrites/load_forwarding/mod.rs",
        "load_forwarding_with_weir_alias_facts",
        "Store/load forwarding consumes Weir alias facts",
    ),
    (
        "reaching-def-stlf-entrypoint",
        "src/rewrites/load_forwarding/mod.rs",
        "load_forwarding_with_dataflow_analysis_facts",
        "Store/load forwarding consumes Weir reaching-definition facts",
    ),
    (
        "dataflow-analysis-pipeline-entrypoint",
        "src/rewrites/mod.rs",
        "run_all_with_dataflow_analysis_facts",
        "Canonical rewrite pipeline consumes Weir alias and reaching-definition facts",
    ),
    (
        "alias-aware-licm-entrypoint",
        "src/rewrites/licm/mod.rs",
        "licm_with_weir_alias_facts",
        "LICM consumes Weir alias facts",
    ),
    (
        "reaching-def-licm-entrypoint",
        "src/rewrites/licm/mod.rs",
        "licm_with_dataflow_analysis_facts",
        "LICM consumes Weir reaching-definition facts",
    ),
    (
        "alias-aware-loop-fusion-entrypoint",
        "src/rewrites/loop_fusion/mod.rs",
        "loop_fusion_with_weir_alias_facts",
        "Loop fusion consumes Weir alias facts",
    ),
    (
        "reaching-def-loop-fusion-entrypoint",
        "src/rewrites/loop_fusion/mod.rs",
        "loop_fusion_with_dataflow_analysis_facts",
        "Loop fusion consumes Weir reaching-definition facts",
    ),
    (
        "alias-aware-loop-fission-entrypoint",
        "src/rewrites/loop_fission/mod.rs",
        "loop_fission_with_weir_alias_facts",
        "Loop fission consumes Weir alias facts",
    ),
    (
        "reaching-def-loop-fission-entrypoint",
        "src/rewrites/loop_fission/mod.rs",
        "loop_fission_with_dataflow_analysis_facts",
        "Loop fission consumes Weir reaching-definition facts",
    ),
    (
        "weir-alias-analysis-api",
        "src/analyses/weir_alias/mod.rs",
        "AliasFactSet",
        "Vyre-lower imports Weir alias facts through an explicit API",
    ),
    (
        "weir-reaching-def-analysis-api",
        "src/analyses/weir_reaching_def/mod.rs",
        "import_descriptor_reaching_defs",
        "Vyre-lower imports Weir reaching-def facts through an explicit API",
    ),
    (
        "weir-copy-chain-reaching-def-import",
        "src/analyses/weir_reaching_def/mod.rs",
        "resolve_copy_alias",
        "Weir reaching-def import canonicalizes descriptor Copy chains before rewrite legality checks",
    ),
    (
        "dataflow-analysis-dse-firing-test",
        "tests/dead_store_dataflow_analysis.rs",
        "dataflow_analysis_pipeline_applies_reaching_def_memory_dse",
        "Integration test proves Weir reaching-def facts fire DSE in the canonical pipeline",
    ),
    (
        "dataflow-analysis-stlf-firing-test",
        "src/rewrites/load_forwarding/mod.rs",
        "load_forwarding_with_dataflow_analysis_facts(&desc",
        "Local test proves Weir reaching-def facts fire store/load forwarding",
    ),
    (
        "dataflow-analysis-loop-fusion-firing-test",
        "tests/dataflow_analysis_loop_rewrites.rs",
        "reaching_defs_unlock_alias_proven_loop_fusion",
        "Integration test proves Weir facts unlock loop fusion",
    ),
    (
        "dataflow-analysis-loop-fission-firing-test",
        "tests/dataflow_analysis_loop_rewrites.rs",
        "reaching_defs_unlock_alias_proven_loop_fission",
        "Integration test proves Weir facts unlock loop fission",
    ),
    (
        "dataflow-analysis-licm-firing-test",
        "tests/dataflow_analysis_loop_rewrites.rs",
        "reaching_defs_unlock_alias_proven_licm_load_hoist",
        "Integration test proves Weir facts unlock LICM load hoisting",
    ),
    (
        "egraph-saturation",
        "src/rewrites/egraph_saturation/mod.rs",
        "saturate_descriptor",
        "Bounded e-graph saturation entry point",
    ),
    (
        "egraph-canonical-pipeline-entrypoint",
        "src/rewrites/mod.rs",
        "saturate_algebraic_descriptor",
        "Canonical rewrite pipeline invokes non-recursive e-graph saturation",
    ),
    (
        "egraph-algebraic-reassociation",
        "src/rewrites/egraph_saturation/mod.rs",
        "reassociate_constant_chain",
        "E-graph saturation performs algebraic reassociation before extraction",
    ),
    (
        "egraph-bitwise-reassociation",
        "src/rewrites/egraph_saturation/mod.rs",
        "BitXor",
        "E-graph saturation reassociates bitwise condition chains before extraction",
    ),
];

const UNRESOLVED_MARKERS: &[&str] = &[
    "todo",
    "fixme",
    "placeholder",
    "stub",
    "unimplemented!",
    "todo!",
    "panic!(\"not implemented",
    "tbd",
];

pub(crate) fn run(args: &[String]) {
    let output = match parse_output(args) {
        Ok(output) => output,
        Err(message) => {
            eprintln!("{message}");
            std::process::exit(2);
        }
    };
    let lower_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(|path| path.join("vyre-lower"))
        .unwrap_or_else(|| PathBuf::from("../vyre-lower"));
    let mut blockers = Vec::new();
    let required_passes = collect_requirements(&lower_root, REQUIRED_PASSES, &mut blockers);
    let required_analyses = collect_requirements(&lower_root, REQUIRED_ANALYSES, &mut blockers);
    let analysis_fixture_corpuses =
        collect_fixture_requirements(&lower_root, REQUIRED_ANALYSIS_FIXTURES, &mut blockers);
    let integration_markers =
        collect_fixture_requirements(&lower_root, REQUIRED_INTEGRATION_MARKERS, &mut blockers);
    let matrix = OptimizationMatrix {
        schema_version: 1,
        required_passes,
        required_analyses,
        analysis_fixture_corpuses,
        integration_markers,
        blockers,
    };
    let json = match serde_json::to_string_pretty(&matrix) {
        Ok(json) => json,
        Err(error) => {
            eprintln!("Fix: failed to serialize optimization matrix: {error}");
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
    if let Err(error) = write_derived_evidence(&output, &matrix) {
        eprintln!("Fix: failed to write derived optimization evidence: {error}");
        std::process::exit(1);
    }
    println!("optimization-matrix: wrote {}", output.display());
    if !matrix.blockers.is_empty() {
        std::process::exit(1);
    }
}

fn write_derived_evidence(output: &Path, matrix: &OptimizationMatrix) -> Result<(), String> {
    let dir = output
        .parent()
        .ok_or_else(|| "optimization matrix output has no parent".to_string())?;
    write_marker_evidence(
        dir,
        "alias-aware-dse",
        "alias-aware-dse.json",
        matrix,
        &[
            "alias-aware-dse-entrypoint",
            "reaching-def-dse-entrypoint",
            "weir-alias-analysis-api",
            "weir-reaching-def-analysis-api",
            "weir-copy-chain-reaching-def-import",
            "dataflow-analysis-pipeline-entrypoint",
            "dataflow-analysis-dse-firing-test",
            "dataflow-analysis-stlf-firing-test",
            "dataflow-analysis-loop-fusion-firing-test",
            "dataflow-analysis-loop-fission-firing-test",
            "dataflow-analysis-licm-firing-test",
        ],
    )?;
    write_marker_evidence(
        dir,
        "alias-aware-stlf",
        "alias-aware-stlf.json",
        matrix,
        &[
            "alias-aware-stlf-entrypoint",
            "reaching-def-stlf-entrypoint",
            "weir-alias-analysis-api",
            "weir-reaching-def-analysis-api",
            "dataflow-analysis-pipeline-entrypoint",
            "dataflow-analysis-stlf-firing-test",
        ],
    )?;
    write_marker_evidence(
        dir,
        "alias-aware-licm",
        "alias-aware-licm.json",
        matrix,
        &[
            "alias-aware-licm-entrypoint",
            "reaching-def-licm-entrypoint",
            "weir-alias-analysis-api",
            "weir-reaching-def-analysis-api",
            "dataflow-analysis-licm-firing-test",
        ],
    )?;
    write_marker_evidence(
        dir,
        "alias-aware-fusion-fission",
        "alias-aware-fusion-fission.json",
        matrix,
        &[
            "alias-aware-loop-fusion-entrypoint",
            "reaching-def-loop-fusion-entrypoint",
            "alias-aware-loop-fission-entrypoint",
            "reaching-def-loop-fission-entrypoint",
            "weir-alias-analysis-api",
            "weir-reaching-def-analysis-api",
            "dataflow-analysis-loop-fusion-firing-test",
            "dataflow-analysis-loop-fission-firing-test",
        ],
    )?;
    write_marker_evidence(
        dir,
        "weir-facts-pass-firing",
        "weir-facts-pass-firing.json",
        matrix,
        &[
            "alias-aware-dse-entrypoint",
            "reaching-def-dse-entrypoint",
            "alias-aware-stlf-entrypoint",
            "reaching-def-stlf-entrypoint",
            "alias-aware-licm-entrypoint",
            "reaching-def-licm-entrypoint",
            "alias-aware-loop-fusion-entrypoint",
            "reaching-def-loop-fusion-entrypoint",
            "alias-aware-loop-fission-entrypoint",
            "reaching-def-loop-fission-entrypoint",
            "weir-alias-analysis-api",
            "weir-reaching-def-analysis-api",
            "weir-copy-chain-reaching-def-import",
            "dataflow-analysis-pipeline-entrypoint",
            "dataflow-analysis-dse-firing-test",
            "dataflow-analysis-stlf-firing-test",
            "dataflow-analysis-loop-fusion-firing-test",
            "dataflow-analysis-loop-fission-firing-test",
            "dataflow-analysis-licm-firing-test",
        ],
    )?;
    write_marker_evidence(
        dir,
        "egraph-saturation",
        "egraph-saturation-matrix.json",
        matrix,
        &[
            "egraph-saturation",
            "egraph-canonical-pipeline-entrypoint",
            "egraph-algebraic-reassociation",
            "egraph-bitwise-reassociation",
        ],
    )?;
    write_marker_evidence(
        dir,
        "egraph-semantic-contracts",
        "egraph-semantic-contracts.json",
        matrix,
        &[
            "egraph-saturation",
            "egraph-canonical-pipeline-entrypoint",
            "egraph-algebraic-reassociation",
            "egraph-bitwise-reassociation",
        ],
    )?;
    Ok(())
}

fn write_marker_evidence(
    dir: &Path,
    id: &str,
    file_name: &str,
    matrix: &OptimizationMatrix,
    required_markers: &[&str],
) -> Result<(), String> {
    let mut markers = Vec::new();
    let mut blockers = Vec::new();
    for marker in required_markers {
        match matrix
            .integration_markers
            .iter()
            .chain(matrix.analysis_fixture_corpuses.iter())
            .find(|candidate| candidate.id == *marker)
        {
            Some(found) if found.exists && found.marker_present && found.marker_on_code_line => {
                markers.push(found);
            }
            Some(found) => {
                markers.push(found);
                blockers.push(format!(
                    "marker `{}` is not present on a non-comment code line",
                    found.id
                ));
            }
            None => blockers.push(format!("required marker `{marker}` is absent")),
        }
    }
    let evidence = DerivedEvidence {
        schema_version: 1,
        id,
        source_matrix: dir
            .join("optimization-integration-matrix.json")
            .display()
            .to_string(),
        markers,
        blockers,
    };
    let json = serde_json::to_string_pretty(&evidence).map_err(|error| error.to_string())?;
    fs::write(dir.join(file_name), format!("{json}\n"))
        .map_err(|error| format!("{}: {error}", dir.join(file_name).display()))
}

fn collect_fixture_requirements(
    root: &Path,
    requirements: &'static [(&'static str, &'static str, &'static str, &'static str)],
    blockers: &mut Vec<String>,
) -> Vec<AnalysisFixtureRequirement> {
    let mut out = Vec::new();
    for &(id, relative, marker, role) in requirements {
        let path = root.join(relative);
        let exists = path.is_file();
        let (text, read_error) = if exists {
            match read_text_bounded(&path) {
                Ok(text) => (text, None),
                Err(error) => {
                    blockers.push(format!(
                        "analysis fixture corpus `{id}` could not be read at {}: {error}",
                        path.display()
                    ));
                    (String::new(), Some(error.to_string()))
                }
            }
        } else {
            (String::new(), None)
        };
        let marker_present = text.contains(marker);
        let marker_on_code_line = marker_present && marker_is_on_code_line(&text, marker);
        let lowered = text.to_ascii_lowercase();
        let unresolved_markers = UNRESOLVED_MARKERS
            .iter()
            .copied()
            .filter(|marker| lowered.contains(marker))
            .collect::<Vec<_>>();
        if !exists {
            blockers.push(format!(
                "analysis fixture corpus `{id}` is missing at {}",
                path.display()
            ));
        } else if !marker_present {
            blockers.push(format!(
                "analysis fixture corpus `{id}` is missing marker `{marker}` in {}",
                path.display()
            ));
        } else if !marker_on_code_line {
            blockers.push(format!(
                "analysis fixture corpus `{id}` only mentions marker `{marker}` in comments or prose"
            ));
        }
        for unresolved in &unresolved_markers {
            blockers.push(format!(
                "analysis fixture corpus `{id}` contains unresolved marker `{unresolved}`"
            ));
        }
        out.push(AnalysisFixtureRequirement {
            id,
            path: path.display().to_string(),
            marker,
            exists,
            read_error,
            marker_present,
            marker_on_code_line,
            unresolved_markers,
            role,
        });
    }
    out
}

fn collect_requirements(
    root: &Path,
    requirements: &'static [(&'static str, &'static str, &'static str)],
    blockers: &mut Vec<String>,
) -> Vec<OptimizationRequirement> {
    let mut out = Vec::new();
    for &(id, relative, role) in requirements {
        let path = root.join(relative);
        let exists = path.is_file();
        let (text, read_error) = if exists {
            match read_text_bounded(&path) {
                Ok(text) => (text, None),
                Err(error) => {
                    blockers.push(format!(
                        "optimization requirement `{id}` could not be read at {}: {error}",
                        path.display()
                    ));
                    (String::new(), Some(error.to_string()))
                }
            }
        } else {
            (String::new(), None)
        };
        let scope_text = match requirement_scope_text(&path, &text) {
            Ok(scope_text) => scope_text,
            Err(error) => {
                blockers.push(format!(
                    "optimization requirement `{id}` module scope could not be read at {}: {error}",
                    path.display()
                ));
                text.clone()
            }
        };
        let lowered = scope_text.to_ascii_lowercase();
        let source_bytes = scope_text.len();
        let has_transform_entrypoint = scope_text.contains("pub fn ")
            || scope_text.contains("pub(crate) fn ")
            || scope_text.contains("pub(super) fn ")
            || scope_text.contains("pub use ");
        let has_local_tests =
            scope_text.contains("#[cfg(test)]") || scope_text.contains("mod tests");
        let unresolved_markers = UNRESOLVED_MARKERS
            .iter()
            .copied()
            .filter(|marker| lowered.contains(marker))
            .collect::<Vec<_>>();
        if !exists {
            blockers.push(format!(
                "optimization requirement `{id}` is missing at {}",
                path.display()
            ));
        }
        if exists && source_bytes == 0 {
            blockers.push(format!("optimization requirement `{id}` is empty"));
        }
        if exists && !has_transform_entrypoint {
            blockers.push(format!(
                "optimization requirement `{id}` exposes no public transform entrypoint"
            ));
        }
        if exists && !has_local_tests {
            blockers.push(format!(
                "optimization requirement `{id}` has no local test module marker"
            ));
        }
        for unresolved in &unresolved_markers {
            blockers.push(format!(
                "optimization requirement `{id}` contains unresolved marker `{unresolved}`"
            ));
        }
        out.push(OptimizationRequirement {
            id,
            path: path.display().to_string(),
            exists,
            read_error,
            role,
            source_bytes,
            has_transform_entrypoint,
            has_local_tests,
            unresolved_markers,
        });
    }
    out
}

fn marker_is_on_code_line(text: &str, marker: &str) -> bool {
    text.lines().any(|line| {
        let trimmed = line.trim_start();
        !trimmed.starts_with("//")
            && !trimmed.starts_with('*')
            && !trimmed.starts_with("/*")
            && trimmed.contains(marker)
    })
}

fn requirement_scope_text(path: &Path, root_text: &str) -> io::Result<String> {
    let mut text = root_text.to_string();
    if path.file_name().and_then(|name| name.to_str()) != Some("mod.rs") {
        return Ok(text);
    }
    let Some(dir) = path.parent() else {
        return Ok(text);
    };
    let mut entries = fs::read_dir(dir)?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|candidate| candidate.is_file() && candidate != path)
        .filter(|candidate| candidate.extension().and_then(|ext| ext.to_str()) == Some("rs"))
        .collect::<Vec<_>>();
    entries.sort();
    for entry in entries {
        text.push('\n');
        text.push_str(&read_text_bounded(&entry)?);
        if text.len() as u64 > MAX_OPTIMIZATION_EVIDENCE_TEXT_BYTES {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "{} module scope exceeds {MAX_OPTIMIZATION_EVIDENCE_TEXT_BYTES} byte optimization evidence read cap",
                    path.display()
                ),
            ));
        }
    }
    Ok(text)
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
                    "USAGE:\n  cargo_full run --bin xtask -- optimization-matrix [--output PATH]\n\n\
                     Writes optimization integration evidence for release pass families."
                );
                std::process::exit(0);
            }
            other => {
                return Err(format!(
                    "Fix: unknown optimization-matrix option `{other}`."
                ))
            }
        }
    }
    Ok(output.unwrap_or_else(default_output))
}

fn default_output() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(|path| path.join("release/evidence/optimization/optimization-integration-matrix.json"))
        .unwrap_or_else(|| {
            PathBuf::from("release/evidence/optimization/optimization-integration-matrix.json")
        })
}

fn read_text_bounded(path: &Path) -> io::Result<String> {
    let mut reader =
        fs::File::open(path)?.take(MAX_OPTIMIZATION_EVIDENCE_TEXT_BYTES.saturating_add(1));
    let mut text = String::new();
    reader.read_to_string(&mut text)?;
    if text.len() as u64 > MAX_OPTIMIZATION_EVIDENCE_TEXT_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "{} exceeds {MAX_OPTIMIZATION_EVIDENCE_TEXT_BYTES} byte optimization evidence read cap",
                path.display()
            ),
        ));
    }
    Ok(text)
}
