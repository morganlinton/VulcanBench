//! Failure-oriented tests for validator gaps not covered by other suites.
//!
//! Each test constructs a single malformed program and asserts that the
//! validator emits exactly the expected contract-error message. No silent
//! fake paths are allowed.

use vyre_foundation::ir::{AtomicOp, BinOp, BufferDecl, DataType, Expr, Node, Program, UnOp};
use vyre_foundation::validate::validate;
use vyre_foundation::MemoryOrdering;

fn output_program(nodes: Vec<Node>) -> Program {
    Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(4)],
        [1, 1, 1],
        nodes,
    )
}

mod validation_contract_gaps_part1 {

    include!("__split/validation_contract_gaps_part1.rs");
}
mod validation_contract_gaps_part2 {
    include!("__split/validation_contract_gaps_part2.rs");
}
