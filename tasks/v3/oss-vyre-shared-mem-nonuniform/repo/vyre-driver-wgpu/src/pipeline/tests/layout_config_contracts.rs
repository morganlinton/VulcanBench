use super::*;

#[test]
fn hex_short_truncates_to_eight_bytes() {
    let hash = *blake3::hash(b"vyre-pipeline").as_bytes();
    let expected = vyre_driver::pipeline::hex_encode(&hash[..8]);
    assert_eq!(vyre_driver::pipeline::hex_short(&hash).len(), 16);
    assert_eq!(vyre_driver::pipeline::hex_short(&hash), expected);
}

#[test]
fn actual_output_budget_rejects_combined_outputs() {
    let mut config = DispatchConfig::default();
    config.max_output_bytes = Some(3);
    let err = enforce_actual_output_budget(&config, &[vec![0; 2], vec![0; 2]])
        .expect_err("combined readback over budget must fail");
    assert!(
        err.to_string().contains("max_output_bytes"),
        "Fix: budget rejection must name the violated policy, got {err}"
    );
}

#[test]
fn output_layout_matches_trimmed_execution_plan() {
    let program = Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32)
            .with_count(1024)
            .with_output_byte_range(4..12)],
        [1, 1, 1],
        vec![Node::store("out", Expr::u32(0), Expr::u32(7))],
    );
    let plan = execution_plan::plan(&program)
        .expect("Fix: trimmed output program must plan; restore this invariant before continuing.");
    assert_eq!(
        plan.strategy.readback,
        ReadbackStrategy::Trimmed {
            visible_bytes: 8,
            avoided_bytes: 4088,
        }
    );
    let layouts = vyre_driver::program_walks::output_binding_layouts(&program)
        .expect("Fix: layout must derive; restore this invariant before continuing.");
    assert_eq!(layouts[0].layout.read_size, 8);
    assert_eq!(layouts[0].layout.copy_size, 8);
}

#[test]
fn wgpu_compile_config_receives_natural_gradient_workgroup_before_lowering() {
    let program = Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(4096)],
        [32, 1, 1],
        vec![Node::store("out", Expr::u32(0), Expr::u32(7))],
    );
    let limits = LaunchGeometryLimits {
        backend: "wgpu-test",
        max_threads_per_block: 1024,
        max_block_dim: [1024, 1024, 64],
        max_grid_dim: [u32::MAX, u32::MAX, u32::MAX],
    };

    let effective = super::wgpu_effective_dispatch_config_for_limits(
        &program,
        &DispatchConfig::default(),
        limits,
        Mode::NaturalGradient,
    )
    .expect("Fix: WGPU natural-gradient config derivation must be pure and valid");

    assert_eq!(
        effective.workgroup_override,
        Some([1024, 1, 1]),
        "Fix: WGPU lowering config must include the natural-gradient workgroup so WGSL @workgroup_size and dispatch metadata agree."
    );
}

#[test]
fn wgpu_natural_gradient_compile_config_preserves_semantic_safety_gates() {
    let program = Program::wrapped(
        vec![
            BufferDecl::output("out", 0, DataType::U32).with_count(4096),
            BufferDecl::workgroup("scratch", 64, DataType::U32).with_kind(MemoryKind::Shared),
        ],
        [64, 1, 1],
        vec![Node::store("out", Expr::u32(0), Expr::u32(7))],
    );
    let limits = LaunchGeometryLimits {
        backend: "wgpu-test",
        max_threads_per_block: 1024,
        max_block_dim: [1024, 1024, 64],
        max_grid_dim: [u32::MAX, u32::MAX, u32::MAX],
    };
    let mut explicit = DispatchConfig::default();
    explicit.workgroup_override = Some([256, 1, 1]);

    let explicit_effective = super::wgpu_effective_dispatch_config_for_limits(
        &program,
        &explicit,
        limits,
        Mode::NaturalGradient,
    )
    .expect("Fix: explicit WGPU workgroup override must stay valid");
    assert_eq!(explicit_effective.workgroup_override, Some([256, 1, 1]));

    let shared_effective = super::wgpu_effective_dispatch_config_for_limits(
        &program,
        &DispatchConfig::default(),
        limits,
        Mode::NaturalGradient,
    )
    .expect("Fix: shared-memory WGPU config should remain valid without autotuning");
    assert_eq!(
        shared_effective.workgroup_override, None,
        "Fix: workgroup-local scratch kernels must keep the Program-declared WGPU workgroup."
    );
}

#[test]
fn pipeline_production_uses_fallible_binding_and_trap_staging() {
    let production = include_str!("../../pipeline.rs")
        .split("\n#[cfg(test)]\nmod tests")
        .next()
        .expect("Fix: pipeline production section should precede tests");

    assert!(
        !production.contains("with_capacity_and_hasher"),
        "Fix: WGPU pipeline binding classification must not use infallible hash-set constructors."
    );
    assert!(
        !production.contains("Vec::with_capacity(trap_sidecar_bytes)"),
        "Fix: WGPU trap sidecar readback must not allocate infallibly."
    );
    assert!(
        production.contains("reserve_hash_set_to_capacity"),
        "Fix: WGPU pipeline binding classification should use the shared fallible hash-set reservation helper."
    );
    assert!(
        production.contains(
            "reserve_backend_vec(&mut bytes, trap_sidecar_bytes, \"trap sidecar readback\")?"
        ),
        "Fix: WGPU trap sidecar readback should reserve through the shared staging helper."
    );
}
