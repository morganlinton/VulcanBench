//! C parser corpus evidence runner for the Vyre/Weir release gate.

use std::collections::BTreeSet;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use serde::Serialize;
use vyre_frontend_c::api::{compile, CliMacroAction, VyreCompileOptions};
use vyre_frontend_c::object_format::{SectionTag, VYRECOB2_MAGIC};

use vyre_driver_cuda as _;
use vyre_driver_wgpu as _;

const MIN_LINUX_SUBSYSTEM_C_FILES: usize = 250;
const MIN_LINUX_SUBSYSTEM_SOURCE_BYTES: u64 = 4 * 1024 * 1024;
const MAX_C_PARSER_OBJECT_BYTES: u64 = 64 * 1024 * 1024;
static TEMP_OBJECT_COUNTER: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Serialize)]
struct CorpusReport {
    schema_version: u32,
    corpus_root: String,
    corpus_root_canonical: String,
    linux_subsystem_candidate: bool,
    linux_root: Option<String>,
    linux_subsystem: Option<String>,
    linux_subsystem_depth: usize,
    linux_kbuild_file: Option<String>,
    linux_kbuild_file_in_corpus: bool,
    corpus_fingerprint: String,
    source_collection_mode: String,
    visited_dir_count: usize,
    include_dirs: Vec<String>,
    macros: Vec<String>,
    total_files: usize,
    parsed_files: usize,
    failed_files: usize,
    total_source_bytes: u64,
    total_ast_bytes: u64,
    total_vast_bytes: u64,
    total_semantic_graph_bytes: u64,
    wall_ns: u128,
    files: Vec<FileReport>,
    failures: Vec<FileFailure>,
    blockers: Vec<String>,
}

#[derive(Debug, Serialize)]
struct CorpusManifest {
    schema_version: u32,
    corpus_root: String,
    corpus_root_canonical: String,
    linux_subsystem_candidate: bool,
    linux_root: Option<String>,
    linux_subsystem: Option<String>,
    linux_subsystem_depth: usize,
    linux_kbuild_file: Option<String>,
    linux_kbuild_file_in_corpus: bool,
    corpus_fingerprint: String,
    source_collection_mode: String,
    visited_dir_count: usize,
    include_dirs: Vec<String>,
    macros: Vec<String>,
    file_count: usize,
    total_source_bytes: u64,
    files: Vec<CorpusManifestFile>,
}

#[derive(Debug, Serialize)]
struct CorpusManifestFile {
    path: String,
    source_bytes: u64,
    parsed: bool,
    object_bytes: u64,
    ast_bytes: u64,
    vast_bytes: u64,
    semantic_graph_bytes: u64,
    wall_ns: u128,
    error: Option<String>,
}

#[derive(Debug, Serialize)]
struct DiagnosticsSummary {
    schema_version: u32,
    failed_files: usize,
    failures: Vec<FileFailureSummary>,
}

#[derive(Debug, Serialize)]
struct FileFailureSummary {
    path: String,
    error: String,
}

#[derive(Debug, Serialize)]
struct ThroughputSummary {
    schema_version: u32,
    corpus_root_canonical: String,
    linux_subsystem_candidate: bool,
    linux_root: Option<String>,
    linux_subsystem: Option<String>,
    linux_subsystem_depth: usize,
    linux_kbuild_file: Option<String>,
    linux_kbuild_file_in_corpus: bool,
    corpus_fingerprint: String,
    source_collection_mode: String,
    visited_dir_count: usize,
    include_dirs: Vec<String>,
    macros: Vec<String>,
    total_files: usize,
    parsed_files: usize,
    total_source_bytes: u64,
    wall_ns: u128,
    files_per_second_x1000: u128,
    mib_per_second_x1000: u128,
}

#[derive(Debug, Serialize)]
struct FileReport {
    path: String,
    source_bytes: u64,
    object_bytes: u64,
    ast_bytes: u64,
    vast_bytes: u64,
    semantic_graph_bytes: u64,
    wall_ns: u128,
}

