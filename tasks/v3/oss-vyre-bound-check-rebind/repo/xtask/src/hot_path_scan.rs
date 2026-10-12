//! `cargo_full run --bin xtask -- hot-path-scan`  -  ROADMAP S11 enforcement.
//!
//! Reads `docs/optimization/HOT_PATHS.toml` and scans every listed file
//! for allocation, clone, lock, sleep, panic, and string-construction patterns that
//! are usually evidence of hot-path waste:
//!
//! - `.clone()`  -  almost always hidden allocation; scratch reuse or
//!   `Cow` / `Arc` is cheaper.
//! - `.to_owned()` / `.to_string()`  -  allocates on every call.
//! - `Vec::new()` / `Vec::with_capacity(N)` (in non-init code)  -
//!   per-call vector; consider scratch reuse.
//! - `HashMap::new()` / `BTreeMap::new()`  -  per-call map.
//! - `String::new()` / `String::from(...)`  -  per-call string.
//! - `Mutex::new(...)` / `RwLock::new(...)`  -  per-call lock primitive
//!   in code that runs many times per dispatch.
//! - `std::thread::sleep(...)` / `tokio::time::sleep(...)`  -  fixed
//!   wait on a measured path.
//! - `panic!(...)` / `todo!(...)` / `unimplemented!(...)`  -  fail-open
//!   runtime behavior where hot paths need structured errors.
//!
//! Each finding prints `file:line | pattern | line content`. Exit 0
//! when the scan is informational (passed `--report` or default), exit
//! 1 when `--strict` is set and any finding fires.
//!
//! The scanner is line-oriented + regex-free to keep it deterministic
//! across rust-fmt rewrites; no AST parsing. It does NOT short-circuit
//! on test modules  -  hot-path files often have inline `#[cfg(test)]`
//! blocks that legitimately allocate; the audit ignores `#[cfg(test)]`
//! lines but does NOT skip the rest of the file.

use std::collections::BTreeMap;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::process::{self};

use serde::{Deserialize, Serialize};

use crate::ownership::{load_ownership_lanes, owner_lane_for_file, OwnershipLaneRule};

const MAX_HOT_PATH_SCAN_FILE_BYTES: u64 = 2_097_152;

#[derive(Debug, Deserialize)]
struct HotPathsConfig {
    #[serde(default)]
    schema: u32,
    #[serde(default)]
    hot_path: Vec<HotPathEntry>,
}

#[derive(Debug, Deserialize)]
struct HotPathEntry {
    file: String,
    #[serde(default)]
    reason: String,
    #[serde(default)]
    max_findings: Option<usize>,
    #[serde(default)]
    max_allocation_findings: Option<usize>,
    #[serde(default)]
    max_clone_findings: Option<usize>,
    #[serde(default)]
    max_lock_findings: Option<usize>,
    #[serde(default)]
    max_sleep_findings: Option<usize>,
    #[serde(default)]
    max_panic_findings: Option<usize>,
}

