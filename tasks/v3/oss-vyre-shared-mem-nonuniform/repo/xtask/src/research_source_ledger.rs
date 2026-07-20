use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use serde::Deserialize;

use crate::research_key::is_research_key;

pub(crate) const RESEARCH_SOURCE_LEDGER_PATH: &str =
    "docs/optimization/RESEARCH_SOURCE_LEDGER.toml";
pub(crate) const COMPETITOR_ISSUE_LEDGER_PATH: &str =
    "docs/optimization/COMPETITOR_ISSUE_LEDGER.toml";
const RESEARCH_SOURCE_LEDGER_TOML: &str =
    include_str!("../../docs/optimization/RESEARCH_SOURCE_LEDGER.toml");
const RESEARCH_SOURCE_LEDGER_SCHEMA_VERSION: u32 = 1;
const RESEARCH_SOURCE_LEDGER_ID: &str = "vyre-research-source-ledger";
const COMPETITOR_ISSUE_LEDGER_SCHEMA_VERSION: u32 = 1;
const COMPETITOR_ISSUE_LEDGER_ID: &str = "vyre-competitor-issue-ledger";

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct ResearchSourceLedger {
    schema: Option<ResearchSourceLedgerSchema>,
    pub(crate) sources: Option<Vec<ResearchSourceEntry>>,
}