#[derive(Debug, Serialize)]
struct FileFailure {
    path: String,
    source_bytes: u64,
    error: String,
}

#[derive(Debug, Clone)]
struct LinuxSubsystemEvidence {
    candidate: bool,
    linux_root: Option<PathBuf>,
    subsystem: Option<String>,
    subsystem_depth: usize,
    linux_kbuild_file: Option<PathBuf>,
}

pub(crate) fn run(args: &[String]) {
    let config = match parse_args(args) {
        Ok(config) => config,
        Err(message) => {
            eprintln!("{message}");
            std::process::exit(2);
        }
    };
    if !config.corpus.is_dir() {
        eprintln!(
            "Fix: --corpus must point at a Linux subsystem directory; `{}` is not a directory.",
            config.corpus.display()
        );
        std::process::exit(1);
    }

    let mut sources = Vec::new();
    let mut visited_dirs = BTreeSet::new();
    let mut collection_failures = Vec::new();
    collect_c_sources(
        &config.corpus,
        &mut sources,
        &mut visited_dirs,
        &mut collection_failures,
    );
    sources.sort();
    let visited_dir_count = visited_dirs.len();
    let source_collection_mode = "recursive_all_c_files".to_string();
    if sources.is_empty() {
        eprintln!(
            "Fix: C parser corpus `{}` contains no .c files.",
            config.corpus.display()
        );
        std::process::exit(1);
    }

    let corpus_root_canonical = config
        .corpus
        .canonicalize()
        .unwrap_or_else(|_| config.corpus.clone());
    let linux_evidence = classify_linux_subsystem(&corpus_root_canonical);
    let linux_subsystem_candidate = linux_evidence.candidate;
    let linux_kbuild_file_in_corpus = linux_evidence
        .linux_kbuild_file
        .as_ref()
        .is_some_and(|path| path.starts_with(&corpus_root_canonical));
    let effective_include_dirs = effective_include_dirs(
        &config.include_dirs,
        &corpus_root_canonical,
        &linux_evidence,
    );
    let effective_macros = effective_macros(&config.macros);
    let total_start = std::time::Instant::now();
    let mut files = Vec::new();
    let mut failures = Vec::new();
    let mut cleanup_failures = Vec::new();
    let mut total_source_bytes = 0u64;
    let mut total_ast_bytes = 0u64;
    let mut total_vast_bytes = 0u64;
    let mut total_semantic_graph_bytes = 0u64;

    for source in sources {
        let source_bytes = match fs::metadata(&source) {
            Ok(metadata) if metadata.is_file() => metadata.len(),
            Ok(_) => {
                failures.push(FileFailure {
                    path: source.display().to_string(),
                    source_bytes: 0,
                    error: "corpus entry is not a file".to_string(),
                });
                continue;
            }
            Err(error) => {
                failures.push(FileFailure {
                    path: source.display().to_string(),
                    source_bytes: 0,
                    error: format!("failed to stat corpus entry: {error}"),
                });
                continue;
            }
        };
        total_source_bytes = total_source_bytes.saturating_add(source_bytes);
        let object = temp_object_path(&source);
        let start = std::time::Instant::now();
        let result = compile(VyreCompileOptions {
            is_compile_only: true,
            input_files: vec![source.clone()],
            output_file: Some(object.clone()),
            include_dirs: effective_include_dirs.clone(),
            quote_include_dirs: Vec::new(),
            system_include_dirs: Vec::new(),
            after_include_dirs: Vec::new(),
            forced_include_files: Vec::new(),
            imacro_files: Vec::new(),
            macros: Vec::new(),
            undefs: Vec::new(),
            macro_actions: define_actions(&effective_macros),
            disable_system_include_dirs: false,
            system_include_sysroot: None,
            target: vyre_frontend_c::api::CTargetOptions::default(),
        });
        let wall_ns = start.elapsed().as_nanos();
        match result {
            Ok(()) => {
                let object_data = match read_object_bounded(&object) {
                    Ok(data) => data,
                    Err(error) => {
                        failures.push(FileFailure {
                            path: source.display().to_string(),
                            source_bytes,
                            error: format!(
                                "compile succeeded but object `{}` could not be read: {error}",
                                object.display()
                            ),
                        });
                        remove_temp_object(&object, &mut cleanup_failures);
                        continue;
                    }
                };
                let object_bytes = object_data.len() as u64;
                let sections = inspect_object_sections(&object_data);
                if sections.ast_bytes == 0
                    || sections.vast_bytes == 0
                    || sections.semantic_graph_bytes == 0
                {
                    failures.push(FileFailure {
                        path: source.display().to_string(),
                        source_bytes,
                        error: format!(
                            "compile succeeded but object sections are incomplete: ast_bytes={}, vast_bytes={}, semantic_graph_bytes={}",
                            sections.ast_bytes,
                            sections.vast_bytes,
                            sections.semantic_graph_bytes
                        ),
                    });
                    remove_temp_object(&object, &mut cleanup_failures);
                    continue;
                }
                total_ast_bytes = total_ast_bytes.saturating_add(sections.ast_bytes);
                total_vast_bytes = total_vast_bytes.saturating_add(sections.vast_bytes);
                total_semantic_graph_bytes =
                    total_semantic_graph_bytes.saturating_add(sections.semantic_graph_bytes);
                files.push(FileReport {
                    path: source.display().to_string(),
                    source_bytes,
                    object_bytes,
                    ast_bytes: sections.ast_bytes,
                    vast_bytes: sections.vast_bytes,
                    semantic_graph_bytes: sections.semantic_graph_bytes,
                    wall_ns,
                });
            }
            Err(error) => failures.push(FileFailure {
                path: source.display().to_string(),
                source_bytes,
                error,
            }),
        }
        remove_temp_object(&object, &mut cleanup_failures);
    }

    let total_files = files.len() + failures.len();
    let mut blockers = Vec::new();
    if total_files < MIN_LINUX_SUBSYSTEM_C_FILES {
        blockers.push(format!(
            "corpus contains {total_files} C file(s), below Linux subsystem floor {MIN_LINUX_SUBSYSTEM_C_FILES}"
        ));
    }
    if !linux_subsystem_candidate {
        blockers.push(format!(
            "corpus root `{}` does not look like a Linux subsystem checkout path",
            corpus_root_canonical.display()
        ));
    }
    if linux_evidence.linux_kbuild_file.is_none() {
        blockers.push(
            "Linux subsystem parser proof must find a Makefile, Kbuild, or Kconfig sentinel"
                .to_string(),
        );
    }
    if !linux_kbuild_file_in_corpus {
        blockers.push(
            "Linux subsystem parser proof must find a Makefile, Kbuild, or Kconfig sentinel inside the selected corpus root"
                .to_string(),
        );
    }
    if linux_evidence.subsystem_depth == 0 {
        blockers.push(
            "Linux subsystem parser proof must point below a subsystem directory, not only at the Linux root or top-level subsystem"
                .to_string(),
        );
    }
    if effective_include_dirs.is_empty() {
        blockers.push(
            "Linux subsystem parser proof must record at least one include directory".to_string(),
        );
    }
    if effective_macros.is_empty() {
        blockers.push(
            "Linux subsystem parser proof must record at least one macro definition".to_string(),
        );
    }
    if total_source_bytes < MIN_LINUX_SUBSYSTEM_SOURCE_BYTES {
        blockers.push(format!(
            "corpus contains {total_source_bytes} source byte(s), below Linux subsystem floor {MIN_LINUX_SUBSYSTEM_SOURCE_BYTES}"
        ));
    }
    if total_ast_bytes == 0 {
        blockers.push("compiled corpus emitted zero AST section bytes".to_string());
    }
    if total_vast_bytes == 0 {
        blockers.push("compiled corpus emitted zero VAST section bytes".to_string());
    }
    if total_semantic_graph_bytes == 0 {
        blockers
            .push("compiled corpus emitted zero semantic ProgramGraph section bytes".to_string());
    }
    if !failures.is_empty() {
        blockers.push(format!("{} C parser failure(s) remain", failures.len()));
    }
    blockers.extend(collection_failures);
    blockers.extend(cleanup_failures);
    let corpus_fingerprint = corpus_fingerprint(&corpus_root_canonical, &files, &failures);
    let report = CorpusReport {
        schema_version: 1,
        corpus_root: config.corpus.display().to_string(),
        corpus_root_canonical: corpus_root_canonical.display().to_string(),
        linux_subsystem_candidate,
        linux_root: linux_evidence
            .linux_root
            .as_ref()
            .map(|path| path.display().to_string()),
        linux_subsystem: linux_evidence.subsystem.clone(),
        linux_subsystem_depth: linux_evidence.subsystem_depth,
        linux_kbuild_file: linux_evidence
            .linux_kbuild_file
            .as_ref()
            .map(|path| path.display().to_string()),
        linux_kbuild_file_in_corpus,
        corpus_fingerprint,
        source_collection_mode,
        visited_dir_count,
        include_dirs: effective_include_dirs
            .iter()
            .map(|path| path.display().to_string())
            .collect(),
        macros: render_macros(&effective_macros),
        total_files,
        parsed_files: files.len(),
        failed_files: failures.len(),
        total_source_bytes,
        total_ast_bytes,
        total_vast_bytes,
        total_semantic_graph_bytes,
        wall_ns: total_start.elapsed().as_nanos(),
        files,
        failures,
        blockers,
    };
    let json = match serde_json::to_string_pretty(&report) {
        Ok(json) => json,
        Err(error) => {
            eprintln!("Fix: failed to serialize C parser corpus report: {error}");
            std::process::exit(1);
        }
    };
    if let Some(parent) = config.output.parent() {
        if let Err(error) = fs::create_dir_all(parent) {
            eprintln!("Fix: failed to create `{}`: {error}", parent.display());
            std::process::exit(1);
        }
    }
    if let Err(error) = fs::write(&config.output, format!("{json}\n")) {
        eprintln!(
            "Fix: failed to write `{}`: {error}",
            config.output.display()
        );
        std::process::exit(1);
    }
    if let Err(error) = write_sibling_evidence(&config.output, &report) {
        eprintln!("Fix: failed to write C parser sibling evidence: {error}");
        std::process::exit(1);
    }
    println!(
        "c-parser-corpus: parsed {}/{} file(s), wrote {}",
        report.parsed_files,
        report.total_files,
        config.output.display()
    );
    if !report.blockers.is_empty() {
        std::process::exit(1);
    }
}

