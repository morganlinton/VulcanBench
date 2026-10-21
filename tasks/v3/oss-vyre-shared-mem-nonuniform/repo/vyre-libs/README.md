# vyre-libs

Category A composition ecosystem over vyre's hardware-intrinsic
primitives. Every function here returns a `vyre::Program` (or a
list of `vyre::ir::Node` a consumer can embed in a larger Program)
built entirely from existing vyre-ops primitives. No shader source,
no new `inventory::submit!`, no backend-specific code.

## Modules

- `vyre_libs::math`: linear algebra, scans, broadcasts
- `vyre_libs::nn`: neural-net primitives (linear, ReLU, softmax,
  layer_norm, attention)
- `vyre_libs::matching`: string scanning (substring, DFA, multi-string): one building block inside arbitrary programs
- `vyre_libs::crypto`: hashing (FNV-1a, BLAKE3, SHA-256, CRC32)

## Design

Every public function wraps its IR body in a `Node::Region` with a
stable generator name. The optimizer treats Regions as opaque by
default (preserves source-mapping + debuggability); explicit inline
passes can unroll. This is LLVM's function-vs-always-inline split at
IR level.

One crate with four public modules today; each module promotes to
its own crates.io identity (`vyre-nn`, `vyre-math`, `vyre-scan`,
`vyre-crypto`) when its consumer base justifies the fragmentation.

## Usage

```rust
use vyre::VyreBackend;
use vyre_driver::backend::acquire_preferred_dispatch_backend;
use vyre_libs::math::dot;

let program = dot("x", "y", "result");
let backend = acquire_preferred_dispatch_backend()?;
let result = backend.dispatch(
    &program,
    &[vec![1, 2, 3, 4], vec![5, 6, 7, 8], vec![0]],
    &vyre::DispatchConfig::default(),
)?;
```

## Feature flags

```toml
[dependencies]
vyre-libs = { version = "0.1", default-features = false, features = ["nn"] }
```

- `math` (default): linear algebra
- `nn` (default, implies `math`): neural-net primitives
- `matching` (default): string scanning primitives
- `crypto` (default): hashing

## License

MIT OR Apache-2.0
