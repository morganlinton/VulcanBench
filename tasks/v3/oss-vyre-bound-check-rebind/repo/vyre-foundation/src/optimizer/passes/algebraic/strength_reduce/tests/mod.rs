use super::*;
use crate::ir::{BufferDecl, DataType, Expr, Node};
use crate::optimizer::passes::const_fold::ConstFold;
use crate::optimizer::{PassScheduler, ProgramPassKind};

#[test]
fn analyze_skips_program_with_no_expression_bearing_nodes() {
    let program = crate::ir::Program::wrapped(Vec::new(), [1, 1, 1], vec![Node::Return]);
    match crate::optimizer::ProgramPass::analyze(&StrengthReduce, &program) {
        PassAnalysis::SKIP => {}
        other => panic!("expected SKIP for expression-free program, got {other:?}"),
    }
}

mod complement_bounds;
mod float_division;
mod modulo_constant;
mod reciprocal;
mod scheduler_smoke;
mod self_inverse_select;
mod shift_add_horner;
mod shift_negation_fma;
