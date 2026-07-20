# vyre-spec  -  architecture

Frozen data contracts. Every wire-stable enum (BinOp, AtomicOp,
DataType, BufferAccess, Convention) plus every algebraic law
declaration ships from this crate.

This is the smallest crate in the workspace by design  -  the
"spec" is the smallest immutable surface of vyre, separate from
the larger `vyre-foundation` so consumers can take just the wire
contract without the IR transforms.

## Modules

### `bin_op.rs`
`BinOp` enum: Add/Sub/Mul/Div/Mod/Wrapping{Add,Sub}/BitAnd/BitOr/
BitXor/Shl/Shr/Eq/Ne/Lt/Gt/Le/Ge/And/Or/AbsDiff/Min/Max/
Saturating{Add,Sub}. `#[non_exhaustive]`, frozen.

### `atomic_op.rs`
`AtomicOp` enum. Frozen.

### `buffer_access.rs`
`BufferAccess`: ReadOnly / ReadWrite / Uniform / Workgroup. Frozen.

### `category.rs`
Op category taxonomy: Memory / Bitwise / Compare / Arithmetic /
Cast / Atomic / Subgroup / ControlFlow / Region / etc.

### `by_id.rs` + `by_category.rs`
Op-id ↔ category index, wire-frozen.

### `algebraic_law.rs` + `all_algebraic_laws.rs`
Algebraic-law marker enum + the canonical list. Optimizer
passes consult this to know which transformations are sound.

### `adversarial_input.rs`
Adversarial-input fixtures shared by every conform probe. Here
because the wire format encodes the fixture set as a frozen
constant.

### `catalog_is_complete.rs`
Build-time gate that asserts the catalog has every op the
foundation declares. A new op without a category trip the build.

## Public types

- **`BinOp` / `UnOp` / `AtomicOp`**  -  frozen enums.
- **`DataType` / `BufferAccess` / `Convention` / `OpSignature`**
   -  frozen wire types.
- **`AlgebraicLaw`**  -  marker enum.
- **`Category`**  -  op-category enum.

## Integration points

- Consumed by `vyre-foundation` for the IR types.
- Consumed by `vyre-conform-spec` for the spec gate.
- Wire-stability contract: changing a variant tag here is a
  major-version bump for every downstream crate.
