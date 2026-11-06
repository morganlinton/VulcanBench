//! `vmlang`: a small expression language compiled to a flat instruction list
//! and executed on a stack machine.
//!
//! The crate is split in three. [`isa`] holds the shared contract (values,
//! the AST, the instruction set). [`compiler`] lowers an AST to instructions
//! and [`vm`] executes them. The compiler and the VM share no code beyond the
//! types in `isa`; they must agree on the instruction set exactly, including
//! how short-circuit `and`/`or` and `if` are lowered to forward jumps.

pub mod compiler;
pub mod isa;
pub mod vm;

pub use compiler::compile_expr;
pub use isa::{BinOp, Env, Expr, Instr, Value, VmError};
pub use vm::run;
