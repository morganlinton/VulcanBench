//! `cargo_full run --bin xtask -- lego-quick`  -  fast pre-commit gate.
//!
//! Runs the file-only subset of `lego-audit` against the staged diff
//! only. Target wall-clock ≤ 2s on a 10-file diff so it can sit in
//! `.git/hooks/pre-commit` without writers reaching for `--no-verify`.
//!
//! Three default checks, no inventory walk, no fingerprinting:
//!
//! 1. **Raw IR construction** (delegates to `vyre_lints`): no
//!    `Node::*` / `Expr::*` constructors in `vyre-libs/src/**`.
//! 2. **Cross-dialect reach-through**: a Tier-3 dialect under
//!    `vyre-libs/src/<dialect>/` cannot import from
//!    `crate::<other_dialect>` or `vyre_libs::<other_dialect>`.
//! 3. **Large-file advisory**: a staged `*.rs` file over the
//!    `LARGE_FILE_ADVISORY_LINES` line guideline is *flagged for review*,
//!    not failed. Crossing 500 lines is a prompt to ask whether the file
//!    has grown a second responsibility worth splitting — it is a
//!    guideline, not a law. The hard size cap (a genuine god-file ceiling
//!    with a ratcheted per-file exception list) lives in
//!    `scripts/check_max_file_size.sh`, not here.
//!
//! The full op-fingerprint reinvention check (`lego-audit` check 1)
//! requires loading every registered op via inventory and is too slow
//! for a default pre-commit hook  -  it stays in CI. Pass
//! `--source-similar` to also run the repo-wide Rust source duplicate
//! scanner as an explicit dedup gate.
//!
//! Exit code 0 on clean and on advisory-only runs; 1 only when a hard
//! law (raw-IR / cross-dialect / source-duplication) is violated. Each
//! finding prints `file:line | category | message | fix:` so writers can
//! act on it without re-reading docs.

use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::process::{self, Command};

/// Line count at which a source file is *flagged for a split-by-responsibility
/// review*. This is a guideline, not a law: crossing it does not fail the gate.
/// The hard god-file ceiling is enforced separately by
/// `scripts/check_max_file_size.sh` with its ratcheted per-file exception list.
const LARGE_FILE_ADVISORY_LINES: usize = 500;
const MAX_LEGO_QUICK_SOURCE_BYTES: u64 = 2_097_152;

pub(crate) fn run(args: &[String]) {
    let staged_only = !args.iter().any(|a| a == "--all");
    let source_similar = args.iter().any(|a| a == "--source-similar");
    let root = match workspace_root() {
        Some(r) => r,
        None => {
            eprintln!(
                "Fix: cargo_full run --bin xtask -- lego-quick must run from a git checkout of the vyre workspace."
            );
            process::exit(1);
        }
    };

    let files = if staged_only {
        match staged_rust_files(&root) {
            Ok(files) => files,
            Err(err) => {
                eprintln!(
                    "Fix: failed to list staged files via `git diff --cached --name-only`: {err}"
                );
                process::exit(1);
            }
        }
    } else {
        all_rust_files(&root)
    };

    if files.is_empty() {
        println!("lego-quick: no staged Rust files; nothing to check.");
        return;
    }

    let mut findings: Vec<Finding> = Vec::new();
    findings.extend(check_raw_ir(&root, &files));
    findings.extend(check_cross_dialect(&root, &files));
    findings.extend(check_god_files(&root, &files));
    if source_similar {
        findings.extend(check_source_similarity(&root));
    }

    let check_count = 3 + usize::from(source_similar);

    // `large-file` findings are advisory: they flag a split-by-responsibility
    // review, they do not fail the gate. Everything else is a hard law.
    let sort_key = |f: &Finding| (f.file.clone(), f.line, f.category.clone());
    let (mut advisories, mut blockers): (Vec<Finding>, Vec<Finding>) =
        findings.into_iter().partition(|f| f.category == "large-file");
    advisories.sort_by(|a, b| sort_key(a).cmp(&sort_key(b)));
    blockers.sort_by(|a, b| sort_key(a).cmp(&sort_key(b)));

    if blockers.is_empty() && advisories.is_empty() {
        println!(
            "lego-quick: ✓ {} staged Rust file(s) clean ({} checks).",
            files.len(),
            check_count
        );
        return;
    }

    for f in &blockers {
        println!(
            "  ✗ {}:{} | {} | {} | fix: {}",
            f.file, f.line, f.category, f.message, f.fix
        );
    }
    for f in &advisories {
        println!(
            "  • {}:{} | {} | {} | {}",
            f.file, f.line, f.category, f.message, f.fix
        );
    }
    println!();

    if blockers.is_empty() {
        println!(
            "lego-quick: ✓ {} staged Rust file(s) pass ({} checks); \
             {} large-file advisory note(s) for review (non-blocking).",
            files.len(),
            check_count,
            advisories.len()
        );
        return;
    }

    println!(
        "lego-quick: FAILED  -  {} blocking finding(s) ({} advisory) across {} staged file(s). \
         Resolve the blocking findings before commit, or run \
         `cargo_full run --bin xtask -- lego-quick --all` to scan the whole tree.",
        blockers.len(),
        advisories.len(),
        files.len()
    );
    process::exit(1);
}

