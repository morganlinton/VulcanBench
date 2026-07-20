//! Data-backed catalog rows for security bitset predicates.
//!
//! The source of truth is `vyre-libs/rules/security_predicates.toml`.
//! Public security primitives keep their stable Rust functions, while release
//! gates and inventory witness registration consume these rows for op id,
//! inputs, soundness, witness fixtures, and Weir mapping metadata.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

const SECURITY_PREDICATES_TOML: &str = include_str!("../../rules/security_predicates.toml");
const EXPECTED_SCHEMA_VERSION: u32 = 1;

/// Primitive operation used by a data-backed security predicate row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecurityPredicateOperation {
    /// Per-word `lhs & rhs` bitset intersection.
    BitsetAnd,
    /// Per-word `lhs & !rhs` bitset subtraction.
    BitsetAndNot,
}

impl SecurityPredicateOperation {
    /// Return the Tier-B TOML spelling for this operation.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::BitsetAnd => "bitset_and",
            Self::BitsetAndNot => "bitset_and_not",
        }
    }
}

/// One security bitset predicate row parsed from Tier-B TOML.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecurityPredicateRow {
    /// Stable short row id.
    pub id: String,
    /// Rust module that owns the public primitive.
    pub module: String,
    /// Public function exported by the module.
    pub function: String,
    /// Stable VyrE op id registered in the harness inventory.
    pub op_id: String,
    /// Bitset operation used by this predicate.
    pub operation: SecurityPredicateOperation,
    /// Ordered input buffer names from the public primitive contract.
    pub inputs: Vec<String>,
    /// Output buffer name from the public primitive contract.
    pub output: String,
    /// Declared soundness lattice value for the predicate.
    pub soundness: String,
    /// Stable fixture id for the row's CPU witness vectors.
    pub witness_fixture: String,
    /// Weir/dataflow concept this predicate maps onto.
    pub weir_mapping: String,
    /// Left-hand witness input words.
    pub witness_lhs: Vec<u32>,
    /// Right-hand witness input words.
    pub witness_rhs: Vec<u32>,
    /// Expected CPU reference output words.
    pub witness_expected: Vec<u32>,
}

static SECURITY_PREDICATE_ROWS: OnceLock<Result<Vec<SecurityPredicateRow>, String>> =
    OnceLock::new();

/// Parse and return all bundled security predicate rows.
pub fn try_security_predicate_rows() -> Result<&'static [SecurityPredicateRow], &'static str> {
    match SECURITY_PREDICATE_ROWS.get_or_init(|| parse_security_predicates(SECURITY_PREDICATES_TOML))
    {
        Ok(rows) => Ok(rows.as_slice()),
        Err(error) => Err(error.as_str()),
    }
}

/// Return parsed security predicate rows, or an empty slice if bundled data is invalid.
#[must_use]
pub fn security_predicate_rows() -> &'static [SecurityPredicateRow] {
    try_security_predicate_rows().unwrap_or(&[])
}

/// Find one security predicate row by stable op id.
#[must_use]
pub fn security_predicate_row_by_op_id(op_id: &str) -> Option<&'static SecurityPredicateRow> {
    try_security_predicate_rows()
        .ok()?
        .iter()
        .find(|row| row.op_id == op_id)
}

pub(crate) fn packed_witness_inputs(op_id: &str) -> Vec<Vec<Vec<u8>>> {
    security_predicate_row_by_op_id(op_id)
        .map(|row| {
            vec![vec![
                vyre_primitives::wire::pack_u32_slice(&row.witness_lhs),
                vyre_primitives::wire::pack_u32_slice(&row.witness_rhs),
                vyre_primitives::wire::pack_u32_slice(&[0]),
            ]]
        })
        .unwrap_or_default()
}

pub(crate) fn packed_witness_expected(op_id: &str) -> Vec<Vec<Vec<u8>>> {
    security_predicate_row_by_op_id(op_id)
        .map(|row| vec![vec![vyre_primitives::wire::pack_u32_slice(
            &row.witness_expected,
        )]])
        .unwrap_or_default()
}