#[derive(Debug, Clone, Deserialize)]
struct ResearchSourceLedgerSchema {
    version: Option<u32>,
    ledger: Option<String>,
    recorded_on: Option<String>,
    contract: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct ResearchSourceEntry {
    pub(crate) key: Option<String>,
    pub(crate) url: Option<String>,
    pub(crate) source_class: Option<String>,
    pub(crate) baseline_type: Option<String>,
    pub(crate) reproducibility_class: Option<String>,
    pub(crate) artifact_state: Option<String>,
    pub(crate) release_floor_eligible: Option<bool>,
    pub(crate) artifact_url: Option<String>,
    pub(crate) vx_rows: Option<Vec<String>>,
    pub(crate) digest_material: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct CompetitorIssueLedger {
    schema: Option<CompetitorIssueLedgerSchema>,
    pub(crate) issues: Option<Vec<CompetitorIssueEntry>>,
}

#[derive(Debug, Clone, Deserialize)]
struct CompetitorIssueLedgerSchema {
    version: Option<u32>,
    ledger: Option<String>,
    recorded_on: Option<String>,
    contract: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct CompetitorIssueEntry {
    pub(crate) id: Option<String>,
    pub(crate) source_key: Option<String>,
    pub(crate) url: Option<String>,
    pub(crate) issue_type: Option<String>,
    pub(crate) status: Option<String>,
    pub(crate) affected_version: Option<String>,
    pub(crate) labels: Option<Vec<String>>,
    pub(crate) local_fixture: Option<String>,
    pub(crate) vx_rows: Option<Vec<String>>,
    pub(crate) digest_material: Option<String>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) struct ResearchSourceUnknownVxRow {
    pub(crate) key: String,
    pub(crate) vx_row: String,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) struct CompetitorIssueUnknownVxRow {
    pub(crate) id: String,
    pub(crate) vx_row: String,
}

pub(crate) fn read_research_source_ledger(root: &Path) -> Result<ResearchSourceLedger, String> {
    let path = root.join(RESEARCH_SOURCE_LEDGER_PATH);
    let text = fs::read_to_string(&path)
        .map_err(|error| format!("failed to read {RESEARCH_SOURCE_LEDGER_PATH}: {error}"))?;
    parse_research_source_ledger_text(&text)
}

pub(crate) fn read_competitor_issue_ledger(root: &Path) -> Result<CompetitorIssueLedger, String> {
    let path = root.join(COMPETITOR_ISSUE_LEDGER_PATH);
    let text = fs::read_to_string(&path)
        .map_err(|error| format!("failed to read {COMPETITOR_ISSUE_LEDGER_PATH}: {error}"))?;
    parse_competitor_issue_ledger_text(&text)
}

pub(crate) fn embedded_research_source_keys() -> Result<BTreeSet<String>, String> {
    let ledger = parse_research_source_ledger_text(RESEARCH_SOURCE_LEDGER_TOML)?;
    Ok(research_source_keys(&ledger))
}

fn parse_research_source_ledger_text(text: &str) -> Result<ResearchSourceLedger, String> {
    let ledger = toml::from_str::<ResearchSourceLedger>(text)
        .map_err(|error| format!("failed to parse {RESEARCH_SOURCE_LEDGER_PATH}: {error}"))?;
    validate_research_source_ledger_schema(&ledger)?;
    validate_research_source_ledger_rows(&ledger)?;
    Ok(ledger)
}

fn parse_competitor_issue_ledger_text(text: &str) -> Result<CompetitorIssueLedger, String> {
    let ledger = toml::from_str::<CompetitorIssueLedger>(text)
        .map_err(|error| format!("failed to parse {COMPETITOR_ISSUE_LEDGER_PATH}: {error}"))?;
    validate_competitor_issue_ledger_schema(&ledger)?;
    validate_competitor_issue_ledger_rows(&ledger)?;
    Ok(ledger)
}

fn validate_research_source_ledger_schema(ledger: &ResearchSourceLedger) -> Result<(), String> {
    let Some(schema) = ledger.schema.as_ref() else {
        return Err(format!("{RESEARCH_SOURCE_LEDGER_PATH} is missing [schema]"));
    };
    if schema.version != Some(RESEARCH_SOURCE_LEDGER_SCHEMA_VERSION) {
        return Err(format!(
            "{RESEARCH_SOURCE_LEDGER_PATH} schema.version must be {RESEARCH_SOURCE_LEDGER_SCHEMA_VERSION}"
        ));
    }
    if schema.ledger.as_deref() != Some(RESEARCH_SOURCE_LEDGER_ID) {
        return Err(format!(
            "{RESEARCH_SOURCE_LEDGER_PATH} schema.ledger must be `{RESEARCH_SOURCE_LEDGER_ID}`"
        ));
    }
    let recorded_on = required_schema_field(schema.recorded_on.as_deref(), "recorded_on")?;
    if !date_yyyy_mm_dd(recorded_on) {
        return Err(format!(
            "{RESEARCH_SOURCE_LEDGER_PATH} schema.recorded_on `{recorded_on}` must use YYYY-MM-DD"
        ));
    }
    let contract = required_schema_field(schema.contract.as_deref(), "contract")?;
    if !contract.contains("source row")
        || !contract.contains("reproducibility")
        || !contract.contains("release-floor")
    {
        return Err(format!(
            "{RESEARCH_SOURCE_LEDGER_PATH} schema.contract must describe source row reproducibility and release-floor requirements"
        ));
    }
    Ok(())
}

fn validate_competitor_issue_ledger_schema(
    ledger: &CompetitorIssueLedger,
) -> Result<(), String> {
    let Some(schema) = ledger.schema.as_ref() else {
        return Err(format!("{COMPETITOR_ISSUE_LEDGER_PATH} is missing [schema]"));
    };
    if schema.version != Some(COMPETITOR_ISSUE_LEDGER_SCHEMA_VERSION) {
        return Err(format!(
            "{COMPETITOR_ISSUE_LEDGER_PATH} schema.version must be {COMPETITOR_ISSUE_LEDGER_SCHEMA_VERSION}"
        ));
    }
    if schema.ledger.as_deref() != Some(COMPETITOR_ISSUE_LEDGER_ID) {
        return Err(format!(
            "{COMPETITOR_ISSUE_LEDGER_PATH} schema.ledger must be `{COMPETITOR_ISSUE_LEDGER_ID}`"
        ));
    }
    let recorded_on = required_competitor_schema_field(schema.recorded_on.as_deref(), "recorded_on")?;
    if !date_yyyy_mm_dd(recorded_on) {
        return Err(format!(
            "{COMPETITOR_ISSUE_LEDGER_PATH} schema.recorded_on `{recorded_on}` must use YYYY-MM-DD"
        ));
    }
    let contract = required_competitor_schema_field(schema.contract.as_deref(), "contract")?;
    if !contract.contains("issue row")
        || !contract.contains("fixture")
        || !contract.contains("regression")
    {
        return Err(format!(
            "{COMPETITOR_ISSUE_LEDGER_PATH} schema.contract must describe issue row fixture and regression requirements"
        ));
    }
    Ok(())
}

fn validate_research_source_ledger_rows(ledger: &ResearchSourceLedger) -> Result<(), String> {
    let Some(sources) = ledger.sources.as_ref() else {
        return Err(format!(
            "{RESEARCH_SOURCE_LEDGER_PATH} has no [[sources]] entries"
        ));
    };
    if sources.is_empty() {
        return Err(format!(
            "{RESEARCH_SOURCE_LEDGER_PATH} has no [[sources]] entries"
        ));
    }
    let mut keys = BTreeSet::new();
    for (index, source) in sources.iter().enumerate() {
        let key = required_source_field(source.key.as_deref(), index, "key")?;
        if !is_research_key(key) {
            return Err(format!(
                "{RESEARCH_SOURCE_LEDGER_PATH} source[{index}].key `{key}` must use uppercase letters, digits, and underscores"
            ));
        }
        if !keys.insert(key.to_string()) {
            return Err(format!(
                "{RESEARCH_SOURCE_LEDGER_PATH} duplicates source key `{key}`"
            ));
        }
        let url = required_source_field(source.url.as_deref(), index, "url")?;
        let normalized_url = normalize_research_source_url(url);
        if !normalized_url.starts_with("https://") {
            return Err(format!(
                "{RESEARCH_SOURCE_LEDGER_PATH} source[{index}] key `{key}` must use a canonical https URL"
            ));
        }
        required_source_field(source.source_class.as_deref(), index, "source_class")?;
        required_source_field(source.baseline_type.as_deref(), index, "baseline_type")?;
        let reproducibility_class = required_source_field(
            source.reproducibility_class.as_deref(),
            index,
            "reproducibility_class",
        )?;
        if !is_research_vocabulary_tag(reproducibility_class) {
            return Err(format!(
                "{RESEARCH_SOURCE_LEDGER_PATH} source[{index}] key `{key}` reproducibility_class `{reproducibility_class}` must be a lowercase kebab-case vocabulary tag"
            ));
        }
        let artifact_state =
            required_source_field(source.artifact_state.as_deref(), index, "artifact_state")?;
        if !is_research_vocabulary_tag(artifact_state) {
            return Err(format!(
                "{RESEARCH_SOURCE_LEDGER_PATH} source[{index}] key `{key}` artifact_state `{artifact_state}` must be a lowercase kebab-case vocabulary tag"
            ));
        }
        let Some(release_floor_eligible) = source.release_floor_eligible else {
            return Err(format!(
                "{RESEARCH_SOURCE_LEDGER_PATH} source[{index}] key `{key}` is missing release_floor_eligible"
            ));
        };
        if release_floor_eligible && reproducibility_class == "preprint" {
            return Err(format!(
                "{RESEARCH_SOURCE_LEDGER_PATH} source[{index}] key `{key}` cannot mark preprint sources release_floor_eligible"
            ));
        }
        let artifact_url = required_source_field(source.artifact_url.as_deref(), index, "artifact_url")?;
        let normalized_artifact_url = normalize_research_source_url(artifact_url);
        if !normalized_artifact_url.starts_with("https://") {
            return Err(format!(
                "{RESEARCH_SOURCE_LEDGER_PATH} source[{index}] key `{key}` artifact_url must use a canonical https URL"
            ));
        }
        let Some(vx_rows) = source.vx_rows.as_ref().filter(|rows| !rows.is_empty()) else {
            return Err(format!(
                "{RESEARCH_SOURCE_LEDGER_PATH} source[{index}] key `{key}` is missing vx_rows"
            ));
        };
        let mut source_vx_rows = BTreeSet::new();
        for vx_row in vx_rows {
            let vx_row = vx_row.trim();
            if vx_row.is_empty() {
                return Err(format!(
                    "{RESEARCH_SOURCE_LEDGER_PATH} source[{index}] key `{key}` has an empty vx_rows entry"
                ));
            }
            let Some(digits) = vx_row.strip_prefix("VX-") else {
                return Err(format!(
                    "{RESEARCH_SOURCE_LEDGER_PATH} source[{index}] key `{key}` vx_rows entry `{vx_row}` must use VX-###"
                ));
            };
            if digits.len() != 3 || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
                return Err(format!(
                    "{RESEARCH_SOURCE_LEDGER_PATH} source[{index}] key `{key}` vx_rows entry `{vx_row}` must use VX-###"
                ));
            }
            if !source_vx_rows.insert(vx_row.to_string()) {
                return Err(format!(
                    "{RESEARCH_SOURCE_LEDGER_PATH} source[{index}] key `{key}` duplicates vx_rows entry `{vx_row}`"
                ));
            }
        }
        let digest_material = required_source_field(
            source.digest_material.as_deref(),
            index,
            "digest_material",
        )?;
        if !digest_material.contains(key)
            || (!digest_material.contains(url) && !digest_material.contains(&normalized_url))
            || !digest_material.contains(reproducibility_class)
            || !digest_material.contains(artifact_state)
            || !digest_material.contains(&format!(
                "release_floor_eligible={release_floor_eligible}"
            ))
            || (!digest_material.contains(artifact_url)
                && !digest_material.contains(&normalized_artifact_url))
        {
            return Err(format!(
                "{RESEARCH_SOURCE_LEDGER_PATH} source[{index}] key `{key}` digest_material must include its key, URL, reproducibility class, artifact state, release-floor eligibility, and artifact URL"
            ));
        }
    }
    Ok(())
}

fn validate_competitor_issue_ledger_rows(ledger: &CompetitorIssueLedger) -> Result<(), String> {
    let Some(issues) = ledger.issues.as_ref() else {
        return Err(format!(
            "{COMPETITOR_ISSUE_LEDGER_PATH} has no [[issues]] entries"
        ));
    };
    if issues.is_empty() {
        return Err(format!(
            "{COMPETITOR_ISSUE_LEDGER_PATH} has no [[issues]] entries"
        ));
    }
    let mut ids = BTreeSet::new();
    for (index, issue) in issues.iter().enumerate() {
        let id = required_competitor_issue_field(issue.id.as_deref(), index, "id")?;
        if !id
            .bytes()
            .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit() || byte == b'-')
        {
            return Err(format!(
                "{COMPETITOR_ISSUE_LEDGER_PATH} issue[{index}].id `{id}` must use uppercase letters, digits, and dashes"
            ));
        }
        if !ids.insert(id.to_string()) {
            return Err(format!(
                "{COMPETITOR_ISSUE_LEDGER_PATH} duplicates issue id `{id}`"
            ));
        }
        let source_key =
            required_competitor_issue_field(issue.source_key.as_deref(), index, "source_key")?;
        if !is_research_key(source_key) {
            return Err(format!(
                "{COMPETITOR_ISSUE_LEDGER_PATH} issue[{index}] id `{id}` has malformed source_key `{source_key}`"
            ));
        }
        let url = required_competitor_issue_field(issue.url.as_deref(), index, "url")?;
        let normalized_url = normalize_research_source_url(url);
        if !normalized_url.starts_with("https://") {
            return Err(format!(
                "{COMPETITOR_ISSUE_LEDGER_PATH} issue[{index}] id `{id}` must use a canonical https URL"
            ));
        }
        let issue_type =
            required_competitor_issue_field(issue.issue_type.as_deref(), index, "issue_type")?;
        if !allowed_competitor_issue_type(issue_type) {
            return Err(format!(
                "{COMPETITOR_ISSUE_LEDGER_PATH} issue[{index}] id `{id}` has unsupported issue_type `{issue_type}`"
            ));
        }
        let status = required_competitor_issue_field(issue.status.as_deref(), index, "status")?;
        if !allowed_competitor_issue_status(status) {
            return Err(format!(
                "{COMPETITOR_ISSUE_LEDGER_PATH} issue[{index}] id `{id}` has unsupported status `{status}`"
            ));
        }
        let affected_version = required_competitor_issue_field(
            issue.affected_version.as_deref(),
            index,
            "affected_version",
        )?;
        let Some(labels) = issue.labels.as_ref().filter(|labels| !labels.is_empty()) else {
            return Err(format!(
                "{COMPETITOR_ISSUE_LEDGER_PATH} issue[{index}] id `{id}` is missing labels"
            ));
        };
        if labels.iter().any(|label| label.trim().is_empty()) {
            return Err(format!(
                "{COMPETITOR_ISSUE_LEDGER_PATH} issue[{index}] id `{id}` has an empty label"
            ));
        }
        let local_fixture = required_competitor_issue_field(
            issue.local_fixture.as_deref(),
            index,
            "local_fixture",
        )?;
        if !local_fixture
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        {
            return Err(format!(
                "{COMPETITOR_ISSUE_LEDGER_PATH} issue[{index}] id `{id}` local_fixture `{local_fixture}` must use lowercase letters, digits, and dashes"
            ));
        }
        let Some(vx_rows) = issue.vx_rows.as_ref().filter(|rows| !rows.is_empty()) else {
            return Err(format!(
                "{COMPETITOR_ISSUE_LEDGER_PATH} issue[{index}] id `{id}` is missing vx_rows"
            ));
        };
        let mut issue_vx_rows = BTreeSet::new();
        for vx_row in vx_rows {
            validate_vx_row_id(COMPETITOR_ISSUE_LEDGER_PATH, index, id, vx_row)?;
            if !issue_vx_rows.insert(vx_row.clone()) {
                return Err(format!(
                    "{COMPETITOR_ISSUE_LEDGER_PATH} issue[{index}] id `{id}` duplicates vx_rows entry `{vx_row}`"
                ));
            }
        }
        let digest_material = required_competitor_issue_field(
            issue.digest_material.as_deref(),
            index,
            "digest_material",
        )?;
        if !digest_material.contains(id)
            || !digest_material.contains(source_key)
            || (!digest_material.contains(url) && !digest_material.contains(&normalized_url))
            || !digest_material.contains(issue_type)
            || !digest_material.contains(status)
            || !digest_material.contains(affected_version)
            || !digest_material.contains(local_fixture)
        {
            return Err(format!(
                "{COMPETITOR_ISSUE_LEDGER_PATH} issue[{index}] id `{id}` digest_material must include id, source key, URL, issue type, status, affected version, and local fixture"
            ));
        }
        for label in labels {
            if !digest_material.contains(label) {
                return Err(format!(
                    "{COMPETITOR_ISSUE_LEDGER_PATH} issue[{index}] id `{id}` digest_material must include label `{label}`"
                ));
            }
        }
    }
    Ok(())
}

/// Validate a research-ledger taxonomy tag by SHAPE rather than against a
/// hardcoded value set.
///
/// `reproducibility_class` and `artifact_state` are an OPEN, domain-evolving
/// vocabulary: the Tier-B `RESEARCH_SOURCE_LEDGER.toml` legitimately carries 50+
/// distinct `reproducibility_class` values (`official-spec`, `rfc`,
/// `w3c-recommendation`, `oasis-standard`, `nist-final-publication`, …) and ~17
/// `artifact_state` values (`living-specification`, `working-draft`,
/// `candidate-recommendation`, …). A closed `match` arm silently drifts from that
/// data and rejects valid rows — the exact failure that aborted the CUDA frontier
/// leaderboard load. These fields are therefore validated structurally: non-empty,
/// lowercase ASCII kebab-case (letters, digits, single internal hyphens), bounded
/// to 64 bytes. Typo/intent defense is the `digest_material` integrity guard, which
/// requires the exact tag to appear in the row's signed digest string; the one
/// value with load-bearing semantics (`preprint`) is enforced separately.
fn is_research_vocabulary_tag(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        && !value.starts_with('-')
        && !value.ends_with('-')
        && !value.contains("--")
}

fn allowed_competitor_issue_type(value: &str) -> bool {
    matches!(
        value,
        "benchmark-telemetry"
            | "gpu-memory-pressure"
            | "performance-cliff"
            | "portability-correctness"
            | "recall-regression"
            | "unsupported-construct"
    )
}

fn allowed_competitor_issue_status(value: &str) -> bool {
    matches!(value, "closed" | "documentation" | "open" | "reproduced")
}


fn required_source_field<'a>(
    raw: Option<&'a str>,
    index: usize,
    field: &str,
) -> Result<&'a str, String> {
    raw.map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            format!("{RESEARCH_SOURCE_LEDGER_PATH} source[{index}] is missing {field}")
        })
}

