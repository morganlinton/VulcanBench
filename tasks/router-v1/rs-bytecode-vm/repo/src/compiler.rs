//! Lower an [`Expr`] to a flat `Vec<Instr>`.
//!
//! The output is one flat list: no nesting, no labels. Jump operands are
//! absolute indices into that list. `and`, `or` and `if` jump forward over
//! code that has not been emitted yet, so their targets are patched in once
//! the skipped code's length is known (backpatching). Targets must stay
//! correct when these forms nest.

use crate::isa::{Expr, Instr};

/// Return a flat list of instructions that computes `expr` on the VM stack.
pub fn compile_expr(expr: &Expr) -> Vec<Instr> {
    let _ = expr;
    todo!("lower the AST to the instruction set in `isa`")
}
