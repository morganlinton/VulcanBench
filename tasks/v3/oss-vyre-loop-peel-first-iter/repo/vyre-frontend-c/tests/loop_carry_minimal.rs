//! Carrier-mechanism smoke tests, smallest-first.

use vyre::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};
use vyre::DispatchConfig;

#[allow(unused_imports)]
use vyre_driver_wgpu as _;

mod loop_carry_minimal_part1 {

    include!("__split/loop_carry_minimal_part1.rs");
}
mod loop_carry_minimal_part2 {
    include!("__split/loop_carry_minimal_part2.rs");
}
mod loop_carry_minimal_part3 {
    include!("__split/loop_carry_minimal_part3.rs");
}
