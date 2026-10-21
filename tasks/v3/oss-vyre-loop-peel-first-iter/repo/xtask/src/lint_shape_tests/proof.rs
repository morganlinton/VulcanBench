use std::fs;
use std::path::Path;

const PLAN_PROOF_TRUTH_MARKERS: &[&str] = &[
    "exact",
    "expected",
    "specific",
    "field",
    "fields",
    "schema",
    "digest",
    "fingerprint",
    "count",
    "counts",
    "coverage",
    "zero",
    "empty missing",
    "missing_",
    "missing ",
    "failed count",
    "failed pairs",
    "positive",
    "negative",
    "adversarial",
    "boundary",
    "rejects",
    "rejection",
    "compares",
    "comparison",
    "parity",
    "output bytes",
    "error",
    "line",
    "cwe",
    "value",
    "values",
    "records",
    "effective",
    "at least",
    "fn main",
    "import",
    "runnable",
    "regression",
    "release-blocking",
    "must use",
    "every required",
];
const PLAN_PROOF_SHAPE_ONLY_MARKERS: &[&str] = &[
    "shape test",
    "shape-style",
    "shape-only",
    "smoke",
    "is_ok",
    "is_err",
    "non-empty",
    "status-only",
    "status only",
    "status.success",
    "success-only",
    "roundtrip-only",
    "compiles",
    "does not panic",
];
const PLAN_PROOF_SURFACE_MARKERS: &[&str] = &[
    "test",
    "tests",
    "gate",
    "gates",
    "evidence",
    "artifact",
    "artifacts",
    "proof",
    "check",
    "checks",
    "release",
    "ci",
    "workflow",
    "workflows",
    "weir",
];

pub(crate) fn plan_proof_shape_failures(plan_text: &str) -> Vec<String> {
    let mut failures = Vec::new();
    for line in plan_text.lines() {
        let trimmed = line.trim();
        if !trimmed.starts_with("| VX-") {
            continue;
        }
        let cells = trimmed
            .trim_matches('|')
            .split('|')
            .map(str::trim)
            .collect::<Vec<_>>();
        if cells.len() < 6 {
            continue;
        }
        let row_id = cells[0];
        let proof = cells[5];
        if let Some(failure) = proof_text_shape_failure(&format!("{row_id} proof gate"), proof) {
            failures.push(failure);
        }
    }
    failures
}

pub(crate) fn release_evidence_shape_failures(
    release_evidence_docs: &Path,
    scan_errors: &mut Vec<String>,
) -> Vec<String> {
    let mut failures = Vec::new();
    let entries = match fs::read_dir(release_evidence_docs) {
        Ok(entries) => entries,
        Err(error) => {
            scan_errors.push(format!(
                "could not read release evidence docs for proof-shape audit `{}`: {error}",
                release_evidence_docs.display()
            ));
            return failures;
        }
    };

    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) => {
                scan_errors.push(format!(
                    "could not read release evidence docs entry in `{}`: {error}",
                    release_evidence_docs.display()
                ));
                continue;
            }
        };
        let path = entry.path();
        if !path.is_file() || !path.extension().is_some_and(|ext| ext == "md") {
            continue;
        }
        let text = match super::read_text_bounded(&path) {
            Ok(text) => text,
            Err(error) => {
                scan_errors.push(format!(
                    "could not read release evidence proof doc `{}`: {error}",
                    path.display()
                ));
                continue;
            }
        };
        failures.extend(release_evidence_doc_shape_failures(&path.display().to_string(), &text));
    }

    failures
}

pub(crate) fn release_evidence_doc_shape_failures(
    path_label: &str,
    doc_text: &str,
) -> Vec<String> {
    doc_text
        .lines()
        .enumerate()
        .filter_map(|(line_index, line)| {
            let trimmed = line.trim();
            if !trimmed.starts_with("- ") {
                return None;
            }
            proof_text_shape_failure(&format!("{path_label}:{}", line_index + 1), trimmed)
        })
        .collect()
}

fn proof_text_shape_failure(label: &str, proof: &str) -> Option<String> {
    let proof_lower = proof.to_ascii_lowercase();
    if !proof_mentions_audited_surface(&proof_lower) || !proof_has_shape_only_marker(&proof_lower) {
        return None;
    }
    if proof_has_truth_marker(&proof_lower) {
        return None;
    }
    Some(format!(
        "{label} relies only on shape-style/status/smoke/non-empty proof language: `{proof}`"
    ))
}

fn proof_mentions_audited_surface(proof_lower: &str) -> bool {
    PLAN_PROOF_SURFACE_MARKERS
        .iter()
        .any(|marker| proof_lower.contains(marker))
}

fn proof_has_shape_only_marker(proof_lower: &str) -> bool {
    PLAN_PROOF_SHAPE_ONLY_MARKERS
        .iter()
        .any(|marker| proof_lower.contains(marker))
}

fn proof_has_truth_marker(proof_lower: &str) -> bool {
    PLAN_PROOF_TRUTH_MARKERS
        .iter()
        .any(|marker| proof_lower.contains(marker))
}