fn parse_security_predicates(source: &str) -> Result<Vec<SecurityPredicateRow>, String> {
    let mut schema_version = None;
    let mut current = None::<BTreeMap<String, String>>;
    let mut raw_rows = Vec::new();
    for (line_index, raw_line) in source.lines().enumerate() {
        let line_no = line_index + 1;
        let line = raw_line
            .split_once('#')
            .map_or(raw_line, |(before, _)| before)
            .trim();
        if line.is_empty() {
            continue;
        }
        if line == "[[predicate]]" {
            if let Some(row) = current.take() {
                raw_rows.push(row);
            }
            current = Some(BTreeMap::new());
            continue;
        }
        let (key, value) = line.split_once('=').ok_or_else(|| {
            format!(
                "Fix: security predicate Tier-B TOML line {line_no} must be `key = value`, got `{line}`."
            )
        })?;
        let key = key.trim();
        let value = value.trim().to_string();
        if let Some(row) = current.as_mut() {
            if row.insert(key.to_string(), value).is_some() {
                return Err(format!(
                    "Fix: security predicate Tier-B TOML line {line_no} duplicates key `{key}` in one [[predicate]] row."
                ));
            }
        } else if key == "schema_version" {
            schema_version = Some(parse_u32_scalar(&value, key, line_no)?);
        } else {
            return Err(format!(
                "Fix: security predicate Tier-B TOML line {line_no} sets `{key}` before the first [[predicate]] row."
            ));
        }
    }
    if let Some(row) = current.take() {
        raw_rows.push(row);
    }
    match schema_version {
        Some(EXPECTED_SCHEMA_VERSION) => {}
        Some(version) => {
            return Err(format!(
                "Fix: security predicate Tier-B TOML schema_version={version}, expected {EXPECTED_SCHEMA_VERSION}."
            ));
        }
        None => {
            return Err(
                "Fix: security predicate Tier-B TOML must declare schema_version = 1.".to_string(),
            );
        }
    }
    if raw_rows.is_empty() {
        return Err("Fix: security predicate Tier-B TOML must declare at least one [[predicate]] row.".to_string());
    }

    let mut seen_ids = BTreeSet::new();
    let mut seen_op_ids = BTreeSet::new();
    let mut rows = Vec::with_capacity(raw_rows.len());
    for (index, raw) in raw_rows.into_iter().enumerate() {
        let row_no = index + 1;
        let row = SecurityPredicateRow {
            id: required_string(&raw, "id", row_no)?,
            module: required_string(&raw, "module", row_no)?,
            function: required_string(&raw, "function", row_no)?,
            op_id: required_string(&raw, "op_id", row_no)?,
            operation: parse_operation(&required_string(&raw, "operation", row_no)?, row_no)?,
            inputs: required_string_array(&raw, "inputs", row_no)?,
            output: required_string(&raw, "output", row_no)?,
            soundness: required_string(&raw, "soundness", row_no)?,
            witness_fixture: required_string(&raw, "witness_fixture", row_no)?,
            weir_mapping: required_string(&raw, "weir_mapping", row_no)?,
            witness_lhs: required_u32_array(&raw, "witness_lhs", row_no)?,
            witness_rhs: required_u32_array(&raw, "witness_rhs", row_no)?,
            witness_expected: required_u32_array(&raw, "witness_expected", row_no)?,
        };
        validate_row(&row, row_no)?;
        if !seen_ids.insert(row.id.clone()) {
            return Err(format!(
                "Fix: security predicate Tier-B TOML duplicates id `{}`.",
                row.id
            ));
        }
        if !seen_op_ids.insert(row.op_id.clone()) {
            return Err(format!(
                "Fix: security predicate Tier-B TOML duplicates op_id `{}`.",
                row.op_id
            ));
        }
        rows.push(row);
    }
    rows.sort_by(|left, right| left.op_id.cmp(&right.op_id));
    Ok(rows)
}

fn validate_row(row: &SecurityPredicateRow, row_no: usize) -> Result<(), String> {
    if !row.op_id.starts_with("vyre-libs::security::") {
        return Err(format!(
            "Fix: security predicate row {row_no} op_id `{}` must start with `vyre-libs::security::`.",
            row.op_id
        ));
    }
    if row.inputs.len() != 2 {
        return Err(format!(
            "Fix: security predicate row {row_no} `{}` must declare exactly two bitset inputs.",
            row.id
        ));
    }
    if row.soundness != "Exact" {
        return Err(format!(
            "Fix: security predicate row {row_no} `{}` declares soundness `{}`; VX-096 bitset predicates must be Exact.",
            row.id, row.soundness
        ));
    }
    if row.witness_lhs.len() != row.witness_rhs.len()
        || row.witness_lhs.len() != row.witness_expected.len()
    {
        return Err(format!(
            "Fix: security predicate row {row_no} `{}` witness_lhs/rhs/expected lengths must match.",
            row.id
        ));
    }
    if row.witness_fixture.trim().is_empty() || row.weir_mapping.trim().is_empty() {
        return Err(format!(
            "Fix: security predicate row {row_no} `{}` must declare witness_fixture and weir_mapping.",
            row.id
        ));
    }
    Ok(())
}

