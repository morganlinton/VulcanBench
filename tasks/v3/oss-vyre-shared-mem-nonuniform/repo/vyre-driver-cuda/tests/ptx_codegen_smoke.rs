//! PTX codegen smoke tests  -  validate emitted PTX structure without GPU hardware.

use vyre_driver::DispatchConfig;
use vyre_driver_cuda::codegen::{
    program_to_ptx, program_to_ptx_for_sm, program_to_ptx_for_sm_and_subgroup,
};
use vyre_foundation::ir::{BinOp, BufferDecl, DataType, Expr, Node, Program, UnOp};

fn default_config() -> DispatchConfig {
    DispatchConfig::default()
}

fn identity_program() -> Program {
    Program::wrapped(
        vec![
            BufferDecl::read("input", 0, DataType::U32).with_count(4),
            BufferDecl::output("output", 1, DataType::U32).with_count(4),
        ],
        [64, 1, 1],
        vec![Node::store(
            "output",
            Expr::gid_x(),
            Expr::load("input", Expr::gid_x()),
        )],
    )
}

mod ptx_codegen_smoke_part1 {

    include!("__split/ptx_codegen_smoke_part1.rs");
}
mod ptx_codegen_smoke_part2 {
    include!("__split/ptx_codegen_smoke_part2.rs");
}
mod ptx_codegen_smoke_part3 {
    include!("__split/ptx_codegen_smoke_part3.rs");
}
