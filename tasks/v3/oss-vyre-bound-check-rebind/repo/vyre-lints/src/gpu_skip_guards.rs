//! GPU skip guard.
//!
//! Vyre release validation is GPU-first. A missing adapter/probe failure must
//! fail loudly with an actionable diagnostic; it must not silently return
//! success or skip a CUDA/WGPU test path.

use crate::{paths::workspace_relative, Violation, ViolationKind};
use anyhow::Result;
use std::path::Path;

const DIRECT_SKIP_FRAGMENTS: &[&str] = &[
    "skipping gpu",
    "skipped gpu",
    "skip gpu",
    "skipping cuda",
    "skipped cuda",
    "skip cuda",
    "skipped: no gpu",
    "skipped: no cuda",
    "no gpu, skipping",
    "no cuda, skipping",
    "gpu unavailable, skipping",
    "cuda unavailable, skipping",
];

const PROBE_FAILURE_FRAGMENTS: &[&str] = &[
    "no gpu",
    "no_gpu",
    "no cuda",
    "no_cuda",
    "gpu unavailable",
    "gpu_unavailable",
    "cuda unavailable",
    "cuda_unavailable",
];

const SILENT_SUCCESS_FRAGMENTS: &[&str] = &[
    "return ok",
    "return;",
    "fallback",
    "cpu path",
    "ignore",
    "skip",
];

const LOUD_DIAGNOSTIC_FRAGMENTS: &[&str] = &[
    "do not skip gpu",
    "do not skip cuda",
    "must not skip gpu",
    "must not skip cuda",
    "never skip gpu",
    "never skip cuda",
    "fails loudly",
    "fail loudly",
    "fix:",
];

pub fn scan_tree(root: &Path) -> Result<Vec<Violation>> {
    let mut all = Vec::new();
    for entry in walkdir::WalkDir::new(root)
        .into_iter()
        .filter_map(|entry| entry.ok())
    {
        let path = entry.path();
        if !path.is_file() || path.extension().and_then(|ext| ext.to_str()) != Some("rs") {
            continue;
        }
        let workspace_rel = workspace_relative(path);
        all.extend(scan_file(path, &workspace_rel)?);
    }
    Ok(all)
}

fn scan_file(path: &Path, workspace_rel: &str) -> Result<Vec<Violation>> {
    let source = crate::read_source_bounded(path)?;
    let mut violations = Vec::new();
    let mut pending_probe_failure: Option<(u32, usize, u8)> = None;
    for (line_idx, line) in source.lines().enumerate() {
        let lower = line.to_ascii_lowercase();
        if is_loud_gpu_diagnostic(&lower) {
            pending_probe_failure = None;
            continue;
        }
        if let Some(column) = silent_gpu_skip_column(&lower) {
            violations.push(Violation {
                file: workspace_rel.to_string(),
                line: (line_idx + 1) as u32,
                column: column as u32,
                kind: ViolationKind::GpuSkipGuard,
                message: "GPU validation path silently skips or returns success when GPU probing fails. Fix: fail loudly with probe diagnostics; do not skip CUDA/WGPU validation on this fleet.".to_string(),
            });
            pending_probe_failure = None;
            continue;
        }
        if let Some((probe_line, probe_column, remaining)) = pending_probe_failure {
            if contains_silent_success(&lower) {
                violations.push(Violation {
                    file: workspace_rel.to_string(),
                    line: probe_line,
                    column: probe_column as u32,
                    kind: ViolationKind::GpuSkipGuard,
                    message: "GPU validation path silently skips or returns success when GPU probing fails. Fix: fail loudly with probe diagnostics; do not skip CUDA/WGPU validation on this fleet.".to_string(),
                });
                pending_probe_failure = None;
                continue;
            }
            pending_probe_failure = remaining
                .checked_sub(1)
                .filter(|next| *next > 0)
                .map(|next| (probe_line, probe_column, next));
        }
        if let Some(column) = probe_failure_column(&lower) {
            pending_probe_failure = Some(((line_idx + 1) as u32, column, 4));
        }
    }
    Ok(violations)
}

fn is_loud_gpu_diagnostic(lower: &str) -> bool {
    LOUD_DIAGNOSTIC_FRAGMENTS
        .iter()
        .any(|fragment| lower.contains(fragment))
}

fn silent_gpu_skip_column(lower: &str) -> Option<usize> {
    for fragment in DIRECT_SKIP_FRAGMENTS {
        if let Some(column) = lower.find(fragment) {
            return Some(column);
        }
    }
    for probe in PROBE_FAILURE_FRAGMENTS {
        if let Some(probe_col) = lower.find(probe) {
            let tail = &lower[probe_col..];
            if contains_silent_success(tail) {
                return Some(probe_col);
            }
        }
    }
    None
}

fn probe_failure_column(lower: &str) -> Option<usize> {
    PROBE_FAILURE_FRAGMENTS
        .iter()
        .filter_map(|probe| lower.find(probe))
        .min()
}

fn contains_silent_success(lower: &str) -> bool {
    SILENT_SUCCESS_FRAGMENTS
        .iter()
        .any(|fragment| lower.contains(fragment))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_direct_skip_phrases() {
        assert!(silent_gpu_skip_column("eprintln!(\"skipped: no gpu\")").is_some());
        assert!(silent_gpu_skip_column("warn!(\"cuda unavailable, skipping\")").is_some());
    }

    #[test]
    fn detects_no_gpu_success_return() {
        assert!(silent_gpu_skip_column("if no_gpu { return ok(()); }").is_some());
    }

    #[test]
    fn detects_underscore_probe_names() {
        assert_eq!(probe_failure_column("if no_gpu_adapter() {"), Some(3));
        assert_eq!(probe_failure_column("if gpu_unavailable() {"), Some(3));
    }

    #[test]
    fn permits_loud_probe_diagnostics() {
        let diagnostic = "no gpu adapter found. fix: inspect adapter probe and fail loudly.";
        assert!(is_loud_gpu_diagnostic(diagnostic));
        assert_eq!(silent_gpu_skip_column(diagnostic), None);
    }
}