fn required_string(
    row: &BTreeMap<String, String>,
    key: &str,
    row_no: usize,
) -> Result<String, String> {
    let value = row.get(key).ok_or_else(|| {
        format!("Fix: security predicate Tier-B row {row_no} is missing `{key}`.")
    })?;
    parse_string_scalar(value, key, row_no)
}

fn required_string_array(
    row: &BTreeMap<String, String>,
    key: &str,
    row_no: usize,
) -> Result<Vec<String>, String> {
    let value = row.get(key).ok_or_else(|| {
        format!("Fix: security predicate Tier-B row {row_no} is missing `{key}`.")
    })?;
    parse_string_array(value, key, row_no)
}

fn required_u32_array(
    row: &BTreeMap<String, String>,
    key: &str,
    row_no: usize,
) -> Result<Vec<u32>, String> {
    let value = row.get(key).ok_or_else(|| {
        format!("Fix: security predicate Tier-B row {row_no} is missing `{key}`.")
    })?;
    parse_u32_array(value, key, row_no)
}

fn parse_operation(value: &str, row_no: usize) -> Result<SecurityPredicateOperation, String> {
    match value {
        "bitset_and" => Ok(SecurityPredicateOperation::BitsetAnd),
        "bitset_and_not" => Ok(SecurityPredicateOperation::BitsetAndNot),
        other => Err(format!(
            "Fix: security predicate Tier-B row {row_no} operation `{other}` is unsupported; expected bitset_and or bitset_and_not."
        )),
    }
}

fn parse_string_scalar(value: &str, key: &str, row_no: usize) -> Result<String, String> {
    value
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
        .map(str::to_string)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| {
            format!(
                "Fix: security predicate Tier-B row {row_no} field `{key}` must be a non-empty quoted string."
            )
        })
}

fn parse_u32_scalar(value: &str, key: &str, line_no: usize) -> Result<u32, String> {
    value.parse::<u32>().map_err(|error| {
        format!(
            "Fix: security predicate Tier-B TOML line {line_no} field `{key}` must be u32: {error}."
        )
    })
}

fn parse_string_array(value: &str, key: &str, row_no: usize) -> Result<Vec<String>, String> {
    let body = value
        .strip_prefix('[')
        .and_then(|value| value.strip_suffix(']'))
        .ok_or_else(|| {
            format!(
                "Fix: security predicate Tier-B row {row_no} field `{key}` must be a TOML string array."
            )
        })?;
    if body.trim().is_empty() {
        return Ok(Vec::new());
    }
    body.split(',')
        .map(|item| parse_string_scalar(item.trim(), key, row_no))
        .collect()
}

