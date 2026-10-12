//! Failure-oriented tests for subgroup support detection in lowering.
//!
//! Pins that subgroup builtins are emitted only when the program
//! actually contains subgroup ops, and that capability reporting
//! matches the emitted shader.

use vyre::ir::{BinOp, BufferDecl, DataType, Expr, Node, Program};
use vyre::VyreBackend;
use vyre_driver_wgpu::emit;
use vyre_driver_wgpu::WgpuBackend;

fn lower_wgsl(program: &Program) -> String {
    emit::lower(program).expect("Fix: test program must lower to WGSL")
}

#[test]
fn subgroup_builtins_absent_when_program_has_no_subgroup_ops() {
    let program = Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32)],
        [1, 1, 1],
        vec![Node::store("out", Expr::u32(0), Expr::u32(1))],
    );
    let wgsl = lower_wgsl(&program);
    assert!(
        !wgsl.contains("subgroup"),
        "Fix: program without subgroup ops must not emit subgroup builtins or intrinsics. WGSL:\n{wgsl}"
    );
    assert!(
        !wgsl.contains("SubgroupInvocationId"),
        "Fix: subgroup builtin must not appear when unused. WGSL:\n{wgsl}"
    );
    assert!(
        !wgsl.contains("SubgroupSize"),
        "Fix: subgroup builtin must not appear when unused. WGSL:\n{wgsl}"
    );
}

#[test]
fn subgroup_builtins_present_when_program_uses_subgroup_ops() {
    let program = Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32)],
        [1, 1, 1],
        vec![Node::store(
            "out",
            Expr::u32(0),
            Expr::BinOp {
                op: BinOp::WaveReduce,
                left: Box::new(Expr::u32(7)),
                right: Box::new(Expr::u32(0)),
            },
        )],
    );
    let wgsl = lower_wgsl(&program);
    assert!(
        wgsl.contains("subgroup"),
        "Fix: program with WaveReduce must emit subgroup intrinsics. WGSL:\n{wgsl}"
    );
}

#[test]
fn subgroup_shuffle_lowers_to_subgroup_builtin() {
    let program = Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32)],
        [1, 1, 1],
        vec![Node::store(
            "out",
            Expr::u32(0),
            Expr::subgroup_shuffle(Expr::u32(1), Expr::u32(2)),
        )],
    );
    let wgsl = lower_wgsl(&program);
    assert!(
        wgsl.contains("subgroupShuffle"),
        "Fix: subgroup_shuffle must lower to subgroupShuffle intrinsic. WGSL:\n{wgsl}"
    );
}

#[test]
fn subgroup_local_id_builtin_present_when_used() {
    let program = Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32)],
        [1, 1, 1],
        vec![Node::store("out", Expr::u32(0), Expr::subgroup_local_id())],
    );
    let wgsl = lower_wgsl(&program);
    assert!(
        wgsl.contains("@builtin(subgroup_invocation_id)"),
        "Fix: subgroup_local_id() must cause subgroup_invocation_id builtin to be declared. WGSL:\n{wgsl}"
    );
}

#[test]
fn subgroup_size_matches_backend_report() {
    let backend = WgpuBackend::acquire().expect(
        "WgpuBackend::acquire failed on a machine that must have a GPU. \
         Fix: repair adapter/driver probing instead of skipping subgroup capability checks.",
    );

    let reported = backend.subgroup_size();
    let info = backend.adapter_info();

    if let Some(size) = reported {
        assert!(
            size > 0,
            "Fix: reported subgroup size must be positive when Some. Adapter: {}",
            info.name
        );
    }
}
