//! Execute a flat instruction list on a stack machine.
//!
//! The instruction pointer advances by one each step except on jumps, whose
//! operands are absolute indices into the code. A well-formed program leaves
//! exactly one value on the stack; `run` returns it.

use crate::isa::{Env, Instr, Value, VmError};

/// Execute `code` against the bindings in `env` and return the result.
pub fn run(code: &[Instr], env: &Env) -> Result<Value, VmError> {
    let _ = (code, env);
    todo!("execute the instruction set in `isa` on a value stack")
}
