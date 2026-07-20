//! `measurement-gate`  -  enforce that every wired substrate path has
//! a perf benchmark in the criterion suites (P-XTASK-3).
//!
//! Walks `vyre-driver/src/self_substrate/` enumerating every
//! `pub mod` entry, then checks that for each consumed module
//! (verified by grep against the workspace) at least one criterion
//! benchmark file exists under `vyre-foundation/benches/`,
//! `vyre-driver-wgpu/benches/`, `vyre-runtime/benches/`, or
//! `vyre-harness/benches/` references the module.
//!
//! Catches the "shipped wire but no perf coverage" failure mode  - 
//! prevents wires from regressing silently.

use std::collections::BTreeSet;
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::process;

const VYRE_ROOT: &str = "libs/performance/matching/vyre";
const SELF_SUBSTRATE_SRC: &str = "vyre-driver/src/self_substrate";
const MAX_MEASUREMENT_GATE_SOURCE_BYTES: u64 = 2_097_152;

const BENCH_DIRS: &[&str] = &[
    "vyre-foundation/benches",
    "vyre-driver-wgpu/benches",
    "vyre-runtime/benches",
    "vyre-harness/benches",
    "vyre-libs/benches",
];

pub(crate) fn run(args: &[String]) {
    let workspace_root = locate_workspace_root();
    let substrate_dir = workspace_root.join(VYRE_ROOT).join(SELF_SUBSTRATE_SRC);

    if !substrate_dir.exists() {
        eprintln!(
            "Fix: substrate directory not found at {}. Run `cargo_full run --bin xtask -- measurement-gate` from the Santh workspace root.",
            substrate_dir.display(),
        );
        process::exit(2);
    }

    let strict = args.iter().any(|a| a == "--strict");
    let mut scan_errors = Vec::new();

    // Enumerate the substrate module names from `pub mod` entries.
    let modules = collect_substrate_modules(&substrate_dir, &mut scan_errors);

    // Determine which modules are CONSUMED (have non-self_substrate,
    // non-test callers in the workspace).
    let consumed = filter_consumed(&workspace_root, &modules, &mut scan_errors);

    // Determine which consumed modules have a bench reference.
    let bench_refs = collect_bench_references(&workspace_root, &consumed, &mut scan_errors);

    let missing: Vec<&String> = consumed
        .iter()
        .filter(|m| !bench_refs.contains(m.as_str()))
        .collect();

    if !consumed.is_empty() {
        let coverage_pct =
            ((consumed.len() - missing.len()) as f64 / consumed.len() as f64) * 100.0;
        println!(
            "vyre measurement-gate: {} consumed module(s), {} have benches ({:.1}% coverage)",
            consumed.len(),
            consumed.len() - missing.len(),
            coverage_pct,
        );
    }

    if !scan_errors.is_empty() {
        println!(
            "vyre measurement-gate: {} scan/read error(s) make measurement coverage incomplete:",
            scan_errors.len()
        );
        for error in &scan_errors {
            println!("  - {error}");
        }
        println!("Fix: make all substrate source and benchmark files readable before release.");
        process::exit(1);
    }

    if missing.is_empty() {
        println!("vyre measurement-gate: every consumed substrate module has perf coverage. ✓");
        return;
    }

    println!(
        "vyre measurement-gate: {} consumed substrate module(s) lack perf coverage:",
        missing.len(),
    );
    for m in &missing {
        println!("  - {m}");
    }
    println!(
        "Fix: add a criterion bench under one of {} that exercises each module.",
        BENCH_DIRS.join(", "),
    );

    if strict {
        process::exit(1);
    }
}

fn locate_workspace_root() -> PathBuf {
    // Walk up from CWD until a Cargo.toml with [workspace] is found.
    let mut current =
        std::env::current_dir().expect("Fix: no cwd; restore this invariant before continuing.");
    loop {
        let candidate = current.join("Cargo.toml");
        if candidate.exists() {
            let content = match read_text_bounded(&candidate) {
                Ok(content) => content,
                Err(error) => {
                    eprintln!(
                        "Fix: could not read workspace candidate `{}`: {error}",
                        candidate.display()
                    );
                    process::exit(2);
                }
            };
            if content.contains("[workspace]") {
                return current;
            }
        }
        match current.parent() {
            Some(p) => current = p.to_path_buf(),
            None => {
                eprintln!(
                    "Fix: could not locate a Cargo.toml containing [workspace] from the current directory."
                );
                process::exit(2);
            }
        }
    }
}

