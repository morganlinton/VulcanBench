# vyre-spec

Frozen data contracts for the vyre GPU compute IR.

## What this crate is

`vyre-spec` is the leaf crate of the vyre ecosystem. It contains only frozen
data types: IR scalar and operation types, algebraic laws, conformance
categories, verification evidence, engine invariants, and the `I1..I15`
intrinsic catalog. Zero dependencies. Every vyre consumer transitively depends
on this crate, so backend vendors may depend on `vyre-spec` alone to prove
conformance without pulling in the compiler or the conform runtime.

## What you get

- `Category`: operation classification (`A`, `C`, etc.)
- `AlgebraicLaw`: declarative laws (commutative, associative, identity,
  distributive, monotonic, bounded, custom predicates, and more)
- `OpDef`: frozen operation specification
- `OpSignature`: input/output type signature for an IR operation
- `DataType`: IR types (`U32`, `I32`, `U64`, `Vec2U32`, `Vec4U32`, `Bool`,
  `Bytes`, `F32`)
- `Convention`: calling conventions for op dispatch
- `IntrinsicTable`: hardware intrinsic lookup tables
- `EngineInvariant` / `Invariant` / `InvariantId`: engine-level invariant
  declarations and the `I1..I15` catalog
- `Verification`: verification result types
- `Layer`: conformance layer declarations (`L0`–`L8`)
- `BackendAvailability`: per-backend availability predicates
- `GoldenSample` / `KatVector` / `AdversarialInput`: test vectors and hostile
  witnesses
- `BinOp` / `UnOp` / `AtomicOp`: operation descriptors
- `LawCatalog` / `LAW_CATALOG`: static registry of all declared laws
- `INVARIANTS` / `by_id` / `by_category`: static invariant registry and
  lookups

## Stability

All public enums are marked `#[non_exhaustive]`. The surface of this crate is
frozen under a 5-year stability contract. Any breaking change to a public type
or exported constant requires a major version bump. Patch and minor releases
add only new variants, new constants, or documentation fixes.

## Usage

```rust
use vyre_spec::{AlgebraicLaw, DataType, OpSignature};

let sig = OpSignature {
    inputs: vec![DataType::U32, DataType::U32],
    output: DataType::U32,
    input_params: None,
    output_params: None,
    contract: None,
};
let law = AlgebraicLaw::Commutative;
assert_eq!(sig.output, DataType::U32);
```

## See also

- `vyre` (the compiler): https://crates.io/crates/vyre
- Book: the vyre book at https://github.com/santhsecurity/vyre

## License

MIT OR Apache-2.0