fn define_actions(macros: &[(String, Option<String>)]) -> Vec<CliMacroAction> {
    macros
        .iter()
        .map(|(name, value)| CliMacroAction::Define {
            name: name.clone(),
            value: value.clone(),
        })
        .collect()
}

fn remove_temp_object(object: &Path, cleanup_failures: &mut Vec<String>) {
    match fs::remove_file(object) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => cleanup_failures.push(format!(
                "temporary object cleanup failed for `{}`: {error}. Fix: verify parser corpus output directory permissions and cleanup stale artifacts before rerunning.",
                object.display()
            )),
    }
}

fn write_sibling_evidence(output: &Path, report: &CorpusReport) -> Result<(), String> {
    let dir = output
        .parent()
        .ok_or_else(|| "output path has no parent directory".to_string())?;
    let manifest = CorpusManifest {
        schema_version: 1,
        corpus_root: report.corpus_root.clone(),
        corpus_root_canonical: report.corpus_root_canonical.clone(),
        linux_subsystem_candidate: report.linux_subsystem_candidate,
        linux_root: report.linux_root.clone(),
        linux_subsystem: report.linux_subsystem.clone(),
        linux_subsystem_depth: report.linux_subsystem_depth,
        linux_kbuild_file: report.linux_kbuild_file.clone(),
        linux_kbuild_file_in_corpus: report.linux_kbuild_file_in_corpus,
        corpus_fingerprint: report.corpus_fingerprint.clone(),
        source_collection_mode: report.source_collection_mode.clone(),
        visited_dir_count: report.visited_dir_count,
        include_dirs: report.include_dirs.clone(),
        macros: report.macros.clone(),
        file_count: report.total_files,
        total_source_bytes: report.total_source_bytes,
        files: report
            .files
            .iter()
            .map(|file| CorpusManifestFile {
                path: file.path.clone(),
                source_bytes: file.source_bytes,
                parsed: true,
                object_bytes: file.object_bytes,
                ast_bytes: file.ast_bytes,
                vast_bytes: file.vast_bytes,
                semantic_graph_bytes: file.semantic_graph_bytes,
                wall_ns: file.wall_ns,
                error: None,
            })
            .chain(report.failures.iter().map(|failure| CorpusManifestFile {
                path: failure.path.clone(),
                source_bytes: failure.source_bytes,
                parsed: false,
                object_bytes: 0,
                ast_bytes: 0,
                vast_bytes: 0,
                semantic_graph_bytes: 0,
                wall_ns: 0,
                error: Some(failure.error.clone()),
            }))
            .collect(),
    };
    let diagnostics = DiagnosticsSummary {
        schema_version: 1,
        failed_files: report.failed_files,
        failures: report
            .failures
            .iter()
            .map(|failure| FileFailureSummary {
                path: failure.path.clone(),
                error: failure.error.clone(),
            })
            .collect(),
    };
    let throughput = ThroughputSummary {
        schema_version: 1,
        corpus_root_canonical: report.corpus_root_canonical.clone(),
        linux_subsystem_candidate: report.linux_subsystem_candidate,
        linux_root: report.linux_root.clone(),
        linux_subsystem: report.linux_subsystem.clone(),
        linux_subsystem_depth: report.linux_subsystem_depth,
        linux_kbuild_file: report.linux_kbuild_file.clone(),
        linux_kbuild_file_in_corpus: report.linux_kbuild_file_in_corpus,
        corpus_fingerprint: report.corpus_fingerprint.clone(),
        source_collection_mode: report.source_collection_mode.clone(),
        visited_dir_count: report.visited_dir_count,
        include_dirs: report.include_dirs.clone(),
        macros: report.macros.clone(),
        total_files: report.total_files,
        parsed_files: report.parsed_files,
        total_source_bytes: report.total_source_bytes,
        wall_ns: report.wall_ns,
        files_per_second_x1000: rate_x1000(report.parsed_files as u128, report.wall_ns),
        mib_per_second_x1000: mib_rate_x1000(report.total_source_bytes, report.wall_ns),
    };
    write_json(&dir.join("linux-subsystem-corpus-manifest.json"), &manifest)?;
    write_json(&dir.join("c-parser-diagnostics-summary.json"), &diagnostics)?;
    write_json(&dir.join("c-parser-throughput.json"), &throughput)?;
    Ok(())
}

