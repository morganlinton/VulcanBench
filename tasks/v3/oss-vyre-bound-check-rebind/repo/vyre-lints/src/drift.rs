//! Allowlist drift sentinel.
//!
//! `vyre-lints/allowlist.toml` is the migration release valve for the
//! `raw_ir_in_libs` lint. Entries exempt files while their migration
//! ticket is in flight. Without a drift gate, entries silently park
//! forever and the lint becomes advisory.
//!
//! This module loads the allowlist, asks `git blame` when each entry
//! line first appeared, and reports any entry older than the budget
//! (default 14 days). CI runs the sentinel via `vyre-lints` binary
//! `--check-drift`; >0 stale entries → exit 1.
//!
//! The age resolver is injectable so tests don't shell out to git.

use anyhow::{Context, Result};
use std::path::Path;
use std::process::Command;

/// Maximum age, in days, that an allowlist entry may sit before
/// counting as drift.
pub const DEFAULT_AGE_BUDGET_DAYS: i64 = 14;

/// One stale allowlist entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DriftFinding {
    /// The exempt file path as it appears in the allowlist.
    pub exempt_path: String,
    /// Days since the entry was first added.
    pub age_days: i64,
    /// Date the entry was added (YYYY-MM-DD).
    pub added: String,
}

/// Resolve when an entry first appeared. Returns the (`age_days`,
/// `added_date_iso`) pair. Implementors decide where the data comes
/// from  -  production uses git blame, tests inject deterministic
/// values.
pub trait AgeResolver {
    fn age(&self, allowlist_path: &Path, exempt_path: &str) -> Option<(i64, String)>;
}

/// Default git-blame-backed resolver.
pub struct GitBlameResolver {
    today_iso: String,
}

impl GitBlameResolver {
    /// Construct a resolver anchored at today's date in `YYYY-MM-DD`
    /// form. The caller passes the date so tests don't have to mock
    /// the clock at this layer.
    #[must_use]
    pub fn with_today(today_iso: impl Into<String>) -> Self {
        Self {
            today_iso: today_iso.into(),
        }
    }
}

impl AgeResolver for GitBlameResolver {
    fn age(&self, allowlist_path: &Path, exempt_path: &str) -> Option<(i64, String)> {
        let repo_root = allowlist_path.parent()?.parent()?;
        let allowlist_rel = allowlist_path.strip_prefix(repo_root).ok()?;
        let blame = Command::new("git")
            .args([
                "blame",
                "--date=short",
                "--line-porcelain",
                allowlist_rel.to_str()?,
            ])
            .current_dir(repo_root)
            .output()
            .ok()?;
        if !blame.status.success() {
            return None;
        }
        let text = String::from_utf8_lossy(&blame.stdout);
        let added = first_blame_date_for_entry(&text, exempt_path)?;
        let age = days_between_iso(&added, &self.today_iso)?;
        Some((age, added))
    }
}

/// Run the drift check.
///
/// Returns one finding per allowlist entry whose age exceeds the
/// budget. An empty allowlist returns zero findings. Errors only on
/// I/O or parse failure of the allowlist file.
pub fn run<R: AgeResolver>(
    allowlist_path: &Path,
    budget_days: i64,
    resolver: &R,
) -> Result<Vec<DriftFinding>> {
    let entries = load_entries(allowlist_path)?;
    let mut out = Vec::new();
    for exempt in entries {
        if let Some((age, added)) = resolver.age(allowlist_path, &exempt) {
            if age > budget_days {
                out.push(DriftFinding {
                    exempt_path: exempt,
                    age_days: age,
                    added,
                });
            }
        }
    }
    out.sort_by(|a, b| b.age_days.cmp(&a.age_days));
    Ok(out)
}

fn load_entries(path: &Path) -> Result<Vec<String>> {
    let bytes = crate::read_source_bounded(path)
        .with_context(|| format!("read allowlist {}", path.display()))?;
    #[derive(serde::Deserialize)]
    struct Raw {
        #[serde(default)]
        exempt_files: Vec<String>,
    }
    let raw: Raw =
        toml::from_str(&bytes).with_context(|| format!("parse allowlist {}", path.display()))?;
    Ok(raw.exempt_files)
}

