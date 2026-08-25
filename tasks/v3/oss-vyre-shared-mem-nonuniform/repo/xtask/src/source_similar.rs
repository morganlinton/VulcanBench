//! Repo-wide Rust source duplication scanner.
//!
//! `whats-similar` catches duplicate registered IR programs. This command
//! catches the other class: forked Rust source that has not reached inventory
//! registration yet. It uses normalized token shingles so renamed variables do
//! not hide duplicated implementation skeletons.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process;

use crate::dedup_report::{
    duplicate_family_report, duplicate_report_generator_command, duplicate_report_json_path,
    duplicate_severity, source_duplicate_family_id, source_duplicate_subject,
    source_token_fingerprint,
    write_duplicate_report_json, DuplicateEvidence, DuplicateFamilyFinding, DuplicateFamilyReport,
};
use crate::ownership::{load_ownership_lanes, owner_lane_for_file, OwnershipLaneRule};

const DEFAULT_TOP_N: usize = 20;
const DEFAULT_MIN_SCORE: f64 = 0.86;
const DEFAULT_MAX_FILE_BYTES: u64 = 512 * 1024;
const SHINGLE_WIDTH: usize = 8;
const MIN_SOURCE_TOKENS: usize = 96;
const MAX_CANDIDATE_SHINGLE_FANOUT: usize = 64;
const MIN_SHARED_RARE_SHINGLES: u16 = 16;

#[derive(Debug, Clone)]
struct Config {
    roots: Vec<PathBuf>,
    top_n: usize,
    min_score: f64,
    max_file_bytes: u64,
    fail_on_findings: bool,
    include_untracked: bool,
    duplicate_report_json: Option<PathBuf>,
}

#[derive(Debug, Clone)]
struct SourceFingerprint {
    path: PathBuf,
    bytes: u64,
    tokens: usize,
    fingerprint: String,
    shingles: HashMap<u64, u32>,
    magnitude: f64,
}

