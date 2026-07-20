//! Coverage for the new Sub/BitAnd/BitOr/BitXor identity rules in
//! the GPU pattern-match pass. Each test runs a Program through the
//! full persistent-resident pipeline and asserts the post-pipeline IR
//! has the expected collapsed form.

#![cfg(test)]

mod common;

use common::live_backend;
use vyre::ir::{BinOp, BufferAccess, BufferDecl, DataType, Expr, Node, Program};
use vyre_driver_cuda::CudaOptimizerDispatcher;
use vyre_self_substrate::optimizer::pipeline_resident::gpu_pipeline_resident;

/// Bind `x` to a non-literal value (`Load(input, 0)`) so const-prop
/// at the end of the pipeline can't fold `Var(x)` into a literal.
/// The `input` buffer is declared on the Program so the IR is
/// well-typed.
fn program_with_x_load_then(value: Expr) -> Program {
    Program::wrapped(
        vec![
            BufferDecl::storage("input", 0, BufferAccess::ReadOnly, DataType::U32).with_count(1),
            BufferDecl::storage("buf", 1, BufferAccess::ReadWrite, DataType::U32).with_count(1),
        ],
        [1, 1, 1],
        vec![
            Node::let_bind("x", Expr::load("input", Expr::u32(0))),
            Node::store("buf", Expr::u32(0), value),
        ],
    )
}

fn run_pipeline(p: Program) -> Program {
    let backend = live_backend();
    let dispatcher = CudaOptimizerDispatcher::new(&backend);
    gpu_pipeline_resident(p, &dispatcher).expect("pipeline must succeed")
}

fn body_of(out: &Program) -> Vec<Node> {
    match out.entry() {
        [Node::Region { body, .. }] => body.as_ref().clone(),
        entry => entry.to_vec(),
    }
}

fn binop(op: BinOp, left: Expr, right: Expr) -> Expr {
    Expr::BinOp {
        op,
        left: Box::new(left),
        right: Box::new(right),
    }
}


#[path = "self_optimizer_pattern_match_extended/arithmetic_identity_contracts.rs"]
mod arithmetic_identity_contracts;
#[path = "self_optimizer_pattern_match_extended/arithmetic_cse_contracts.rs"]
mod arithmetic_cse_contracts;
#[path = "self_optimizer_pattern_match_extended/bitwise_shift_contracts.rs"]
mod bitwise_shift_contracts;
#[path = "self_optimizer_pattern_match_extended/boolean_comparison_contracts.rs"]
mod boolean_comparison_contracts;
#[path = "self_optimizer_pattern_match_extended/self_cse_contracts.rs"]
mod self_cse_contracts;
#[path = "self_optimizer_pattern_match_extended/bitxor_chain_contracts.rs"]
mod bitxor_chain_contracts;
#[path = "self_optimizer_pattern_match_extended/minmax_contracts.rs"]
mod minmax_contracts;