#[derive(Debug)]
struct Finding {
    file: String,
    line: u32,
    pattern: &'static str,
    kind: PatternKind,
    content: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PatternKind {
    Allocation,
    Clone,
    Lock,
    Sleep,
    Panic,
}

#[derive(Clone, Copy, Debug)]
struct PatternSpec {
    name: &'static str,
    text: &'static str,
    kind: PatternKind,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct FindingCounts {
    total: usize,
    allocations: usize,
    clones: usize,
    locks: usize,
    sleeps: usize,
    panics: usize,
    formats: usize,
}

impl FindingCounts {
    fn add(&mut self, finding: &Finding) {
        self.total = self.total.saturating_add(1);
        match finding.kind {
            PatternKind::Allocation => self.allocations = self.allocations.saturating_add(1),
            PatternKind::Clone => self.clones = self.clones.saturating_add(1),
            PatternKind::Lock => self.locks = self.locks.saturating_add(1),
            PatternKind::Sleep => self.sleeps = self.sleeps.saturating_add(1),
            PatternKind::Panic => self.panics = self.panics.saturating_add(1),
        }
        if finding.pattern == "format!" {
            self.formats = self.formats.saturating_add(1);
        }
    }
}

#[derive(Debug)]
struct BudgetDelta {
    file: String,
    budget: &'static str,
    actual: usize,
    limit: usize,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize)]
struct BudgetVxCandidate {
    file: String,
    line: u32,
    owner_lane: String,
    budget: String,
    actual: usize,
    limit: usize,
    delta: usize,
    gate: String,
    suggested_vx: String,
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct HotPathHeatmapRow {
    owner_lane: String,
    file: String,
    code_lines: usize,
    score: u64,
    findings_per_kloc: u64,
    allocations_per_kloc: u64,
    clones_per_kloc: u64,
    locks_per_kloc: u64,
    formats_per_kloc: u64,
    panics_per_kloc: u64,
}

const PATTERNS: &[PatternSpec] = &[
    PatternSpec {
        name: "clone",
        text: ".clone()",
        kind: PatternKind::Clone,
    },
    PatternSpec {
        name: "to_owned",
        text: ".to_owned()",
        kind: PatternKind::Allocation,
    },
    PatternSpec {
        name: "to_string",
        text: ".to_string()",
        kind: PatternKind::Allocation,
    },
    PatternSpec {
        name: "Vec::new",
        text: "Vec::new()",
        kind: PatternKind::Allocation,
    },
    PatternSpec {
        name: "Vec::with_capacity",
        text: "Vec::with_capacity",
        kind: PatternKind::Allocation,
    },
    PatternSpec {
        name: "HashMap::new",
        text: "HashMap::new()",
        kind: PatternKind::Allocation,
    },
    PatternSpec {
        name: "BTreeMap::new",
        text: "BTreeMap::new()",
        kind: PatternKind::Allocation,
    },
    PatternSpec {
        name: "FxHashMap::new",
        text: "FxHashMap::new()",
        kind: PatternKind::Allocation,
    },
    PatternSpec {
        name: "String::new",
        text: "String::new()",
        kind: PatternKind::Allocation,
    },
    PatternSpec {
        name: "String::from",
        text: "String::from(",
        kind: PatternKind::Allocation,
    },
    PatternSpec {
        name: "Mutex::new",
        text: "Mutex::new(",
        kind: PatternKind::Lock,
    },
    PatternSpec {
        name: "RwLock::new",
        text: "RwLock::new(",
        kind: PatternKind::Lock,
    },
    PatternSpec {
        name: "format!",
        text: "format!(",
        kind: PatternKind::Allocation,
    },
    PatternSpec {
        name: "std_thread_sleep",
        text: "std::thread::sleep(",
        kind: PatternKind::Sleep,
    },
    PatternSpec {
        name: "tokio_sleep",
        text: "tokio::time::sleep(",
        kind: PatternKind::Sleep,
    },
    PatternSpec {
        name: "panic!",
        text: "panic!(",
        kind: PatternKind::Panic,
    },
    PatternSpec {
        name: "todo!",
        text: "todo!(",
        kind: PatternKind::Panic,
    },
    PatternSpec {
        name: "unimplemented!",
        text: "unimplemented!(",
        kind: PatternKind::Panic,
    },
];

pub(crate) fn run(args: &[String]) {
    let strict = args.iter().any(|a| a == "--strict");
    let budget_vx_json = match parse_budget_vx_json(args) {
        Ok(path) => path,
        Err(error) => {
            eprintln!("Fix: {error}");
            process::exit(2);
        }
    };
    let root = match workspace_root() {
        Some(r) => r,
        None => {
            eprintln!("Fix: hot-path-scan must run from the vyre workspace.");
            process::exit(1);
        }
    };
    let config_path = root
        .join("docs")
        .join("optimization")
        .join("HOT_PATHS.toml");
    let entries = match load_config(&config_path) {
        Ok(e) => e,
        Err(err) => {
            eprintln!("Fix: failed to load {}: {err}", config_path.display());
            process::exit(1);
        }
    };
    let ownership_path = root
        .join("docs")
        .join("optimization")
        .join("OWNERSHIP.toml");
    let ownership_lanes = match load_ownership_lanes(&ownership_path) {
        Ok(lanes) => lanes,
        Err(err) => {
            eprintln!("Fix: failed to load {}: {err}", ownership_path.display());
            process::exit(1);
        }
    };
    let mut findings: Vec<Finding> = Vec::new();
    let mut code_lines_by_file: BTreeMap<String, usize> = BTreeMap::new();
    let mut scanned = 0usize;
    let mut missing: Vec<String> = Vec::new();
    for entry in &entries {
        let path = root.join(&entry.file);
        if !path.exists() {
            missing.push(entry.file.clone());
            continue;
        }
        scanned += 1;
        match read_text_bounded(&path) {
            Ok(text) => {
                code_lines_by_file.insert(entry.file.clone(), count_code_lines(&text));
                collect_findings(&entry.file, &text, &mut findings);
            }
            Err(err) => eprintln!("warn: could not read {}: {err}", path.display()),
        }
    }
    findings.sort_by(|a, b| {
        (a.file.as_str(), a.line, a.pattern).cmp(&(b.file.as_str(), b.line, b.pattern))
    });
    let mut by_file: BTreeMap<&str, FindingCounts> = BTreeMap::new();
    for f in &findings {
        by_file.entry(f.file.as_str()).or_default().add(f);
    }
    let budget_deltas = collect_budget_deltas(&entries, &by_file);
    let unowned_hot_paths = unowned_hot_path_files(&entries, &ownership_lanes);
    let budget_vx_candidates = budget_vx_candidates(&budget_deltas, &findings, &ownership_lanes);
    let heatmap = build_hot_path_heatmap(&entries, &by_file, &code_lines_by_file, &ownership_lanes);

    println!("=== vyre hot-path scan ===");
    println!(
        "Listed: {} | scanned: {} | missing: {} | findings: {}",
        entries.len(),
        scanned,
        missing.len(),
        findings.len()
    );
    if !missing.is_empty() {
        println!();
        println!("Missing files (listed in HOT_PATHS.toml but not on disk):");
        for path in &missing {
            println!("  ✗ {path}");
        }
    }
    if !unowned_hot_paths.is_empty() {
        println!();
        println!(
            "Unowned hot paths (listed in HOT_PATHS.toml but missing OWNERSHIP.toml coverage):"
        );
        for path in &unowned_hot_paths {
            println!("  ✗ {path}");
        }
    }
    if !heatmap.is_empty() {
        print_hot_path_heatmap(&heatmap);
    }
    if !findings.is_empty() {
        println!();
        println!("Per-file finding counts:");
        // Attach the operator-supplied `reason` (from HOT_PATHS.toml) so
        // the report explains WHY each file is on the hot-path watchlist.
        // Without this the `reason` field is read but never surfaced  -
        // dead documentation.
        let reason_by_file: std::collections::BTreeMap<&str, &str> = entries
            .iter()
            .map(|e| (e.file.as_str(), e.reason.as_str()))
            .collect();
        for (file, counts) in &by_file {
            let reason = reason_by_file.get(file).copied().unwrap_or("");
            if reason.is_empty() {
                println!(
                    "  {file}: total={} allocations={} clones={} locks={} sleeps={} panics={} formats={}",
                    counts.total, counts.allocations, counts.clones, counts.locks, counts.sleeps, counts.panics, counts.formats
                );
            } else {
                println!(
                    "  {file}: total={} allocations={} clones={} locks={} sleeps={} panics={} formats={}   -  {reason}",
                    counts.total, counts.allocations, counts.clones, counts.locks, counts.sleeps, counts.panics, counts.formats
                );
            }
        }
        if !budget_deltas.is_empty() {
            println!();
            println!("Budget deltas:");
            for delta in &budget_deltas {
                println!(
                    "  {} | {} | actual={} budget={} delta=+{}",
                    delta.file,
                    delta.budget,
                    delta.actual,
                    delta.limit,
                    delta.actual.saturating_sub(delta.limit)
                );
            }
        }
        println!();
        println!("Findings:");
        for f in &findings {
            println!(
                "  {}:{} | {} | {}",
                f.file,
                f.line,
                f.pattern,
                f.content.trim()
            );
        }
    } else {
        println!();
        println!("✓ no hot-path patterns found");
    }
    if let Some(path) = budget_vx_json {
        if let Err(error) = write_budget_vx_candidates(&path, &budget_vx_candidates) {
            eprintln!(
                "Fix: could not write hot-path budget VX candidates `{}`: {error}",
                path.display()
            );
            process::exit(1);
        }
        println!(
            "hot-path-scan: wrote {} budget VX candidate(s) to {}",
            budget_vx_candidates.len(),
            path.display()
        );
    }
    if strict && (!budget_deltas.is_empty() || !missing.is_empty() || !unowned_hot_paths.is_empty())
    {
        println!();
        println!(
            "hot-path-scan: STRICT mode failed  -  {} budget overage(s), {} missing file(s), {} unowned hot path(s).",
            budget_deltas.len(),
            missing.len(),
            unowned_hot_paths.len()
        );
        process::exit(1);
    }
}

fn workspace_root() -> Option<PathBuf> {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(Path::to_path_buf)
}

fn parse_budget_vx_json(args: &[String]) -> Result<Option<PathBuf>, String> {
    let mut index = 2usize;
    while index < args.len() {
        match args[index].as_str() {
            "--budget-vx-json" => {
                let Some(path) = args.get(index + 1) else {
                    return Err("--budget-vx-json requires a path".to_string());
                };
                return Ok(Some(PathBuf::from(path)));
            }
            _ => index += 1,
        }
    }
    Ok(None)
}

fn load_config(path: &Path) -> Result<Vec<HotPathEntry>, String> {
    let text = read_text_bounded(path).map_err(|e| e.to_string())?;
    let cfg: HotPathsConfig = toml::from_str(&text).map_err(|e| e.to_string())?;
    if cfg.schema != 1 {
        return Err(format!(
            "expected schema = 1, got {}  -  update the loader before changing the schema",
            cfg.schema
        ));
    }
    Ok(cfg.hot_path)
}

fn read_text_bounded(path: &Path) -> io::Result<String> {
    let mut reader =
        std::fs::File::open(path)?.take(MAX_HOT_PATH_SCAN_FILE_BYTES.saturating_add(1));
    let mut text = String::new();
    reader.read_to_string(&mut text)?;
    if text.len() as u64 > MAX_HOT_PATH_SCAN_FILE_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "{} exceeds {MAX_HOT_PATH_SCAN_FILE_BYTES} byte hot-path scan read cap",
                path.display()
            ),
        ));
    }
    Ok(text)
}

