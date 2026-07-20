# vyre

GPU compute intermediate representation with a proven standard operation library.

## What this crate does

vyre is the compiler stack for GPU compute. Construct `ir::Program` values with
the IR builder, compose operations from the standard library, validate, and
dispatch through a registered backend.

## Install

```sh
cargo add vyre
```

## Quick example

The snippet below builds a small program and validates it. Dispatch lives behind
the `vyre-driver` registry; `vyre` itself stays substrate-neutral and does not
depend on any concrete GPU runtime.

```rust
use vyre::ir::*;

let program = Program::wrapped(
    vec![
        BufferDecl::read("a", 0, DataType::U32),
        BufferDecl::read("b", 1, DataType::U32),
        BufferDecl::read_write("out", 2, DataType::U32),
    ],
    [64, 1, 1],
    vec![
        Node::let_bind("idx", Expr::gid_x()),
        Node::store(
            "out",
            Expr::var("idx"),
            Expr::bitxor(
                Expr::load("a", Expr::var("idx")),
                Expr::load("b", Expr::var("idx")),
            ),
        ),
    ],
);
assert!(vyre::validate(&program).is_empty());
```

GPU dispatch and backend-specific readback examples belong in the concrete
driver crates that own those runtimes.

## Public facade: what `vyre::*` exports

`vyre` is the single user-facing import path. Reaching for `vyre_core::*`,
`vyre_foundation::*`, `vyre_driver::*`, or any backend crate from a
consumer is a smell: the gate
`scripts/check_examples_public_facade.sh` enforces this for every
example under `examples/`.

| Module                  | Source crate          | Purpose                                              |
|-------------------------|-----------------------|------------------------------------------------------|
| `vyre::ir::*`           | `vyre-foundation`     | `Program`, `BufferDecl`, `Node`, `Expr`, `validate`. |
| `vyre::lower`           | `vyre-foundation`     | IR-to-target lowering used by backends.              |
| `vyre::optimizer`       | `vyre-foundation`     | Pass scheduler + reference passes.                   |
| `vyre::cpu_op`          | `vyre-foundation`     | Wire-format CPU-reference byte ABI.                  |
| `vyre::cpu_references`  | `vyre-foundation`     | CPU reference implementations.                       |
| `vyre::memory_model`    | `vyre-foundation`     | Substrate-neutral memory ordering model.             |
| `vyre::execution_plan`  | `vyre-foundation`     | Performance/accuracy execution planning.             |
| `vyre::routing`         | `vyre-driver`         | Distribution-aware runtime algorithm selection.      |
| `vyre::error`           | `vyre-driver`         | Unified error types.                                 |
| `vyre::diagnostics`     | `vyre-driver`         | Structured machine-readable diagnostics.             |
| `vyre::backend`         | `vyre-driver`         | `VyreBackend`, `Executable`, dispatch config types.  |
| `vyre::match_result`    | `vyre-foundation`     | `Match` and `ByteRange` byte-range types.            |
| `vyre::pipeline`        | `vyre-driver`         | Pipeline-mode (compile-once-dispatch-many) API.      |
| `vyre::ByteRange`       | `vyre-foundation`     | Domain-neutral byte-range type.                      |

Top-level re-exports also include `BackendError`, `BackendRegistration`,
`CompiledPipeline`, `DispatchConfig`, `Error`, `Executable`, `Memory`,
`MemoryRef`, `OutputBuffers`, `TypedDispatchExt`, `VyreBackend`, 
everything a consumer needs without reaching beyond `vyre`.

## Why vyre

- **Composable primitives (Cat A):** any algorithm is a composition of simpler
  ops with zero-cost lowering.
- **Hardware intrinsics (Cat C):** ops declare GPU instruction backing per-target;
  swap hardware, swap intrinsics.
- **Link-time registration:** dialect ops, backends, and optimizer passes register
  with `inventory::submit!`; consumers discover those registries through
  `inventory::iter` instead of generated build-scan files.
- **Forbidden patterns (Cat B):** no typetag, no trait-object execution routing,
  no CPU fallback dispatch. Closed-enum semantics throughout.

## Conformance

Pair vyre with `vyre-reference` and backend KAT parity tests for a binary
verdict on backend correctness.

## The book

Documentation and tutorials live in `core/docs/`. Read them locally or build the
mdbook when a rendered site is available.

## License

MIT OR Apache-2.0.
