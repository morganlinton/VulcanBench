use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct OwnershipConfig {
    #[serde(default)]
    lane: BTreeMap<String, OwnershipLaneConfig>,
}

#[derive(Debug, Deserialize)]
struct OwnershipLaneConfig {
    #[serde(default)]
    write: Vec<String>,
    #[serde(default)]
    parent_axis: Option<String>,
    #[serde(default)]
    support_reason: Option<String>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) struct OwnershipLaneRule {
    pub(crate) lane: String,
    pub(crate) write_patterns: Vec<String>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) struct OwnershipLaneClassification {
    pub(crate) lane: String,
    pub(crate) write_patterns: Vec<String>,
    pub(crate) parent_axis: Option<String>,
    pub(crate) support_reason: Option<String>,
}

pub(crate) fn load_ownership_lanes(path: &Path) -> Result<Vec<OwnershipLaneRule>, String> {
    let text = fs::read_to_string(path).map_err(|error| error.to_string())?;
    parse_ownership_lane_rules(&text)
}

pub(crate) fn parse_ownership_lane_rules(text: &str) -> Result<Vec<OwnershipLaneRule>, String> {
    let cfg = parse_ownership_config(text)?;
    let mut lanes = cfg
        .lane
        .into_iter()
        .filter_map(|(lane, cfg)| {
            if cfg.write.is_empty() {
                None
            } else {
                Some(OwnershipLaneRule {
                    lane,
                    write_patterns: cfg.write,
                })
            }
        })
        .collect::<Vec<_>>();
    lanes.sort_by(|a, b| {
        let a_len = a.write_patterns.iter().map(String::len).max().unwrap_or(0);
        let b_len = b.write_patterns.iter().map(String::len).max().unwrap_or(0);
        b_len.cmp(&a_len).then_with(|| a.lane.cmp(&b.lane))
    });
    if lanes.is_empty() {
        return Err("OWNERSHIP.toml has no lane write patterns".to_string());
    }
    Ok(lanes)
}

pub(crate) fn parse_ownership_lane_classifications(
    text: &str,
) -> Result<BTreeMap<String, OwnershipLaneClassification>, String> {
    let cfg = parse_ownership_config(text)?;
    if cfg.lane.is_empty() {
        return Err("OWNERSHIP.toml has no lane entries".to_string());
    }
    let mut lanes = BTreeMap::new();
    for (lane, cfg) in cfg.lane {
        lanes.insert(
            lane.clone(),
            OwnershipLaneClassification {
                lane,
                write_patterns: cfg.write,
                parent_axis: normalized_optional_text(cfg.parent_axis),
                support_reason: normalized_optional_text(cfg.support_reason),
            },
        );
    }
    Ok(lanes)
}

pub(crate) fn parse_ownership_lane_names(text: &str) -> Result<BTreeSet<String>, String> {
    Ok(parse_ownership_lane_rules(text)?
        .into_iter()
        .map(|lane| lane.lane)
        .collect())
}

pub(crate) fn owner_lane_for_file<'a>(
    file: &str,
    ownership_lanes: &'a [OwnershipLaneRule],
) -> &'a str {
    let mut best = None::<(usize, &'a str)>;
    for lane in ownership_lanes {
        for pattern in &lane.write_patterns {
            let Some(score) = ownership_pattern_specificity(pattern, file) else {
                continue;
            };
            match best {
                Some((best_score, best_lane))
                    if score < best_score || (score == best_score && lane.lane.as_str() >= best_lane) => {}
                _ => best = Some((score, lane.lane.as_str())),
            }
        }
    }
    best.map(|(_, lane)| lane).unwrap_or("unowned")
}

fn parse_ownership_config(text: &str) -> Result<OwnershipConfig, String> {
    toml::from_str(text).map_err(|error| error.to_string())
}

