use std::collections::BTreeSet;
use std::path::Path;

use super::model::{SourceLedgerFinding, VxRow};
use crate::research_key::backtick_research_keys;
use crate::research_source_ledger::{
    competitor_issue_source_keys, read_competitor_issue_ledger, read_research_source_ledger,
    unknown_competitor_issue_vx_rows, unknown_research_source_vx_rows,
    COMPETITOR_ISSUE_LEDGER_PATH, RESEARCH_SOURCE_LEDGER_PATH,
};

pub(super) fn collect_source_ledger_findings(
    root: &Path,
    defined_research_keys: &BTreeSet<String>,
    used_research_keys: &BTreeSet<String>,
    rows: &[VxRow],
) -> Vec<SourceLedgerFinding> {
    let mut findings = Vec::new();
    let ledger = match read_research_source_ledger(root) {
        Ok(ledger) => ledger,
        Err(error) => {
            findings.push(finding(
                "<ledger>",
                error,
                "research-source-ledger-toml",
            ));
            return findings;
        }
    };
    let Some(sources) = ledger.sources.as_ref() else {
        findings.push(finding(
            "<ledger>",
            format!("{RESEARCH_SOURCE_LEDGER_PATH} has no [[sources]] entries"),
            "research-source-ledger-sources",
        ));
        return findings;
    };

    let row_ids = rows
        .iter()
        .map(|row| row.id.clone())
        .collect::<BTreeSet<_>>();
    let mut ledger_keys = BTreeSet::new();
    for entry in sources {
        let key = entry.key.clone().unwrap_or_default();
        ledger_keys.insert(key.clone());
        if !defined_research_keys.contains(&key) && !used_research_keys.contains(&key) {
            findings.push(finding(
                &key,
                format!("source ledger key `{key}` is not defined or used by the plan"),
                "research-source-ledger-plan-linkage",
            ));
        }
        for vx_row in entry.vx_rows.as_deref().unwrap_or_default() {
            let Some(row) = rows.iter().find(|row| row.id.as_str() == vx_row.as_str()) else {
                continue;
            };
            if !source_key_is_cited_by_row(&key, row) {
                findings.push(finding(
                    &key,
                    format!(
                        "source ledger key `{key}` maps to `{vx_row}` but that row does not cite `{key}` in research_basis"
                    ),
                    "research-source-ledger-impact",
                ));
            }
            if !row_has_implementation_impact(row) {
                findings.push(finding(
                    &key,
                    format!(
                        "source ledger key `{key}` maps to `{vx_row}` without artifact, benchmark, test, comparator, evidence, validator, actionable gate, or stale-source impact"
                    ),
                    "research-source-ledger-impact",
                ));
            }
        }
    }
    for unknown in unknown_research_source_vx_rows(&ledger, &row_ids) {
        findings.push(finding(
            &unknown.key,
            format!(
                "source ledger key `{}` links unknown VX row `{}`",
                unknown.key, unknown.vx_row
            ),
            "research-source-ledger-vx-row-exists",
        ));
    }

    for required in defined_research_keys.union(used_research_keys) {
        if !ledger_keys.contains(required) {
            findings.push(finding(
                required,
                format!("research key `{required}` is missing from {RESEARCH_SOURCE_LEDGER_PATH}"),
                "research-source-ledger-required-key",
            ));
        }
    }

    findings
}