#[derive(Debug)]
struct Finding {
    file: String,
    line: u32,
    category: String,
    message: String,
    fix: String,
}

fn workspace_root() -> Option<PathBuf> {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(Path::to_path_buf)
}

fn staged_rust_files(root: &Path) -> Result<Vec<PathBuf>, String> {
    let output = Command::new("git")
        .args(["diff", "--cached", "--name-only", "--diff-filter=ACMR"])
        .current_dir(root)
        .output()
        .map_err(|e| e.to_string())?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).to_string());
    }
    let text = String::from_utf8_lossy(&output.stdout).to_string();
    let mut out = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || !trimmed.ends_with(".rs") {
            continue;
        }
        let path = root.join(trimmed);
        if path.is_file() {
            out.push(path);
        }
    }
    Ok(out)
}

fn all_rust_files(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for entry in walkdir::WalkDir::new(root)
        .into_iter()
        .filter_entry(|e| {
            let name = e.file_name().to_string_lossy();
            !matches!(
                name.as_ref(),
                ".git" | "target" | "target-codex" | "target-fusion-fix"
            )
        })
        .filter_map(|e| e.ok())
    {
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some("rs") {
            out.push(path.to_path_buf());
        }
    }
    out
}

fn check_raw_ir(root: &Path, files: &[PathBuf]) -> Vec<Finding> {
    let allowlist_path = root.join("vyre-lints").join("allowlist.toml");
    let allow = match vyre_lints::allowlist::load(&allowlist_path) {
        Ok(a) => a,
        Err(_) => vyre_lints::allowlist::Allowlist::empty(),
    };

    let mut out = Vec::new();
    for path in files {
        let path_str = path.to_string_lossy();
        if !path_str.contains("vyre-libs/src/") {
            continue;
        }
        let workspace_rel = workspace_relative(&path_str, "vyre-libs/");
        if allow.contains(&workspace_rel) {
            continue;
        }
        let Ok(violations) = scan_file_for_raw_ir(path, &workspace_rel) else {
            continue;
        };
        for v in violations {
            out.push(Finding {
                file: v.file,
                line: v.line,
                category: "raw-ir".to_string(),
                message: v.message,
                fix: "use vyre-primitives builders or region::wrap_anonymous instead of constructing Node/Expr directly".to_string(),
            });
        }
    }
    out
}

fn workspace_relative(path: &str, marker: &str) -> String {
    if let Some(idx) = path.find(marker) {
        path[idx..].to_string()
    } else {
        path.to_string()
    }
}

fn scan_file_for_raw_ir(
    path: &Path,
    workspace_rel: &str,
) -> anyhow::Result<Vec<vyre_lints::Violation>> {
    let allow = vyre_lints::allowlist::Allowlist::empty();
    // Single-file scan: build a one-element root from the file's parent
    // and let the existing scanner do its thing, then filter to the file.
    let parent = path.parent().unwrap_or(Path::new("."));
    let all = vyre_lints::raw_ir_in_libs::scan_tree(parent, &allow)?;
    let me_basename = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_default();
    Ok(all
        .into_iter()
        .filter(|v| v.file.ends_with(me_basename) && v.file.contains(workspace_rel))
        .collect())
}