fn write_budget_vx_candidates(path: &Path, candidates: &[BudgetVxCandidate]) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let text = serde_json::to_string_pretty(candidates).map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("budget VX candidate JSON serialization failed: {error}"),
        )
    })?;
    std::fs::write(path, format!("{text}\n"))
}

fn collect_findings(file: &str, text: &str, out: &mut Vec<Finding>) {
    for (line_no, line) in text.lines().enumerate() {
        let scan_line = runtime_code_segment(line);
        let trimmed = scan_line.trim_start();
        // Skip comments and cfg(test) attributes  -  those are intentional
        // dev-only or annotation lines, not runtime cost.
        if trimmed.is_empty() || trimmed.starts_with("#[cfg(test)]") {
            continue;
        }
        for spec in PATTERNS {
            if scan_line.contains(spec.text) {
                out.push(Finding {
                    file: file.to_string(),
                    line: (line_no + 1) as u32,
                    pattern: spec.name,
                    kind: spec.kind,
                    content: line.to_string(),
                });
            }
        }
    }
}

fn count_code_lines(text: &str) -> usize {
    text.lines()
        .filter(|line| {
            let code = runtime_code_segment(line).trim_start();
            !code.is_empty() && !code.starts_with("#[cfg(test)]")
        })
        .count()
}

