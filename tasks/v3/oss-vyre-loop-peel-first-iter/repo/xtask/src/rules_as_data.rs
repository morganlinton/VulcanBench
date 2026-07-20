//! Shared rules-as-data manifest validation.
//!
//! Expandable scanner, planner, issue, benchmark, and baseline lists must live
//! in TOML data files with ownership, source registration, and malformed-data
//! proof. This helper is consumed by both acceleration-plan and research-audit
//! evidence so the policy does not drift.

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use serde::Deserialize;

pub(crate) const RULES_AS_DATA_MANIFEST_PATH: &str =
    "docs/optimization/RULES_AS_DATA_MANIFEST.toml";
const RULES_AS_DATA_SCHEMA_VERSION: u32 = 1;
const RULES_AS_DATA_CONTRACT: &str = "vyre-rules-as-data-manifest:v1";
const COMMAND_MATRIX_PATH: &str = "docs/optimization/XTASK_COMMAND_MATRIX.md";
const OWNERSHIP_PATH: &str = "docs/optimization/OWNERSHIP.toml";

/// One validation finding for rules-as-data policy.
#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) struct RulesAsDataFinding {
    pub(crate) path: String,
    pub(crate) key: String,
    pub(crate) text: String,
    pub(crate) policy: String,
}

#[derive(Debug, Deserialize)]
struct RulesAsDataManifest {
    schema_version: u32,
    contract: String,
    data: Vec<RulesAsDataEntry>,
}

#[derive(Debug, Deserialize)]
struct RulesAsDataEntry {
    id: String,
    path: String,
    owner_lane: String,
    schema_token: String,
    command_matrix_source: bool,
    malformed_fixture: String,
}

pub(crate) fn validate_rules_as_data_manifest(root: &Path) -> Vec<String> {
    rules_as_data_findings(root)
        .into_iter()
        .map(|finding| {
            format!(
                "rules-as-data `{}` violates {} in {}: {}",
                finding.key, finding.policy, finding.path, finding.text
            )
        })
        .collect()
}

pub(crate) fn rules_as_data_findings(root: &Path) -> Vec<RulesAsDataFinding> {
    let manifest_text = match fs::read_to_string(root.join(RULES_AS_DATA_MANIFEST_PATH)) {
        Ok(text) => text,
        Err(error) => {
            return vec![finding(
                RULES_AS_DATA_MANIFEST_PATH,
                "manifest",
                format!("could not read {RULES_AS_DATA_MANIFEST_PATH}: {error}"),
                "rules-as-data-manifest-readable",
            )];
        }
    };
    let command_matrix = fs::read_to_string(root.join(COMMAND_MATRIX_PATH)).unwrap_or_default();
    let ownership_text = fs::read_to_string(root.join(OWNERSHIP_PATH)).unwrap_or_default();
    validate_rules_as_data_manifest_text(&manifest_text, &command_matrix, &ownership_text, root)
}

fn validate_rules_as_data_manifest_text(
    manifest_text: &str,
    command_matrix: &str,
    ownership_text: &str,
    root: &Path,
) -> Vec<RulesAsDataFinding> {
    let manifest = match toml::from_str::<RulesAsDataManifest>(manifest_text) {
        Ok(manifest) => manifest,
        Err(error) => {
            return vec![finding(
                RULES_AS_DATA_MANIFEST_PATH,
                "manifest",
                format!("invalid TOML: {error}"),
                "rules-as-data-manifest-toml",
            )];
        }
    };
    let mut findings = Vec::new();
    if manifest.schema_version != RULES_AS_DATA_SCHEMA_VERSION {
        findings.push(finding(
            RULES_AS_DATA_MANIFEST_PATH,
            "manifest",
            format!("schema_version must be {RULES_AS_DATA_SCHEMA_VERSION}"),
            "rules-as-data-manifest-schema",
        ));
    }
    if manifest.contract != RULES_AS_DATA_CONTRACT {
        findings.push(finding(
            RULES_AS_DATA_MANIFEST_PATH,
            "manifest",
            format!("contract must be {RULES_AS_DATA_CONTRACT}"),
            "rules-as-data-manifest-contract",
        ));
    }
    if manifest.data.is_empty() {
        findings.push(finding(
            RULES_AS_DATA_MANIFEST_PATH,
            "manifest",
            "manifest must declare at least one [[data]] entry",
            "rules-as-data-manifest-nonempty",
        ));
    }
    let ownership_lanes = crate::ownership::parse_ownership_lane_names(ownership_text)
        .unwrap_or_else(|_| BTreeSet::new());
    let mut ids = BTreeSet::new();
    let mut paths = BTreeSet::new();
    for entry in &manifest.data {
        validate_entry(
            entry,
            &mut ids,
            &mut paths,
            &ownership_lanes,
            command_matrix,
            root,
            &mut findings,
        );
    }
    findings
}