fn normalized_optional_text(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

fn ownership_pattern_matches(pattern: &str, file: &str) -> bool {
    if let Some(prefix) = pattern.strip_suffix("/**") {
        file == prefix || file.starts_with(&format!("{prefix}/"))
    } else if pattern.contains('*') {
        wildcard_match(pattern, file)
    } else {
        pattern == file
    }
}

fn ownership_pattern_specificity(pattern: &str, file: &str) -> Option<usize> {
    if let Some(prefix) = pattern.strip_suffix("/**") {
        if file == prefix || file.starts_with(&format!("{prefix}/")) {
            Some(prefix.len())
        } else {
            None
        }
    } else if pattern.contains('*') {
        if wildcard_match(pattern, file) {
            Some(pattern.chars().filter(|ch| *ch != '*').count())
        } else {
            None
        }
    } else if pattern == file {
        Some(pattern.len() + 1)
    } else {
        None
    }
}

fn wildcard_match(pattern: &str, text: &str) -> bool {
    let pattern = pattern.as_bytes();
    let text = text.as_bytes();
    let mut pattern_index = 0usize;
    let mut text_index = 0usize;
    let mut star_index = None;
    let mut star_text_index = 0usize;

    while text_index < text.len() {
        if pattern_index < pattern.len() && pattern[pattern_index] == text[text_index] {
            pattern_index += 1;
            text_index += 1;
        } else if pattern_index < pattern.len() && pattern[pattern_index] == b'*' {
            star_index = Some(pattern_index);
            star_text_index = text_index;
            pattern_index += 1;
        } else if let Some(star) = star_index {
            pattern_index = star + 1;
            star_text_index += 1;
            text_index = star_text_index;
        } else {
            return false;
        }
    }

    while pattern_index < pattern.len() && pattern[pattern_index] == b'*' {
        pattern_index += 1;
    }
    pattern_index == pattern.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ownership_rules_choose_most_specific_write_pattern() {
        let text = r#"
[lane.driver_shared]
write = ["vyre-driver/src/**"]

[lane.driver_cuda]
write = ["vyre-driver-cuda/src/**"]
"#;

        let lanes = parse_ownership_lane_rules(text).unwrap();

        assert_eq!(
            owner_lane_for_file("vyre-driver-cuda/src/backend/dispatch.rs", &lanes),
            "driver_cuda"
        );
        assert_eq!(
            owner_lane_for_file("vyre-driver/src/backend/compiled_pipeline.rs", &lanes),
            "driver_shared"
        );
    }

    #[test]
    fn exact_shared_helper_owner_beats_broad_xtask_lanes() {
        let text = r#"
[lane.security_reliability]
write = ["vyre-driver-cuda/src/backend/allocations.rs", "xtask/src/**"]

[lane.testing_evidence]
write = ["xtask/src/research_key.rs", "xtask/src/**"]

[lane.evidence_truth]
write = ["xtask/src/**"]
"#;

        let lanes = parse_ownership_lane_rules(text).unwrap();

        assert_eq!(
            owner_lane_for_file("xtask/src/research_key.rs", &lanes),
            "testing_evidence"
        );
    }

    #[test]
    fn ownership_rule_supports_file_globs() {
        let text = r#"
[lane.foundation_optimizer]
write = ["vyre-foundation/tests/*optimizer*"]
"#;

        let lanes = parse_ownership_lane_rules(text).unwrap();

        assert_eq!(
            owner_lane_for_file("vyre-foundation/tests/range_optimizer.rs", &lanes),
            "foundation_optimizer"
        );
        assert_eq!(
            owner_lane_for_file("vyre-foundation/tests/wire.rs", &lanes),
            "unowned"
        );
    }

    #[test]
    fn ownership_classifications_preserve_supporting_lane_metadata() {
        let text = r#"
[lane.coordination]
write = ["docs/optimization/**"]

[lane.op_matrix]
parent_axis = "coordination"
support_reason = "Op coverage files support coordination evidence."
write = ["docs/optimization/OP_MATRIX.toml"]
"#;

        let lanes = parse_ownership_lane_classifications(text).unwrap();
        let op_matrix = lanes.get("op_matrix").unwrap();

        assert_eq!(op_matrix.parent_axis.as_deref(), Some("coordination"));
        assert_eq!(
            op_matrix.support_reason.as_deref(),
            Some("Op coverage files support coordination evidence.")
        );
        assert_eq!(op_matrix.write_patterns, ["docs/optimization/OP_MATRIX.toml"]);
    }
}
