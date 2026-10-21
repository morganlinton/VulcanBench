# vyre  -  architecture

The meta-shim crate. Re-exports the public-facing names of every
internal vyre crate so consumers write `vyre::ir::Program` instead
of `vyre_foundation::ir::Program`.

## Module

`src/lib.rs` is the only file. It is intentionally thin: every
public type lands here as a single `pub use` so the consumer-
facing import surface stays one path deep regardless of how the
internal crates are sliced.

## Public surface

The full re-export catalogue lives in the lib.rs, but the
canonical entry points consumers reach for are:

- **`vyre::ir`**  -  `Program`, `Node`, `Expr`, `BufferDecl`,
  `Ident`, `BinOp`, `UnOp`, `AtomicOp`, `BufferAccess`,
  `DataType`. From `vyre_foundation::ir`.
- **`vyre::backend`**  -  `VyreBackend`, `BackendError`,
  `DispatchConfig`. From `vyre_driver::backend`.
- **`vyre::execution_plan`**  -  `fuse_programs`, `fuse_programs_vec`,
  `FusionError`. From `vyre_foundation::execution_plan`.
- **`vyre::lower`**  -  `inline_calls`, `optimize`. From the
  foundation transform stack.
- **`vyre::backend::private`**  -  sealed-trait gate; consumers
  implement `VyreBackend` only by going through this private
  marker.

## Integration points

- Every public crate name is documented in
  `docs/CRATE_GRAPH.md`.
- The shim's stability contract: any name re-exported here MUST
  stay accessible at the same path until the next major release;
  it is the public ABI.