fn runtime_code_segment(line: &str) -> &str {
    let bytes = line.as_bytes();
    let mut in_string = false;
    let mut in_char = false;
    let mut escaped = false;
    let mut index = 0usize;

    while index < bytes.len() {
        let byte = bytes[index];
        if escaped {
            escaped = false;
            index += 1;
            continue;
        }
        if in_string {
            if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                in_string = false;
            }
            index += 1;
            continue;
        }
        if in_char {
            if byte == b'\\' {
                escaped = true;
            } else if byte == b'\'' {
                in_char = false;
            }
            index += 1;
            continue;
        }
        if byte == b'"' {
            in_string = true;
        } else if byte == b'\'' {
            in_char = true;
        } else if byte == b'/' && bytes.get(index + 1) == Some(&b'/') {
            return &line[..index];
        }
        index += 1;
    }
    line
}

fn collect_budget_deltas(
    entries: &[HotPathEntry],
    by_file: &BTreeMap<&str, FindingCounts>,
) -> Vec<BudgetDelta> {
    let mut deltas = Vec::new();
    for entry in entries {
        let counts = by_file
            .get(entry.file.as_str())
            .copied()
            .unwrap_or_default();
        push_budget_delta(
            &mut deltas,
            &entry.file,
            "max_findings",
            counts.total,
            entry.max_findings,
        );
        push_budget_delta(
            &mut deltas,
            &entry.file,
            "max_allocation_findings",
            counts.allocations,
            entry.max_allocation_findings,
        );
        push_budget_delta(
            &mut deltas,
            &entry.file,
            "max_clone_findings",
            counts.clones,
            entry.max_clone_findings,
        );
        push_budget_delta(
            &mut deltas,
            &entry.file,
            "max_lock_findings",
            counts.locks,
            entry.max_lock_findings,
        );
        push_budget_delta(
            &mut deltas,
            &entry.file,
            "max_sleep_findings",
            counts.sleeps,
            entry.max_sleep_findings,
        );
        push_budget_delta(
            &mut deltas,
            &entry.file,
            "max_panic_findings",
            counts.panics,
            entry.max_panic_findings,
        );
    }
    deltas
}

