//! Distributed C parser ownership evidence.

use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use serde::Serialize;

#[derive(Debug, Serialize)]
struct ParserCoherence {
    schema_version: u32,
    components: Vec<ParserComponent>,
    blockers: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
struct ParserComponent {
    id: &'static str,
    role: &'static str,
    path: String,
    exists: bool,
    required_files: Vec<ComponentFile>,
    required_terms: Vec<&'static str>,
    missing_terms: Vec<&'static str>,
    required_contract_topics: Vec<&'static str>,
    missing_contract_topics: Vec<&'static str>,
    required_test_categories: Vec<&'static str>,
    missing_test_categories: Vec<&'static str>,
    required_evidence_trees: Vec<ComponentEvidenceTree>,
    unresolved_ownership_markers: Vec<&'static str>,
}

#[derive(Debug, Clone, Serialize)]
struct ComponentFile {
    path: String,
    exists: bool,
    read_error: Option<String>,
    source_bytes: usize,
}

#[derive(Debug, Serialize)]
struct ComponentContract {
    schema_version: u32,
    component_id: String,
    role: String,
    root: String,
    required_files: Vec<ComponentFile>,
    required_terms: Vec<&'static str>,
    missing_terms: Vec<&'static str>,
    required_contract_topics: Vec<&'static str>,
    missing_contract_topics: Vec<&'static str>,
    required_test_categories: Vec<&'static str>,
    missing_test_categories: Vec<&'static str>,
    required_evidence_trees: Vec<ComponentEvidenceTree>,
    unresolved_ownership_markers: Vec<&'static str>,
    blockers: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
struct ComponentEvidenceTree {
    tree: &'static str,
    path: String,
    exists: bool,
    source_bytes: usize,
    unreadable_file_count: usize,
}

const COMPONENTS: &[(&str, &str, &str, &[&str], &[&str], &[&str], &[&str])] = &[
    (
        "vyre-frontend-c",
        "Core GPU-first C frontend pipeline, parser contracts, object container, C fixture tests",
        "libs/performance/matching/vyre/vyre-frontend-c",
        &["Cargo.toml", "src/lib.rs", "README.md"],
        &["parser", "compile", "object"],
        &[
            "syntax",
            "ast",
            "diagnostic",
            "span",
            "preprocessor",
            "gnu",
            "unsupported",
        ],
        REQUIRED_PARSER_TEST_CATEGORIES,
    ),
    (
        "vyrec",
        "CLI/compiler user workflow over vyre-frontend-c",
        "tools/vyrec",
        &[
            "Cargo.toml",
            "src/main.rs",
            "README.md",
            "tests/cli_contracts.rs",
            "tests/adversarial_cli_contracts.rs",
            "tests/property_cli_contracts.rs",
            "tests/corpus_linux_contracts.rs",
            "tests/benchmark_cli_contracts.rs",
            "tests/conformance_cli_contracts.rs",
            "tests/gap_cli_contracts.rs",
            "tests/fuzz_cli_contracts.rs",
        ],
        &["vyre", "compile", "cli", "evidence"],
        &[
            "cli",
            "diagnostic",
            "include",
            "macro",
            "corpus",
            "cuda",
            "fuzz",
            "gap",
            "conformance",
            "fix:",
        ],
        REQUIRED_PARSER_TEST_CATEGORIES,
    ),
    (
        "weir",
        "Dataflow facts consumed by parser/compiler optimization and downstream analysis",
        "libs/dataflow/weir",
        &["Cargo.toml", "src/lib.rs", "README.md"],
        &["dataflow", "analysis", "program"],
        &["parser", "dataflow", "alias", "reaching", "callgraph"],
        REQUIRED_PARSER_TEST_CATEGORIES,
    ),
    (
        "surgec",
        "Security compiler consumer integration surface",
        "libs/surge/surgec",
        &["Cargo.toml", "src/lib.rs", "README.md"],
        &["surge", "compile"],
        &["consumer", "surge", "parser", "condition"],
        REQUIRED_PARSER_TEST_CATEGORIES,
    ),
    (
        "surgec-grammar-gen",
        "Shared grammar generation substrate",
        "libs/performance/matching/vyre/vyre-grammar-gen",
        &["Cargo.toml", "src/lib.rs", "README.md"],
        &["grammar", "generate"],
        &["grammar", "generate", "token", "parser"],
        REQUIRED_PARSER_TEST_CATEGORIES,
    ),
];

const REQUIRED_PARSER_TEST_CATEGORIES: &[&str] = &[
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

const REQUIRED_PARSER_EVIDENCE_TREES: &[&str] = &["tests", "benches", "fuzz"];

const MAX_PARSER_CONTRACT_FILE_BYTES: u64 = 2_097_152;
const UNRESOLVED_OWNERSHIP_MARKERS: &[&str] = &[
    "owner: tbd",
    "ownership: tbd",
    "owner: unknown",
    "ownership: unknown",
    "unresolved ownership",
    "placeholder",
    "todo",
    "fixme",
];

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
    let mut components = Vec::new();
    let mut blockers = Vec::new();
    for &(
        id,
        role,
        relative,
        required_files,
        required_terms,
        required_contract_topics,
        required_test_categories,
    ) in COMPONENTS
    {
        let path = santh_root.join(relative);
        let exists = path.exists();
        if !exists {
            blockers.push(format!(
                "parser component `{id}` is missing at {}",
                path.display()
            ));
        }
        let mut component_text = String::new();
        let mut ownership_text = String::new();
        let required_files = required_files
            .iter()
            .map(|required| {
                let file_path = path.join(required);
                let exists = file_path.is_file();
                let (text, read_error) = if exists {
                    match read_text_bounded(&file_path) {
                        Ok(text) => (text, None),
                        Err(error) => {
                            blockers.push(format!(
                                "parser component `{id}` required file {} could not be read: {error}",
                                file_path.display()
                            ));
                            (String::new(), Some(error.to_string()))
                        }
                    }
                } else {
                    (String::new(), None)
                };
                component_text.push_str(&text);
                ownership_text.push_str(&text);
                if !exists {
                    blockers.push(format!(
                        "parser component `{id}` is missing required file {}",
                        file_path.display()
                    ));
                } else if text.trim().is_empty() {
                    blockers.push(format!(
                        "parser component `{id}` required file {} is empty",
                        file_path.display()
                    ));
                }
                ComponentFile {
                    path: file_path.display().to_string(),
                    exists,
                    read_error,
                    source_bytes: text.len(),
                }
            })
            .collect();
        let component_test_unreadable = append_component_test_text(&path, &mut component_text);
        if component_test_unreadable != 0 {
            blockers.push(format!(
                "parser component `{id}` test/bench/fuzz evidence has {component_test_unreadable} unreadable source file(s)"
            ));
        }
        let lowered = component_text.to_ascii_lowercase();
        let missing_terms = required_terms
            .iter()
            .copied()
            .filter(|term| !lowered.contains(term))
            .collect::<Vec<_>>();
        for term in &missing_terms {
            blockers.push(format!(
                "parser component `{id}` does not document or expose required term `{term}`"
            ));
        }
        let missing_contract_topics = required_contract_topics
            .iter()
            .copied()
            .filter(|topic| !lowered.contains(topic))
            .collect::<Vec<_>>();
        for topic in &missing_contract_topics {
            blockers.push(format!(
                "parser component `{id}` does not document parser contract topic `{topic}`"
            ));
        }
        let missing_test_categories = required_test_categories
            .iter()
            .copied()
            .filter(|category| !lowered.contains(category))
            .collect::<Vec<_>>();
        for category in &missing_test_categories {
            blockers.push(format!(
                "parser component `{id}` does not expose required test category `{category}`"
            ));
        }
        let required_evidence_trees = REQUIRED_PARSER_EVIDENCE_TREES
            .iter()
            .map(|tree| {
                let tree = *tree;
                let tree_path = path.join(tree);
                let exists = tree_path.is_dir();
                let (source_bytes, unreadable_file_count) = tree_source_bytes(&tree_path);
                if !exists {
                    blockers.push(format!(
                        "parser component `{id}` is missing required `{tree}` evidence tree"
                    ));
                } else if unreadable_file_count != 0 {
                    blockers.push(format!(
                        "parser component `{id}` required `{tree}` evidence tree has {unreadable_file_count} unreadable source file(s)"
                    ));
                } else if source_bytes == 0 {
                    blockers.push(format!(
                        "parser component `{id}` required `{tree}` evidence tree is empty"
                    ));
                }
                ComponentEvidenceTree {
                    tree,
                    path: tree_path.display().to_string(),
                    exists,
                    source_bytes,
                    unreadable_file_count,
                }
            })
            .collect::<Vec<_>>();
        let lowered_ownership = normalized_ownership_text(&ownership_text);
        let unresolved_ownership_markers = UNRESOLVED_OWNERSHIP_MARKERS
            .iter()
            .copied()
            .filter(|marker| lowered_ownership.contains(marker))
            .collect::<Vec<_>>();
        for marker in &unresolved_ownership_markers {
            blockers.push(format!(
                "parser component `{id}` contains unresolved ownership marker `{marker}`"
            ));
        }
        components.push(ParserComponent {
            id,
            role,
            path: path.display().to_string(),
            exists,
            required_files,
            required_terms: required_terms.to_vec(),
            missing_terms,
            required_contract_topics: required_contract_topics.to_vec(),
            missing_contract_topics,
            required_test_categories: required_test_categories.to_vec(),
            missing_test_categories,
            required_evidence_trees,
            unresolved_ownership_markers,
        });
    }
    let matrix = ParserCoherence {
        schema_version: 1,
        components,
        blockers,
    };
    let json = match serde_json::to_string_pretty(&matrix) {
        Ok(json) => json,
        Err(error) => {
            eprintln!("Fix: failed to serialize parser coherence matrix: {error}");
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
    write_sibling_contracts(&output, &matrix);
    println!("parser-coherence: wrote {}", output.display());
    if !matrix.blockers.is_empty() {
        std::process::exit(1);
    }
}

fn write_sibling_contracts(output: &Path, matrix: &ParserCoherence) {
    let Some(parent) = output.parent() else {
        eprintln!(
            "Fix: parser coherence output `{}` has no parent directory.",
            output.display()
        );
        std::process::exit(1);
    };
    for component in &matrix.components {
        let blockers = component
            .required_files
            .iter()
            .filter(|file| !file.exists)
            .map(|file| {
                format!(
                    "parser component `{}` is missing required file {}",
                    component.id, file.path
                )
            })
            .chain((!component.exists).then(|| {
                format!(
                    "parser component `{}` is missing at {}",
                    component.id, component.path
                )
            }))
            .chain(component.required_files.iter().filter(|file| file.source_bytes == 0).map(
                |file| {
                    format!(
                        "parser component `{}` required file {} is empty",
                        component.id, file.path
                    )
                },
            ))
            .chain(component.missing_terms.iter().map(|term| {
                format!(
                    "parser component `{}` is missing required term `{term}`",
                    component.id
                )
            }))
            .chain(component.missing_contract_topics.iter().map(|topic| {
                format!(
                    "parser component `{}` is missing parser contract topic `{topic}`",
                    component.id
                )
            }))
            .chain(component.missing_test_categories.iter().map(|category| {
                format!(
                    "parser component `{}` is missing test category `{category}`",
                    component.id
                )
            }))
            .chain(component.required_evidence_trees.iter().filter(|tree| !tree.exists).map(
                |tree| {
                    format!(
                        "parser component `{}` is missing required `{}` evidence tree {}",
                        component.id, tree.tree, tree.path
                    )
                },
            ))
            .chain(component.required_evidence_trees.iter().filter(|tree| tree.unreadable_file_count != 0).map(
                |tree| {
                    format!(
                        "parser component `{}` required `{}` evidence tree {} has {} unreadable source file(s)",
                        component.id, tree.tree, tree.path, tree.unreadable_file_count
                    )
                },
            ))
            .chain(component.required_evidence_trees.iter().filter(|tree| tree.source_bytes == 0).map(
                |tree| {
                    format!(
                        "parser component `{}` required `{}` evidence tree {} is empty",
                        component.id, tree.tree, tree.path
                    )
                },
            ))
            .chain(component.unresolved_ownership_markers.iter().map(|marker| {
                format!(
                    "parser component `{}` contains unresolved ownership marker `{marker}`",
                    component.id
                )
            }))
            .collect::<Vec<_>>();
        write_json(
            &parent.join(component_contract_artifact(component.id)),
            &ComponentContract {
                schema_version: 1,
                component_id: component.id.to_string(),
                role: component.role.to_string(),
                root: component.path.clone(),
                required_files: component.required_files.clone(),
                required_terms: component.required_terms.clone(),
                missing_terms: component.missing_terms.clone(),
                required_contract_topics: component.required_contract_topics.clone(),
                missing_contract_topics: component.missing_contract_topics.clone(),
                required_test_categories: component.required_test_categories.clone(),
                missing_test_categories: component.missing_test_categories.clone(),
                required_evidence_trees: component.required_evidence_trees.clone(),
                unresolved_ownership_markers: component.unresolved_ownership_markers.clone(),
                blockers,
            },
        );
    }
}

fn component_contract_artifact(component_id: &str) -> String {
    match component_id {
        "vyrec" => "vyrec-cli-contracts.json".to_string(),
        other => format!("{other}-contracts.json"),
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

fn append_component_test_text(root: &Path, output: &mut String) -> usize {
    let mut unreadable = 0usize;
    for relative in ["tests", "benches", "fuzz"] {
        unreadable = unreadable.saturating_add(append_tree_text(&root.join(relative), output));
    }
    unreadable
}

fn normalized_ownership_text(text: &str) -> String {
    text.to_ascii_lowercase()
        .replace("clippy::todo", "clippy-lint")
}

fn tree_source_bytes(root: &Path) -> (usize, usize) {
    let mut text = String::new();
    let unreadable = append_tree_text(root, &mut text);
    (text.len(), unreadable)
}

fn append_tree_text(root: &Path, output: &mut String) -> usize {
    let Ok(entries) = fs::read_dir(root) else {
        return usize::from(root.exists());
    };
    let mut unreadable = 0usize;
    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(_) => {
                unreadable = unreadable.saturating_add(1);
                continue;
            }
        };
        let path = entry.path();
        if path.is_dir() {
            unreadable = unreadable.saturating_add(append_tree_text(&path, output));
            continue;
        }
        let extension = path.extension().and_then(|extension| extension.to_str());
        if !matches!(extension, Some("rs" | "toml" | "md" | "c" | "h")) {
            continue;
        }
        if let Ok(text) = read_text_bounded(&path) {
            output.push('\n');
            output.push_str(&text);
        } else {
            unreadable = unreadable.saturating_add(1);
        }
    }
    unreadable
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
                    "USAGE:\n  cargo_full run --bin xtask -- parser-coherence [--output PATH]\n\n\
                     Writes distributed C parser ownership evidence."
                );
                std::process::exit(0);
            }
            other => return Err(format!("Fix: unknown parser-coherence option `{other}`.")),
        }
    }
    Ok(output.unwrap_or_else(default_output))
}

fn default_output() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(|path| path.join("release/evidence/parser/distributed-parser-map.json"))
        .unwrap_or_else(|| PathBuf::from("release/evidence/parser/distributed-parser-map.json"))
}

fn read_text_bounded(path: &Path) -> io::Result<String> {
    let mut reader = fs::File::open(path)?.take(MAX_PARSER_CONTRACT_FILE_BYTES.saturating_add(1));
    let mut text = String::new();
    reader.read_to_string(&mut text)?;
    if text.len() as u64 > MAX_PARSER_CONTRACT_FILE_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "{} exceeds {MAX_PARSER_CONTRACT_FILE_BYTES} byte parser contract read cap",
                path.display()
            ),
        ));
    }
    Ok(text)
}