fn required_schema_field<'a>(raw: Option<&'a str>, field: &str) -> Result<&'a str, String> {
    raw.map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| format!("{RESEARCH_SOURCE_LEDGER_PATH} schema is missing {field}"))
}

fn required_competitor_schema_field<'a>(
    raw: Option<&'a str>,
    field: &str,
) -> Result<&'a str, String> {
    raw.map(str::trim).filter(|value| !value.is_empty()).ok_or_else(|| {
        format!("{COMPETITOR_ISSUE_LEDGER_PATH} schema is missing {field}")
    })
}

fn required_competitor_issue_field<'a>(
    raw: Option<&'a str>,
    index: usize,
    field: &str,
) -> Result<&'a str, String> {
    raw.map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            format!("{COMPETITOR_ISSUE_LEDGER_PATH} issue[{index}] is missing {field}")
        })
}

fn validate_vx_row_id(
    path: &str,
    index: usize,
    owner: &str,
    vx_row: &str,
) -> Result<(), String> {
    let vx_row = vx_row.trim();
    if vx_row.is_empty() {
        return Err(format!(
            "{path} issue[{index}] id `{owner}` has an empty vx_rows entry"
        ));
    }
    let Some(digits) = vx_row.strip_prefix("VX-") else {
        return Err(format!(
            "{path} issue[{index}] id `{owner}` vx_rows entry `{vx_row}` must use VX-###"
        ));
    };
    if digits.len() != 3 || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(format!(
            "{path} issue[{index}] id `{owner}` vx_rows entry `{vx_row}` must use VX-###"
        ));
    }
    Ok(())
}

