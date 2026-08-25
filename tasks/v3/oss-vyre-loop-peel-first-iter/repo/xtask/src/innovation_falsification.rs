//! Shared falsification checks for frontier innovation claims.

const FRONTIER_VX_MIN_ID: u32 = 421;

struct FalsificationRequirement {
    field: &'static str,
    markers: &'static [&'static str],
}

const FALSIFICATION_REQUIREMENTS: &[FalsificationRequirement] = &[
    FalsificationRequirement {
        field: "comparator",
        markers: &[
            "against",
            "baseline",
            "compare",
            "comparator",
            "differential",
            "parity",
            "versus",
        ],
    },
    FalsificationRequirement {
        field: "dataset",
        markers: &[
            "corpus",
            "dataset",
            "fixture",
            "graph",
            "haystack",
            "rule set",
            "workload",
        ],
    },
    FalsificationRequirement {
        field: "metric",
        markers: &[
            "bandwidth",
            "bytes",
            "count",
            "divergence",
            "latency",
            "metric",
            "qps",
            "recall",
            "throughput",
            "time",
        ],
    },
    FalsificationRequirement {
        field: "floor",
        markers: &[
            "at least",
            "bounded",
            "budget",
            "floor",
            "minimum",
            "parity",
            "threshold",
            "tolerance",
        ],
    },
    FalsificationRequirement {
        field: "failure-mode",
        markers: &[
            "blocker",
            "diagnostic",
            "fail-closed",
            "failure mode",
            "failure-mode",
            "failure reason",
            "fallback",
            "negative",
            "reject",
            "rejected",
            "unsupported",
        ],
    },
    FalsificationRequirement {
        field: "evidence-artifact-path",
        markers: &["release/evidence/", ".json"],
    },
];

pub(crate) fn missing_frontier_falsification_fields(row_id: &str, text: &str) -> Vec<&'static str> {
    if !requires_frontier_falsification(row_id) {
        return Vec::new();
    }
    let lower = text.to_ascii_lowercase();
    FALSIFICATION_REQUIREMENTS
        .iter()
        .filter_map(|requirement| {
            (!requirement
                .markers
                .iter()
                .any(|marker| lower.contains(marker)))
            .then_some(requirement.field)
        })
        .collect()
}

fn requires_frontier_falsification(row_id: &str) -> bool {
    row_id
        .strip_prefix("VX-")
        .and_then(|raw| raw.parse::<u32>().ok())
        .is_some_and(|id| id >= FRONTIER_VX_MIN_ID)
}

#[cfg(test)]
mod tests {
    use super::missing_frontier_falsification_fields;

    #[test]
    fn non_frontier_rows_do_not_require_frontier_tuple() {
        assert!(missing_frontier_falsification_fields("VX-420", "benchmark").is_empty());
    }

    #[test]
    fn frontier_rows_require_full_falsification_tuple() {
        let missing = missing_frontier_falsification_fields("VX-421", "benchmark");
        assert!(missing.contains(&"comparator"));
        assert!(missing.contains(&"dataset"));
        assert!(missing.contains(&"metric"));
        assert!(missing.contains(&"floor"));
        assert!(missing.contains(&"failure-mode"));
        assert!(missing.contains(&"evidence-artifact-path"));
    }
}
