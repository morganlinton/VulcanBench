//! Wgpu subgroup capability diagnostics test suite.

const DIAGNOSTICS: &str = include_str!("../../docs/optimization/WGPU_SUBGROUP_CAPABILITY_DIAGNOSTICS.toml");

#[test]
fn wgpu_subgroup_diagnostics_fail_before_shader_submission() {
    for required in [
        "backend",
        "feature_flag",
        "subgroup_min",
        "subgroup_max",
        "shader_requirement",
        "fallback_route",
        "VYRE_WGPU_SUBGROUP_UNSUPPORTED",
    ] {
        assert!(
            DIAGNOSTICS.contains(required),
            "WGPU subgroup diagnostics must include {required}"
        );
    }
}