fn unowned_hot_path_files(
    entries: &[HotPathEntry],
    ownership_lanes: &[OwnershipLaneRule],
) -> Vec<String> {
    entries
        .iter()
        .filter(|entry| owner_lane_for_file(&entry.file, ownership_lanes) == "unowned")
        .map(|entry| entry.file.clone())
        .collect()
}

fn budget_vx_candidates(
    deltas: &[BudgetDelta],
    findings: &[Finding],
    ownership_lanes: &[OwnershipLaneRule],
) -> Vec<BudgetVxCandidate> {
    deltas
        .iter()
        .map(|delta| {
            let line = first_budget_finding_line(delta, findings);
            let owner_lane = owner_lane_for_file(&delta.file, ownership_lanes).to_string();
            BudgetVxCandidate {
                file: delta.file.clone(),
                line,
                owner_lane,
                budget: delta.budget.to_string(),
                actual: delta.actual,
                limit: delta.limit,
                delta: delta.actual.saturating_sub(delta.limit),
                gate: "cargo_full run -p xtask --bin xtask -- hot-path-scan --strict".to_string(),
                suggested_vx: budget_candidate_id(&delta.file, delta.budget),
            }
        })
        .collect()
}

fn first_budget_finding_line(delta: &BudgetDelta, findings: &[Finding]) -> u32 {
    findings
        .iter()
        .find(|finding| finding.file == delta.file && finding_matches_budget(finding, delta.budget))
        .map(|finding| finding.line)
        .unwrap_or(0)
}

fn finding_matches_budget(finding: &Finding, budget: &str) -> bool {
    match budget {
        "max_findings" => true,
        "max_allocation_findings" => finding.kind == PatternKind::Allocation,
        "max_clone_findings" => finding.kind == PatternKind::Clone,
        "max_lock_findings" => finding.kind == PatternKind::Lock,
        "max_sleep_findings" => finding.kind == PatternKind::Sleep,
        "max_panic_findings" => finding.kind == PatternKind::Panic,
        _ => false,
    }
}

fn budget_candidate_id(file: &str, budget: &str) -> String {
    let mut out = String::from("HOTPATH-");
    for byte in file.bytes().chain([b'-']).chain(budget.bytes()) {
        if byte.is_ascii_alphanumeric() {
            out.push((byte as char).to_ascii_uppercase());
        } else {
            out.push('-');
        }
    }
    while out.contains("--") {
        out = out.replace("--", "-");
    }
    out.trim_end_matches('-').to_string()
}

fn build_hot_path_heatmap(
    entries: &[HotPathEntry],
    by_file: &BTreeMap<&str, FindingCounts>,
    code_lines_by_file: &BTreeMap<String, usize>,
    ownership_lanes: &[OwnershipLaneRule],
) -> Vec<HotPathHeatmapRow> {
    let mut rows = Vec::new();
    for entry in entries {
        let Some(code_lines) = code_lines_by_file.get(&entry.file).copied() else {
            continue;
        };
        let counts = by_file
            .get(entry.file.as_str())
            .copied()
            .unwrap_or_default();
        let findings_per_kloc = per_kloc(counts.total, code_lines);
        let allocations_per_kloc = per_kloc(counts.allocations, code_lines);
        let clones_per_kloc = per_kloc(counts.clones, code_lines);
        let locks_per_kloc = per_kloc(counts.locks, code_lines);
        let formats_per_kloc = per_kloc(counts.formats, code_lines);
        let panics_per_kloc = per_kloc(counts.panics, code_lines);
        let score = code_lines as u64
            + findings_per_kloc
            + allocations_per_kloc.saturating_mul(4)
            + clones_per_kloc.saturating_mul(3)
            + locks_per_kloc.saturating_mul(8)
            + formats_per_kloc.saturating_mul(5)
            + panics_per_kloc.saturating_mul(12);
        rows.push(HotPathHeatmapRow {
            owner_lane: owner_lane_for_file(&entry.file, ownership_lanes).to_string(),
            file: entry.file.clone(),
            code_lines,
            score,
            findings_per_kloc,
            allocations_per_kloc,
            clones_per_kloc,
            locks_per_kloc,
            formats_per_kloc,
            panics_per_kloc,
        });
    }
    rows.sort_by(|a, b| {
        b.score
            .cmp(&a.score)
            .then_with(|| a.owner_lane.cmp(&b.owner_lane))
            .then_with(|| a.file.cmp(&b.file))
    });
    rows
}

