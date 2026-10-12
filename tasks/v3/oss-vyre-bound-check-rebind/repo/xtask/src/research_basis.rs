use std::collections::BTreeMap;

use crate::markdown_table::{markdown_cells, trim_code_ticks};
use crate::research_key::is_research_key;
use crate::research_source_ledger::normalize_research_source_url;

const EXTERNAL_RESEARCH_BASIS_START: &str = "## External research basis";
const EXTERNAL_RESEARCH_BASIS_END: &str = "## Evidence-backed plan items";

pub(crate) fn external_research_basis_entries(
    plan: &str,
) -> Result<BTreeMap<String, String>, Vec<String>> {
    let Some(section) = section_between(
        plan,
        EXTERNAL_RESEARCH_BASIS_START,
        EXTERNAL_RESEARCH_BASIS_END,
    ) else {
        return Err(vec![
            "missing `External research basis` section before plan items".to_string(),
        ]);
    };
    let mut entries = BTreeMap::new();
    let mut failures = Vec::new();
    for line in section.lines() {
        let cells = markdown_cells(line);
        if cells.len() != 3 {
            continue;
        }
        if cells[0] == "Key" || cells[0].starts_with("---") {
            continue;
        }
        let key = trim_code_ticks(&cells[0]).trim().to_string();
        if key.is_empty() {
            continue;
        }
        if !is_research_key(&key) {
            failures.push(format!(
                "external research basis key `{key}` must use uppercase letters, digits, and underscores"
            ));
            continue;
        }
        let url = normalize_research_source_url(&cells[1]);
        if url.is_empty() {
            failures.push(format!("external research basis key `{key}` is missing source URL"));
        }
        if entries.insert(key.clone(), url).is_some() {
            failures.push(format!("external research basis key `{key}` is duplicated"));
        }
    }
    if entries.is_empty() {
        failures.push("external research basis table has no keys".to_string());
    }
    if failures.is_empty() {
        Ok(entries)
    } else {
        Err(failures)
    }
}

fn section_between<'a>(text: &'a str, start: &str, end: &str) -> Option<&'a str> {
    let start_idx = text.find(start)?;
    let after_start = &text[start_idx + start.len()..];
    let end_idx = after_start.find(end)?;
    Some(&after_start[..end_idx])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn external_research_basis_entries_accept_digit_keys() {
        let plan = r#"
## External research basis

| Key | Source | Use in this plan |
| --- | --- | --- |
| `FLASH_ATTN2` | <https://example.invalid/flash/> | Test key. |
| `XAV_FPGA` | <https://example.invalid/xav/> | Test key. |

## Evidence-backed plan items
"#;

        let entries = external_research_basis_entries(plan)
            .expect("Fix: valid external research basis rows must parse.");

        assert_eq!(
            entries.get("FLASH_ATTN2").map(String::as_str),
            Some("https://example.invalid/flash")
        );
        assert!(entries.contains_key("XAV_FPGA"));
    }
}
