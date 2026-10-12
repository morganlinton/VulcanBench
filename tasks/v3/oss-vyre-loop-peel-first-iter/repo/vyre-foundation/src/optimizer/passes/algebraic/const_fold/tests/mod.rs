//! Const-fold test suite  -  split per audit cleanup A13 (2026-04-30) so
//! no single file exceeds the 1000-LOC hygiene cap. Original `tests.rs`
//! (1340 LOC, 108 tests) split into 4 per-section files + a shared
//! helpers module.

mod helpers;

mod binop_identity;
mod early;
mod structural;
mod unary;

#[test]
fn analyze_skips_program_with_no_expression_bearing_nodes() {
    use crate::ir::{Node, Program};
    use crate::optimizer::passes::algebraic::const_fold::ConstFold;
    use crate::optimizer::PassAnalysis;

    let program = Program::wrapped(Vec::new(), [1, 1, 1], vec![Node::Return]);
    match crate::optimizer::ProgramPass::analyze(&ConstFold, &program) {
        PassAnalysis::SKIP => {}
        other => panic!("expected SKIP for expression-free program, got {other:?}"),
    }
}