/// Walk the porcelain blame output for the line whose content
/// contains the given exempt path, returning the commit date of that
/// line.
fn first_blame_date_for_entry(blame: &str, exempt_path: &str) -> Option<String> {
    let mut current_date: Option<String> = None;
    for line in blame.lines() {
        if let Some(rest) = line.strip_prefix("author-time ") {
            // author-time is unix epoch; we prefer the YYYY-MM-DD
            // emitted by --date=short in the line that starts with
            // "author-mail" / "summary"... actually --date=short
            // controls the `^FFFFFFFF (Author YYYY-MM-DD HH ...) line`
            // header; the porcelain still gives epoch in author-time.
            if let Ok(ts) = rest.trim().parse::<i64>() {
                current_date = Some(epoch_to_iso(ts));
            }
        } else if let Some(content) = line.strip_prefix('\t') {
            if content.contains(exempt_path) {
                if let Some(date) = current_date.clone() {
                    return Some(date);
                }
            }
        }
    }
    None
}

fn epoch_to_iso(ts: i64) -> String {
    let days = ts.div_euclid(86_400);
    iso_from_days(days)
}

/// Convert a count of days since the Unix epoch into a `YYYY-MM-DD`
/// string. Avoids pulling in chrono.
fn iso_from_days(mut days: i64) -> String {
    let mut y = 1970i64;
    loop {
        let len = if is_leap(y) { 366 } else { 365 };
        if days < len {
            break;
        }
        days -= len;
        y += 1;
    }
    let months_in_year: [i64; 12] = if is_leap(y) {
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };
    let mut m = 0usize;
    while days >= months_in_year[m] {
        days -= months_in_year[m];
        m += 1;
    }
    let d = days + 1;
    format!("{y:04}-{:02}-{:02}", m + 1, d)
}

fn is_leap(y: i64) -> bool {
    (y % 4 == 0 && y % 100 != 0) || (y % 400 == 0)
}

fn days_between_iso(start_iso: &str, end_iso: &str) -> Option<i64> {
    let start = days_since_epoch(start_iso)?;
    let end = days_since_epoch(end_iso)?;
    Some(end - start)
}

fn days_since_epoch(iso: &str) -> Option<i64> {
    let parts: Vec<&str> = iso.split('-').collect();
    if parts.len() != 3 {
        return None;
    }
    let y: i64 = parts[0].parse().ok()?;
    let m: i64 = parts[1].parse().ok()?;
    let d: i64 = parts[2].parse().ok()?;
    if !(1..=12).contains(&m) {
        return None;
    }
    let mut days: i64 = 0;
    let mut iy = 1970i64;
    while iy < y {
        days += if is_leap(iy) { 366 } else { 365 };
        iy += 1;
    }
    let months: [i64; 12] = if is_leap(y) {
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };
    for &month_days in months.iter().take((m - 1) as usize) {
        days += month_days;
    }
    days += d - 1;
    Some(days)
}

