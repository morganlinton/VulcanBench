//! Shared duplicate-family report schema for dedup gates.

use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::hash::sha256_hex;

pub(crate) use crate::artifact_paths::{
    LEGO_AUDIT_DUPLICATES_ARTIFACT, REGISTERED_OP_DUPLICATES_ARTIFACT,
    SOURCE_SIMILAR_DUPLICATES_ARTIFACT,
};

pub(crate) const DUPLICATE_FAMILY_SCHEMA_VERSION: u32 = 2;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub(crate) struct DuplicateFamilyReport {
    pub(crate) schema_version: u32,
    pub(crate) generator_command: String,
    pub(crate) detector_family: String,
    pub(crate) family_count: usize,
    pub(crate) families: Vec<DuplicateFamilyFinding>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub(crate) struct DuplicateFamilyFinding {
    pub(crate) family_id: String,
    pub(crate) detector: String,
    pub(crate) severity: &'static str,
    pub(crate) score: f64,
    pub(crate) left: DuplicateSubject,
    pub(crate) right: DuplicateSubject,
    pub(crate) import_owner: String,
    pub(crate) import_target: String,
    pub(crate) evidence: DuplicateEvidence,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub(crate) struct DuplicateSubject {
    pub(crate) id: String,
    pub(crate) owner_lane: String,
    pub(crate) fingerprint: Option<String>,
    pub(crate) tokens: Option<usize>,
    pub(crate) bytes: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub(crate) struct DuplicateEvidence {
    pub(crate) similarity_metric: &'static str,
    pub(crate) left_metric: String,
    pub(crate) right_metric: String,
    pub(crate) dedup_action: &'static str,
}

pub(crate) fn duplicate_family_report(
    generator_command: &str,
    detector_family: &str,
    families: Vec<DuplicateFamilyFinding>,
) -> DuplicateFamilyReport {
    let families = deduplicate_families(families);
    DuplicateFamilyReport {
        schema_version: DUPLICATE_FAMILY_SCHEMA_VERSION,
        generator_command: generator_command.to_string(),
        detector_family: detector_family.to_string(),
        family_count: families.len(),
        families,
    }
}

fn deduplicate_families(families: Vec<DuplicateFamilyFinding>) -> Vec<DuplicateFamilyFinding> {
    let mut by_family_id = BTreeMap::<String, DuplicateFamilyFinding>::new();
    for finding in families {
        match by_family_id.get_mut(&finding.family_id) {
            Some(existing) => merge_duplicate_family(existing, finding),
            None => {
                by_family_id.insert(finding.family_id.clone(), finding);
            }
        }
    }
    by_family_id.into_values().collect()
}

fn merge_duplicate_family(existing: &mut DuplicateFamilyFinding, incoming: DuplicateFamilyFinding) {
    let detector = merged_detector_label(&existing.detector, &incoming.detector);
    if incoming.score > existing.score {
        *existing = incoming;
    }
    existing.detector = detector;
    existing.severity = duplicate_severity(existing.score);
}

fn merged_detector_label(left: &str, right: &str) -> String {
    if left == right {
        return left.to_string();
    }
    let mut detectors = left.split('+').chain(right.split('+')).collect::<Vec<_>>();
    detectors.sort_unstable();
    detectors.dedup();
    detectors.join("+")
}

pub(crate) fn write_duplicate_report_json(
    path: &Path,
    report: &DuplicateFamilyReport,
) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("create parent directory `{}`: {error}", parent.display()))?;
    }
    let json = serde_json::to_string_pretty(report)
        .map_err(|error| format!("serialize duplicate family report: {error}"))?;
    fs::write(path, format!("{json}\n"))
        .map_err(|error| format!("write duplicate family report: {error}"))
}

pub(crate) fn duplicate_report_json_path(
    flag: &str,
    raw: Option<&str>,
    missing_message: &str,
) -> Result<PathBuf, String> {
    let Some(path) = raw else {
        return Err(missing_message.to_string());
    };
    if path.starts_with("--") {
        return Err(format!("{flag} requires a path, not another flag"));
    }
    Ok(PathBuf::from(path))
}

