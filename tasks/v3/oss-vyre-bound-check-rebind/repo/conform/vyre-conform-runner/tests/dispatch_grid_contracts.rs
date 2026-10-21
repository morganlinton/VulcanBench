//! Contract tests for `dispatch_grid` workgroup configuration validation.
use vyre::ir::{BufferDecl, DataType, Program};
use vyre_conform_runner::dispatch_grid;

#[test]
fn rejects_zero_workgroup_dimension_before_backend_dispatch() {
    let program = Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
        [0, 1, 1],
        Vec::new(),
    );

    let error = dispatch_grid::config_for_program(&program)
        .expect_err("Fix: zero workgroup dimensions must fail before backend dispatch");
    assert!(
        error.contains("workgroup_size[0] is 0"),
        "Fix: zero-dimension dispatch-grid errors must identify the invalid axis. Got: {error}"
    );
}

#[test]
fn non_1d_single_workgroup_fixture_gets_explicit_grid() {
    let program = Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(4)],
        [16, 16, 1],
        Vec::new(),
    );

    let config = dispatch_grid::config_for_program(&program)
        .expect("Fix: small non-1D conform fixtures must receive an explicit grid_override");
    assert_eq!(
        config.grid_override,
        Some([1, 1, 1]),
        "Fix: non-1D conform fixtures need an explicit one-workgroup grid"
    );
}

#[test]
fn rejects_zero_workgroup_y_dimension() {
    let program = Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
        [64, 0, 1],
        Vec::new(),
    );
    let error = dispatch_grid::config_for_program(&program)
        .expect_err("Fix: zero Y workgroup dimension must be rejected.");
    assert!(
        error.contains("workgroup_size[1] is 0"),
        "Fix: error must identify axis 1, got: {error}"
    );
}

#[test]
fn rejects_3d_workgroup_exceeding_writable_count() {
    // 2×2×2 = 8 lanes, but output has 100 elements → cannot cover in one workgroup
    let program = Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(100)],
        [2, 2, 2],
        Vec::new(),
    );
    let error = dispatch_grid::config_for_program(&program)
        .expect_err("Fix: non-1D workgroup too small for output must be rejected.");
    assert!(
        error.contains("non-1D workgroup_size"),
        "Fix: error must explain the non-1D lane shortage, got: {error}"
    );
}

#[test]
fn accepts_1d_workgroup_any_output_size() {
    // 1D workgroup → the wgpu backend computes the grid from output length
    let program = Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(10000)],
        [64, 1, 1],
        Vec::new(),
    );
    let config = dispatch_grid::config_for_program(&program)
        .expect("Fix: 1D workgroup must always succeed regardless of output size.");
    assert!(config.grid_override.is_none());
}

#[test]
fn accepts_minimal_1x1x1_workgroup() {
    let program = Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        Vec::new(),
    );
    dispatch_grid::config_for_program(&program)
        .expect("Fix: minimal 1×1×1 workgroup must succeed.");
}

#[test]
fn read_only_buffers_do_not_count_as_writable() {
    // Non-1D workgroup, but only read-only buffers → max_writable_count = 1
    let program = Program::wrapped(
        vec![BufferDecl::read("input", 0, DataType::U32).with_count(1024)],
        [4, 4, 4],
        Vec::new(),
    );
    dispatch_grid::config_for_program(&program)
        .expect("Fix: read-only buffers should not count toward writable lane requirements.");
}

#[test]
fn accepts_3d_exact_fit() {
    // 8×8×1 = 64 lanes, output has exactly 64 elements → fits
    let program = Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(64)],
        [8, 8, 1],
        Vec::new(),
    );
    let config = dispatch_grid::config_for_program(&program)
        .expect("Fix: 3D workgroup with exact lane count must succeed.");
    assert_eq!(config.grid_override, Some([1, 1, 1]));
}

#[test]
fn accepts_large_1d_workgroup() {
    let program = Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
        [256, 1, 1],
        Vec::new(),
    );
    let config =
        dispatch_grid::config_for_program(&program).expect("Fix: large 1D workgroup must succeed.");
    assert!(config.grid_override.is_none());
}