fn collect_substrate_modules(dir: &Path, scan_errors: &mut Vec<String>) -> Vec<String> {
    let mod_rs = dir.join("mod.rs");
    let content = match read_text_bounded(&mod_rs) {
        Ok(c) => c,
        Err(error) => {
            scan_errors.push(format!(
                "could not read substrate module registry `{}`: {error}",
                mod_rs.display()
            ));
            return Vec::new();
        }
    };
    let mut modules = Vec::new();
    for line in content.lines() {
        let line = line.trim();
        let prefix = "pub mod ";
        if let Some(rest) = line.strip_prefix(prefix) {
            let name = rest.trim_end_matches(';').trim().to_string();
            // Skip the meta modules that are substrate infrastructure
            // (observability, decision_telemetry)  -  they don't need
            // their own perf benches.
            if matches!(name.as_str(), "observability" | "decision_telemetry") {
                continue;
            }
            modules.push(name);
        }
    }
    modules
}

fn filter_consumed(root: &Path, modules: &[String], scan_errors: &mut Vec<String>) -> Vec<String> {
    let scan_root = root.join(VYRE_ROOT);
    let mut consumed = Vec::new();
    for module in modules {
        let pattern = format!("self_substrate::{module}");
        if grep_outside_self_substrate(&scan_root, &pattern, scan_errors) {
            consumed.push(module.clone());
        }
    }
    consumed
}

fn grep_outside_self_substrate(root: &Path, pattern: &str, scan_errors: &mut Vec<String>) -> bool {
    fn walk(dir: &Path, pattern: &str, scan_errors: &mut Vec<String>) -> bool {
        let entries = match fs::read_dir(dir) {
            Ok(e) => e,
            Err(error) => {
                scan_errors.push(format!(
                    "could not read source directory `{}` while searching `{pattern}`: {error}",
                    dir.display()
                ));
                return false;
            }
        };
        for entry in entries {
            let entry = match entry {
                Ok(entry) => entry,
                Err(error) => {
                    scan_errors.push(format!(
                        "could not read source entry in `{}` while searching `{pattern}`: {error}",
                        dir.display()
                    ));
                    continue;
                }
            };
            let path = entry.path();
            // Skip the self_substrate directory itself  -  internal
            // cross-references don't count as consumption.
            if path.components().any(|c| c.as_os_str() == "self_substrate") {
                continue;
            }
            // Skip target dirs.
            if path.components().any(|c| c.as_os_str() == "target") {
                continue;
            }
            if path.is_dir() {
                if walk(&path, pattern, scan_errors) {
                    return true;
                }
            } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
                let content = match read_text_bounded(&path) {
                    Ok(content) => content,
                    Err(error) => {
                        scan_errors.push(format!(
                            "could not read source file `{}` while searching `{pattern}`: {error}",
                            path.display()
                        ));
                        continue;
                    }
                };
                if content.contains(pattern) {
                    return true;
                }
            }
        }
        false
    }
    walk(root, pattern, scan_errors)
}

fn collect_bench_references(
    root: &Path,
    modules: &[String],
    scan_errors: &mut Vec<String>,
) -> BTreeSet<String> {
    let mut refs = BTreeSet::new();
    let scan_root = root.join(VYRE_ROOT);
    for bench_dir in BENCH_DIRS {
        let dir = scan_root.join(bench_dir);
        if !dir.exists() {
            continue;
        }
        let entries = match fs::read_dir(&dir) {
            Ok(e) => e,
            Err(error) => {
                scan_errors.push(format!(
                    "could not read benchmark directory `{}`: {error}",
                    dir.display()
                ));
                continue;
            }
        };
        for entry in entries {
            let entry = match entry {
                Ok(entry) => entry,
                Err(error) => {
                    scan_errors.push(format!(
                        "could not read benchmark entry in `{}`: {error}",
                        dir.display()
                    ));
                    continue;
                }
            };
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("rs") {
                continue;
            }
            let content = match read_text_bounded(&path) {
                Ok(content) => content,
                Err(error) => {
                    scan_errors.push(format!(
                        "could not read benchmark file `{}`: {error}",
                        path.display()
                    ));
                    continue;
                }
            };
            for module in modules {
                if content.contains(module.as_str()) {
                    refs.insert(module.clone());
                }
            }
        }
    }
    refs
}

fn read_text_bounded(path: &Path) -> io::Result<String> {
    let mut reader =
        fs::File::open(path)?.take(MAX_MEASUREMENT_GATE_SOURCE_BYTES.saturating_add(1));
    let mut text = String::new();
    reader.read_to_string(&mut text)?;
    if text.len() as u64 > MAX_MEASUREMENT_GATE_SOURCE_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "{} exceeds {MAX_MEASUREMENT_GATE_SOURCE_BYTES} byte measurement gate read cap",
                path.display()
            ),
        ));
    }
    Ok(text)
}