fn date_yyyy_mm_dd(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() == 10
        && bytes[4] == b'-'
        && bytes[7] == b'-'
        && bytes
            .iter()
            .enumerate()
            .all(|(index, byte)| index == 4 || index == 7 || byte.is_ascii_digit())
}

pub(crate) fn research_source_urls_by_key(
    ledger: &ResearchSourceLedger,
) -> BTreeMap<String, String> {
    ledger
        .sources
        .as_deref()
        .unwrap_or_default()
        .iter()
        .filter_map(|source| {
            let key = source.key.as_deref()?;
            let url = source.url.as_deref()?;
            Some((key.to_string(), normalize_research_source_url(url)))
        })
        .collect()
}

pub(crate) fn competitor_issue_source_keys(
    ledger: &CompetitorIssueLedger,
) -> BTreeSet<String> {
    ledger
        .issues
        .as_deref()
        .unwrap_or_default()
        .iter()
        .filter_map(|issue| issue.source_key.clone())
        .collect()
}

fn research_source_keys(ledger: &ResearchSourceLedger) -> BTreeSet<String> {
    ledger
        .sources
        .as_deref()
        .unwrap_or_default()
        .iter()
        .filter_map(|source| source.key.clone())
        .collect()
}

pub(crate) fn unknown_research_source_vx_rows(
    ledger: &ResearchSourceLedger,
    known_vx_rows: &BTreeSet<String>,
) -> Vec<ResearchSourceUnknownVxRow> {
    ledger
        .sources
        .as_deref()
        .unwrap_or_default()
        .iter()
        .flat_map(|source| {
            let key = source.key.clone().unwrap_or_default();
            source
                .vx_rows
                .as_deref()
                .unwrap_or_default()
                .iter()
                .filter_map(move |vx_row| {
                    if known_vx_rows.contains(vx_row) {
                        None
                    } else {
                        Some(ResearchSourceUnknownVxRow {
                            key: key.clone(),
                            vx_row: vx_row.clone(),
                        })
                    }
                })
        })
        .collect()
}

