//! Metal icb dispatch replay test suite.

const REPLAY: &str = include_str!("../../docs/optimization/METAL_ICB_DISPATCH_REPLAY.toml");

#[test]
fn metal_icb_dispatch_replay_records_submit_gpu_and_output_parity() {
    for required in [
        "direct_command_encoding",
        "icb_reuse",
        "descriptor_digest",
        "command_reuse_evidence",
        "cpu_submit_ns",
        "gpu_ns",
        "output_digest",
    ] {
        assert!(
            REPLAY.contains(required),
            "Metal ICB replay registry must include {required}"
        );
    }
}
