# Implement a bytecode compiler and a stack VM

The `vmlang` package compiles a small expression language to a flat list of
stack-machine instructions and executes it. The work is split across two files
and both must be implemented and kept consistent:

- `vmlang/compiler.py` — `compile_expr(ast)` lowers an AST to a flat instruction
  list.
- `vmlang/vm.py` — `run(code, env=None)` executes that instruction list and
  returns the result.

The two files share no code; their only coupling is the instruction set below.
They must agree on it exactly.

## AST shape

The AST is nested tuples. Nodes:

- `("lit", value)` — a literal (number, bool, or string).
- `("var", name)` — read a variable from `env`.
- `("not", a)` — logical negation.
- `(op, a, b)` where `op` is one of `+ - * / == < >` — a binary operation.
- `("and", a, b)`, `("or", a, b)` — short-circuit boolean operators.
- `("if", cond, then, else)` — choose a branch by `cond`'s truthiness.

## Instruction set (the shared contract)

Each instruction is a tuple `(OPCODE, *operands)`. `compile_expr` returns a flat
Python `list` of these (no nesting). The VM treats jump operands as **absolute
indices** into that list.

| Opcode | Operand | Stack effect |
| --- | --- | --- |
| `PUSH` | value | push `value` |
| `LOAD` | name | push `env[name]`; raise `NameError` if `name` is unbound |
| `ADD` `SUB` `MUL` `DIV` | — | pop `b`, pop `a`, push `a <op> b` |
| `EQ` `LT` `GT` | — | pop `b`, pop `a`, push the comparison (`a == b`, `a < b`, `a > b`) |
| `NOT` | — | pop `a`, push `not a` |
| `DUP` | — | duplicate the top of the stack |
| `POP` | — | discard the top of the stack |
| `JUMP` | target | set the instruction pointer to `target` |
| `JUMP_IF_FALSE` | target | pop `a`; if `a` is falsy, jump to `target` |

## Required behavior

- **Execution model.** `run` is a stack machine with an instruction pointer that
  advances by one each step except on jumps. A well-formed program leaves exactly
  one value on the stack; `run` returns it.
- **Short-circuit `and` / `or`.** These return an operand value, not a forced
  bool: `and` returns its left operand if that is falsy, otherwise its right
  operand; `or` returns its left operand if that is truthy, otherwise its right
  operand. The operand that is not selected **must not be executed** (so a
  division by zero or an unbound variable in the skipped branch never happens).
- **`if`.** Only the taken branch runs.
- **Forward jumps.** `and`, `or`, and `if` jump forward over code that has not
  been emitted yet, so the targets must be filled in (backpatched) once their
  length is known. Targets stay correct even when these forms nest.
