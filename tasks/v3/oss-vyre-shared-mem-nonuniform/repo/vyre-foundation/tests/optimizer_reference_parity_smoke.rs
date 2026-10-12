//! P0 inventory #34 (seed)  -  `optimizer::pre_lowering::optimize` must preserve
//! reference semantics for programs in this test corpus.
//!
//! This is a smoke oracle; property coverage for idempotence and wire stability
//! lives in `optimizer_idempotence_proptest.rs` (inventories #34–#35, #109).

use vyre_foundation::ir::{BufferDecl, DataType, Expr, Node, Program};
use vyre_foundation::optimizer::pre_lowering as optimize;
use vyre_reference::value::Value;

fn output_only_store(expr: Expr) -> Program {
    Program::wrapped(
        vec![BufferDecl::read_write("out", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![Node::store("out", Expr::u32(0), expr)],
    )
}

#[test]
fn optimize_preserves_reference_result_for_arithmetic_store() {
    // Enough surface for canonicalize, const fold, and CSE/DCE to do real work.
    let program = output_only_store(Expr::add(
        Expr::mul(Expr::u32(3), Expr::u32(4)),
        Expr::sub(Expr::u32(10), Expr::u32(2)),
    ));
    let reference_inputs = [Value::U32(0)];
    let base = vyre_reference::reference_eval(&program, &reference_inputs)
        .expect("Fix: unoptimized program must execute on the reference interpreter");

    let optimized = optimize::optimize(program.clone());
    let opt = vyre_reference::reference_eval(&optimized, &reference_inputs)
        .expect("Fix: optimized program must execute on the reference interpreter");

    assert_eq!(
        base, opt,
        "Fix: optimizer::pre_lowering::optimize changed observable reference semantics. Inventory P0 #34."
    );
}