/// Check 4 (file-only): a Tier-3 dialect under `vyre-libs/src/<X>/`
/// must not import `crate::<Y>::...` or `vyre_libs::<Y>::...` for
/// `Y != X`. The cross-dialect coupling belongs in `vyre-primitives`.
fn check_cross_dialect(root: &Path, files: &[PathBuf]) -> Vec<Finding> {
    let libs_root = root.join("vyre-libs").join("src");
    let dialects: Vec<String> = match std::fs::read_dir(&libs_root) {
        Ok(rd) => rd
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_dir())
            .filter_map(|e| e.file_name().into_string().ok())
            .filter(|n| {
                !matches!(
                    n.as_str(),
                    "region" | "tensor_ref" | "builder" | "buffer_names" | "descriptor"
                )
            })
            .collect(),
        Err(_) => return Vec::new(),
    };

    let mut out = Vec::new();
    for path in files {
        let path_str = path.to_string_lossy();
        let Some(idx) = path_str.find("vyre-libs/src/") else {
            continue;
        };
        let after = &path_str[idx + "vyre-libs/src/".len()..];
        let Some(this_dialect) = after.split('/').next() else {
            continue;
        };
        if !dialects.iter().any(|d| d == this_dialect) {
            continue;
        }
        let Ok(text) = read_text_bounded(path) else {
            continue;
        };
        let Ok(file) = syn::parse_file(&text) else {
            out.push(Finding {
                file: workspace_relative(&path_str, "vyre-libs/"),
                line: 0,
                category: "parse".to_string(),
                message: "failed to parse Rust source".to_string(),
                fix: "make the file syntactically valid before committing".to_string(),
            });
            continue;
        };
        for use_path in collect_use_paths(&file) {
            for other in &dialects {
                if other == this_dialect {
                    continue;
                }
                if use_path.imports_dialect(other) {
                    out.push(Finding {
                        file: workspace_relative(&path_str, "vyre-libs/"),
                        line: use_path.line as u32,
                        category: "cross-dialect".to_string(),
                        message: format!(
                            "imports `{}` from sibling dialect `{}`",
                            use_path.segments.join("::"),
                            other
                        ),
                        fix: "hoist the shared piece into vyre-primitives, or route via a public re-export at crate root".to_string(),
                    });
                }
            }
        }
    }
    out
}

fn check_god_files(root: &Path, files: &[PathBuf]) -> Vec<Finding> {
    let mut out = Vec::new();
    for path in files {
        let Ok(text) = read_text_bounded(path) else {
            continue;
        };
        let line_count = text.lines().count();
        if line_count <= LARGE_FILE_ADVISORY_LINES {
            continue;
        }
        let rel = path
            .strip_prefix(root)
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| path.to_string_lossy().to_string());
        out.push(Finding {
            file: rel,
            line: line_count as u32,
            category: "large-file".to_string(),
            message: format!(
                "{line_count} lines is over the {LARGE_FILE_ADVISORY_LINES}-line review guideline"
            ),
            fix: "review whether this file has grown a second responsibility; \
                  split by responsibility if so (this is advisory, not a build failure)"
                .to_string(),
        });
    }
    out
}

fn check_source_similarity(root: &Path) -> Vec<Finding> {
    let roots = vec![root.to_path_buf()];
    let report = match crate::source_similar::find_similar_sources(
        &roots,
        20,
        0.97,
        512 * 1024,
        false,
    ) {
        Ok(report) => report,
        Err(error) => {
            return vec![Finding {
                file: ".".to_string(),
                line: 0,
                category: "source-similar".to_string(),
                message: format!("source duplicate scan failed: {error}"),
                fix: "fix unreadable source paths or run `cargo_full run --bin xtask -- source-similar` for the raw scanner error".to_string(),
            }]
        }
    };
    report
        .findings
        .into_iter()
        .map(|finding| Finding {
            file: finding.left.clone(),
            line: 0,
            category: "source-similar".to_string(),
            message: format!(
                "{:.1}% similar to {} ({} vs {} normalized tokens)",
                finding.score * 100.0,
                finding.right,
                finding.left_tokens,
                finding.right_tokens
            ),
            fix: "extract the shared implementation into one module or lower the threshold only for exploratory source-similar runs".to_string(),
        })
        .collect()
}

fn read_text_bounded(path: &Path) -> io::Result<String> {
    let mut reader = std::fs::File::open(path)?.take(MAX_LEGO_QUICK_SOURCE_BYTES.saturating_add(1));
    let mut text = String::new();
    reader.read_to_string(&mut text)?;
    if text.len() as u64 > MAX_LEGO_QUICK_SOURCE_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "{} exceeds {MAX_LEGO_QUICK_SOURCE_BYTES} byte lego quick read cap",
                path.display()
            ),
        ));
    }
    Ok(text)
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct UsePath {
    segments: Vec<String>,
    line: usize,
}

impl UsePath {
    fn imports_dialect(&self, other_name: &str) -> bool {
        matches!(
            self.segments.as_slice(),
            [first, second, ..]
                if (first == "crate" || first == "vyre_libs") && second == other_name
        )
    }
}

fn collect_use_paths(file: &syn::File) -> Vec<UsePath> {
    let mut collector = UsePathCollector::default();
    syn::visit::visit_file(&mut collector, file);
    collector.paths
}

#[derive(Default)]
struct UsePathCollector {
    paths: Vec<UsePath>,
}

impl<'ast> syn::visit::Visit<'ast> for UsePathCollector {
    fn visit_item_use(&mut self, item: &'ast syn::ItemUse) {
        collect_use_tree(&item.tree, &mut Vec::new(), &mut self.paths);
    }
}

