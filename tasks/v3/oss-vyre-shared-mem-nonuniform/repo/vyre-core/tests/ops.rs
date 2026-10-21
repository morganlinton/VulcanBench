//! Sanity check that the `vyre` crate re-exports the minimal IR surface
//! downstream ops crates depend on. Failing here means `vyre-ops` or
//! `vyre-libs` would not compile.

use vyre::ir::{BufferDecl, DataType, Expr, Node, Program};

#[test]
fn reexports_minimal_ir_surface() {
    let program = Program::wrapped(
        vec![BufferDecl::read_write("out", 0, DataType::U32)],
        [1, 1, 1],
        vec![Node::store("out", Expr::u32(0), Expr::u32(42))],
    );
    assert_eq!(program.buffers().len(), 1);
    assert_eq!(program.workgroup_size(), [1, 1, 1]);
}