fn validate_entry(
    entry: &RulesAsDataEntry,
    ids: &mut BTreeSet<String>,
    paths: &mut BTreeSet<String>,
    ownership_lanes: &BTreeSet<String>,
    command_matrix: &str,
    root: &Path,
    findings: &mut Vec<RulesAsDataFinding>,
) {
    let key = entry.id.trim();
    if key.is_empty() {
        findings.push(finding(
            RULES_AS_DATA_MANIFEST_PATH,
            "manifest",
            "rules-as-data entry has blank id",
            "rules-as-data-id",
        ));
    } else if !ids.insert(key.to_string()) {
        findings.push(finding(
            RULES_AS_DATA_MANIFEST_PATH,
            key,
            "duplicate rules-as-data id",
            "rules-as-data-id-unique",
        ));
    }
    if entry.path.trim().is_empty() {
        findings.push(finding(
            RULES_AS_DATA_MANIFEST_PATH,
            key,
            "rules-as-data entry has blank path",
            "rules-as-data-path",
        ));
    } else if !paths.insert(entry.path.clone()) {
        findings.push(finding(
            RULES_AS_DATA_MANIFEST_PATH,
            key,
            format!("duplicate rules-as-data path `{}`", entry.path),
            "rules-as-data-path-unique",
        ));
    }
    let data_text = match fs::read_to_string(root.join(&entry.path)) {
        Ok(text) => text,
        Err(error) => {
            findings.push(finding(
                &entry.path,
                key,
                format!("data file is missing or unreadable: {error}"),
                "rules-as-data-file-readable",
            ));
            String::new()
        }
    };
    if entry.schema_token.trim().is_empty() {
        findings.push(finding(
            RULES_AS_DATA_MANIFEST_PATH,
            key,
            "schema_token is blank",
            "rules-as-data-schema-token",
        ));
    } else if !data_text.contains(&entry.schema_token) {
        findings.push(finding(
            &entry.path,
            key,
            format!("data file does not contain schema token `{}`", entry.schema_token),
            "rules-as-data-schema-token-present",
        ));
    }
    if entry.owner_lane.trim().is_empty() {
        findings.push(finding(
            RULES_AS_DATA_MANIFEST_PATH,
            key,
            "owner_lane is blank",
            "rules-as-data-owner-lane",
        ));
    } else if !ownership_lanes.contains(entry.owner_lane.trim()) {
        findings.push(finding(
            RULES_AS_DATA_MANIFEST_PATH,
            key,
            format!("owner_lane `{}` is not declared in {OWNERSHIP_PATH}", entry.owner_lane),
            "rules-as-data-owner-lane-declared",
        ));
    }
    if entry.command_matrix_source && !command_matrix.contains(&entry.path) {
        findings.push(finding(
            RULES_AS_DATA_MANIFEST_PATH,
            key,
            format!("{} does not list `{}` as a shared source", COMMAND_MATRIX_PATH, entry.path),
            "rules-as-data-command-matrix-source",
        ));
    }
    validate_malformed_fixture(entry, root, findings);
}

fn validate_malformed_fixture(
    entry: &RulesAsDataEntry,
    root: &Path,
    findings: &mut Vec<RulesAsDataFinding>,
) {
    let Some((path, token)) = entry.malformed_fixture.split_once(':') else {
        findings.push(finding(
            RULES_AS_DATA_MANIFEST_PATH,
            &entry.id,
            "malformed_fixture must use `path:token`",
            "rules-as-data-malformed-fixture-shape",
        ));
        return;
    };
    if path.trim().is_empty() || token.trim().is_empty() {
        findings.push(finding(
            RULES_AS_DATA_MANIFEST_PATH,
            &entry.id,
            "malformed_fixture path and token must be non-empty",
            "rules-as-data-malformed-fixture-shape",
        ));
        return;
    }
    let fixture_text = match fs::read_to_string(root.join(path)) {
        Ok(text) => text,
        Err(error) => {
            findings.push(finding(
                path,
                &entry.id,
                format!("malformed fixture source is missing or unreadable: {error}"),
                "rules-as-data-malformed-fixture-readable",
            ));
            return;
        }
    };
    if !fixture_text.contains(token.trim()) {
        findings.push(finding(
            path,
            &entry.id,
            format!("malformed fixture token `{}` is missing", token.trim()),
            "rules-as-data-malformed-fixture-token",
        ));
    }
}

fn finding(
    path: impl Into<String>,
    key: impl Into<String>,
    text: impl Into<String>,
    policy: impl Into<String>,
) -> RulesAsDataFinding {
    RulesAsDataFinding {
        path: path.into(),
        key: key.into(),
        text: text.into(),
        policy: policy.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_rejects_missing_command_matrix_source() {
        let dir = tempfile::tempdir().expect("Fix: create rules-as-data fixture directory.");
        std::fs::create_dir_all(dir.path().join("docs/optimization"))
            .expect("Fix: create docs fixture directory.");
        std::fs::create_dir_all(dir.path().join("tests"))
            .expect("Fix: create tests fixture directory.");
        std::fs::write(
            dir.path().join("docs/optimization/RULES.toml"),
            "schema = \"fixture-schema\"",
        )
        .expect("Fix: write rules fixture.");
        std::fs::write(
            dir.path().join("tests/rules.rs"),
            "fn malformed_fixture_rejects_bad_rules() {}",
        )
        .expect("Fix: write malformed fixture source.");
        let manifest = r#"
schema_version = 1
contract = "vyre-rules-as-data-manifest:v1"

[[data]]
id = "fixture-rules"
path = "docs/optimization/RULES.toml"
owner_lane = "coordination"
schema_token = "fixture-schema"
command_matrix_source = true
malformed_fixture = "tests/rules.rs:malformed_fixture_rejects_bad_rules"
"#;
        let ownership = r#"
[lane.coordination]
purpose = "fixture"
layer = "fixture"
write = ["docs/optimization/**"]
"#;

        let findings = validate_rules_as_data_manifest_text(
            manifest,
            "| command | shared sources |\n| fixture | none |\n",
            ownership,
            dir.path(),
        );

        assert!(
            findings
                .iter()
                .any(|finding| finding.policy == "rules-as-data-command-matrix-source"),
            "Fix: command-matrix source coverage must be enforced; findings={findings:?}"
        );
    }
}