fn collect_use_tree(tree: &syn::UseTree, prefix: &mut Vec<String>, out: &mut Vec<UsePath>) {
    use syn::spanned::Spanned;
    match tree {
        syn::UseTree::Path(path) => {
            prefix.push(path.ident.to_string());
            collect_use_tree(&path.tree, prefix, out);
            prefix.pop();
        }
        syn::UseTree::Name(name) => {
            let mut segments = prefix.clone();
            segments.push(name.ident.to_string());
            out.push(UsePath {
                segments,
                line: name.span().start().line,
            });
        }
        syn::UseTree::Rename(rename) => {
            let mut segments = prefix.clone();
            segments.push(rename.ident.to_string());
            out.push(UsePath {
                segments,
                line: rename.span().start().line,
            });
        }
        syn::UseTree::Glob(glob) => {
            let mut segments = prefix.clone();
            segments.push("*".to_string());
            out.push(UsePath {
                segments,
                line: glob.span().start().line,
            });
        }
        syn::UseTree::Group(group) => {
            for item in &group.items {
                collect_use_tree(item, prefix, out);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    fn write(dir: &Path, rel: &str, body: &str) -> PathBuf {
        let p = dir.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        let mut f = std::fs::File::create(&p).unwrap();
        f.write_all(body.as_bytes()).unwrap();
        p
    }

    #[test]
    fn large_file_check_flags_oversize_as_advisory() {
        let dir = TempDir::new().unwrap();
        let body = "fn _f() {}\n".repeat(LARGE_FILE_ADVISORY_LINES + 5);
        let p = write(dir.path(), "vyre-libs/src/math/big.rs", &body);
        let findings = check_god_files(dir.path(), &[p]);
        assert_eq!(findings.len(), 1);
        // Advisory category: partitioned out of the blocking set at exit, so a
        // large file is flagged for review but never fails the gate.
        assert_eq!(findings[0].category, "large-file");
    }

    #[test]
    fn large_file_check_quiet_within_guideline() {
        let dir = TempDir::new().unwrap();
        let body = "fn _f() {}\n".repeat(50);
        let p = write(dir.path(), "vyre-libs/src/math/small.rs", &body);
        let findings = check_god_files(dir.path(), &[p]);
        assert!(findings.is_empty());
    }

    #[test]
    fn cross_dialect_check_flags_sibling_import() {
        let dir = TempDir::new().unwrap();
        // Set up two dialects so the dialect-name discovery succeeds.
        write(dir.path(), "vyre-libs/src/math/mod.rs", "");
        write(dir.path(), "vyre-libs/src/parsing/mod.rs", "");
        let p = write(
            dir.path(),
            "vyre-libs/src/math/uses_parsing.rs",
            "use crate::parsing::lexer;\nfn _f() {}\n",
        );
        let findings = check_cross_dialect(dir.path(), &[p]);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].category, "cross-dialect");
        assert!(findings[0].message.contains("parsing"));
    }

    #[test]
    fn cross_dialect_check_allows_same_dialect_import() {
        let dir = TempDir::new().unwrap();
        write(dir.path(), "vyre-libs/src/math/mod.rs", "");
        write(dir.path(), "vyre-libs/src/parsing/mod.rs", "");
        let p = write(
            dir.path(),
            "vyre-libs/src/math/uses_self.rs",
            "use crate::math::reduce;\nfn _f() {}\n",
        );
        let findings = check_cross_dialect(dir.path(), &[p]);
        assert!(findings.is_empty());
    }

    #[test]
    fn cross_dialect_check_allows_vyre_primitives_import() {
        let dir = TempDir::new().unwrap();
        write(dir.path(), "vyre-libs/src/math/mod.rs", "");
        write(dir.path(), "vyre-libs/src/parsing/mod.rs", "");
        let p = write(
            dir.path(),
            "vyre-libs/src/math/uses_primitives.rs",
            "use vyre_primitives::reduce_sum;\nfn _f() {}\n",
        );
        let findings = check_cross_dialect(dir.path(), &[p]);
        assert!(findings.is_empty());
    }

    #[test]
    fn source_similarity_check_reports_duplicate_source_pairs() {
        let dir = TempDir::new().unwrap();
        let body_a = "pub fn alpha(input: &[u32]) -> u32 {\n    let mut acc = 0;\n".to_string()
            + &"    for value in input { acc = acc.wrapping_add(*value); }\n".repeat(24)
            + "    acc\n}\n";
        let body_b = body_a.replace("alpha", "beta").replace("acc", "sum");
        write(dir.path(), "vyre-primitives/src/a.rs", &body_a);
        write(dir.path(), "vyre-primitives/src/b.rs", &body_b);

        let findings = check_source_similarity(dir.path());

        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].category, "source-similar");
        assert!(findings[0].message.contains("similar to"));
    }
}
