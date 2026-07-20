//! Dispatch grid shape contracts for non-1D workgroups.

mod common;
use common::acquire_live_backend as live_backend;

use vyre::ir::{BufferDecl, DataType, Expr, Node, Program};
use vyre::{DispatchConfig, VyreBackend};

fn two_dimensional_program() -> Program {
    Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32)
            .with_count(64)
            .with_output_byte_range(0..256)],
        [8, 8, 1],
        vec![Node::store("out", Expr::gid_x(), Expr::u32(7))],
    )
}

#[test]
fn non_1d_workgroup_without_grid_override_fails_loudly() {
    let backend = live_backend();
    let err = backend
        .dispatch(&two_dimensional_program(), &[], &DispatchConfig::default())
        .expect_err("2D workgroups need an explicit logical grid");
    let msg = err.to_string();
    assert!(
        msg.contains("grid_override") && msg.contains("Fix:"),
        "error must explain the missing explicit grid override: {msg}"
    );
}

#[test]
fn non_1d_workgroup_with_grid_override_dispatches() {
    let backend = live_backend();
    let mut config = DispatchConfig::default();
    config.grid_override = Some([1, 1, 1]);
    let outputs = backend
        .dispatch(&two_dimensional_program(), &[], &config)
        .expect("explicit grid_override must make the non-1D dispatch unambiguous");
    assert_eq!(outputs.len(), 1);
    assert_eq!(outputs[0].len(), 256);
}
