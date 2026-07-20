//! `heuristic-audit`  -  surface hand-rolled heuristics that should
//! be replaced by recursion-thesis self-consumers.
//!
//! The recursion thesis says every ad-hoc heuristic in vyre's
//! optimizer / scheduler / cache layer is technical debt  -  a place
//! where vyre is using less than the math it ships. This subcommand
//! greps for the canonical "I should be a self-consumer call" markers
//! so they show up on every CI run instead of getting forgotten.
//!
//! Default mode: warning. `--strict` exits non-zero  -  the gate.

use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::process;

const VYRE_ROOT: &str = "libs/performance/matching/vyre";
const MAX_HEURISTIC_AUDIT_SOURCE_BYTES: u64 = 2_097_152;

/// Crates whose source we audit. Excludes test fixtures, examples,
/// benchmarks, and documentation.
const CRATES: &[&str] = &[
    "vyre-foundation",
    "vyre-driver",
    "vyre-driver-wgpu",
    "vyre-driver-cuda",
    "vyre-driver-spirv",
    "vyre-runtime",
    "vyre-libs",
    "vyre-aot",
    "vyre-frontend-c",
    "vyre-harness",
    "vyre-spec",
];

/// Markers that flag a hand-rolled heuristic. Each pattern points at
/// a known class of "use math here" debt. Adding a new pattern
/// requires a one-line note explaining what self-consumer should
/// replace it.
const MARKERS: &[(&str, &str)] = &[
    // Fusion / cost heuristics → tensor_network_fusion_order (#35).
    (
        "Heuristic fusion pressure",
        "use tensor_network_fusion_order::optimal_fusion_order",
    ),
    (
        "// HEURISTIC",
        "audit + replace with the appropriate self-consumer",
    ),
    // Per-pass match-on-Node validators → knowledge_compile_pass_precondition (#38).
    (
        "// hand-rolled validator",
        "use knowledge_compile_pass_precondition::pass_applies",
    ),
    // Pass-dependency hand-curation → adjustment_set_pass_dependency (#37).
    (
        "// pass dependency table",
        "derive via adjustment_set_pass_dependency::ordering_is_safe",
    ),
    // Sequential host-driven fixpoint loops → persistent_fixpoint.
    (
        "// host-side fixpoint",
        "use vyre_primitives::fixpoint::persistent_fixpoint",
    ),
    // LRU eviction / hit-rate heuristics → submodular_cache_eviction (#45).
    (
        "// LRU eviction",
        "use submodular_cache_eviction::select_retention_set",
    ),
    // Plain-gradient autotuner → natural_gradient_autotuner (#56).
    (
        "// plain gradient autotune",
        "use natural_gradient_autotuner::autotune_step",
    ),
    // Hand-coded cache invalidation → do_calculus_change_impact (#36).
    (
        "// hand-coded invalidation",
        "use do_calculus_change_impact",
    ),
];

pub(crate) fn run(args: &[String]) {
    let strict = args.iter().any(|a| a == "--strict");
    let workspace_root = locate_workspace_root();
    let vyre_dir = workspace_root.join(VYRE_ROOT);

    let mut findings: Vec<(PathBuf, usize, &str, &str)> = Vec::new();
    let mut scan_errors = Vec::new();
    for crate_name in CRATES {
        let src = vyre_dir.join(crate_name).join("src");
        if !src.exists() {
            scan_errors.push(format!(
                "heuristic audit crate source root `{}` does not exist",
                src.display()
            ));
            continue;
        }
        scan_dir(&src, &mut findings, &mut scan_errors);
    }

    if !scan_errors.is_empty() {
        eprintln!(
            "heuristic-audit: {} scan/read error(s) make heuristic evidence incomplete:",
            scan_errors.len()
        );
        for error in &scan_errors {
            eprintln!("  - {error}");
        }
        eprintln!("Fix: make every audited production source root/file readable before release.");
        process::exit(1);
    }

    if findings.is_empty() {
        println!(
            "heuristic-audit: zero hand-rolled heuristics flagged across {} crate(s).",
            CRATES.len()
        );
        return;
    }

    eprintln!(
        "heuristic-audit: {} hand-rolled heuristic(s) flagged for self-consumer replacement.",
        findings.len()
    );
    for (path, line, marker, fix) in &findings {
        eprintln!("  {}:{}  -  {} → {}", path.display(), line, marker, fix);
    }

    if strict {
        eprintln!("\n--strict mode: build gate failed.");
        process::exit(1);
    } else {
        eprintln!("\n(non-strict mode: warning only; pass --strict to gate the build)");
    }
}