pub(super) fn collect_competitor_issue_findings(
    root: &Path,
    defined_research_keys: &BTreeSet<String>,
    used_research_keys: &BTreeSet<String>,
    rows: &[VxRow],
) -> Vec<SourceLedgerFinding> {
    let mut findings = Vec::new();
    let ledger = match read_competitor_issue_ledger(root) {
        Ok(ledger) => ledger,
        Err(error) => {
            findings.push(SourceLedgerFinding {
                path: COMPETITOR_ISSUE_LEDGER_PATH.to_string(),
                key: "<ledger>".to_string(),
                text: error,
                policy: "competitor-issue-ledger-toml".to_string(),
            });
            return findings;
        }
    };
    let row_ids = rows
        .iter()
        .map(|row| row.id.clone())
        .collect::<BTreeSet<_>>();
    for source_key in competitor_issue_source_keys(&ledger) {
        if !defined_research_keys.contains(&source_key) && !used_research_keys.contains(&source_key)
        {
            findings.push(SourceLedgerFinding {
                path: COMPETITOR_ISSUE_LEDGER_PATH.to_string(),
                key: source_key.clone(),
                text: format!(
                    "competitor issue source key `{source_key}` is not defined or used by the plan"
                ),
                policy: "competitor-issue-plan-linkage".to_string(),
            });
        }
    }
    let Some(issues) = ledger.issues.as_ref() else {
        findings.push(SourceLedgerFinding {
            path: COMPETITOR_ISSUE_LEDGER_PATH.to_string(),
            key: "<ledger>".to_string(),
            text: format!("{COMPETITOR_ISSUE_LEDGER_PATH} has no [[issues]] entries"),
            policy: "competitor-issue-ledger-issues".to_string(),
        });
        return findings;
    };
    for issue in issues {
        let id = issue.id.clone().unwrap_or_default();
        let source_key = issue.source_key.clone().unwrap_or_default();
        let fixture = issue.local_fixture.clone().unwrap_or_default();
        for vx_row in issue.vx_rows.as_deref().unwrap_or_default() {
            let Some(row) = rows.iter().find(|row| row.id.as_str() == vx_row.as_str()) else {
                continue;
            };
            if !source_key_is_cited_by_row(&source_key, row) {
                findings.push(SourceLedgerFinding {
                    path: COMPETITOR_ISSUE_LEDGER_PATH.to_string(),
                    key: id.clone(),
                    text: format!(
                        "competitor issue `{id}` maps to `{vx_row}` but that row does not cite `{source_key}`"
                    ),
                    policy: "competitor-issue-row-citation".to_string(),
                });
            }
            if !row_has_implementation_impact(row) {
                findings.push(SourceLedgerFinding {
                    path: COMPETITOR_ISSUE_LEDGER_PATH.to_string(),
                    key: id.clone(),
                    text: format!(
                        "competitor issue `{id}` maps to `{vx_row}` without artifact, benchmark, test, comparator, evidence, validator, actionable gate, or stale-source impact"
                    ),
                    policy: "competitor-issue-impact".to_string(),
                });
            }
        }
        if fixture.trim().is_empty() {
            findings.push(SourceLedgerFinding {
                path: COMPETITOR_ISSUE_LEDGER_PATH.to_string(),
                key: id,
                text: "competitor issue is missing local fixture id".to_string(),
                policy: "competitor-issue-fixture".to_string(),
            });
        }
    }
    for unknown in unknown_competitor_issue_vx_rows(&ledger, &row_ids) {
        findings.push(SourceLedgerFinding {
            path: COMPETITOR_ISSUE_LEDGER_PATH.to_string(),
            key: unknown.id,
            text: format!("competitor issue links unknown VX row `{}`", unknown.vx_row),
            policy: "competitor-issue-vx-row-exists".to_string(),
        });
    }
    findings
}

fn source_key_is_cited_by_row(key: &str, row: &VxRow) -> bool {
    backtick_research_keys(&row.research_basis)
        .iter()
        .any(|candidate| candidate == key)
}

fn row_has_implementation_impact(row: &VxRow) -> bool {
    !source_impact_markers(row).is_empty()
}

fn source_impact_markers(row: &VxRow) -> BTreeSet<&'static str> {
    let impact_text = format!(
        "{} {} {} {}",
        row.local_evidence, row.work, row.proof_gate, row.research_basis
    )
    .to_ascii_lowercase();
    let proof_gate = row.proof_gate.to_ascii_lowercase();
    let mut markers = BTreeSet::new();
    if impact_text.contains("artifact") || impact_text.contains("release/evidence/") {
        markers.insert("artifact");
    }
    if impact_text.contains("bench") || impact_text.contains("benchmark") {
        markers.insert("benchmark");
    }
    if impact_text.contains("test")
        || impact_text.contains("fixture")
        || impact_text.contains("corpus")
        || impact_text.contains("fuzz")
    {
        markers.insert("test");
    }
    if impact_text.contains("comparator")
        || impact_text.contains("baseline")
        || impact_text.contains("differential")
        || impact_text.contains("parity")
    {
        markers.insert("comparator");
    }
    if impact_text.contains("evidence")
        || impact_text.contains("source_digest")
        || impact_text.contains("schema")
        || impact_text.contains("report")
    {
        markers.insert("evidence");
    }
    if impact_text.contains("validator")
        || impact_text.contains("validate")
        || impact_text.contains("semantic validation")
    {
        markers.insert("validator");
    }
    if impact_text.contains("stale-source")
        || impact_text.contains("stale source")
        || impact_text.contains("source_tree_fingerprint")
        || impact_text.contains("freshness")
    {
        markers.insert("stale-source");
    }
    if proof_gate.contains("gate")
        && [
            "reject",
            "assert",
            "block",
            "validate",
            "fail",
            "enforce",
        ]
        .iter()
        .any(|verb| proof_gate.contains(verb))
    {
        markers.insert("gate");
    }
    markers
}

