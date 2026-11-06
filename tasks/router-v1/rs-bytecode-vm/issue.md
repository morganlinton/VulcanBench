# Implement the bytecode compiler and the stack VM

The `vmlang` crate compiles a small expression language to a flat list of
stack-machine instructions and executes it. The shared contract is already
written in `src/isa.rs` (the `Value`, `Expr`, `Instr`, `Env` and `VmError`
types and their documentation). The two halves that use it are stubs that
`todo!()`, and both must be implemented and kept consistent:

- `src/compiler.rs`: `compile_expr(&Expr) -> Vec<Instr>` lowers an AST to a
  flat instruction list.
- `src/vm.rs`: `run(&[Instr], &Env) -> Result<Value, VmError>` executes that
  list and returns the result.

The two files share no code; their only coupling is the instruction set in
`isa.rs`. They must agree on it exactly. Do not change `isa.rs`.

## AST

`Expr` nodes: `Lit(Value)`, `Var(name)`, `Not(a)`, `Bin(op, a, b)` with
`op` one of `Add Sub Mul Div Eq Lt Gt`, the short-circuit `And(a, b)` and
`Or(a, b)`, and `If(cond, then, else)`. `Value` is `Num(f64)`, `Bool` or
`Str`; `Value::truthy` gives truthiness (zero, `false` and the empty string
are falsy).

## Instruction set (the shared contract)

`compile_expr` returns one flat `Vec<Instr>` (no nesting). The VM treats jump
operands as **absolute indices** into that list.

| Instr | Stack effect |
| --- | --- |
| `Push(v)` | push `v` |
| `Load(name)` | push `env[name]`; `Err(UnboundVariable(name))` if unbound |
| `Add` `Sub` `Mul` `Div` | pop `b`, pop `a`, push `a <op> b`; `Div` by zero is `Err(DivisionByZero)`; non-numbers are `Err(TypeError(_))` |
| `Eq` | pop `b`, pop `a`, push `Bool(a == b)` (any values) |
| `Lt` `Gt` | pop `b`, pop `a`, push the numeric comparison as `Bool` |
| `Not` | pop `a`, push `Bool(!a.truthy())` |
| `Dup` | duplicate the top of the stack |
| `Pop` | discard the top of the stack |
| `Jump(t)` | set the instruction pointer to `t` |
| `JumpIfFalse(t)` | pop `a`; if `a` is falsy, set the instruction pointer to `t` |

## Required behaviour

- **Execution model.** `run` is a stack machine with an instruction pointer
  that advances by one each step except on jumps. A well-formed program
  leaves exactly one value on the stack and `run` returns it; any other
  final stack is `Err(BadFinalStack(n))`. Popping an empty stack is
  `Err(StackUnderflow)`; a jump past the end of the program is
  `Err(BadJump(t))` (a target equal to the length ends the program).
- **Short-circuit `And` / `Or`.** These return an operand value, not a
  forced `Bool`: `And` returns its left operand if that is falsy, otherwise
  its right operand; `Or` returns its left operand if that is truthy,
  otherwise its right operand. The operand that is not selected **must not
  be executed**, so a division by zero or an unbound variable in the skipped
  operand never happens.
- **`If`.** Only the taken branch runs.
- **Forward jumps.** `And`, `Or` and `If` jump forward over code that has not
  been emitted yet, so the targets must be filled in (backpatched) once their
  length is known. Targets stay correct when these forms nest, including a
  short-circuit expression used as an `If` condition.

Run `cargo test --offline` to check your work; the crate has no dependencies.