fn scan_dir(
    dir: &Path,
    findings: &mut Vec<(PathBuf, usize, &'static str, &'static str)>,
    scan_errors: &mut Vec<String>,
) {
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(error) => {
            scan_errors.push(format!(
                "could not read heuristic audit directory `{}`: {error}",
                dir.display()
            ));
            return;
        }
    };
    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) => {
                scan_errors.push(format!(
                    "could not read heuristic audit entry in `{}`: {error}",
                    dir.display()
                ));
                continue;
            }
        };
        let path = entry.path();
        if path.is_dir() {
            // Skip test directories  -  heuristic markers in tests are
            // intentional fixtures, not production debt.
            if let Some(name) = path.file_name().and_then(|s| s.to_str()) {
                if matches!(name, "tests" | "fuzz" | "benches" | "examples") {
                    continue;
                }
            }
            scan_dir(&path, findings, scan_errors);
            continue;
        }
        if path.extension().and_then(|s| s.to_str()) != Some("rs") {
            continue;
        }
        let body = match read_text_bounded(&path) {
            Ok(b) => b,
            Err(error) => {
                scan_errors.push(format!(
                    "could not read heuristic audit source `{}`: {error}",
                    path.display()
                ));
                continue;
            }
        };
        for (lineno, line) in body.lines().enumerate() {
            for &(marker, fix) in MARKERS {
                if line.contains(marker) {
                    findings.push((path.clone(), lineno + 1, marker, fix));
                }
            }
        }
    }
}

fn locate_workspace_root() -> PathBuf {
    let mut cur = std::env::current_dir()
        .expect("Fix: cargo_full run --bin xtask -- must be runnable from a directory.");
    loop {
        let manifest = cur.join("Cargo.toml");
        if manifest.exists() {
            match is_workspace_root(&cur) {
                Ok(true) => return cur,
                Ok(false) => {}
                Err(error) => {
                    eprintln!(
                        "Fix: could not read workspace candidate `{}`: {error}",
                        manifest.display()
                    );
                    process::exit(2);
                }
            }
        }
        if !cur.pop() {
            eprintln!(
                "Fix: could not locate a Cargo.toml containing [workspace] and members from the current directory."
            );
            process::exit(2);
        }
    }
}

fn is_workspace_root(path: &Path) -> io::Result<bool> {
    let manifest = path.join("Cargo.toml");
    let text = read_text_bounded(&manifest)?;
    Ok(text.contains("[workspace]") && text.contains("members"))
}

fn read_text_bounded(path: &Path) -> io::Result<String> {
    let mut reader = fs::File::open(path)?.take(MAX_HEURISTIC_AUDIT_SOURCE_BYTES.saturating_add(1));
    let mut text = String::new();
    reader.read_to_string(&mut text)?;
    if text.len() as u64 > MAX_HEURISTIC_AUDIT_SOURCE_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "{} exceeds {MAX_HEURISTIC_AUDIT_SOURCE_BYTES} byte heuristic audit read cap",
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
    fn markers_have_unique_patterns() {
        let mut patterns: Vec<&str> = MARKERS.iter().map(|(p, _)| *p).collect();
        patterns.sort();
        let original_len = patterns.len();
        patterns.dedup();
        assert_eq!(
            patterns.len(),
            original_len,
            "duplicate marker pattern in MARKERS"
        );
    }
}
