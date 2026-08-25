//! Shared backend launch validation contracts.

use vyre_driver::{BackendError, DispatchConfig, VyreBackend};
use vyre_foundation::ir::{BufferDecl, DataType, Node, Program};

struct GridLimitBackend;

impl vyre_driver::backend::private::Sealed for GridLimitBackend {}

impl VyreBackend for GridLimitBackend {
    fn id(&self) -> &'static str {
        "grid-limit-test"
    }

    fn dispatch(
        &self,
        _program: &Program,
        _inputs: &[Vec<u8>],
        _config: &DispatchConfig,
    ) -> Result<Vec<Vec<u8>>, BackendError> {
        Ok(Vec::new())
    }

    fn max_workgroup_size(&self) -> [u32; 3] {
        [256, 256, 64]
    }

    fn max_compute_invocations_per_workgroup(&self) -> u32 {
        256
    }

    fn max_compute_workgroups_per_dimension(&self) -> u32 {
        7
    }
}

fn tiny_program() -> Program {
    Program::wrapped(Vec::new(), [1, 1, 1], vec![Node::Return])
}

#[test]
fn validate_program_for_backend_rejects_grid_override_past_backend_axis_limit() {
    let backend = GridLimitBackend;
    let program = tiny_program();

    for (axis, grid) in [(0, [8, 1, 1]), (1, [1, 8, 1]), (2, [1, 1, 8])] {
        let mut config = DispatchConfig::default();
        config.grid_override = Some(grid);

        let err = vyre_driver::validate_program_for_backend(&backend, &program, &config)
            .expect_err("Fix: grid_override above the backend per-dimension limit must fail.");
        let msg = err.to_string();
        assert!(
            msg.contains("Fix:"),
            "backend validation errors must remain actionable; got: {msg}"
        );
        assert!(
            msg.contains(&format!("axis {axis}")),
            "grid validation must identify the failing axis; got: {msg}"
        );
        assert!(
            msg.contains("max is 7"),
            "grid validation must include the backend limit; got: {msg}"
        );
    }
}

#[test]
fn validate_program_for_backend_accepts_grid_override_at_backend_axis_limit() {
    let backend = GridLimitBackend;
    let program = tiny_program();
    let mut config = DispatchConfig::default();
    config.grid_override = Some([7, 7, 7]);

    vyre_driver::validate_program_for_backend(&backend, &program, &config)
        .expect("Fix: grid_override equal to the backend per-dimension limit must be valid.");
}

#[test]
fn validate_program_for_backend_rejects_zero_grid_override_dimension() {
    let backend = GridLimitBackend;
    let program = tiny_program();
    let mut config = DispatchConfig::default();
    config.grid_override = Some([1, 0, 1]);

    let err = vyre_driver::validate_program_for_backend(&backend, &program, &config)
        .expect_err("Fix: zero grid_override dimensions must fail before backend dispatch.");
    let msg = err.to_string();
    assert!(
        msg.contains("Fix:") && msg.contains("zero-sized grid dimensions"),
        "zero-grid validation must be actionable; got: {msg}"
    );
}

#[test]
fn validate_launch_geometry_rejects_per_axis_block_overflow() {
    let err = vyre_driver::validation::validate_launch_geometry(
        [1, 1, 65],
        [1, 1, 1],
        vyre_driver::validation::LaunchGeometryLimits {
            backend: "test",
            max_threads_per_block: 256,
            max_block_dim: [256, 256, 64],
            max_grid_dim: [1024, 1024, 1024],
        },
    )
    .expect_err("Fix: per-axis block overflow must fail even when total threads are legal.");
    let msg = err.to_string();
    assert!(
        msg.contains("axis 2") && msg.contains("max is 64"),
        "per-axis launch validation must identify the failed axis and limit; got: {msg}"
    );
}

#[test]
fn launch_plan_prepares_geometry_and_param_words_once() {
    let program = Program::wrapped(
        vec![
            BufferDecl::read("input", 0, DataType::U32).with_count(1_000),
            BufferDecl::output("out", 1, DataType::U32).with_count(1_000),
        ],
        [128, 1, 1],
        vec![Node::Return],
    );
    let bindings = vyre_driver::BindingPlan::build(&program)
        .expect("Fix: shared launch-plan test program must build a binding plan.");
    let mut config = DispatchConfig::default();
    config.workgroup_override = Some([128, 1, 1]);
    let launch = vyre_driver::LaunchPlan::from_bindings(
        &program,
        &bindings.bindings,
        &config,
        vyre_driver::validation::LaunchGeometryLimits {
            backend: "test",
            max_threads_per_block: 256,
            max_block_dim: [256, 256, 64],
            max_grid_dim: [1024, 1024, 1024],
        },
    )
    .expect("Fix: shared launch plan must infer legal geometry.");

    assert_eq!(launch.element_count, 1_000);
    assert_eq!(launch.workgroup, [128, 1, 1]);
    assert_eq!(launch.grid, [8, 1, 1]);
    assert_eq!(launch.param_words[0], 1_000);
    assert_eq!(launch.param_words[1], 1_000);
    assert_eq!(launch.param_words[2], 1_000);
}

#[test]
fn launch_plan_rejects_zero_grid_override_before_driver_entry() {
    let program = Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(4)],
        [64, 1, 1],
        vec![Node::Return],
    );
    let bindings = vyre_driver::BindingPlan::build(&program)
        .expect("Fix: shared launch-plan test program must build a binding plan.");
    let mut config = DispatchConfig::default();
    config.grid_override = Some([1, 0, 1]);

    let err = vyre_driver::LaunchPlan::from_bindings(
        &program,
        &bindings.bindings,
        &config,
        vyre_driver::validation::LaunchGeometryLimits {
            backend: "test",
            max_threads_per_block: 256,
            max_block_dim: [256, 256, 64],
            max_grid_dim: [1024, 1024, 1024],
        },
    )
    .expect_err("Fix: shared launch preparation must reject zero grid overrides.");
    assert!(
        err.to_string().contains("non-zero"),
        "Fix: shared launch preparation must return actionable geometry errors; got: {err}"
    );
}