/// Format a finding line as it should appear in CLI output and CI logs.
#[must_use]
pub fn format_finding(f: &DriftFinding, budget_days: i64) -> String {
    format!(
        "  ✗ {} | added {} | age {} days | exceeds {} day budget",
        f.exempt_path, f.added, f.age_days, budget_days
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::io::Write;
    use tempfile::TempDir;

    struct StaticResolver {
        ages: HashMap<String, (i64, String)>,
    }

    impl AgeResolver for StaticResolver {
        fn age(&self, _: &Path, exempt: &str) -> Option<(i64, String)> {
            self.ages.get(exempt).cloned()
        }
    }

    fn write_allowlist(dir: &Path, entries: &[&str]) -> std::path::PathBuf {
        let body: String = entries
            .iter()
            .map(|e| format!("  \"{e}\","))
            .collect::<Vec<_>>()
            .join("\n");
        let toml = format!("exempt_files = [\n{body}\n]\n");
        let path = dir.join("allowlist.toml");
        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(toml.as_bytes()).unwrap();
        path
    }

    #[test]
    fn empty_allowlist_yields_no_findings() {
        let dir = TempDir::new().unwrap();
        let path = write_allowlist(dir.path(), &[]);
        let resolver = StaticResolver {
            ages: HashMap::new(),
        };
        let findings = run(&path, DEFAULT_AGE_BUDGET_DAYS, &resolver).unwrap();
        assert!(findings.is_empty());
    }

    #[test]
    fn entry_under_budget_does_not_drift() {
        let dir = TempDir::new().unwrap();
        let path = write_allowlist(dir.path(), &["vyre-libs/src/foo.rs"]);
        let mut ages = HashMap::new();
        ages.insert(
            "vyre-libs/src/foo.rs".to_string(),
            (5_i64, "2026-04-27".to_string()),
        );
        let resolver = StaticResolver { ages };
        let findings = run(&path, DEFAULT_AGE_BUDGET_DAYS, &resolver).unwrap();
        assert!(findings.is_empty());
    }

    #[test]
    fn entry_over_budget_is_flagged() {
        let dir = TempDir::new().unwrap();
        let path = write_allowlist(dir.path(), &["vyre-libs/src/old.rs"]);
        let mut ages = HashMap::new();
        ages.insert(
            "vyre-libs/src/old.rs".to_string(),
            (30_i64, "2026-04-02".to_string()),
        );
        let resolver = StaticResolver { ages };
        let findings = run(&path, DEFAULT_AGE_BUDGET_DAYS, &resolver).unwrap();
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].exempt_path, "vyre-libs/src/old.rs");
        assert_eq!(findings[0].age_days, 30);
    }

    #[test]
    fn findings_sorted_oldest_first() {
        let dir = TempDir::new().unwrap();
        let path = write_allowlist(
            dir.path(),
            &[
                "vyre-libs/src/a.rs",
                "vyre-libs/src/b.rs",
                "vyre-libs/src/c.rs",
            ],
        );
        let mut ages = HashMap::new();
        ages.insert(
            "vyre-libs/src/a.rs".to_string(),
            (20_i64, "2026-04-12".to_string()),
        );
        ages.insert(
            "vyre-libs/src/b.rs".to_string(),
            (45_i64, "2026-03-18".to_string()),
        );
        ages.insert(
            "vyre-libs/src/c.rs".to_string(),
            (16_i64, "2026-04-16".to_string()),
        );
        let resolver = StaticResolver { ages };
        let findings = run(&path, DEFAULT_AGE_BUDGET_DAYS, &resolver).unwrap();
        let order: Vec<&str> = findings.iter().map(|f| f.exempt_path.as_str()).collect();
        assert_eq!(
            order,
            vec![
                "vyre-libs/src/b.rs",
                "vyre-libs/src/a.rs",
                "vyre-libs/src/c.rs"
            ]
        );
    }

    #[test]
    fn unresolved_entry_does_not_panic() {
        let dir = TempDir::new().unwrap();
        let path = write_allowlist(dir.path(), &["vyre-libs/src/missing.rs"]);
        let resolver = StaticResolver {
            ages: HashMap::new(),
        };
        let findings = run(&path, DEFAULT_AGE_BUDGET_DAYS, &resolver).unwrap();
        assert!(findings.is_empty());
    }

    #[test]
    fn iso_from_days_round_trips() {
        // 2026-05-02 should round-trip
        let days = days_since_epoch("2026-05-02").unwrap();
        assert_eq!(iso_from_days(days), "2026-05-02");
        // 1970-01-01 = day 0
        assert_eq!(iso_from_days(0), "1970-01-01");
        // Leap-year boundary: 2024-02-29
        let leap = days_since_epoch("2024-02-29").unwrap();
        assert_eq!(iso_from_days(leap), "2024-02-29");
    }

    #[test]
    fn days_between_iso_simple_intervals() {
        assert_eq!(days_between_iso("2026-04-27", "2026-05-02").unwrap(), 5);
        assert_eq!(days_between_iso("2026-05-02", "2026-04-27").unwrap(), -5);
        assert_eq!(days_between_iso("2026-05-02", "2026-05-02").unwrap(), 0);
    }

    #[test]
    fn first_blame_date_for_entry_picks_matching_line() {
        let blame = "\
0000000000000000000000000000000000000000 1 1 5
author Alice
author-time 1714521600
author-tz +0000
\texempt_files = [
0000000000000000000000000000000000000001 2 2
author Bob
author-time 1745020800
author-tz +0000
\t  \"vyre-libs/src/old.rs\",
0000000000000000000000000000000000000002 3 3
author Carol
author-time 1745625600
author-tz +0000
\t  \"vyre-libs/src/newer.rs\",
0000000000000000000000000000000000000003 4 4
author Dan
author-time 1745020800
author-tz +0000
\t]
";
        let date = first_blame_date_for_entry(blame, "vyre-libs/src/old.rs").unwrap();
        // 1745020800 = 2025-04-19
        assert_eq!(date, "2025-04-19");
    }

    #[test]
    fn format_finding_renders_expected_line() {
        let f = DriftFinding {
            exempt_path: "vyre-libs/src/old.rs".to_string(),
            age_days: 30,
            added: "2026-04-02".to_string(),
        };
        let line = format_finding(&f, DEFAULT_AGE_BUDGET_DAYS);
        assert!(line.contains("vyre-libs/src/old.rs"));
        assert!(line.contains("2026-04-02"));
        assert!(line.contains("30 days"));
        assert!(line.contains("14 day budget"));
    }
}