pub(crate) fn unknown_competitor_issue_vx_rows(
    ledger: &CompetitorIssueLedger,
    known_vx_rows: &BTreeSet<String>,
) -> Vec<CompetitorIssueUnknownVxRow> {
    ledger
        .issues
        .as_deref()
        .unwrap_or_default()
        .iter()
        .flat_map(|issue| {
            let id = issue.id.clone().unwrap_or_default();
            issue
                .vx_rows
                .as_deref()
                .unwrap_or_default()
                .iter()
                .filter_map(move |vx_row| {
                    if known_vx_rows.contains(vx_row) {
                        None
                    } else {
                        Some(CompetitorIssueUnknownVxRow {
                            id: id.clone(),
                            vx_row: vx_row.clone(),
                        })
                    }
                })
        })
        .collect()
}

pub(crate) fn normalize_research_source_url(raw: &str) -> String {
    raw.trim()
        .trim_start_matches('<')
        .trim_end_matches('>')
        .trim_end_matches('/')
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ledger_rejects_wrong_schema_version() {
        let error = parse_research_source_ledger_text(
            r#"
[schema]
version = 99
ledger = "vyre-research-source-ledger"

[[sources]]
key = "MLIR_PASS"
url = "https://mlir.llvm.org/docs/Passes/"
"#,
        )
        .expect_err("Fix: research source ledger parser must reject wrong schema version.");

        assert!(error.contains("schema.version must be 1"));
    }

    #[test]
    fn ledger_rejects_missing_required_source_fields() {
        let error = parse_research_source_ledger_text(
            r#"
[schema]
version = 1
ledger = "vyre-research-source-ledger"
recorded_on = "2026-06-10"
contract = "Each source row records source row reproducibility and release-floor requirements."

[[sources]]
key = "MLIR_PASS"
url = "https://mlir.llvm.org/docs/Passes/"
"#,
        )
        .expect_err("Fix: research source ledger parser must reject incomplete source rows.");

        assert!(error.contains("source_class"));
    }

    #[test]
    fn ledger_reports_unknown_vx_row_references() {
        let ledger = parse_research_source_ledger_text(
            r#"
[schema]
version = 1
ledger = "vyre-research-source-ledger"
recorded_on = "2026-06-10"
contract = "Each source row records source row reproducibility and release-floor requirements."

[[sources]]
key = "MLIR_PASS"
url = "https://mlir.llvm.org/docs/Passes/"
source_class = "compiler-official-documentation"
baseline_type = "compiler-pass-infrastructure"
reproducibility_class = "official-doc"
artifact_state = "documentation-only"
release_floor_eligible = true
artifact_url = "https://mlir.llvm.org/docs/Passes/"
vx_rows = ["VX-999"]
digest_material = "MLIR_PASS|https://mlir.llvm.org/docs/Passes/|compiler-official-documentation|compiler-pass-infrastructure|official-doc|documentation-only|release_floor_eligible=true|https://mlir.llvm.org/docs/Passes/"
"#,
        )
        .expect("Fix: complete research source ledger fixture must parse.");

        let findings = unknown_research_source_vx_rows(&ledger, &BTreeSet::new());

        assert_eq!(
            findings,
            vec![ResearchSourceUnknownVxRow {
                key: "MLIR_PASS".to_string(),
                vx_row: "VX-999".to_string()
            }]
        );
    }

    #[test]
    fn ledger_rejects_malformed_vx_row_references() {
        let error = parse_research_source_ledger_text(
            r#"
[schema]
version = 1
ledger = "vyre-research-source-ledger"
recorded_on = "2026-06-10"
contract = "Each source row records source row reproducibility and release-floor requirements."

[[sources]]
key = "MLIR_PASS"
url = "https://mlir.llvm.org/docs/Passes/"
source_class = "compiler-official-documentation"
baseline_type = "compiler-pass-infrastructure"
reproducibility_class = "official-doc"
artifact_state = "documentation-only"
release_floor_eligible = true
artifact_url = "https://mlir.llvm.org/docs/Passes/"
vx_rows = ["VX-1"]
digest_material = "MLIR_PASS|https://mlir.llvm.org/docs/Passes/|compiler-official-documentation|compiler-pass-infrastructure|official-doc|documentation-only|release_floor_eligible=true|https://mlir.llvm.org/docs/Passes/"
"#,
        )
        .expect_err("Fix: research source ledger parser must reject malformed VX row ids.");

        assert!(error.contains("must use VX-###"));
    }

    #[test]
    fn ledger_rejects_missing_schema_recorded_on() {
        let error = parse_research_source_ledger_text(
            r#"
[schema]
version = 1
ledger = "vyre-research-source-ledger"
contract = "Each source row records source row reproducibility and release-floor requirements."

[[sources]]
key = "MLIR_PASS"
url = "https://mlir.llvm.org/docs/Passes/"
source_class = "compiler-official-documentation"
baseline_type = "compiler-pass-infrastructure"
reproducibility_class = "official-doc"
artifact_state = "documentation-only"
release_floor_eligible = true
artifact_url = "https://mlir.llvm.org/docs/Passes/"
vx_rows = ["VX-001"]
digest_material = "MLIR_PASS|https://mlir.llvm.org/docs/Passes/|compiler-official-documentation|compiler-pass-infrastructure|official-doc|documentation-only|release_floor_eligible=true|https://mlir.llvm.org/docs/Passes/"
"#,
        )
        .expect_err("Fix: research source ledger parser must reject missing recorded_on.");

        assert!(error.contains("schema is missing recorded_on"));
    }

    #[test]
    fn ledger_rejects_missing_reproducibility_metadata() {
        let error = parse_research_source_ledger_text(
            r#"
[schema]
version = 1
ledger = "vyre-research-source-ledger"
recorded_on = "2026-06-10"
contract = "Each source row records source row reproducibility and release-floor requirements."

[[sources]]
key = "MLIR_PASS"
url = "https://mlir.llvm.org/docs/Passes/"
source_class = "compiler-official-documentation"
baseline_type = "compiler-pass-infrastructure"
vx_rows = ["VX-001"]
digest_material = "MLIR_PASS|https://mlir.llvm.org/docs/Passes/|compiler-official-documentation|compiler-pass-infrastructure"
"#,
        )
        .expect_err("Fix: research source ledger parser must reject missing reproducibility metadata.");

        assert!(error.contains("reproducibility_class"));
    }

    #[test]
    fn ledger_rejects_preprint_release_floor_eligibility() {
        let error = parse_research_source_ledger_text(
            r#"
[schema]
version = 1
ledger = "vyre-research-source-ledger"
recorded_on = "2026-06-10"
contract = "Each source row records source row reproducibility and release-floor requirements."

[[sources]]
key = "VECTOR_MATON"
url = "https://arxiv.org/html/2603.01525v1"
source_class = "research-preprint"
baseline_type = "joint-pattern-constrained-vector-search"
reproducibility_class = "preprint"
artifact_state = "preprint-observation-only"
release_floor_eligible = true
artifact_url = "https://arxiv.org/html/2603.01525v1"
vx_rows = ["VX-001"]
digest_material = "VECTOR_MATON|https://arxiv.org/html/2603.01525v1|research-preprint|joint-pattern-constrained-vector-search|preprint|preprint-observation-only|release_floor_eligible=true|https://arxiv.org/html/2603.01525v1"
"#,
        )
        .expect_err("Fix: research source ledger parser must reject preprint release floors.");

        assert!(error.contains("preprint sources release_floor_eligible"));
    }

    #[test]
    fn research_vocabulary_tag_accepts_open_kebab_case_and_rejects_garbage() {
        // Open vocabulary the live ledger actually uses (old closed enums rejected these).
        for accepted in [
            "official-spec",
            "living-specification",
            "w3c-recommendation",
            "nist-final-publication",
            "rfc",
            "official-government-statistics-handbook",
        ] {
            assert!(
                is_research_vocabulary_tag(accepted),
                "Fix: `{accepted}` is a legitimate open-vocabulary tag and must be accepted"
            );
        }
        // Structural garbage stays rejected (shape, not enum membership).
        for rejected in [
            "",
            "Official_Spec",
            "official spec",
            "official-spec!",
            "-official-spec",
            "official-spec-",
            "official--spec",
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", // 67 bytes, exceeds 64 bound
        ] {
            assert!(
                !is_research_vocabulary_tag(rejected),
                "Fix: `{rejected}` is malformed and must be rejected"
            );
        }
    }

    #[test]
    fn ledger_accepts_open_vocabulary_reproducibility_and_artifact_state() {
        // `official-spec` + `living-specification` were both rejected by the old
        // hardcoded enums; the real ledger depends on them loading.
        let ledger = parse_research_source_ledger_text(
            r#"
[schema]
version = 1
ledger = "vyre-research-source-ledger"
recorded_on = "2026-06-10"
contract = "Each source row records source row reproducibility and release-floor requirements."

[[sources]]
key = "SLSA_PROVENANCE"
url = "https://slsa.dev/spec/v1.2/"
source_class = "official-supply-chain-specification"
baseline_type = "artifact-provenance-and-build-integrity"
reproducibility_class = "official-spec"
artifact_state = "living-specification"
release_floor_eligible = true
artifact_url = "https://slsa.dev/spec/v1.2/"
vx_rows = ["VX-701"]
digest_material = "SLSA_PROVENANCE|https://slsa.dev/spec/v1.2/|official-supply-chain-specification|artifact-provenance-and-build-integrity|official-spec|living-specification|release_floor_eligible=true|https://slsa.dev/spec/v1.2/"
"#,
        )
        .expect("Fix: open-vocabulary kebab-case taxonomy tags must load");

        let sources = ledger
            .sources
            .as_ref()
            .expect("Fix: parsed ledger must expose sources");
        assert_eq!(sources.len(), 1);
        assert_eq!(
            sources[0].reproducibility_class.as_deref(),
            Some("official-spec")
        );
        assert_eq!(
            sources[0].artifact_state.as_deref(),
            Some("living-specification")
        );
    }

    #[test]
    fn ledger_rejects_malformed_reproducibility_class() {
        let error = parse_research_source_ledger_text(
            r#"
[schema]
version = 1
ledger = "vyre-research-source-ledger"
recorded_on = "2026-06-10"
contract = "Each source row records source row reproducibility and release-floor requirements."

[[sources]]
key = "MLIR_PASS"
url = "https://mlir.llvm.org/docs/Passes/"
source_class = "compiler-official-documentation"
baseline_type = "compiler-pass-infrastructure"
reproducibility_class = "Official_Spec"
artifact_state = "documentation-only"
release_floor_eligible = true
artifact_url = "https://mlir.llvm.org/docs/Passes/"
vx_rows = ["VX-001"]
digest_material = "MLIR_PASS|https://mlir.llvm.org/docs/Passes/|compiler-official-documentation|compiler-pass-infrastructure|Official_Spec|documentation-only|release_floor_eligible=true|https://mlir.llvm.org/docs/Passes/"
"#,
        )
        .expect_err("Fix: research source ledger parser must reject malformed reproducibility_class.");

        assert!(error.contains("reproducibility_class"));
        assert!(error.contains("lowercase kebab-case"));
    }

    #[test]
    fn ledger_rejects_duplicate_vx_row_references_per_source() {
        let error = parse_research_source_ledger_text(
            r#"
[schema]
version = 1
ledger = "vyre-research-source-ledger"
recorded_on = "2026-06-10"
contract = "Each source row records source row reproducibility and release-floor requirements."

[[sources]]
key = "MLIR_PASS"
url = "https://mlir.llvm.org/docs/Passes/"
source_class = "compiler-official-documentation"
baseline_type = "compiler-pass-infrastructure"
reproducibility_class = "official-doc"
artifact_state = "documentation-only"
release_floor_eligible = true
artifact_url = "https://mlir.llvm.org/docs/Passes/"
vx_rows = ["VX-001", "VX-001"]
digest_material = "MLIR_PASS|https://mlir.llvm.org/docs/Passes/|compiler-official-documentation|compiler-pass-infrastructure|official-doc|documentation-only|release_floor_eligible=true|https://mlir.llvm.org/docs/Passes/"
"#,
        )
        .expect_err("Fix: research source ledger parser must reject duplicate VX row ids.");

        assert!(error.contains("duplicates vx_rows entry"));
    }

    #[test]
    fn competitor_issue_ledger_rejects_missing_fixture_mapping() {
        let error = parse_competitor_issue_ledger_text(
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
vx_rows = ["VX-451"]
digest_material = "HYPERSCAN-ISSUE-68|HYPERSCAN|https://github.com/intel/hyperscan/issues/68|performance-cliff|closed|4.5.2|regex-performance-cliff"
"#,
        )
        .expect_err("Fix: competitor issue ledger must reject rows without fixture ids.");

        assert!(error.contains("local_fixture"));
    }

    #[test]
    fn competitor_issue_ledger_reports_unknown_vx_rows() {
        let ledger = parse_competitor_issue_ledger_text(
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
        .expect("Fix: complete competitor issue ledger fixture must parse.");

        let findings = unknown_competitor_issue_vx_rows(&ledger, &BTreeSet::new());

        assert_eq!(
            findings,
            vec![CompetitorIssueUnknownVxRow {
                id: "HYPERSCAN-ISSUE-68".to_string(),
                vx_row: "VX-999".to_string()
            }]
        );
    }
}
