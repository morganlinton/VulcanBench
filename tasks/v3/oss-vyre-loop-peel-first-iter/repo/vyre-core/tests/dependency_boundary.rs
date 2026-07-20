//! Crate-boundary contracts for the public `vyre` facade.

fn manifest_dependencies() -> toml::map::Map<String, toml::Value> {
    let manifest = include_str!("../Cargo.toml");
    let parsed: toml::Value =
        toml::from_str(manifest).expect("Fix: vyre-core/Cargo.toml must remain parseable TOML");
    parsed
        .get("dependencies")
        .and_then(toml::Value::as_table)
        .expect("Fix: vyre-core/Cargo.toml must contain a dependencies table")
        .clone()
}

#[test]
fn vyre_core_has_no_concrete_gpu_runtime_dependencies() {
    let deps = manifest_dependencies();

    for forbidden in [
        "wgpu",
        "bytemuck",
        "pollster",
        "vyre-primitives",
        "vyre-libs",
        "vyre-intrinsics",
        "vyre-self-substrate",
    ] {
        assert!(
            !deps.contains_key(forbidden),
            "Fix: vyre-core must not depend directly on `{forbidden}`; concrete runtime crates belong in driver crates."
        );
    }
}

#[test]
fn vyre_core_lower_facade_uses_canonical_vyre_lower_crate() {
    let deps = manifest_dependencies();

    assert!(
        deps.contains_key("vyre-lower"),
        "Fix: vyre-core::lower must expose the canonical vyre-lower crate, not the legacy foundation lower module."
    );
    let _lower_fn: fn(&vyre::ir::Program) -> Result<vyre::lower::KernelDescriptor, _> =
        vyre::lower::lower;
}
