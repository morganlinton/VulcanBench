//! Workgroup-size naming contracts.

#[test]
fn default_workgroup_constant_is_explicitly_1d() {
    assert_eq!(
        vyre_driver::pipeline::DEFAULT_1D_WORKGROUP_SIZE,
        [64, 1, 1],
        "Fix: the shared driver default must be explicitly named as a 1D fallback, not a universal workgroup shape."
    );
}
