use crate::markdown_table::markdown_cells;

pub(crate) const VX_PLAN_TABLE_HEADER: &str =
    "| ID | Axis | Local evidence | Research basis | Work | Proof gate | Dedup seam |";
pub(crate) const VX_PLAN_MIN_ROWS: usize = 480;

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) struct RawVxPlanRow {
    pub(crate) line: usize,
    pub(crate) id: String,
    pub(crate) axis: String,
    pub(crate) local_evidence: String,
    pub(crate) research_basis: String,
    pub(crate) work: String,
    pub(crate) proof_gate: String,
    pub(crate) dedup_seam: String,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) struct RawVxPlanTable {
    pub(crate) rows: Vec<RawVxPlanRow>,
    pub(crate) failures: Vec<String>,
    pub(crate) saw_header: bool,
}

pub(crate) fn parse_raw_vx_plan_table(plan: &str) -> RawVxPlanTable {
    let mut rows = Vec::new();
    let mut failures = Vec::new();
    let mut saw_header = false;
    for (idx, line) in plan.lines().enumerate() {
        let line_no = idx + 1;
        if line.trim() == VX_PLAN_TABLE_HEADER {
            saw_header = true;
        }
        if !line.starts_with("| VX-") {
            continue;
        }
        let cells = markdown_cells(line);
        if cells.len() != 7 {
            failures.push(format!(
                "line {line_no}: VX row has {} cells, expected 7",
                cells.len()
            ));
            continue;
        }
        rows.push(RawVxPlanRow {
            line: line_no,
            id: cells[0].to_string(),
            axis: cells[1].to_string(),
            local_evidence: cells[2].to_string(),
            research_basis: cells[3].to_string(),
            work: cells[4].to_string(),
            proof_gate: cells[5].to_string(),
            dedup_seam: cells[6].to_string(),
        });
    }
    RawVxPlanTable {
        rows,
        failures,
        saw_header,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raw_vx_plan_rows_preserve_line_and_cells() {
        let plan = "\n| VX-001 | coordination | local | `MLIR_PASS` | Fix: x | Proof gate. | seam |\n";

        let table = parse_raw_vx_plan_table(plan);

        assert_eq!(table.failures, Vec::<String>::new());
        assert_eq!(table.rows[0].line, 2);
        assert_eq!(table.rows[0].id, "VX-001");
        assert_eq!(table.rows[0].research_basis, "`MLIR_PASS`");
    }
}
