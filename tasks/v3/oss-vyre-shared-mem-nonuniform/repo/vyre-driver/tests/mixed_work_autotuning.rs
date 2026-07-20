//! Mixed work autotuning test suite.

const AUTOTUNING: &str = include_str!("../../docs/optimization/MIXED_WORK_AUTOTUNING.toml");

#[test]
fn mixed_work_autotuning_records_queue_inputs_and_fairness() {
    for required in [
        "scan_density",
        "parser_edit_size",
        "frontier_size",
        "flow_delta_size",
        "queue_weights",
        "starvation_count",
        "route_reason",
        "output_parity_digest",
    ] {
        assert!(
            AUTOTUNING.contains(required),
            "mixed-work autotuning registry must include {required}"
        );
    }
}
