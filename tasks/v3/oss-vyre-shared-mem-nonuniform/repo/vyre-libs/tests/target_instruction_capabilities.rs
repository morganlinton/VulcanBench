//! Target instruction capabilities test suite.

const CAPABILITIES: &str = include_str!("../../docs/optimization/TARGET_INSTRUCTION_CAPABILITIES.toml");

#[test]
fn target_instruction_capabilities_gate_emitter_instruction_selection() {
    for required in [
        "ptx",
        "wgsl",
        "spirv",
        "msl",
        "atomics",
        "subgroup_ops",
        "memory_ordering",
        "async_copies",
        "tensor_ops",
        "scan_intrinsics",
        "fallback_instruction",
        "VYRE_WGSL_SUBGROUP_CAPABILITY_MISSING",
    ] {
        assert!(
            CAPABILITIES.contains(required),
            "target instruction capability registry must include {required}"
        );
    }
}
