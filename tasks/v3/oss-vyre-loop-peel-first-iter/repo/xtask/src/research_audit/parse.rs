use std::collections::BTreeSet;

use crate::research_basis::external_research_basis_entries;
use crate::vx_plan_table::parse_raw_vx_plan_table;

use super::model::VxRow;

pub(super) fn parse_vx_rows(plan: &str) -> Vec<VxRow> {
    parse_raw_vx_plan_table(plan)
        .rows
        .into_iter()
        .map(|row| VxRow {
            line: row.line,
            id: row.id,
            axis: row.axis,
            local_evidence: row.local_evidence,
            research_basis: row.research_basis,
            work: row.work,
            proof_gate: row.proof_gate,
            dedup_seam: row.dedup_seam,
        })
        .collect()
}

pub(super) fn parse_defined_research_keys(plan: &str) -> BTreeSet<String> {
    external_research_basis_entries(plan)
        .map(|entries| entries.into_keys().collect())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::research_key::backtick_research_keys;

    #[test]
    fn research_keys_allow_digits() {
        let plan = r#"
## External research basis

| Key | Source | Use in this plan |
| --- | --- | --- |
| `FLASH_ATTN2` | <https://example.invalid/flash/> | Test key. |
| `XAV_FPGA` | <https://example.invalid/xav/> | Test key. |

## Evidence-backed plan items
"#;

        let keys = parse_defined_research_keys(plan);

        assert!(keys.contains("FLASH_ATTN2"));
        assert!(keys.contains("XAV_FPGA"));
        assert_eq!(
            backtick_research_keys("Uses `FLASH_ATTN2` and `XAV_FPGA`."),
            vec!["FLASH_ATTN2".to_string(), "XAV_FPGA".to_string()]
        );
    }
}
