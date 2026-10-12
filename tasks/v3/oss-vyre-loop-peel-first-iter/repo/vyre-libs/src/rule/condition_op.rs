// Shared builders for rule condition operations.

use vyre_foundation::ir::{BufferDecl, DataType, Expr, Node, Program};
use vyre_spec::AlgebraicLaw;

/// Rule condition operation input types.
pub const INPUTS: &[DataType] = &[
    DataType::U32,
    DataType::U32,
    DataType::U32,
    DataType::U32,
    DataType::U32,
    DataType::U32,
];

/// Rule condition operation output types.
pub const OUTPUTS: &[DataType] = &[DataType::U32];

/// Rule condition operation laws.
pub const LAWS: &[AlgebraicLaw] = &[AlgebraicLaw::Bounded { lo: 0, hi: 1 }];

/// Stable workgroup size for scalar rule leaf operations.
pub const WORKGROUP_SIZE: [u32; 3] = [1, 1, 1];

/// Build a scalar condition program, wrapped in a stable-id Region so
/// the optimizer and the universal region-chain discipline test see
/// each rule leaf as an atomic unit.
#[must_use]
pub fn condition_program(op_id: &'static str, compute: fn() -> Expr) -> Program {
    Program::wrapped(
        vec![
            BufferDecl::read("rule_id", 0, DataType::U32),
            BufferDecl::read("pattern_id", 1, DataType::U32),
            BufferDecl::read("pattern_state", 2, DataType::U32),
            BufferDecl::read("pattern_count", 3, DataType::U32),
            BufferDecl::read("file_size", 4, DataType::U32),
            BufferDecl::read("threshold", 5, DataType::U32),
            BufferDecl::output("out", 6, DataType::U32),
        ],
        WORKGROUP_SIZE,
        vec![crate::region::wrap_anonymous(
            op_id,
            vec![Node::store("out", Expr::u32(0), compute())],
        )],
    )
}

/// Read the pattern state argument.
#[must_use]
pub fn pattern_state() -> Expr {
    Expr::load("pattern_state", Expr::u32(0))
}

/// Read the pattern count argument.
#[must_use]
pub fn pattern_count() -> Expr {
    Expr::load("pattern_count", Expr::u32(0))
}

/// Read the file size argument.
#[must_use]
pub fn file_size() -> Expr {
    Expr::load("file_size", Expr::u32(0))
}

/// Read the threshold argument.
#[must_use]
pub fn threshold() -> Expr {
    Expr::load("threshold", Expr::u32(0))
}