pub(crate) fn duplicate_report_generator_command(prefix: &str, output_path: &Path) -> String {
    format!(
        "xtask {prefix} --duplicate-report-json {}",
        output_path.display()
    )
}

pub(crate) fn validate_duplicate_family_report_artifact(
    bytes: &[u8],
    expected_generator_command: &str,
) -> Vec<String> {
    let mut blockers = Vec::new();
    let value = match serde_json::from_slice::<serde_json::Value>(bytes) {
        Ok(value) => value,
        Err(error) => {
            return vec![format!("duplicate family artifact is not valid JSON: {error}")];
        }
    };
    if value.get("schema_version").and_then(|raw| raw.as_u64())
        != Some(u64::from(DUPLICATE_FAMILY_SCHEMA_VERSION))
    {
        blockers.push(format!(
            "duplicate family artifact must use schema_version={DUPLICATE_FAMILY_SCHEMA_VERSION}"
        ));
    }
    if value
        .get("generator_command")
        .and_then(|raw| raw.as_str())
        != Some(expected_generator_command)
    {
        blockers.push(
            "duplicate family artifact generator_command must match the release evidence command"
                .to_string(),
        );
    }
    let Some(families) = value.get("families").and_then(|raw| raw.as_array()) else {
        blockers.push("duplicate family artifact must contain a families array".to_string());
        return blockers;
    };
    if value.get("family_count").and_then(|raw| raw.as_u64()) != Some(families.len() as u64) {
        blockers
            .push("duplicate family artifact family_count must match families length".to_string());
    }
    for (index, family) in families.iter().enumerate() {
        let family_id = family
            .get("family_id")
            .and_then(|raw| raw.as_str())
            .unwrap_or_default();
        if !family_id.starts_with("duplicate-family:v1:") {
            blockers.push(format!(
                "duplicate family artifact family[{index}].family_id is missing or unstable"
            ));
        }
        if family
            .get("detector")
            .and_then(|raw| raw.as_str())
            .unwrap_or_default()
            .is_empty()
        {
            blockers.push(format!(
                "duplicate family artifact family[{index}].detector is missing"
            ));
        }
        for side in ["left", "right"] {
            let fingerprint = family
                .get(side)
                .and_then(|subject| subject.get("fingerprint"))
                .and_then(|raw| raw.as_str())
                .unwrap_or_default();
            if fingerprint.is_empty() {
                blockers.push(format!(
                    "duplicate family artifact family[{index}].{side}.fingerprint is missing"
                ));
            } else if !duplicate_subject_fingerprint_is_supported(fingerprint) {
                blockers.push(format!(
                    "duplicate family artifact family[{index}].{side}.fingerprint uses an unsupported namespace"
                ));
            }
        }
    }
    blockers
}

fn duplicate_subject_fingerprint_is_supported(fingerprint: &str) -> bool {
    fingerprint.starts_with("source-token-fingerprint:v1:")
        || fingerprint.starts_with("registered-op-ir-fingerprint:v1:")
}

pub(crate) fn duplicate_family_id(detector: &str, left: &str, right: &str) -> String {
    let (first, second) = if left <= right {
        (left, right)
    } else {
        (right, left)
    };
    let material = format!("duplicate-family:v1\ndetector={detector}\nleft={first}\nright={second}\n");
    format!("duplicate-family:v1:{}", sha256_hex(material.as_bytes()))
}

pub(crate) fn registered_op_duplicate_family_id(left: &str, right: &str) -> String {
    duplicate_family_id("registered-op", left, right)
}

pub(crate) fn source_duplicate_family_id(left: &str, right: &str) -> String {
    duplicate_family_id("source-similar", left, right)
}