fn parse_u32_array(value: &str, key: &str, row_no: usize) -> Result<Vec<u32>, String> {
    let body = value
        .strip_prefix('[')
        .and_then(|value| value.strip_suffix(']'))
        .ok_or_else(|| {
            format!(
                "Fix: security predicate Tier-B row {row_no} field `{key}` must be a TOML u32 array."
            )
        })?;
    if body.trim().is_empty() {
        return Ok(Vec::new());
    }
    body.split(',')
        .map(|item| {
            item.trim().parse::<u32>().map_err(|error| {
                format!(
                    "Fix: security predicate Tier-B row {row_no} field `{key}` has non-u32 array item `{}`: {error}.",
                    item.trim()
                )
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use vyre::soundness::SoundnessTagged;

    const EXPECTED_BITSET_PREDICATE_COUNT: usize = 10;

    #[test]
    fn tier_b_security_predicate_rows_parse_and_cover_bitset_surface() {
        let rows = try_security_predicate_rows()
            .expect("Fix: bundled security predicate Tier-B TOML must parse");
        assert_eq!(rows.len(), EXPECTED_BITSET_PREDICATE_COUNT);
        for row in rows {
            assert_eq!(row.soundness, "Exact");
            assert_eq!(row.inputs.len(), 2);
            assert!(row.op_id.starts_with("vyre-libs::security::"));
            assert!(!row.witness_fixture.trim().is_empty());
            assert!(!row.weir_mapping.trim().is_empty());
            assert_eq!(row.witness_lhs.len(), row.witness_rhs.len());
            assert_eq!(row.witness_lhs.len(), row.witness_expected.len());
        }
    }

    #[test]
    fn tier_b_security_rows_match_module_op_ids() {
        let expected = [
            super::super::auth_check_dominates::OP_ID,
            super::super::buffer_size_check::OP_ID,
            super::super::format_string_check::OP_ID,
            super::super::lock_dominates::OP_ID,
            super::super::path_canonical::OP_ID,
            super::super::sanitizer_dominates::OP_ID,
            super::super::sql_param_bound::OP_ID,
            super::super::taint_kill::OP_ID,
            super::super::unchecked_return::OP_ID,
            super::super::xss_escape::OP_ID,
        ];
        for op_id in expected {
            let row = security_predicate_row_by_op_id(op_id)
                .unwrap_or_else(|| panic!("Fix: missing Tier-B security predicate row for {op_id}"));
            assert_eq!(row.op_id, op_id);
            assert_eq!(row.function, row.module);
        }
    }

    #[test]
    fn tier_b_witnesses_match_current_cpu_references() {
        for row in security_predicate_rows() {
            let expected = match row.op_id.as_str() {
                super::super::auth_check_dominates::OP_ID => {
                    super::super::auth_check_dominates::cpu_ref(&row.witness_lhs, &row.witness_rhs)
                }
                super::super::buffer_size_check::OP_ID => {
                    super::super::buffer_size_check::cpu_ref(&row.witness_lhs, &row.witness_rhs)
                }
                super::super::format_string_check::OP_ID => {
                    super::super::format_string_check::cpu_ref(&row.witness_lhs, &row.witness_rhs)
                }
                super::super::lock_dominates::OP_ID => {
                    super::super::lock_dominates::cpu_ref(&row.witness_lhs, &row.witness_rhs)
                }
                super::super::path_canonical::OP_ID => {
                    super::super::path_canonical::cpu_ref(&row.witness_lhs, &row.witness_rhs)
                }
                super::super::sanitizer_dominates::OP_ID => {
                    super::super::sanitizer_dominates::cpu_ref(&row.witness_lhs, &row.witness_rhs)
                }
                super::super::sql_param_bound::OP_ID => {
                    super::super::sql_param_bound::cpu_ref(&row.witness_lhs, &row.witness_rhs)
                }
                super::super::taint_kill::OP_ID => {
                    super::super::taint_kill::cpu_ref(&row.witness_lhs, &row.witness_rhs)
                }
                super::super::unchecked_return::OP_ID => {
                    super::super::unchecked_return::cpu_ref(&row.witness_lhs, &row.witness_rhs)
                }
                super::super::xss_escape::OP_ID => {
                    super::super::xss_escape::cpu_ref(&row.witness_lhs, &row.witness_rhs)
                }
                other => panic!("Fix: unknown security predicate Tier-B op id `{other}`"),
            };
            assert_eq!(
                expected, row.witness_expected,
                "Fix: Tier-B witness fixture {} drifted from CPU ref for {}",
                row.witness_fixture, row.op_id
            );
        }
    }

    #[test]
    fn tier_b_soundness_rows_match_marker_types() {
        let exact = vyre::soundness::Soundness::Exact;
        for (op_id, soundness) in [
            (
                super::super::auth_check_dominates::OP_ID,
                super::super::auth_check_dominates::AuthCheckDominates.soundness(),
            ),
            (
                super::super::buffer_size_check::OP_ID,
                super::super::buffer_size_check::BufferSizeCheck.soundness(),
            ),
            (
                super::super::format_string_check::OP_ID,
                super::super::format_string_check::FormatStringCheck.soundness(),
            ),
            (
                super::super::lock_dominates::OP_ID,
                super::super::lock_dominates::LockDominates.soundness(),
            ),
            (
                super::super::path_canonical::OP_ID,
                super::super::path_canonical::PathCanonical.soundness(),
            ),
            (
                super::super::sanitizer_dominates::OP_ID,
                super::super::sanitizer_dominates::SanitizerDominates.soundness(),
            ),
            (
                super::super::sql_param_bound::OP_ID,
                super::super::sql_param_bound::SqlParamBound.soundness(),
            ),
            (
                super::super::taint_kill::OP_ID,
                super::super::taint_kill::TaintKill.soundness(),
            ),
            (
                super::super::unchecked_return::OP_ID,
                super::super::unchecked_return::UncheckedReturn.soundness(),
            ),
            (
                super::super::xss_escape::OP_ID,
                super::super::xss_escape::XssEscape.soundness(),
            ),
        ] {
            let row = security_predicate_row_by_op_id(op_id)
                .unwrap_or_else(|| panic!("Fix: missing Tier-B row for soundness marker {op_id}"));
            assert_eq!(soundness, exact);
            assert_eq!(row.soundness, "Exact");
        }
    }
}
