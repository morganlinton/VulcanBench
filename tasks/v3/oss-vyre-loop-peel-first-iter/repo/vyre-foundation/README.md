# vyre-foundation

The hardware-neutral compiler foundation for vyre: IR (`Expr`, `Node`,
`Program`), type system, memory model, wire format, visitor traits, and
extension resolvers. Every other vyre crate depends on this one; this
crate depends only on `vyre-spec`, `vyre-macros`, and lightweight
third-party data crates. It does not know what `naga`, `wgpu`, a
dialect, or a backend is.

## Usage

```rust
use vyre_foundation::ir::{BufferDecl, DataType, Expr, Node, Program};

let program = Program::wrapped(
    vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
    [1, 1, 1],
    vec![
        Node::store("out", Expr::u32(0), Expr::u32(42)),
        Node::Return,
    ],
);

let wire = program.to_wire().unwrap();
assert_eq!(Program::from_wire(&wire).unwrap(), program);
```

## Architecture

See [`COMPUTE_2_0.md`](../.internals/planning/COMPUTE_2_0.md) for the
layer DAG. Foundation sits at the bottom; every migration target
listed there: foundation-ir, foundation-visit, foundation-wire: is
contained in this crate until the per-subcrate split lands in a later
phase.
