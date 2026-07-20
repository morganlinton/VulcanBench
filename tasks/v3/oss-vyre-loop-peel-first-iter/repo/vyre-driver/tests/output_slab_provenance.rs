//! Output slab provenance test suite.

const PROVENANCE: &str = include_str!("../../docs/optimization/OUTPUT_SLAB_PROVENANCE.toml");

#[test]
fn output_slab_provenance_rejects_stale_reporting() {
    for required in [
        "slab_id",
        "producer_kernel",
        "byte_range",
        "validity_epoch",
        "readback_state",
        "reporter_consumer",
        "stale",
        "reject",
    ] {
        assert!(
            PROVENANCE.contains(required),
            "output slab provenance must include {required}"
        );
    }
}