#[derive(Debug, Clone)]
struct SimilarPair {
    score: f64,
    left: usize,
    right: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct SourceSimilarityFinding {
    pub(crate) score: f64,
    pub(crate) left: String,
    pub(crate) right: String,
    pub(crate) left_tokens: usize,
    pub(crate) right_tokens: usize,
    pub(crate) left_bytes: u64,
    pub(crate) right_bytes: u64,
    pub(crate) left_fingerprint: String,
    pub(crate) right_fingerprint: String,
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct DedupGuidance {
    left_owner_lane: String,
    right_owner_lane: String,
    import_owner: String,
    import_target: String,
}

pub(crate) fn run(args: &[String]) {
    let config = match parse_args(args) {
        Ok(config) => config,
        Err(error) => {
            eprintln!("Fix: {error}");
            print_usage();
            process::exit(1);
        }
    };

    let report = match find_similar_sources(
        &config.roots,
        config.top_n,
        config.min_score,
        config.max_file_bytes,
        config.include_untracked,
    ) {
        Ok(report) => report,
        Err(error) => {
            eprintln!("Fix: source-similar scan failed: {error}");
            process::exit(1);
        }
    };
    let workspace_root = match workspace_root() {
        Some(root) => root,
        None => {
            eprintln!("Fix: source-similar must run from an xtask crate with a workspace parent.");
            process::exit(1);
        }
    };
    let ownership_path = workspace_root
        .join("docs")
        .join("optimization")
        .join("OWNERSHIP.toml");
    let ownership_lanes = match load_ownership_lanes(&ownership_path) {
        Ok(lanes) => lanes,
        Err(error) => {
            eprintln!(
                "Fix: source-similar could not load ownership map `{}`: {error}",
                ownership_path.display()
            );
            process::exit(1);
        }
    };
    if let Some(path) = config.duplicate_report_json.as_ref() {
        let generator_command = duplicate_report_generator_command("source-similar", path);
        let duplicate_report =
            source_similarity_duplicate_report(&report, &ownership_lanes, &generator_command);
        if let Err(error) = write_duplicate_report_json(path, &duplicate_report) {
            eprintln!(
                "Fix: source-similar could not write duplicate family report `{}`: {error}",
                path.display()
            );
            process::exit(1);
        }
    }

    println!(
        "source-similar: scanned {} Rust files under {} root(s) (min={:.2}, top={}, shingle_width={})",
        report.scanned_files,
        config.roots.len(),
        config.min_score,
        config.top_n,
        SHINGLE_WIDTH
    );
    if report.findings.is_empty() {
        println!("  no Rust source file pairs crossed the duplication floor.");
        return;
    }
    for (index, finding) in report.findings.iter().enumerate() {
        println!(
            "  {:>2}. {:>5.1}%  {}",
            index + 1,
            finding.score * 100.0,
            pair_verdict(finding.score)
        );
        println!(
            "      A: {} tokens={} bytes={}",
            finding.left, finding.left_tokens, finding.left_bytes
        );
        println!(
            "      B: {} tokens={} bytes={}",
            finding.right, finding.right_tokens, finding.right_bytes
        );
        let guidance = dedup_guidance_for_pair(&finding.left, &finding.right, &ownership_lanes);
        println!(
            "      Dedup: left_owner={} right_owner={} import_owner={} import_target={}",
            guidance.left_owner_lane,
            guidance.right_owner_lane,
            guidance.import_owner,
            guidance.import_target
        );
    }
    if config.fail_on_findings {
        eprintln!(
            "Fix: source-similar found {} duplicate/similar Rust source pair(s) at score >= {:.2}. Extract a shared module or lower --min only for exploratory scans.",
            report.findings.len(),
            config.min_score
        );
        process::exit(1);
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct SourceSimilarityReport {
    pub(crate) scanned_files: usize,
    pub(crate) findings: Vec<SourceSimilarityFinding>,
}

pub(crate) fn find_similar_sources(
    roots: &[PathBuf],
    top_n: usize,
    min_score: f64,
    max_file_bytes: u64,
    include_untracked: bool,
) -> Result<SourceSimilarityReport, String> {
    let files = collect_rust_files(roots, max_file_bytes)?;
    let files = if include_untracked {
        files
    } else {
        filter_to_tracked_rust_files_if_git(roots, files)?
    };
    let fingerprints = fingerprint_files(&files);
    let pairs = score_pairs(&fingerprints, top_n, min_score);
    let findings = pairs
        .into_iter()
        .map(|pair| {
            let left = &fingerprints[pair.left];
            let right = &fingerprints[pair.right];
            SourceSimilarityFinding {
                score: pair.score,
                left: display_path(&left.path),
                right: display_path(&right.path),
                left_tokens: left.tokens,
                right_tokens: right.tokens,
                left_bytes: left.bytes,
                right_bytes: right.bytes,
                left_fingerprint: left.fingerprint.clone(),
                right_fingerprint: right.fingerprint.clone(),
            }
        })
        .collect();
    Ok(SourceSimilarityReport {
        scanned_files: fingerprints.len(),
        findings,
    })
}

fn parse_args(args: &[String]) -> Result<Config, String> {
    let mut roots = Vec::new();
    let mut top_n = DEFAULT_TOP_N;
    let mut min_score = DEFAULT_MIN_SCORE;
    let mut max_file_bytes = DEFAULT_MAX_FILE_BYTES;
    let mut fail_on_findings = false;
    let mut include_untracked = false;
    let mut duplicate_report_json = None;
    let mut index = 2usize;
    while index < args.len() {
        match args[index].as_str() {
            "--root" => {
                index += 1;
                let Some(root) = args.get(index) else {
                    return Err("--root requires a path".to_string());
                };
                roots.push(PathBuf::from(root));
            }
            "--top" => {
                index += 1;
                let Some(raw) = args.get(index) else {
                    return Err("--top requires a positive integer".to_string());
                };
                top_n = raw
                    .parse::<usize>()
                    .map_err(|_| format!("--top must be an integer, got `{raw}`"))?;
                if top_n == 0 {
                    return Err("--top must be greater than zero".to_string());
                }
            }
            "--min" => {
                index += 1;
                let Some(raw) = args.get(index) else {
                    return Err("--min requires a score in 0.0..=1.0".to_string());
                };
                min_score = raw
                    .parse::<f64>()
                    .map_err(|_| format!("--min must be a float, got `{raw}`"))?;
                if !(0.0..=1.0).contains(&min_score) {
                    return Err("--min must be in 0.0..=1.0".to_string());
                }
            }
            "--max-file-bytes" => {
                index += 1;
                let Some(raw) = args.get(index) else {
                    return Err("--max-file-bytes requires a positive integer".to_string());
                };
                max_file_bytes = raw
                    .parse::<u64>()
                    .map_err(|_| format!("--max-file-bytes must be an integer, got `{raw}`"))?;
                if max_file_bytes == 0 {
                    return Err("--max-file-bytes must be greater than zero".to_string());
                }
            }
            "--fail-on-findings" | "--check" => {
                fail_on_findings = true;
            }
            "--include-untracked" => {
                include_untracked = true;
            }
            "--duplicate-report-json" => {
                index += 1;
                duplicate_report_json = Some(duplicate_report_json_path(
                    "--duplicate-report-json",
                    args.get(index).map(String::as_str),
                    "--duplicate-report-json requires a path",
                )?);
            }
            "--help" | "-h" => {
                print_usage();
                process::exit(0);
            }
            other => return Err(format!("unknown source-similar option `{other}`")),
        }
        index += 1;
    }
    if roots.is_empty() {
        roots = default_roots();
    }
    Ok(Config {
        roots,
        top_n,
        min_score,
        max_file_bytes,
        fail_on_findings,
        include_untracked,
        duplicate_report_json,
    })
}

pub(crate) fn source_similarity_duplicate_report(
    report: &SourceSimilarityReport,
    ownership_lanes: &[OwnershipLaneRule],
    generator_command: &str,
) -> DuplicateFamilyReport {
    let families = report
        .findings
        .iter()
        .map(|finding| {
            let guidance =
                dedup_guidance_for_pair(&finding.left, &finding.right, ownership_lanes);
            DuplicateFamilyFinding {
                family_id: source_duplicate_family_id(&finding.left, &finding.right),
                detector: "source-similar".to_string(),
                severity: duplicate_severity(finding.score),
                score: finding.score,
                left: source_duplicate_subject(
                    &finding.left,
                    &guidance.left_owner_lane,
                    &finding.left_fingerprint,
                    finding.left_tokens,
                    finding.left_bytes,
                ),
                right: source_duplicate_subject(
                    &finding.right,
                    &guidance.right_owner_lane,
                    &finding.right_fingerprint,
                    finding.right_tokens,
                    finding.right_bytes,
                ),
                import_owner: guidance.import_owner,
                import_target: guidance.import_target,
                evidence: DuplicateEvidence {
                    similarity_metric: "normalized-token-shingle-cosine",
                    left_metric: format!(
                        "tokens={}:bytes={}",
                        finding.left_tokens, finding.left_bytes
                    ),
                    right_metric: format!(
                        "tokens={}:bytes={}",
                        finding.right_tokens, finding.right_bytes
                    ),
                    dedup_action: "extract_shared_module_or_import_existing_owner",
                },
            }
        })
        .collect();
    duplicate_family_report(
        generator_command,
        "rust-source-token-shingles",
        families,
    )
}

fn print_usage() {
    eprintln!(
        "USAGE:\n  cargo_full run --bin xtask -- source-similar [--root PATH] [--top N] [--min SCORE] [--max-file-bytes BYTES] [--fail-on-findings] [--include-untracked] [--duplicate-report-json PATH]\n\n\
         Defaults scan tracked Rust files under the Vyre workspace source roots and report high-confidence renamed/forked source skeletons."
    );
}

fn workspace_root() -> Option<PathBuf> {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(Path::to_path_buf)
}

fn default_roots() -> Vec<PathBuf> {
    [
        "vyre-core",
        "vyre-foundation",
        "vyre-driver",
        "vyre-driver-cuda",
        "vyre-driver-wgpu",
        "vyre-driver-spirv",
        "vyre-reference",
        "vyre-spec",
        "vyre-primitives",
        "vyre-self-substrate",
        "vyre-runtime",
        "vyre-libs",
        "vyre-intrinsics",
        "vyre-aot",
        "vyre-frontend-c",
        "vyre-bench",
        "vyre-lower",
        "vyre-emit-ptx",
        "vyre-emit-spirv",
        "vyre-emit-naga",
        "xtask",
        "conform",
    ]
    .into_iter()
    .map(PathBuf::from)
    .filter(|path| path.exists())
    .collect()
}

fn collect_rust_files(roots: &[PathBuf], max_file_bytes: u64) -> Result<Vec<PathBuf>, String> {
    let mut files = Vec::new();
    let mut seen = HashSet::new();
    for root in roots {
        collect_rust_files_recursive(root, max_file_bytes, &mut files, &mut seen)?;
    }
    files.sort();
    Ok(files)
}

fn filter_to_tracked_rust_files_if_git(
    roots: &[PathBuf],
    files: Vec<PathBuf>,
) -> Result<Vec<PathBuf>, String> {
    let git_roots = git_roots_for(roots);
    if git_roots.is_empty() {
        return Ok(files);
    }
    let mut tracked = HashSet::new();
    for git_root in &git_roots {
        tracked.extend(tracked_rust_files(git_root)?);
    }
    Ok(files
        .into_iter()
        .filter(|path| {
            let normalized = normalize_existing_path(path);
            !is_under_any(&normalized, &git_roots) || tracked.contains(&normalized)
        })
        .collect())
}

fn git_roots_for(roots: &[PathBuf]) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    for root in roots {
        let output = process::Command::new("git")
            .args([
                "-C",
                &root.to_string_lossy(),
                "rev-parse",
                "--show-toplevel",
            ])
            .output();
        let Ok(output) = output else {
            continue;
        };
        if !output.status.success() {
            continue;
        }
        let text = String::from_utf8_lossy(&output.stdout);
        let normalized = normalize_existing_path(Path::new(text.trim()));
        if seen.insert(normalized.clone()) {
            out.push(normalized);
        }
    }
    out
}

fn tracked_rust_files(git_root: &Path) -> Result<HashSet<PathBuf>, String> {
    let output = process::Command::new("git")
        .args([
            "-C",
            &git_root.to_string_lossy(),
            "ls-files",
            "-z",
            "--",
            "*.rs",
        ])
        .output()
        .map_err(|error| {
            format!(
                "could not list tracked Rust files under `{}`: {error}",
                git_root.display()
            )
        })?;
    if !output.status.success() {
        return Err(format!(
            "git ls-files failed under `{}`: {}",
            git_root.display(),
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(output
        .stdout
        .split(|byte| *byte == 0)
        .filter(|entry| !entry.is_empty())
        .map(|entry| git_root.join(String::from_utf8_lossy(entry).as_ref()))
        .map(|path| normalize_existing_path(&path))
        .collect())
}

fn is_under_any(path: &Path, roots: &[PathBuf]) -> bool {
    roots.iter().any(|root| path.starts_with(root))
}

fn normalize_existing_path(path: &Path) -> PathBuf {
    fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

fn collect_rust_files_recursive(
    path: &Path,
    max_file_bytes: u64,
    files: &mut Vec<PathBuf>,
    seen: &mut HashSet<PathBuf>,
) -> Result<(), String> {
    if should_skip_path(path) {
        return Ok(());
    }
    let meta = fs::metadata(path)
        .map_err(|error| format!("could not stat `{}`: {error}", path.display()))?;
    if meta.is_dir() {
        for entry in fs::read_dir(path)
            .map_err(|error| format!("could not read `{}`: {error}", path.display()))?
        {
            let entry = entry.map_err(|error| {
                format!("could not read entry in `{}`: {error}", path.display())
            })?;
            collect_rust_files_recursive(&entry.path(), max_file_bytes, files, seen)?;
        }
        return Ok(());
    }
    if meta.len() > max_file_bytes || path.extension().and_then(|ext| ext.to_str()) != Some("rs") {
        return Ok(());
    }
    let canonical = fs::canonicalize(path)
        .map_err(|error| format!("could not canonicalize `{}`: {error}", path.display()))?;
    if seen.insert(canonical) {
        files.push(path.to_path_buf());
    }
    Ok(())
}

fn should_skip_path(path: &Path) -> bool {
    path.components().any(|component| {
        let name = component.as_os_str().to_string_lossy();
        matches!(
            name.as_ref(),
            ".git"
                | "target"
                | ".pytest_cache"
                | "__pycache__"
                | ".cursor"
                | ".internals"
                | "jules_tickets"
                | "__split"
                | "__law7_split"
        )
    })
}

fn fingerprint_files(files: &[PathBuf]) -> Vec<SourceFingerprint> {
    let mut out = Vec::new();
    for path in files {
        let Ok(source) = fs::read_to_string(path) else {
            continue;
        };
        if is_declarative_catalog_source(&source) {
            continue;
        }
        let tokens = normalize_tokens(&source);
        if tokens.len() < MIN_SOURCE_TOKENS {
            continue;
        }
        let shingles = shingle_counts(&tokens, SHINGLE_WIDTH);
        if shingles.is_empty() {
            continue;
        }
        let magnitude = magnitude(&shingles);
        let fingerprint = source_token_fingerprint(&tokens);
        out.push(SourceFingerprint {
            path: path.clone(),
            bytes: source.len() as u64,
            tokens: tokens.len(),
            fingerprint,
            shingles,
            magnitude,
        });
    }
    out
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DeclarationBlock {
    ConstCatalog,
    ModuleIndex,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
struct SourceShape {
    meaningful_lines: usize,
    declaration_lines: usize,
    const_catalog_lines: usize,
    module_index_lines: usize,
    function_lines: usize,
    control_flow_lines: usize,
}

fn is_declarative_catalog_source(source: &str) -> bool {
    let shape = source_shape(source);
    if shape.meaningful_lines < 6 {
        return false;
    }

    let declaration_ratio = shape.declaration_lines * 100 / shape.meaningful_lines;
    let const_ratio = shape.const_catalog_lines * 100 / shape.meaningful_lines;
    let module_ratio = shape.module_index_lines * 100 / shape.meaningful_lines;

    let module_index = shape.module_index_lines >= 4
        && module_ratio >= 60
        && declaration_ratio >= 70
        && shape.function_lines == 0
        && shape.control_flow_lines == 0;
    let const_catalog = shape.const_catalog_lines >= 8
        && const_ratio >= 35
        && declaration_ratio >= 55
        && shape.function_lines <= 4;
    let wire_tag_catalog = source.contains("impl_builtin_wire_tag!(")
        && !source.contains("fn ")
        && source.matches("=>").count() >= 4;

    module_index || const_catalog || wire_tag_catalog
}

fn source_shape(source: &str) -> SourceShape {
    let mut shape = SourceShape::default();
    let mut block = None;
    for raw in source.lines() {
        let line = raw.trim();
        if line.is_empty()
            || line.starts_with("//")
            || line.starts_with("#![")
            || line.starts_with("#[")
        {
            continue;
        }
        shape.meaningful_lines += 1;

        if let Some(kind) = block {
            count_declaration_line(&mut shape, kind);
            if line.ends_with(';') {
                block = None;
            }
            continue;
        }

        if let Some(kind) = declaration_block_start(line) {
            count_declaration_line(&mut shape, kind);
            if !line.ends_with(';') {
                block = Some(kind);
            }
            continue;
        }

        if line.contains("fn ") {
            shape.function_lines += 1;
        }
        if has_control_flow_keyword(line) {
            shape.control_flow_lines += 1;
        }
    }
    shape
}

fn count_declaration_line(shape: &mut SourceShape, kind: DeclarationBlock) {
    shape.declaration_lines += 1;
    match kind {
        DeclarationBlock::ConstCatalog => shape.const_catalog_lines += 1,
        DeclarationBlock::ModuleIndex => shape.module_index_lines += 1,
    }
}

fn declaration_block_start(line: &str) -> Option<DeclarationBlock> {
    if line.starts_with("pub const ")
        || line.starts_with("pub(crate) const ")
        || line.starts_with("const ")
        || line.starts_with("pub static ")
        || line.starts_with("pub(crate) static ")
        || line.starts_with("static ")
    {
        return Some(DeclarationBlock::ConstCatalog);
    }
    if line.starts_with("pub mod ")
        || line.starts_with("pub(crate) mod ")
        || line.starts_with("mod ")
        || line.starts_with("pub use ")
        || line.starts_with("pub(crate) use ")
        || line.starts_with("use ")
    {
        return Some(DeclarationBlock::ModuleIndex);
    }
    None
}

fn has_control_flow_keyword(line: &str) -> bool {
    line.starts_with("if ")
        || line.starts_with("for ")
        || line.starts_with("while ")
        || line.starts_with("loop ")
        || line.starts_with("match ")
        || line.contains(" if ")
        || line.contains(" for ")
        || line.contains(" while ")
        || line.contains(" loop ")
        || line.contains(" match ")
}

fn score_pairs(
    fingerprints: &[SourceFingerprint],
    top_n: usize,
    min_score: f64,
) -> Vec<SimilarPair> {
    let candidates = candidate_pairs(fingerprints);
    let mut pairs = Vec::new();
    for (left, right) in candidates {
        if same_generated_family(&fingerprints[left].path, &fingerprints[right].path) {
            continue;
        }
        let score = cosine(&fingerprints[left], &fingerprints[right]);
        if score >= min_score {
            pairs.push(SimilarPair { score, left, right });
        }
    }
    pairs.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    pairs.truncate(top_n);
    pairs
}

fn candidate_pairs(fingerprints: &[SourceFingerprint]) -> HashSet<(usize, usize)> {
    let mut by_shingle: HashMap<u64, Vec<usize>> = HashMap::new();
    for (file_index, fingerprint) in fingerprints.iter().enumerate() {
        for shingle in fingerprint.shingles.keys() {
            by_shingle.entry(*shingle).or_default().push(file_index);
        }
    }
    let mut shared_rare_counts: HashMap<(usize, usize), u16> = HashMap::new();
    for files in by_shingle.values() {
        if files.len() < 2 || files.len() > MAX_CANDIDATE_SHINGLE_FANOUT {
            continue;
        }
        for left_pos in 0..files.len() {
            for &right in &files[left_pos + 1..] {
                let left = files[left_pos];
                let key = if left < right {
                    (left, right)
                } else {
                    (right, left)
                };
                let count = shared_rare_counts.entry(key).or_insert(0);
                *count = count.saturating_add(1);
            }
        }
    }
    shared_rare_counts
        .into_iter()
        .filter_map(|(pair, count)| (count >= MIN_SHARED_RARE_SHINGLES).then_some(pair))
        .collect()
}

fn same_generated_family(left: &Path, right: &Path) -> bool {
    let left = display_path(left);
    let right = display_path(right);
    (left.contains("/tests/__split/") && right.contains("/tests/__split/"))
        || (left.contains("/parse/vast/classify/nodes_")
            && right.contains("/parse/vast/classify/nodes_"))
}

fn normalize_tokens(source: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let bytes = source.as_bytes();
    let mut index = 0usize;
    while index < bytes.len() {
        let b = bytes[index];
        if b.is_ascii_whitespace() {
            index += 1;
            continue;
        }
        if b == b'/' && bytes.get(index + 1) == Some(&b'/') {
            index += 2;
            while index < bytes.len() && bytes[index] != b'\n' {
                index += 1;
            }
            continue;
        }
        if b == b'/' && bytes.get(index + 1) == Some(&b'*') {
            index += 2;
            while index + 1 < bytes.len() && !(bytes[index] == b'*' && bytes[index + 1] == b'/') {
                index += 1;
            }
            index = (index + 2).min(bytes.len());
            continue;
        }
        if b == b'"' {
            tokens.push("str".to_string());
            index += 1;
            while index < bytes.len() {
                if bytes[index] == b'\\' {
                    index = (index + 2).min(bytes.len());
                    continue;
                }
                if bytes[index] == b'"' {
                    index += 1;
                    break;
                }
                index += 1;
            }
            continue;
        }
        if b == b'\'' {
            tokens.push("chr".to_string());
            index += 1;
            while index < bytes.len() {
                if bytes[index] == b'\\' {
                    index = (index + 2).min(bytes.len());
                    continue;
                }
                if bytes[index] == b'\'' {
                    index += 1;
                    break;
                }
                index += 1;
            }
            continue;
        }
        if b.is_ascii_alphabetic() || b == b'_' {
            let start = index;
            index += 1;
            while index < bytes.len()
                && (bytes[index].is_ascii_alphanumeric() || bytes[index] == b'_')
            {
                index += 1;
            }
            let ident = &source[start..index];
            tokens.push(normalize_identifier(ident));
            continue;
        }
        if b.is_ascii_digit() {
            tokens.push("num".to_string());
            index += 1;
            while index < bytes.len()
                && (bytes[index].is_ascii_alphanumeric() || matches!(bytes[index], b'_' | b'.'))
            {
                index += 1;
            }
            continue;
        }
        tokens.push((b as char).to_string());
        index += 1;
    }
    tokens
}

fn normalize_identifier(identifier: &str) -> String {
    if is_rust_keyword(identifier) {
        return identifier.to_string();
    }
    if is_semantic_constant_identifier(identifier) {
        return format!("const:{identifier}");
    }
    "id".to_string()
}

fn is_semantic_constant_identifier(identifier: &str) -> bool {
    let mut has_uppercase = false;
    let mut has_separator_or_digit = false;
    for byte in identifier.bytes() {
        if byte.is_ascii_uppercase() {
            has_uppercase = true;
            continue;
        }
        if byte.is_ascii_digit() || byte == b'_' {
            has_separator_or_digit = true;
            continue;
        }
        return false;
    }
    has_uppercase && (has_separator_or_digit || identifier.len() >= 3)
}

fn is_rust_keyword(token: &str) -> bool {
    matches!(
        token,
        "as" | "async"
            | "await"
            | "break"
            | "const"
            | "continue"
            | "crate"
            | "else"
            | "enum"
            | "extern"
            | "false"
            | "fn"
            | "for"
            | "if"
            | "impl"
            | "in"
            | "let"
            | "loop"
            | "match"
            | "mod"
            | "move"
            | "mut"
            | "pub"
            | "ref"
            | "return"
            | "self"
            | "Self"
            | "static"
            | "struct"
            | "super"
            | "trait"
            | "true"
            | "type"
            | "unsafe"
            | "use"
            | "where"
            | "while"
    )
}

fn shingle_counts(tokens: &[String], width: usize) -> HashMap<u64, u32> {
    let mut counts = HashMap::new();
    if tokens.len() < width {
        return counts;
    }
    for window in tokens.windows(width) {
        *counts.entry(hash_window(window)).or_insert(0) += 1;
    }
    counts
}

fn hash_window(window: &[String]) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    for token in window {
        for &byte in token.as_bytes() {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(0x100_0000_01b3);
        }
        hash ^= 0xff;
        hash = hash.wrapping_mul(0x100_0000_01b3);
    }
    hash
}

fn magnitude(counts: &HashMap<u64, u32>) -> f64 {
    (counts
        .values()
        .map(|count| {
            let c = f64::from(*count);
            c * c
        })
        .sum::<f64>())
    .sqrt()
}

fn cosine(left: &SourceFingerprint, right: &SourceFingerprint) -> f64 {
    if left.magnitude == 0.0 || right.magnitude == 0.0 {
        return 0.0;
    }
    let (small, large) = if left.shingles.len() <= right.shingles.len() {
        (&left.shingles, &right.shingles)
    } else {
        (&right.shingles, &left.shingles)
    };
    let dot = small
        .iter()
        .filter_map(|(key, left_count)| {
            large
                .get(key)
                .map(|right_count| (*left_count, *right_count))
        })
        .map(|(left_count, right_count)| f64::from(left_count) * f64::from(right_count))
        .sum::<f64>();
    dot / (left.magnitude * right.magnitude)
}

fn pair_verdict(score: f64) -> &'static str {
    if score >= 0.97 {
        "DUPLICATE"
    } else if score >= 0.90 {
        "VERY SIMILAR"
    } else {
        "SIMILAR"
    }
}

fn dedup_guidance_for_pair(
    left: &str,
    right: &str,
    ownership_lanes: &[OwnershipLaneRule],
) -> DedupGuidance {
    let left_owner_lane = owner_lane_for_file(left, ownership_lanes).to_string();
    let right_owner_lane = owner_lane_for_file(right, ownership_lanes).to_string();
    let import_owner = preferred_import_owner(left, right, &left_owner_lane, &right_owner_lane);
    let import_target = dedup_import_target(left, right, &import_owner);
    DedupGuidance {
        left_owner_lane,
        right_owner_lane,
        import_owner,
        import_target,
    }
}

fn preferred_import_owner(left: &str, right: &str, left_lane: &str, right_lane: &str) -> String {
    if left_lane == right_lane {
        return left_lane.to_string();
    }
    let mut candidates = [
        (lane_import_priority(left_lane), left_lane, left),
        (lane_import_priority(right_lane), right_lane, right),
    ];
    candidates.sort_by(|a, b| {
        a.0.cmp(&b.0)
            .then_with(|| a.1.cmp(b.1))
            .then_with(|| a.2.cmp(b.2))
    });
    candidates[0].1.to_string()
}

fn lane_import_priority(lane: &str) -> usize {
    match lane {
        "foundation_optimizer" | "foundation_wire" => 10,
        "driver_shared" => 20,
        "driver_cuda" | "driver_wgpu" | "driver_spirv" => 30,
        "runtime_megakernel" => 40,
        "op_matrix" => 50,
        "bench_harness" => 60,
        "coordination" => 90,
        "unowned" => 1000,
        _ => 500,
    }
}

fn dedup_import_target(left: &str, right: &str, import_owner: &str) -> String {
    let module = shared_module_name(left, right);
    let root = owner_import_root(import_owner)
        .or_else(|| common_crate_import_root(left, right))
        .unwrap_or("shared");
    format!("{root}::dedup::{module}")
}

fn owner_import_root(owner: &str) -> Option<&'static str> {
    match owner {
        "foundation_optimizer" | "foundation_wire" => Some("vyre_foundation"),
        "driver_shared" => Some("vyre_driver"),
        "driver_cuda" => Some("vyre_driver_cuda"),
        "driver_wgpu" => Some("vyre_driver_wgpu"),
        "driver_spirv" => Some("vyre_driver_spirv"),
        "runtime_megakernel" => Some("vyre_runtime::megakernel"),
        "bench_harness" => Some("vyre_bench"),
        "coordination" => Some("xtask"),
        "op_matrix" => Some("xtask::op_matrix"),
        _ => None,
    }
}

fn common_crate_import_root(left: &str, right: &str) -> Option<&'static str> {
    let left_crate = left.split('/').next()?;
    let right_crate = right.split('/').next()?;
    if left_crate != right_crate {
        return None;
    }
    match left_crate {
        "vyre-foundation" => Some("vyre_foundation"),
        "vyre-driver" => Some("vyre_driver"),
        "vyre-driver-cuda" => Some("vyre_driver_cuda"),
        "vyre-driver-wgpu" => Some("vyre_driver_wgpu"),
        "vyre-driver-spirv" => Some("vyre_driver_spirv"),
        "vyre-runtime" => Some("vyre_runtime"),
        "vyre-libs" => Some("vyre_libs"),
        "vyre-primitives" => Some("vyre_primitives"),
        "vyre-bench" => Some("vyre_bench"),
        "xtask" => Some("xtask"),
        _ => None,
    }
}

fn shared_module_name(left: &str, right: &str) -> String {
    let mut stems = [file_stem_slug(left), file_stem_slug(right)];
    stems.sort();
    if stems[0] == stems[1] {
        format!("shared_{}", stems[0])
    } else {
        format!("shared_{}_{}", stems[0], stems[1])
    }
}

fn file_stem_slug(path: &str) -> String {
    Path::new(path)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .map(slug_identifier)
        .filter(|stem| !stem.is_empty())
        .unwrap_or_else(|| "helper".to_string())
}

fn slug_identifier(raw: &str) -> String {
    let mut out = String::new();
    for byte in raw.bytes() {
        if byte.is_ascii_alphanumeric() {
            out.push((byte as char).to_ascii_lowercase());
        } else if byte == b'_' || byte == b'-' {
            out.push('_');
        }
    }
    while out.contains("__") {
        out = out.replace("__", "_");
    }
    out.trim_matches('_').to_string()
}

fn display_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalization_catches_renamed_function_skeletons() {
        let left = normalize_tokens(
            "pub fn alpha(input: u32) -> u32 { let value = input + 1; value * 2 }",
        );
        let right =
            normalize_tokens("pub fn beta(other: u32) -> u32 { let tmp = other + 9; tmp * 7 }");
        let left_counts = shingle_counts(&left, 4);
        let right_counts = shingle_counts(&right, 4);
        let left_fp = SourceFingerprint {
            path: PathBuf::from("left.rs"),
            bytes: 1,
            tokens: left.len(),
            fingerprint: source_token_fingerprint(&left),
            magnitude: magnitude(&left_counts),
            shingles: left_counts,
        };
        let right_fp = SourceFingerprint {
            path: PathBuf::from("right.rs"),
            bytes: 1,
            tokens: right.len(),
            fingerprint: source_token_fingerprint(&right),
            magnitude: magnitude(&right_counts),
            shingles: right_counts,
        };
        assert!(
            cosine(&left_fp, &right_fp) > 0.70,
            "renamed/literal-changed function skeletons should stay similar"
        );
    }

    #[test]
    fn comments_and_strings_do_not_dominate_similarity() {
        let tokens = normalize_tokens(
            "//! doc words should vanish\nfn x() { let s = \"different payload\"; /* block */ 7 }",
        );
        assert!(!tokens.iter().any(|token| token == "doc"));
        assert!(tokens.iter().any(|token| token == "str"));
        assert!(tokens.iter().any(|token| token == "num"));
    }

    #[test]
    fn semantic_constants_remain_visible_in_similarity_tokens() {
        let tokens = normalize_tokens(
            "fn classify(kind: u32) -> bool { kind == TOK_IDENTIFIER || kind == VAST_DECL_CONTEXT_STRIDE_U32 }",
        );
        assert!(tokens.iter().any(|token| token == "const:TOK_IDENTIFIER"));
        assert!(tokens
            .iter()
            .any(|token| token == "const:VAST_DECL_CONTEXT_STRIDE_U32"));
        assert!(!tokens.iter().any(|token| token == "classify"));
    }

    #[test]
    fn declarative_catalog_sources_do_not_enter_similarity_scan() {
        let constants = (0..32)
            .map(|idx| format!("pub const TOK_{idx}: u32 = {idx};\n"))
            .collect::<String>();
        assert!(is_declarative_catalog_source(&constants));

        let multiline_constants = [
            "pub const TOKEN_SPECS: &[TokenSpec] = &[",
            "    TokenSpec { id: 1, width: 2 },",
            "    TokenSpec { id: 2, width: 4 },",
            "    TokenSpec { id: 3, width: 8 },",
            "    TokenSpec { id: 4, width: 16 },",
            "    TokenSpec { id: 5, width: 32 },",
            "    TokenSpec { id: 6, width: 64 },",
            "    TokenSpec { id: 7, width: 128 },",
            "];",
            "pub fn token_width(token: u32) -> Option<u16> { TOKEN_SPECS.iter().find(|spec| spec.id == token).map(|spec| spec.width) }",
        ]
        .join("\n");
        assert!(is_declarative_catalog_source(&multiline_constants));

        let module_index = [
            "pub mod alpha;",
            "pub mod beta;",
            "pub mod gamma;",
            "pub mod delta;",
            "pub use alpha::alpha;",
            "pub use beta::beta;",
            "pub use gamma::gamma;",
            "pub use delta::delta;",
        ]
        .join("\n");
        assert!(is_declarative_catalog_source(&module_index));

        let multiline_module_index = [
            "pub mod alpha;",
            "pub mod beta;",
            "pub use alpha::{",
            "    alpha_one,",
            "    alpha_two,",
            "    alpha_three,",
            "};",
            "pub use beta::{",
            "    beta_one,",
            "    beta_two,",
            "};",
        ]
        .join("\n");
        assert!(is_declarative_catalog_source(&multiline_module_index));

        let implementation =
            "pub fn alpha(input: &[u32]) -> u32 {\n    input.iter().copied().sum()\n}\n";
        assert!(!is_declarative_catalog_source(implementation));

        let real_code_with_constants = [
            "const MASK: u32 = 7;",
            "const LIMIT: u32 = 11;",
            "const SHIFT: u32 = 2;",
            "pub fn classify(input: &[u32]) -> u32 {",
            "    let mut acc = 0;",
            "    for value in input {",
            "        if value & MASK != 0 {",
            "            acc ^= value.wrapping_shl(SHIFT);",
            "        }",
            "    }",
            "    acc.min(LIMIT)",
            "}",
        ]
        .join("\n");
        assert!(!is_declarative_catalog_source(&real_code_with_constants));

        let wire_tag_catalog = [
            "pub enum ExampleOp {",
            "    Add,",
            "    Mul,",
            "    Opaque(ExtensionOpId),",
            "}",
            "impl_builtin_wire_tag!(ExampleOp, Opaque, {",
            "    Add => 0x01,",
            "    Mul => 0x02,",
            "    Div => 0x03,",
            "    Rem => 0x04,",
            "});",
        ]
        .join("\n");
        assert!(is_declarative_catalog_source(&wire_tag_catalog));
    }

    #[test]
    fn parse_args_defaults_to_existing_roots() {
        let args = vec!["xtask".to_string(), "source-similar".to_string()];
        let config = parse_args(&args).expect("Fix: replace expect with fallible API or document caller precondition; panic only on programmer error - default args");
        assert!(config.top_n > 0);
        assert!((0.0..=1.0).contains(&config.min_score));
    }

    #[test]
    fn parse_args_rejects_zero_top() {
        let args = vec![
            "xtask".to_string(),
            "source-similar".to_string(),
            "--top".to_string(),
            "0".to_string(),
        ];
        let error = parse_args(&args).unwrap_err();
        assert!(error.contains("--top"));
    }

    #[test]
    fn parse_args_accepts_check_mode_for_ci_duplicate_gates() {
        let args = vec![
            "xtask".to_string(),
            "source-similar".to_string(),
            "--check".to_string(),
            "--min".to_string(),
            "0.95".to_string(),
        ];
        let config = parse_args(&args).expect("Fix: replace expect with fallible API or document caller precondition; panic only on programmer error - check args");
        assert!(config.fail_on_findings);
        assert!(!config.include_untracked);
        assert_eq!(config.min_score, 0.95);
    }

    #[test]
    fn parse_args_accepts_untracked_opt_in_for_exploratory_scans() {
        let args = vec![
            "xtask".to_string(),
            "source-similar".to_string(),
            "--include-untracked".to_string(),
        ];
        let config = parse_args(&args).expect("Fix: replace expect with fallible API or document caller precondition; panic only on programmer error - include untracked args");
        assert!(config.include_untracked);
    }

    #[test]
    fn git_repo_scans_tracked_files_by_default() {
        let dir = tempfile::TempDir::new().expect("Fix: replace expect with fallible API or document caller precondition; panic only on programmer error - tempdir");
        process::Command::new("git")
            .args(["init"])
            .current_dir(dir.path())
            .output()
            .expect("Fix: replace expect with fallible API or document caller precondition; panic only on programmer error - git init");
        let body = "pub fn alpha(input: &[u32]) -> u32 {\n    let mut acc = 0;\n".to_string()
            + &"    for value in input { acc = acc.wrapping_add(*value); }\n".repeat(24)
            + "    acc\n}\n";
        fs::write(dir.path().join("tracked.rs"), &body).expect("Fix: replace expect with fallible API or document caller precondition; panic only on programmer error - tracked fixture");
        fs::write(dir.path().join("untracked.rs"), &body).expect("Fix: replace expect with fallible API or document caller precondition; panic only on programmer error - untracked fixture");
        process::Command::new("git")
            .args(["add", "tracked.rs"])
            .current_dir(dir.path())
            .output()
            .expect("Fix: replace expect with fallible API or document caller precondition; panic only on programmer error - git add");

        let roots = vec![dir.path().to_path_buf()];
        let tracked_only =
            find_similar_sources(&roots, 10, 0.50, 64 * 1024, false).expect("Fix: replace expect with fallible API or document caller precondition; panic only on programmer error - tracked scan");
        let with_untracked =
            find_similar_sources(&roots, 10, 0.50, 64 * 1024, true).expect("Fix: replace expect with fallible API or document caller precondition; panic only on programmer error - untracked scan");

        assert_eq!(tracked_only.scanned_files, 1);
        assert_eq!(with_untracked.scanned_files, 2);
    }

    #[test]
    fn tiny_wrapper_sources_do_not_enter_similarity_scan() {
        let dir = tempfile::TempDir::new().expect("Fix: replace expect with fallible API or document caller precondition; panic only on programmer error - tempdir");
        let path = dir.path().join("wrapper.rs");
        fs::write(
            &path,
            "pub struct AddDualReference;\ndefine_arith_dual_reference!(AddDualReference, u32::wrapping_add, super::common::wrapping_add_bits_reference);\n",
        )
        .expect("Fix: replace expect with fallible API or document caller precondition; panic only on programmer error - wrapper fixture");

        let fingerprints = fingerprint_files(&[path]);
        assert!(
            fingerprints.is_empty(),
            "tiny macro/module wrappers should not outrank implementation duplicates"
        );
    }

    #[test]
    fn skips_generated_split_scratch_and_internal_planning_trees() {
        assert!(should_skip_path(Path::new(
            "vyre-macros/src/__law7_split/lib_part1.rs"
        )));
        assert!(should_skip_path(Path::new(
            "vyre-driver-wgpu/tests/__split/generated_chunk.rs"
        )));
        assert!(should_skip_path(Path::new(
            ".internals/audits/notes/generated.rs"
        )));
        assert!(should_skip_path(Path::new("jules_tickets/ticket.rs")));
        assert!(!should_skip_path(Path::new(
            "vyre-primitives/src/graph/toposort.rs"
        )));
    }

    #[test]
    fn generated_family_filter_suppresses_split_test_pairs_only() {
        assert!(same_generated_family(
            Path::new("vyre-driver-cuda/tests/__split/a.rs"),
            Path::new("vyre-driver-cuda/tests/__split/b.rs")
        ));
        assert!(!same_generated_family(
            Path::new("vyre-driver-cuda/tests/a.rs"),
            Path::new("vyre-driver-cuda/tests/__split/b.rs")
        ));
    }

    #[test]
    fn candidate_pairs_use_shared_rare_shingles_without_full_quadratic_scan() {
        let source_a = normalize_tokens(
            "fn alpha() { let special = 1; special + 2; let again = special + 3; again * 4; let tail = again + special; tail }",
        );
        let source_b = normalize_tokens(
            "fn beta() { let renamed = 9; renamed + 7; let more = renamed + 8; more * 6; let end = more + renamed; end }",
        );
        let source_c = normalize_tokens("struct CompletelyDifferent { field: u32 }");
        let fingerprints = [source_a, source_b, source_c]
            .iter()
            .enumerate()
            .map(|(idx, tokens)| {
                let shingles = shingle_counts(tokens, 4);
                SourceFingerprint {
                    path: PathBuf::from(format!("{idx}.rs")),
                    bytes: 1,
                    tokens: tokens.len(),
                    fingerprint: source_token_fingerprint(tokens),
                    magnitude: magnitude(&shingles),
                    shingles,
                }
            })
            .collect::<Vec<_>>();
        let candidates = candidate_pairs(&fingerprints);
        assert!(
            candidates.contains(&(0, 1)),
            "renamed duplicate skeletons must become scoring candidates"
        );
    }

    #[test]
    fn dedup_guidance_points_duplicate_helpers_at_one_import_owner() {
        let lanes = vec![
            OwnershipLaneRule {
                lane: "foundation_optimizer".to_string(),
                write_patterns: vec!["vyre-foundation/src/optimizer/**".to_string()],
            },
            OwnershipLaneRule {
                lane: "driver_shared".to_string(),
                write_patterns: vec!["vyre-driver/src/**".to_string()],
            },
        ];

        let same_lane = dedup_guidance_for_pair(
            "vyre-foundation/src/optimizer/range_scan.rs",
            "vyre-foundation/src/optimizer/range_scan_fork.rs",
            &lanes,
        );

        assert_eq!(same_lane.left_owner_lane, "foundation_optimizer");
        assert_eq!(same_lane.right_owner_lane, "foundation_optimizer");
        assert_eq!(same_lane.import_owner, "foundation_optimizer");
        assert_eq!(
            same_lane.import_target,
            "vyre_foundation::dedup::shared_range_scan_range_scan_fork"
        );

        let cross_lane = dedup_guidance_for_pair(
            "vyre-driver-cuda/src/backend/dispatch_clone.rs",
            "vyre-driver/src/backend/dispatch.rs",
            &lanes,
        );

        assert_eq!(cross_lane.left_owner_lane, "unowned");
        assert_eq!(cross_lane.right_owner_lane, "driver_shared");
        assert_eq!(cross_lane.import_owner, "driver_shared");
        assert_eq!(
            cross_lane.import_target,
            "vyre_driver::dedup::shared_dispatch_dispatch_clone"
        );
    }
}