fn read_object_bounded(path: &Path) -> std::io::Result<Vec<u8>> {
    use std::io::Read as _;

    let mut file = fs::File::open(path)?;
    let metadata = file.metadata()?;
    if metadata.len() > MAX_C_PARSER_OBJECT_BYTES {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("C parser object exceeds {MAX_C_PARSER_OBJECT_BYTES} byte limit"),
        ));
    }
    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    file.by_ref()
        .take(MAX_C_PARSER_OBJECT_BYTES + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_C_PARSER_OBJECT_BYTES {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "C parser object exceeded bounded read limit",
        ));
    }
    Ok(bytes)
}

fn corpus_fingerprint(root: &Path, files: &[FileReport], failures: &[FileFailure]) -> String {
    let mut entries = files
        .iter()
        .map(|file| (fingerprint_path(root, &file.path), file.source_bytes))
        .chain(
            failures
                .iter()
                .map(|failure| (fingerprint_path(root, &failure.path), failure.source_bytes)),
        )
        .collect::<Vec<_>>();
    entries.sort_by(|left, right| left.0.cmp(&right.0));
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    for (path, bytes) in entries {
        for byte in path.as_bytes() {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
        hash ^= bytes;
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("fnv64:{hash:016x}")
}

fn fingerprint_path(root: &Path, path: &str) -> String {
    let path = Path::new(path);
    path.strip_prefix(root)
        .unwrap_or(path)
        .display()
        .to_string()
}

fn classify_linux_subsystem(path: &Path) -> LinuxSubsystemEvidence {
    let components = path
        .components()
        .filter_map(|component| component.as_os_str().to_str())
        .map(str::to_ascii_lowercase)
        .collect::<Vec<_>>();
    let linux_index = components
        .iter()
        .position(|component| component == "linux" || component.starts_with("linux-"));
    let subsystem_index = components.iter().position(|component| {
        matches!(
            component.as_str(),
            "kernel" | "fs" | "mm" | "net" | "drivers"
        )
    });
    let subsystem = subsystem_index.and_then(|index| components.get(index).cloned());
    let subsystem_depth = match (linux_index, subsystem_index) {
        (Some(linux), Some(subsystem)) if subsystem >= linux => subsystem - linux,
        _ => 0,
    };
    let linux_root = linux_index.map(|index| {
        let mut root = PathBuf::new();
        for component in path.components().take(index + 1) {
            root.push(component.as_os_str());
        }
        root
    });
    let linux_kbuild_file = find_kbuild_sentinel(path, linux_root.as_deref());
    LinuxSubsystemEvidence {
        candidate: linux_index.is_some() && subsystem_index.is_some(),
        linux_root,
        subsystem,
        subsystem_depth,
        linux_kbuild_file,
    }
}

fn find_kbuild_sentinel(path: &Path, linux_root: Option<&Path>) -> Option<PathBuf> {
    let mut current = Some(path);
    while let Some(dir) = current {
        for file in ["Kbuild", "Kconfig", "Makefile"] {
            let candidate = dir.join(file);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
        if linux_root.is_some_and(|root| root == dir) {
            break;
        }
        current = dir.parent();
    }
    None
}

fn effective_include_dirs(
    requested: &[PathBuf],
    corpus_root: &Path,
    linux: &LinuxSubsystemEvidence,
) -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    for dir in requested {
        push_unique_existing_dir(&mut dirs, dir.clone());
    }
    push_unique_existing_dir(&mut dirs, corpus_root.to_path_buf());
    if let Some(parent) = corpus_root.parent() {
        push_unique_existing_dir(&mut dirs, parent.to_path_buf());
    }
    if let Some(root) = linux.linux_root.as_deref() {
        for rel in [
            "include",
            "include/uapi",
            "include/generated",
            "include/generated/uapi",
            "arch/x86/include",
            "arch/x86/include/uapi",
            "arch/x86/include/generated",
            "arch/x86/include/generated/uapi",
            "tools/include",
            "tools/include/uapi",
        ] {
            push_unique_existing_dir(&mut dirs, root.join(rel));
        }
    }
    dirs
}

fn effective_macros(requested: &[(String, Option<String>)]) -> Vec<(String, Option<String>)> {
    let mut macros = requested.to_vec();
    for (name, value) in [
        ("__KERNEL__", None),
        ("CONFIG_64BIT", Some("1")),
        ("CONFIG_X86_64", Some("1")),
    ] {
        if !macros.iter().any(|(existing, _)| existing == name) {
            macros.push((name.to_string(), value.map(str::to_string)));
        }
    }
    macros
}

fn push_unique_existing_dir(dirs: &mut Vec<PathBuf>, dir: PathBuf) {
    if dir.is_dir() && !dirs.iter().any(|existing| existing == &dir) {
        dirs.push(dir);
    }
}

fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<(), String> {
    let json = serde_json::to_string_pretty(value).map_err(|error| error.to_string())?;
    fs::write(path, format!("{json}\n")).map_err(|error| format!("{}: {error}", path.display()))
}

fn rate_x1000(units: u128, wall_ns: u128) -> u128 {
    if wall_ns == 0 {
        return 0;
    }
    units.saturating_mul(1_000_000_000_000) / wall_ns
}

#[derive(Default)]
struct ObjectSectionSummary {
    ast_bytes: u64,
    vast_bytes: u64,
    semantic_graph_bytes: u64,
}

fn inspect_object_sections(object: &[u8]) -> ObjectSectionSummary {
    let Some(start) = find_magic(object, VYRECOB2_MAGIC) else {
        return ObjectSectionSummary::default();
    };
    let mut cursor = start + VYRECOB2_MAGIC.len();
    let Some(_version) = read_u32(object, &mut cursor) else {
        return ObjectSectionSummary::default();
    };
    let Some(section_count) = read_u32(object, &mut cursor) else {
        return ObjectSectionSummary::default();
    };
    let mut summary = ObjectSectionSummary::default();
    for _ in 0..section_count {
        let Some(tag) = read_u32(object, &mut cursor) else {
            break;
        };
        let Some(len) = read_u32(object, &mut cursor) else {
            break;
        };
        let len = len as usize;
        if cursor.saturating_add(len) > object.len() {
            break;
        }
        match tag {
            tag if tag == SectionTag::Ast as u32 => {
                summary.ast_bytes = summary.ast_bytes.saturating_add(len as u64);
            }
            tag if tag == SectionTag::Vast as u32 => {
                summary.vast_bytes = summary.vast_bytes.saturating_add(len as u64);
            }
            tag if tag == SectionTag::SemanticProgramGraphNodes as u32
                || tag == SectionTag::SemanticProgramGraphEdges as u32 =>
            {
                summary.semantic_graph_bytes =
                    summary.semantic_graph_bytes.saturating_add(len as u64);
            }
            _ => {}
        }
        cursor += len;
    }
    summary
}

fn find_magic(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

fn read_u32(bytes: &[u8], cursor: &mut usize) -> Option<u32> {
    let end = cursor.checked_add(4)?;
    let raw: [u8; 4] = bytes.get(*cursor..end)?.try_into().ok()?;
    *cursor = end;
    Some(u32::from_le_bytes(raw))
}

fn mib_rate_x1000(bytes: u64, wall_ns: u128) -> u128 {
    rate_x1000(u128::from(bytes), wall_ns) / 1_048_576
}

struct Config {
    corpus: PathBuf,
    output: PathBuf,
    include_dirs: Vec<PathBuf>,
    macros: Vec<(String, Option<String>)>,
}

fn parse_args(args: &[String]) -> Result<Config, String> {
    let mut corpus = None;
    let mut output = None;
    let mut include_dirs = Vec::new();
    let mut macros = Vec::new();
    let mut index = 2;
    while index < args.len() {
        match args[index].as_str() {
            "--corpus" => {
                let Some(path) = args.get(index + 1) else {
                    return Err("Fix: --corpus requires a directory.".to_string());
                };
                corpus = Some(PathBuf::from(path));
                index += 2;
            }
            "--output" => {
                let Some(path) = args.get(index + 1) else {
                    return Err("Fix: --output requires a path.".to_string());
                };
                output = Some(PathBuf::from(path));
                index += 2;
            }
            "-I" | "--include" => {
                let Some(path) = args.get(index + 1) else {
                    return Err("Fix: include option requires a directory.".to_string());
                };
                include_dirs.push(PathBuf::from(path));
                index += 2;
            }
            "-D" | "--define" => {
                let Some(def) = args.get(index + 1) else {
                    return Err("Fix: define option requires NAME or NAME=VALUE.".to_string());
                };
                macros.push(parse_define(def));
                index += 2;
            }
            "--help" | "-h" => {
                println!(
                    "USAGE:\n  cargo_full run --bin xtask -- c-parser-corpus [--corpus DIR] [--output PATH] [-I DIR] [-D NAME[=VALUE]]\n\n\
                     Compiles every .c file under a Linux subsystem directory through vyre-frontend-c and writes release evidence JSON. If --corpus is omitted, VYRE_LINUX_SUBSYSTEM_CORPUS and standard local Linux checkout locations are probed."
                );
                std::process::exit(0);
            }
            other => return Err(format!("Fix: unknown c-parser-corpus option `{other}`.")),
        }
    }
    let corpus = match corpus {
        Some(corpus) => corpus,
        None => default_corpus().ok_or_else(|| {
            "Fix: c-parser-corpus needs --corpus DIR or VYRE_LINUX_SUBSYSTEM_CORPUS pointing at a real Linux subsystem checkout.".to_string()
        })?,
    };
    Ok(Config {
        corpus,
        output: output.unwrap_or_else(default_output),
        include_dirs,
        macros,
    })
}

fn default_corpus() -> Option<PathBuf> {
    if let Ok(path) = std::env::var("VYRE_LINUX_SUBSYSTEM_CORPUS") {
        let path = PathBuf::from(path);
        if path.is_dir() {
            return Some(path);
        }
    }
    for path in [
        "/media/mukund-thiru/SanthData/linux/drivers",
        "/media/mukund-thiru/SanthData/linux/net",
        "/media/mukund-thiru/SanthData/linux/fs",
        "/media/mukund-thiru/SanthData/linux/kernel",
        "/media/mukund-thiru/SanthData/linux/mm",
        "/media/mukund-thiru/SanthData/Santh/linux/drivers",
        "/media/mukund-thiru/SanthData/Santh/linux/net",
        "/media/mukund-thiru/SanthData/Santh/linux/fs",
        "/media/mukund-thiru/SanthData/Santh/linux/kernel",
        "/media/mukund-thiru/SanthData/Santh/linux/mm",
        "/media/mukund-thiru/SanthData/linux",
        "/media/mukund-thiru/SanthData/Santh/linux",
    ] {
        let path = PathBuf::from(path);
        if path.is_dir() {
            return Some(path);
        }
    }
    None
}

fn parse_define(value: &str) -> (String, Option<String>) {
    match value.split_once('=') {
        Some((name, body)) => (name.to_string(), Some(body.to_string())),
        None => (value.to_string(), None),
    }
}

fn render_macros(macros: &[(String, Option<String>)]) -> Vec<String> {
    macros
        .iter()
        .map(|(name, value)| match value {
            Some(value) => format!("{name}={value}"),
            None => name.clone(),
        })
        .collect()
}

fn collect_c_sources(
    dir: &Path,
    out: &mut Vec<PathBuf>,
    visited_dirs: &mut BTreeSet<PathBuf>,
    collection_failures: &mut Vec<String>,
) {
    let canonical = match dir.canonicalize() {
        Ok(path) => path,
        Err(error) => {
            collection_failures.push(format!(
                "failed to canonicalize corpus directory `{}`: {error}",
                dir.display()
            ));
            return;
        }
    };
    if !visited_dirs.insert(canonical) {
        return;
    }
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(error) => {
            collection_failures.push(format!(
                "failed to read corpus directory `{}`: {error}",
                dir.display()
            ));
            return;
        }
    };
    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) => {
                collection_failures.push(format!(
                    "failed to read corpus directory entry under `{}`: {error}",
                    dir.display()
                ));
                continue;
            }
        };
        let path = entry.path();
        if path.is_dir() {
            collect_c_sources(&path, out, visited_dirs, collection_failures);
        } else if path.extension().and_then(|ext| ext.to_str()) == Some("c") {
            out.push(path);
        }
    }
}

fn temp_object_path(source: &Path) -> PathBuf {
    let pid = std::process::id();
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    let stem = source
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("tu");
    let sequence = TEMP_OBJECT_COUNTER.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "vyre-c-parser-corpus-{stem}-{pid}-{nanos}-{sequence}.o"
    ))
}

fn default_output() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(|path| path.join("release/evidence/parser/c-parser-linux-subsystem.json"))
        .unwrap_or_else(|| PathBuf::from("release/evidence/parser/c-parser-linux-subsystem.json"))
}