pub(crate) fn source_duplicate_subject(
    path: &str,
    owner_lane: &str,
    fingerprint: &str,
    tokens: usize,
    bytes: u64,
) -> DuplicateSubject {
    DuplicateSubject {
        id: path.to_string(),
        owner_lane: owner_lane.to_string(),
        fingerprint: Some(fingerprint.to_string()),
        tokens: Some(tokens),
        bytes: Some(bytes),
    }
}

pub(crate) fn source_token_fingerprint(tokens: &[String]) -> String {
    let mut material = String::from("source-token-fingerprint:v1\n");
    for token in tokens {
        material.push_str(token);
        material.push('\n');
    }
    format!(
        "source-token-fingerprint:v1:{}",
        sha256_hex(material.as_bytes())
    )
}

pub(crate) fn registered_op_duplicate_subject(
    op_id: &str,
    fingerprint: &[u8],
    node_count: usize,
) -> DuplicateSubject {
    DuplicateSubject {
        id: op_id.to_string(),
        owner_lane: registered_op_owner_lane(op_id).to_string(),
        fingerprint: Some(registered_op_fingerprint(fingerprint)),
        tokens: Some(node_count),
        bytes: Some(fingerprint.len() as u64),
    }
}

fn registered_op_fingerprint(fingerprint: &[u8]) -> String {
    format!(
        "registered-op-ir-fingerprint:v1:{}",
        sha256_hex(fingerprint)
    )
}

pub(crate) fn duplicate_severity(score: f64) -> &'static str {
    if score >= 0.95 {
        "duplicate"
    } else if score >= 0.86 {
        "very_similar"
    } else if score >= 0.50 {
        "similar"
    } else {
        "related"
    }
}

/// Structural similarity for registered-op IR fingerprints.
///
/// The metric compares byte-bigram frequency vectors with cosine similarity,
/// so adjacent node-kind order matters instead of only set membership.
pub(crate) fn structural_similarity(a: &[u8], b: &[u8]) -> f64 {
    if a.len() < 4 || b.len() < 4 {
        return 0.0;
    }
    let a_bigrams = bigram_counts(a);
    let b_bigrams = bigram_counts(b);
    let mut dot = 0i64;
    let mut a_norm = 0i64;
    let mut b_norm = 0i64;
    for (bg, &ac) in &a_bigrams {
        let bc = b_bigrams.get(bg).copied().unwrap_or(0);
        dot += (ac as i64) * (bc as i64);
        a_norm += (ac as i64).pow(2);
    }
    for &bc in b_bigrams.values() {
        b_norm += (bc as i64).pow(2);
    }
    if a_norm == 0 || b_norm == 0 {
        return 0.0;
    }
    dot as f64 / ((a_norm as f64).sqrt() * (b_norm as f64).sqrt())
}

fn bigram_counts(bytes: &[u8]) -> HashMap<(u8, u8), u32> {
    let mut out: HashMap<(u8, u8), u32> = HashMap::new();
    for window in bytes.windows(2) {
        *out.entry((window[0], window[1])).or_insert(0) += 1;
    }
    out
}