fn finding(
    key: impl Into<String>,
    text: impl Into<String>,
    policy: impl Into<String>,
) -> SourceLedgerFinding {
    SourceLedgerFinding {
        path: RESEARCH_SOURCE_LEDGER_PATH.to_string(),
        key: key.into(),
        text: text.into(),
        policy: policy.into(),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::{
        collect_competitor_issue_findings, row_has_implementation_impact, source_impact_markers,
        source_key_is_cited_by_row,
    };
    use crate::research_audit::model::VxRow;

    #[test]
    fn source_impact_requires_key_citation_and_implementation_marker() {
        let row = VxRow {
            line: 1,
            id: "VX-999".to_string(),
            axis: "evidence_truth".to_string(),
            local_evidence: "Plan prose only.".to_string(),
            research_basis: "`OTHER_KEY`".to_string(),
            work: "Improvement: describe source.".to_string(),
            proof_gate: "Manual note.".to_string(),
            dedup_seam: "One source ledger seam.".to_string(),
        };
        assert!(!source_key_is_cited_by_row("MLIR_PASS", &row));
        assert!(!row_has_implementation_impact(&row));

        let row = VxRow {
            research_basis: "`MLIR_PASS`".to_string(),
            proof_gate: "Research audit test rejects missing implementation impact.".to_string(),
            ..row
        };
        assert!(source_key_is_cited_by_row("MLIR_PASS", &row));
        assert!(row_has_implementation_impact(&row));
    }

    #[test]
    fn source_impact_rejects_generic_gate_without_enforcement() {
        let row = VxRow {
            line: 1,
            id: "VX-999".to_string(),
            axis: "evidence_truth".to_string(),
            local_evidence: "Plan prose only.".to_string(),
            research_basis: "`MLIR_PASS`".to_string(),
            work: "Improvement: describe source.".to_string(),
            proof_gate: "Manual gate note.".to_string(),
            dedup_seam: "One source ledger seam.".to_string(),
        };
        assert!(
            source_impact_markers(&row).is_empty(),
            "Fix: source impact must not treat generic gate prose as implementation impact."
        );

        let row = VxRow {
            proof_gate: "Research audit gate rejects missing implementation impact.".to_string(),
            ..row
        };
        assert!(row_has_implementation_impact(&row));
        assert_eq!(
            source_impact_markers(&row).into_iter().collect::<Vec<_>>(),
            vec!["gate"],
            "Fix: explicit rejecting gates should remain valid implementation-impact markers."
        );
    }

    #[test]
    fn competitor_issue_findings_reject_unknown_vx_rows() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join("docs/optimization")).unwrap();
        std::fs::write(
            tmp.path().join("docs/optimization/COMPETITOR_ISSUE_LEDGER.toml"),
            r#"
[schema]
version = 1
ledger = "vyre-competitor-issue-ledger"
recorded_on = "2026-06-10"
contract = "Each issue row records competitor regression fixture mapping requirements."

[[issues]]
id = "HYPERSCAN-ISSUE-68"
source_key = "HYPERSCAN"
url = "https://github.com/intel/hyperscan/issues/68"
issue_type = "performance-cliff"
status = "closed"
affected_version = "4.5.2"
labels = ["regex-performance-cliff"]
local_fixture = "hyperscan-anchor-null-performance-cliff"
vx_rows = ["VX-999"]
digest_material = "HYPERSCAN-ISSUE-68|HYPERSCAN|https://github.com/intel/hyperscan/issues/68|performance-cliff|closed|4.5.2|regex-performance-cliff|hyperscan-anchor-null-performance-cliff"
"#,
        )
        .unwrap();
        let rows = vec![VxRow {
            line: 1,
            id: "VX-451".to_string(),
            axis: "evidence_truth".to_string(),
            local_evidence: "`docs/optimization/COMPETITOR_ISSUE_LEDGER.toml`".to_string(),
            research_basis: "`HYPERSCAN`".to_string(),
            work: "Improvement: add competitor issue fixture evidence.".to_string(),
            proof_gate: "Research audit rejects unknown competitor issue VX rows.".to_string(),
            dedup_seam: "One competitor issue seam.".to_string(),
        }];

        let findings = collect_competitor_issue_findings(
            tmp.path(),
            &["HYPERSCAN".to_string()].into_iter().collect(),
            &BTreeSet::new(),
            &rows,
        );

        assert!(findings
            .iter()
            .any(|finding| finding.policy == "competitor-issue-vx-row-exists"));
    }
}
