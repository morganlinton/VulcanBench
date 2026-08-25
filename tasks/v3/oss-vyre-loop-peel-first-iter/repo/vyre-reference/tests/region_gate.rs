//! Reference interpreter region-gate regression tests.

use vyre::ir::{BufferDecl, DataType, Expr, Node, Program};

#[allow(deprecated)]
fn raw_program() -> Program {
    Program::new(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![Node::store("out", Expr::u32(0), Expr::u32(7)), Node::Return],
    )
}

#[test]
fn reference_eval_rejects_non_region_programs() {
    let error = vyre_reference::reference_eval(
        &raw_program(),
        &[vyre_reference::value::Value::from(vec![0u8; 4])],
    )
    .expect_err("Fix: reference_eval must reject raw top-level statements");
    assert!(
        error
            .to_string()
            .contains("top-level Region-wrapped Program"),
        "Fix: reference_eval rejection must mention the region invariant, got: {error}"
    );
}

#[test]
fn flat_cpu_rejects_non_region_programs() {
    let mut output = Vec::new();
    let error = vyre_reference::flat_cpu::run_flat(&raw_program(), &[], &mut output)
        .expect_err("Fix: flat_cpu must reject raw top-level statements");
    assert!(
        error
            .to_string()
            .contains("top-level Region-wrapped Program"),
        "Fix: flat_cpu rejection must mention the region invariant, got: {error}"
    );
}