pub(crate) fn registered_op_owner_lane(op_id: &str) -> &'static str {
    if op_id.starts_with("vyre-intrinsics::") {
        "lower_emit"
    } else if op_id.starts_with("vyre-primitives::graph::")
        || op_id.starts_with("vyre-primitives::bitset::")
        || op_id.starts_with("vyre-primitives::fixpoint::")
        || op_id.starts_with("vyre-libs::graph::")
        || op_id.starts_with("vyre-libs::dataflow::")
    {
        "graph_flow_compiler"
    } else if op_id.starts_with("vyre-primitives::matching::")
        || op_id.starts_with("vyre-primitives::text::")
        || op_id.starts_with("vyre-primitives::nfa::")
        || op_id.starts_with("vyre-libs::scan::")
        || op_id.starts_with("vyre-libs::matching::")
    {
        "scan_automata"
    } else if op_id.starts_with("vyre-primitives::parsing::")
        || op_id.starts_with("vyre-libs::parsing::")
    {
        "parser_frontend"
    } else if op_id.starts_with("vyre-libs::security::")
        || op_id.starts_with("vyre-libs::borrowck::")
        || op_id.starts_with("vyre-libs::rule::")
        || op_id.starts_with("vyre-primitives::predicate::")
    {
        "security_dataflow"
    } else if op_id.starts_with("vyre-primitives::math::")
        || op_id.starts_with("vyre-primitives::reduce::")
        || op_id.starts_with("vyre-primitives::hash::")
        || op_id.starts_with("vyre-primitives::decode::")
        || op_id.starts_with("vyre-primitives::label::")
        || op_id.starts_with("vyre-libs::math::")
        || op_id.starts_with("vyre-libs::nn::")
        || op_id.starts_with("vyre-libs::quant::")
        || op_id.starts_with("vyre-libs::hash::")
        || op_id.starts_with("vyre-libs::decode::")
    {
        "sparse_math_ai"
    } else if op_id.starts_with("vyre-primitives::visual::")
        || op_id.starts_with("vyre-libs::visual::")
    {
        "product_dogfood"
    } else if op_id.starts_with("vyre-primitives::") {
        "foundation_optimizer"
    } else if op_id.starts_with("vyre-libs::") {
        "security_dataflow"
    } else {
        "op_matrix"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn duplicate_family_id_is_pair_order_stable() {
        assert_eq!(
            duplicate_family_id("source-similar", "b.rs", "a.rs"),
            duplicate_family_id("source-similar", "a.rs", "b.rs")
        );
    }

    #[test]
    fn registered_op_duplicate_family_id_is_detector_stable() {
        assert_eq!(
            registered_op_duplicate_family_id("vyre-libs::math::matmul", "vyre-libs::math::dot"),
            registered_op_duplicate_family_id("vyre-libs::math::dot", "vyre-libs::math::matmul")
        );
        assert_ne!(
            registered_op_duplicate_family_id("vyre-libs::math::matmul", "vyre-libs::math::dot"),
            duplicate_family_id(
                "source-similar",
                "vyre-libs::math::matmul",
                "vyre-libs::math::dot"
            )
        );
    }

    #[test]
    fn source_duplicate_subject_preserves_source_identity_fields() {
        let subject = source_duplicate_subject(
            "xtask/src/source_similar.rs",
            "testing_evidence",
            "source-token-fingerprint:v1:abc",
            128,
            4096,
        );

        assert_eq!(subject.id, "xtask/src/source_similar.rs");
        assert_eq!(subject.owner_lane, "testing_evidence");
        assert_eq!(
            subject.fingerprint.as_deref(),
            Some("source-token-fingerprint:v1:abc")
        );
        assert_eq!(subject.tokens, Some(128));
        assert_eq!(subject.bytes, Some(4096));
    }

    #[test]
    fn source_token_fingerprint_is_stable_for_same_tokens() {
        let tokens = vec!["fn".to_string(), "ident".to_string(), "num".to_string()];

        assert_eq!(
            source_token_fingerprint(&tokens),
            source_token_fingerprint(&tokens)
        );
        assert!(source_token_fingerprint(&tokens).starts_with("source-token-fingerprint:v1:"));
    }

    #[test]
    fn registered_op_duplicate_subject_uses_shared_owner_and_fingerprint() {
        let subject = registered_op_duplicate_subject(
            "vyre-libs::scan::literal_set",
            &[1, 2, 3, 4],
            17,
        );

        assert_eq!(subject.owner_lane, "scan_automata");
        assert_eq!(subject.tokens, Some(17));
        assert_eq!(subject.bytes, Some(4));
        assert!(subject
            .fingerprint
            .as_deref()
            .is_some_and(|value| value.starts_with("registered-op-ir-fingerprint:v1:")));
    }


    #[test]
    fn duplicate_family_report_counts_families() {
        let finding = DuplicateFamilyFinding {
            family_id: duplicate_family_id("source-similar", "a.rs", "b.rs"),
            detector: "source-similar".to_string(),
            severity: duplicate_severity(0.96),
            score: 0.96,
            left: DuplicateSubject {
                id: "a.rs".to_string(),
                owner_lane: "coordination".to_string(),
                fingerprint: Some("source-token-fingerprint:v1:left".to_string()),
                tokens: Some(128),
                bytes: Some(2048),
            },
            right: DuplicateSubject {
                id: "b.rs".to_string(),
                owner_lane: "coordination".to_string(),
                fingerprint: Some("source-token-fingerprint:v1:right".to_string()),
                tokens: Some(129),
                bytes: Some(2050),
            },
            import_owner: "coordination".to_string(),
            import_target: "coordination".to_string(),
            evidence: DuplicateEvidence {
                similarity_metric: "normalized-token-shingle-cosine",
                left_metric: "tokens=128:bytes=2048".to_string(),
                right_metric: "tokens=129:bytes=2050".to_string(),
                dedup_action: "extract_shared_module_or_import_existing_owner",
            },
        };

        let report = duplicate_family_report("xtask source-similar", "rust-source", vec![finding]);

        assert_eq!(report.schema_version, DUPLICATE_FAMILY_SCHEMA_VERSION);
        assert_eq!(report.family_count, 1);
        assert_eq!(report.families[0].severity, "duplicate");
    }

    #[test]
    fn duplicate_family_report_merges_same_family_id() {
        let family_id = registered_op_duplicate_family_id("left", "right");
        let left = DuplicateSubject {
            id: "left".to_string(),
            owner_lane: "scan_automata".to_string(),
            fingerprint: Some("registered-op-ir-fingerprint:v1:left".to_string()),
            tokens: Some(10),
            bytes: Some(10),
        };
        let right = DuplicateSubject {
            id: "right".to_string(),
            owner_lane: "scan_automata".to_string(),
            fingerprint: Some("registered-op-ir-fingerprint:v1:right".to_string()),
            tokens: Some(11),
            bytes: Some(11),
        };
        let evidence = DuplicateEvidence {
            similarity_metric: "test",
            left_metric: "left".to_string(),
            right_metric: "right".to_string(),
            dedup_action: "extract_shared_module_or_import_existing_owner",
        };
        let report = duplicate_family_report(
            "xtask test",
            "registered-op",
            vec![
                DuplicateFamilyFinding {
                    family_id: family_id.clone(),
                    detector: "lego-audit:no-reinvention".to_string(),
                    severity: duplicate_severity(0.90),
                    score: 0.90,
                    left: left.clone(),
                    right: right.clone(),
                    import_owner: "scan_automata".to_string(),
                    import_target: "left".to_string(),
                    evidence: evidence.clone(),
                },
                DuplicateFamilyFinding {
                    family_id,
                    detector: "lego-audit:operand-shape".to_string(),
                    severity: duplicate_severity(0.96),
                    score: 0.96,
                    left,
                    right,
                    import_owner: "scan_automata".to_string(),
                    import_target: "left".to_string(),
                    evidence,
                },
            ],
        );

        assert_eq!(report.family_count, 1);
        assert_eq!(
            report.families[0].detector,
            "lego-audit:no-reinvention+lego-audit:operand-shape"
        );
        assert_eq!(report.families[0].score, 0.96);
        assert_eq!(report.families[0].severity, "duplicate");
    }

    #[test]
    fn duplicate_report_json_path_rejects_missing_and_next_flag() {
        assert_eq!(
            duplicate_report_json_path(
                "--duplicate-report-json",
                None,
                "--duplicate-report-json requires a path"
            ),
            Err("--duplicate-report-json requires a path".to_string())
        );
        assert_eq!(
            duplicate_report_json_path(
                "--duplicate-report-json",
                Some("--with-repo"),
                "--duplicate-report-json requires a path"
            ),
            Err("--duplicate-report-json requires a path, not another flag".to_string())
        );
        assert_eq!(
            duplicate_report_json_path(
                "--duplicate-report-json",
                Some("release/evidence/dedup/report.json"),
                "--duplicate-report-json requires a path"
            ),
            Ok(PathBuf::from("release/evidence/dedup/report.json"))
        );
    }

    #[test]
    fn duplicate_report_generator_command_includes_output_path() {
        assert_eq!(
            duplicate_report_generator_command(
                "whats-similar --all",
                Path::new("release/evidence/dedup/registered-op-duplicates.json")
            ),
            "xtask whats-similar --all --duplicate-report-json release/evidence/dedup/registered-op-duplicates.json"
        );
    }

    #[test]
    fn duplicate_family_artifact_validation_rejects_drift() {
        let blockers = validate_duplicate_family_report_artifact(
            br#"{"schema_version":1,"generator_command":"wrong","family_count":1,"families":[{"left":{},"right":{}}]}"#,
            "xtask whats-similar --all --duplicate-report-json release/evidence/dedup/registered-op-duplicates.json",
        );

        assert!(blockers
            .iter()
            .any(|blocker| blocker.contains("schema_version=2")));
        assert!(blockers
            .iter()
            .any(|blocker| blocker.contains("generator_command")));
        assert!(blockers
            .iter()
            .any(|blocker| blocker.contains("family[0].family_id")));
        assert!(blockers
            .iter()
            .any(|blocker| blocker.contains("left.fingerprint")));
        assert!(blockers
            .iter()
            .any(|blocker| blocker.contains("right.fingerprint")));
    }

    #[test]
    fn duplicate_family_artifact_validation_rejects_unknown_fingerprint_namespace() {
        let blockers = validate_duplicate_family_report_artifact(
            br#"{
  "schema_version": 2,
  "generator_command": "xtask source-similar --duplicate-report-json release/evidence/dedup/source-similar-duplicates.json",
  "family_count": 1,
  "families": [
    {
      "family_id": "duplicate-family:v1:abc",
      "detector": "source-similar",
      "left": {"fingerprint": "unknown:v1:left"},
      "right": {"fingerprint": "source-token-fingerprint:v1:right"}
    }
  ]
}"#,
            "xtask source-similar --duplicate-report-json release/evidence/dedup/source-similar-duplicates.json",
        );

        assert!(blockers
            .iter()
            .any(|blocker| blocker.contains("unsupported namespace")));
    }

    #[test]
    fn structural_similarity_is_order_sensitive() {
        assert_eq!(structural_similarity(&[1, 2, 3], &[1, 2, 3]), 0.0);
        assert!((structural_similarity(&[1, 2, 3, 4], &[1, 2, 3, 4]) - 1.0).abs() < 1e-12);
        assert!(structural_similarity(&[1, 2, 3, 4], &[4, 3, 2, 1]) < 1.0);
    }

    #[test]
    fn registered_op_owner_lane_classifies_major_namespaces() {
        assert_eq!(
            registered_op_owner_lane("vyre-primitives::graph::csr_forward_traverse"),
            "graph_flow_compiler"
        );
        assert_eq!(
            registered_op_owner_lane("vyre-libs::dataflow::semi_naive_join"),
            "graph_flow_compiler"
        );
        assert_eq!(
            registered_op_owner_lane("vyre-libs::scan::literal_set"),
            "scan_automata"
        );
        assert_eq!(
            registered_op_owner_lane("vyre-libs::parsing::c11"),
            "parser_frontend"
        );
        assert_eq!(
            registered_op_owner_lane("vyre-primitives::parsing::ssa_dominance_scan"),
            "parser_frontend"
        );
        assert_eq!(
            registered_op_owner_lane("vyre-libs::nn::softmax"),
            "sparse_math_ai"
        );
        assert_eq!(
            registered_op_owner_lane("vyre-primitives::predicate::call_to"),
            "security_dataflow"
        );
        assert_eq!(
            registered_op_owner_lane("vyre-libs::visual::blur"),
            "product_dogfood"
        );
    }
}
