//! Shared helpers for pipeline-cache child-module tests. Kept tiny so
//! every sibling can build a deterministic Program / unique temp name
//! without duplicating boilerplate.

use vyre_foundation::ir::{BufferDecl, DataType, Expr, Node, Program};

pub(in crate::pipeline_cache) fn tiny_program() -> Program {
    Program::wrapped(
        vec![BufferDecl::read_write("out", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![Node::store("out", Expr::u32(0), Expr::u32(42))],
    )
}

pub(in crate::pipeline_cache) fn unique_u64() -> u64 {
    match std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
        Ok(duration) => duration.as_nanos() as u64,
        Err(_) => 0,
    }
}