fn per_kloc(count: usize, code_lines: usize) -> u64 {
    if code_lines == 0 {
        0
    } else {
        (count as u64).saturating_mul(1000) / code_lines as u64
    }
}

fn print_hot_path_heatmap(rows: &[HotPathHeatmapRow]) {
    println!();
    println!("Hot-path bloat heatmap:");
    for row in rows {
        println!(
            "  owner={} | file={} | score={} code_loc={} findings/kLOC={} allocations/kLOC={} clones/kLOC={} locks/kLOC={} formats/kLOC={} panics/kLOC={}",
            row.owner_lane,
            row.file,
            row.score,
            row.code_lines,
            row.findings_per_kloc,
            row.allocations_per_kloc,
            row.clones_per_kloc,
            row.locks_per_kloc,
            row.formats_per_kloc,
            row.panics_per_kloc
        );
    }
}

fn push_budget_delta(
    deltas: &mut Vec<BudgetDelta>,
    file: &str,
    budget: &'static str,
    actual: usize,
    limit: Option<usize>,
) {
    let Some(limit) = limit else {
        return;
    };
    if actual > limit {
        deltas.push(BudgetDelta {
            file: file.to_string(),
            budget,
            actual,
            limit,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collect_findings_picks_up_clone() {
        let mut out = Vec::new();
        collect_findings("x.rs", "let y = x.clone();\n", &mut out);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].pattern, "clone");
        assert_eq!(out[0].kind, PatternKind::Clone);
        assert_eq!(out[0].line, 1);
    }

    #[test]
    fn collect_findings_skips_comments() {
        let mut out = Vec::new();
        collect_findings("x.rs", "// uses x.clone() in docs\n", &mut out);
        assert!(out.is_empty());
    }

    #[test]
    fn collect_findings_picks_up_multiple_patterns() {
        let mut out = Vec::new();
        collect_findings(
            "x.rs",
            "let v: Vec<u32> = Vec::new();\nlet s = String::from(\"a\");\nlet l = Mutex::new(0);\n",
            &mut out,
        );
        let pats: Vec<&str> = out.iter().map(|f| f.pattern).collect();
        assert!(pats.contains(&"Vec::new"));
        assert!(pats.contains(&"String::from"));
        assert!(pats.contains(&"Mutex::new"));
        let mut counts = FindingCounts::default();
        for finding in &out {
            counts.add(finding);
        }
        assert_eq!(counts.total, 3);
        assert_eq!(counts.allocations, 2);
        assert_eq!(counts.locks, 1);
    }

    #[test]
    fn collect_findings_picks_up_sleep_and_panic_patterns() {
        let mut out = Vec::new();
        collect_findings(
            "x.rs",
            "std::thread::sleep(d);\ntokio::time::sleep(d);\npanic!(\"bad\");\ntodo!();\nunimplemented!();\n",
            &mut out,
        );
        let pats: Vec<&str> = out.iter().map(|f| f.pattern).collect();
        assert!(pats.contains(&"std_thread_sleep"));
        assert!(pats.contains(&"tokio_sleep"));
        assert!(pats.contains(&"panic!"));
        assert!(pats.contains(&"todo!"));
        assert!(pats.contains(&"unimplemented!"));
        let mut counts = FindingCounts::default();
        for finding in &out {
            counts.add(finding);
        }
        assert_eq!(counts.sleeps, 2);
        assert_eq!(counts.panics, 3);
    }

    #[test]
    fn collect_findings_picks_up_format_macro() {
        let mut out = Vec::new();
        collect_findings("x.rs", "let s = format!(\"{}\", 5);\n", &mut out);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].pattern, "format!");
    }

    #[test]
    fn load_config_rejects_wrong_schema() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("hp.toml");
        std::fs::write(&path, "schema = 99\nhot_path = []\n").unwrap();
        let err = load_config(&path).unwrap_err();
        assert!(err.contains("schema = 1"));
    }

    #[test]
    fn load_config_parses_entries() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("hp.toml");
        std::fs::write(
            &path,
            "schema = 1\n[[hot_path]]\nfile = \"a.rs\"\nreason = \"x\"\nmax_findings = 2\nmax_allocation_findings = 1\nmax_clone_findings = 1\nmax_lock_findings = 0\nmax_sleep_findings = 0\nmax_panic_findings = 0\n",
        )
        .unwrap();
        let entries = load_config(&path).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].file, "a.rs");
        assert_eq!(entries[0].max_findings, Some(2));
        assert_eq!(entries[0].max_allocation_findings, Some(1));
        assert_eq!(entries[0].max_clone_findings, Some(1));
        assert_eq!(entries[0].max_lock_findings, Some(0));
        assert_eq!(entries[0].max_sleep_findings, Some(0));
        assert_eq!(entries[0].max_panic_findings, Some(0));
    }

    #[test]
    fn unowned_hot_paths_report_exact_missing_owner_files() {
        let entries = vec![
            HotPathEntry {
                file: "vyre-lower/src/pre_emit.rs".to_string(),
                reason: String::new(),
                max_findings: None,
                max_allocation_findings: None,
                max_clone_findings: None,
                max_lock_findings: None,
                max_sleep_findings: None,
                max_panic_findings: None,
            },
            HotPathEntry {
                file: "vyre-driver/src/launch_fusion.rs".to_string(),
                reason: String::new(),
                max_findings: None,
                max_allocation_findings: None,
                max_clone_findings: None,
                max_lock_findings: None,
                max_sleep_findings: None,
                max_panic_findings: None,
            },
        ];
        let ownership = crate::ownership::parse_ownership_lane_rules(
            r#"
[lane.driver_shared]
write = ["vyre-driver/src/**"]
"#,
        )
        .unwrap();

        let unowned = unowned_hot_path_files(&entries, &ownership);

        assert_eq!(unowned, vec!["vyre-lower/src/pre_emit.rs"]);
    }

    #[test]
    fn budget_vx_candidates_serialize_exact_budget_owner_line_and_gate() {
        let deltas = vec![
            BudgetDelta {
                file: "vyre-emit-naga/src/lib.rs".to_string(),
                budget: "max_findings",
                actual: 10,
                limit: 9,
            },
            BudgetDelta {
                file: "vyre-runtime/src/megakernel/telemetry.rs".to_string(),
                budget: "max_panic_findings",
                actual: 10,
                limit: 0,
            },
        ];
        let findings = vec![
            Finding {
                file: "vyre-emit-naga/src/lib.rs".to_string(),
                line: 86,
                pattern: "clone",
                kind: PatternKind::Clone,
                content: "Some(cached.module.clone())".to_string(),
            },
            Finding {
                file: "vyre-runtime/src/megakernel/telemetry.rs".to_string(),
                line: 69,
                pattern: "panic!",
                kind: PatternKind::Panic,
                content: "panic!(\"bad\")".to_string(),
            },
        ];
        let ownership = crate::ownership::parse_ownership_lane_rules(
            r#"
[lane.lower_emit]
write = ["vyre-emit-naga/src/**"]

[lane.runtime_megakernel]
write = ["vyre-runtime/src/megakernel/**"]
"#,
        )
        .unwrap();

        let candidates = budget_vx_candidates(&deltas, &findings, &ownership);
        let json = serde_json::to_value(&candidates).unwrap();

        assert_eq!(json[0]["file"], "vyre-emit-naga/src/lib.rs");
        assert_eq!(json[0]["line"], 86);
        assert_eq!(json[0]["owner_lane"], "lower_emit");
        assert_eq!(json[0]["budget"], "max_findings");
        assert_eq!(json[0]["actual"], 10);
        assert_eq!(json[0]["limit"], 9);
        assert_eq!(json[0]["delta"], 1);
        assert_eq!(
            json[0]["gate"],
            "cargo_full run -p xtask --bin xtask -- hot-path-scan --strict"
        );
        assert_eq!(
            json[0]["suggested_vx"],
            "HOTPATH-VYRE-EMIT-NAGA-SRC-LIB-RS-MAX-FINDINGS"
        );
        assert_eq!(json[1]["file"], "vyre-runtime/src/megakernel/telemetry.rs");
        assert_eq!(json[1]["line"], 69);
        assert_eq!(json[1]["owner_lane"], "runtime_megakernel");
        assert_eq!(json[1]["budget"], "max_panic_findings");
    }

    #[test]
    fn collect_budget_deltas_reports_only_over_budget_categories() {
        let entries = vec![HotPathEntry {
            file: "x.rs".to_string(),
            reason: "hot".to_string(),
            max_findings: Some(4),
            max_allocation_findings: Some(1),
            max_clone_findings: Some(1),
            max_lock_findings: Some(0),
            max_sleep_findings: Some(0),
            max_panic_findings: Some(0),
        }];
        let mut by_file = std::collections::BTreeMap::new();
        by_file.insert(
            "x.rs",
            FindingCounts {
                total: 4,
                allocations: 2,
                clones: 1,
                locks: 0,
                sleeps: 1,
                panics: 1,
                formats: 0,
            },
        );
        let deltas = collect_budget_deltas(&entries, &by_file);
        assert_eq!(deltas.len(), 3);
        assert_eq!(deltas[0].file, "x.rs");
        assert_eq!(deltas[0].budget, "max_allocation_findings");
        assert_eq!(deltas[0].actual, 2);
        assert_eq!(deltas[0].limit, 1);
        assert_eq!(deltas[1].budget, "max_sleep_findings");
        assert_eq!(deltas[1].actual, 1);
        assert_eq!(deltas[1].limit, 0);
        assert_eq!(deltas[2].budget, "max_panic_findings");
        assert_eq!(deltas[2].actual, 1);
        assert_eq!(deltas[2].limit, 0);
    }

    #[test]
    fn collect_findings_ignores_inline_comment_patterns_and_counts_code_lines() {
        let mut out = Vec::new();
        let text = "// format!(\"{}\", x)\nlet keep = 1; // panic!(\"comment\")\nlet msg = format!(\"{}\", keep);\n\n";

        collect_findings("x.rs", text, &mut out);

        assert_eq!(count_code_lines(text), 2);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].pattern, "format!");
    }

    #[test]
    fn hot_path_heatmap_ranks_mega_file_and_assigns_owner_lane() {
        let entries = vec![
            HotPathEntry {
                file: "vyre-foundation/src/optimizer/big.rs".to_string(),
                reason: "mega optimizer".to_string(),
                max_findings: None,
                max_allocation_findings: None,
                max_clone_findings: None,
                max_lock_findings: None,
                max_sleep_findings: None,
                max_panic_findings: None,
            },
            HotPathEntry {
                file: "vyre-driver/src/small.rs".to_string(),
                reason: "small driver".to_string(),
                max_findings: None,
                max_allocation_findings: None,
                max_clone_findings: None,
                max_lock_findings: None,
                max_sleep_findings: None,
                max_panic_findings: None,
            },
        ];
        let mut by_file = BTreeMap::new();
        by_file.insert(
            "vyre-foundation/src/optimizer/big.rs",
            FindingCounts {
                total: 80,
                allocations: 30,
                clones: 20,
                locks: 2,
                sleeps: 0,
                panics: 1,
                formats: 10,
            },
        );
        by_file.insert(
            "vyre-driver/src/small.rs",
            FindingCounts {
                total: 1,
                allocations: 1,
                clones: 0,
                locks: 0,
                sleeps: 0,
                panics: 0,
                formats: 0,
            },
        );
        let mut code_lines = BTreeMap::new();
        code_lines.insert("vyre-foundation/src/optimizer/big.rs".to_string(), 5862);
        code_lines.insert("vyre-driver/src/small.rs".to_string(), 40);
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

        let rows = build_hot_path_heatmap(&entries, &by_file, &code_lines, &lanes);

        assert_eq!(rows[0].file, "vyre-foundation/src/optimizer/big.rs");
        assert_eq!(rows[0].owner_lane, "foundation_optimizer");
        assert!(rows[0].formats_per_kloc > 0);
    }
}
