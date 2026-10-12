# Validator Errors

## Table of Contents

- [V001  -  Validation error V001](#v001)
- [V008  -  duplicate local binding `...` shadows an outer scope](#v008)
- [V009  -  atomic `...` targets unsupported buffer access `...` on `...`](#v009)
- [V010  -  barrier may be reached by only part of a workgroup](#v010)
- [V011  -  assignment to loop variable `...`](#v011)
- [V012  -  unsupported cast from `...` to `...`](#v012)
- [V013  -  load from buffer `...` with element type `bytes` is not supported](#v013)
- [V014  -  atomic on buffer `...` with non-u32 element type `...`](#v014)
- [V016  -  call references unknown op `...`](#v016)
- [V018  -  program nesting depth ... exceeds max ...](#v018)
- [V019  -  program has more than ... statement nodes](#v019)
- [V020  -  call `...` has ... arguments but signature expects ...](#v020)
- [V021  -  call `...` signature input `...` uses unknown type spelling `...`](#v021)
- [V022  -  program declares ... output buffers](#v022)
- [V023  -  cast to Bytes is unsupported in target-text lowering](#v023)
- [V025  -  atomic `...` on workgroup buffer `...` is rejected by the current memory model](#v025)
- [V027  -  atomic index on buffer `...` has type `...`, must be `u32`](#v027)
- [V028  -  Fma operand `...` has type `...`, must be `f32`](#v028)
- [V029  -  Select branches have mismatched types: true=`...`, false=`...`](#v029)
- [V030  -  opaque expression extension `...`/`...` failed validation: ...](#v030)
- [V031  -  opaque node extension `...`/`...` failed validation: ...](#v031)
- [V032  -  duplicate sibling let binding `...` in the same region](#v032)
- [V033  -  expression nesting depth ... exceeds max ...](#v033)
- [V034  -  backend `...` does not support cast target `...`](#v034)
- [V035  -  narrowing cast from `...` to `...` may truncate high bits](#v035)
- [V036  -  store index ... overflows buffer `...` with count ...](#v036)
- [V041  -  subgroup expressions require backend subgroup-ops support](#v041)
- [V042  -  atomic `...` on buffer `...` uses invalid memory ordering `...`](#v042)
- [V043  -  barrier uses memory ordering `...`, but barriers must synchronize memory](#v043)
- [V044  -  binary operation `Mod` has a statically-zero divisor](#v044)
- [V045  -  assignment to `...` has type `...` but the binding was declared as `...`](#v045)
- [V046  -  distributed collective node validation failure](#v046)

## V001  -  Validation error V001

**Description**: Validation error V001

**Common Cause**: A program construct violates Vyre's intermediate representation semantics or target backend constraints.

**Recommended Fix**: See error message for details

### Example

```rust
// Bad input
// (Causes V001)
let bad_ir = ...;

// Corrected input
// (See error message for details)
let good_ir = ...;
```

## V046  -  distributed collective node validation failure

**Description**: distributed collective nodes require backend collective support, valid buffer references, global/device-visible storage, and matching input/output element types.

**Common Cause**: A collective node was validated without distributed collective backend support, referenced an unknown or workgroup-local buffer, or paired buffers with mismatched element types.

**Recommended Fix**: validate with backend collective support, declare every referenced collective buffer, keep collective buffers in device/global storage, and use matching element types before collective lowering.

### Example

```rust
// Bad input
// (Causes V046)
let bad_ir = ...;

// Corrected input
// (validate with collective-capable backend settings and matching device-visible buffers.)
let good_ir = ...;
```

## V008  -  duplicate local binding `...` shadows an outer scope

**Description**: duplicate local binding `{name}` shadows an outer scope.

**Common Cause**: A program construct violates Vyre's intermediate representation semantics or target backend constraints.

**Recommended Fix**: choose a unique local name, or opt into nested shadowing with ValidationOptions::with_shadowing(true).

### Example

```rust
// Bad input
// (Causes V008)
let bad_ir = ...;

// Corrected input
// (choose a unique local name, or opt into nested shadowing with ValidationOptions::with_shadowing(true).)
let good_ir = ...;
```

## V009  -  atomic `...` targets unsupported buffer access `...` on `...`

**Description**: atomic `{op:?}` targets unsupported buffer access `{other:?}` on `{buffer}`.

**Common Cause**: A program construct violates Vyre's intermediate representation semantics or target backend constraints.

**Recommended Fix**: use BufferAccess::ReadWrite storage buffers for atomics.

### Example

```rust
// Bad input
// (Causes V009)
let bad_ir = ...;

// Corrected input
// (use BufferAccess::ReadWrite storage buffers for atomics.)
let good_ir = ...;
```

## V010  -  barrier may be reached by only part of a workgroup

**Description**: barrier may be reached by only part of a workgroup.

**Common Cause**: A program construct violates Vyre's intermediate representation semantics or target backend constraints.

**Recommended Fix**: move the barrier to uniform control flow.

### Example

```rust
// Bad input
// (Causes V010)
let bad_ir = ...;

// Corrected input
// (move the barrier to uniform control flow.)
let good_ir = ...;
```

## V011  -  assignment to loop variable `...`

**Description**: assignment to loop variable `{name}`.

**Common Cause**: A program construct violates Vyre's intermediate representation semantics or target backend constraints.

**Recommended Fix**: loop variables are immutable.

### Example

```rust
// Bad input
// (Causes V011)
let bad_ir = ...;

// Corrected input
// (loop variables are immutable.)
let good_ir = ...;
```

## V012  -  unsupported cast from `...` to `...`

**Description**: unsupported cast from `{src}` to `{target}`. Source type `{src}` legal targets are {legal_targets}. Choose one of those targets or rewrite this cast expression before validation.

**Common Cause**: A program construct violates Vyre's intermediate representation semantics or target backend constraints.

**Recommended Fix**: 

### Example

```rust
// Bad input
// (Causes V012)
let bad_ir = ...;

// Corrected input
// ()
let good_ir = ...;
```

## V013  -  load from buffer `...` with element type `bytes` is not supported

**Description**: load from buffer `{buffer}` with element type `bytes` is not supported.

**Common Cause**: A program construct violates Vyre's intermediate representation semantics or target backend constraints.

**Recommended Fix**: declare the buffer with a typed element (U32/I32/F32/…) or with `.with_bytes_extraction(true)` when the consuming op is a dedicated bytes-extraction op.

### Example

```rust
// Bad input
// (Causes V013)
let bad_ir = ...;

// Corrected input
// (declare the buffer with a typed element (U32/I32/F32/…) or with `.with_bytes_extraction(true)` when the consuming op is a dedicated bytes-extraction op.)
let good_ir = ...;
```

## V014  -  atomic on buffer `...` with non-u32 element type `...`

**Description**: atomic on buffer `{buffer}` with non-u32 element type `{elem}`.

**Common Cause**: A program construct violates Vyre's intermediate representation semantics or target backend constraints.

**Recommended Fix**: atomics only support U32 elements.

### Example

```rust
// Bad input
// (Causes V014)
let bad_ir = ...;

// Corrected input
// (atomics only support U32 elements.)
let good_ir = ...;
```

## V016  -  call references unknown op `...`

**Description**: call references unknown op `{op_id}`.

**Common Cause**: A program construct violates Vyre's intermediate representation semantics or target backend constraints.

**Recommended Fix**: register the dialect that owns `{op_id}` before validation, or inline/remove this call.

### Example

```rust
// Bad input
// (Causes V016)
let bad_ir = ...;

// Corrected input
// (register the dialect that owns `{op_id}` before validation, or inline/remove this call.)
let good_ir = ...;
```

## V018  -  program nesting depth ... exceeds max ...

**Description**: program nesting depth {depth} exceeds max {DEFAULT_MAX_NESTING_DEPTH}.

**Common Cause**: A program construct violates Vyre's intermediate representation semantics or target backend constraints.

**Recommended Fix**: flatten nested If/Loop/Block structures or split the program before lowering.

### Example

```rust
// Bad input
// (Causes V018)
let bad_ir = ...;

// Corrected input
// (flatten nested If/Loop/Block structures or split the program before lowering.)
let good_ir = ...;
```

## V019  -  program has more than ... statement nodes

**Description**: program has more than {DEFAULT_MAX_NODE_COUNT} statement nodes.

**Common Cause**: A program construct violates Vyre's intermediate representation semantics or target backend constraints.

**Recommended Fix**: split the program into smaller kernels or run an optimization pass before lowering.

### Example

```rust
// Bad input
// (Causes V019)
let bad_ir = ...;

// Corrected input
// (split the program into smaller kernels or run an optimization pass before lowering.)
let good_ir = ...;
```

## V020  -  call `...` has ... arguments but signature expects ...

**Description**: call `{op_id}` has {} arguments but signature expects {expected}.

**Common Cause**: A program construct violates Vyre's intermediate representation semantics or target backend constraints.

**Recommended Fix**: pass exactly {expected} arguments in the order declared by the op signature.

### Example

```rust
// Bad input
// (Causes V020)
let bad_ir = ...;

// Corrected input
// (pass exactly {expected} arguments in the order declared by the op signature.)
let good_ir = ...;
```

## V021  -  call `...` signature input `...` uses unknown type spelling `...`

**Description**: call `{op_id}` signature input `{}` uses unknown type spelling `{}`.

**Common Cause**: A program construct violates Vyre's intermediate representation semantics or target backend constraints.

**Recommended Fix**: register a foundation-known scalar/vector type spelling for this parameter or validate it in the dialect layer.

### Example

```rust
// Bad input
// (Causes V021)
let bad_ir = ...;

// Corrected input
// (register a foundation-known scalar/vector type spelling for this parameter or validate it in the dialect layer.)
let good_ir = ...;
```

## V022  -  program declares ... output buffers

**Description**: program declares {outputs} output buffers.

**Common Cause**: A program construct violates Vyre's intermediate representation semantics or target backend constraints.

**Recommended Fix**: mark at most one result buffer with BufferDecl::output(...).

### Example

```rust
// Bad input
// (Causes V022)
let bad_ir = ...;

// Corrected input
// (mark at most one result buffer with BufferDecl::output(...).)
let good_ir = ...;
```

## V023  -  cast to Bytes is unsupported in target-text lowering

**Description**: cast to Bytes is unsupported in target-text lowering.

**Common Cause**: A program construct violates Vyre's intermediate representation semantics or target backend constraints.

**Recommended Fix**: use buffer load/store directly for byte data.

### Example

```rust
// Bad input
// (Causes V023)
let bad_ir = ...;

// Corrected input
// (use buffer load/store directly for byte data.)
let good_ir = ...;
```

## V025  -  atomic `...` on workgroup buffer `...` is rejected by the current memory model

**Description**: atomic `{op:?}` on workgroup buffer `{buffer}` is rejected by the current memory model.

**Common Cause**: A program construct violates Vyre's intermediate representation semantics or target backend constraints.

**Recommended Fix**: use a storage ReadWrite buffer for atomics.

### Example

```rust
// Bad input
// (Causes V025)
let bad_ir = ...;

// Corrected input
// (use a storage ReadWrite buffer for atomics.)
let good_ir = ...;
```

## V027  -  atomic index on buffer `...` has type `...`, must be `u32`

**Description**: atomic index on buffer `{buffer}` has type `{index_ty}`, must be `u32`.

**Common Cause**: A program construct violates Vyre's intermediate representation semantics or target backend constraints.

**Recommended Fix**: cast the index to U32 before the atomic operation.

### Example

```rust
// Bad input
// (Causes V027)
let bad_ir = ...;

// Corrected input
// (cast the index to U32 before the atomic operation.)
let good_ir = ...;
```

## V028  -  Fma operand `...` has type `...`, must be `f32`

**Description**: Fma operand `{slot}` has type `{ty}`, must be `f32`.

**Common Cause**: A program construct violates Vyre's intermediate representation semantics or target backend constraints.

**Recommended Fix**: cast the operand to F32 before Fma, or use the integer mul/add form explicitly.

### Example

```rust
// Bad input
// (Causes V028)
let bad_ir = ...;

// Corrected input
// (cast the operand to F32 before Fma, or use the integer mul/add form explicitly.)
let good_ir = ...;
```

## V029  -  Select branches have mismatched types: true=`...`, false=`...`

**Description**: Select branches have mismatched types: true=`{t}`, false=`{f}`.

**Common Cause**: A program construct violates Vyre's intermediate representation semantics or target backend constraints.

**Recommended Fix**: cast both branches to the same type before Select.

### Example

```rust
// Bad input
// (Causes V029)
let bad_ir = ...;

// Corrected input
// (cast both branches to the same type before Select.)
let good_ir = ...;
```

## V030  -  opaque expression extension `...`/`...` failed validation: ...

**Description**: opaque expression extension `{}`/`{}` failed validation: {message}

**Common Cause**: A program construct violates Vyre's intermediate representation semantics or target backend constraints.

**Recommended Fix**: 

### Example

```rust
// Bad input
// (Causes V030)
let bad_ir = ...;

// Corrected input
// ()
let good_ir = ...;
```

## V031  -  opaque node extension `...`/`...` failed validation: ...

**Description**: opaque node extension `{}`/`{}` failed validation: {message}

**Common Cause**: A program construct violates Vyre's intermediate representation semantics or target backend constraints.

**Recommended Fix**: 

### Example

```rust
// Bad input
// (Causes V031)
let bad_ir = ...;

// Corrected input
// ()
let good_ir = ...;
```

## V032  -  duplicate sibling let binding `...` in the same region

**Description**: duplicate sibling let binding `{name}` in the same region.

**Common Cause**: A program construct violates Vyre's intermediate representation semantics or target backend constraints.

**Recommended Fix**: rename one binding or move one declaration into an inner Block/Region/Loop if a new scope is intended.

### Example

```rust
// Bad input
// (Causes V032)
let bad_ir = ...;

// Corrected input
// (rename one binding or move one declaration into an inner Block/Region/Loop if a new scope is intended.)
let good_ir = ...;
```

## V033  -  expression nesting depth ... exceeds max ...

**Description**: expression nesting depth {depth} exceeds max {DEFAULT_MAX_EXPR_DEPTH}.

**Common Cause**: A program construct violates Vyre's intermediate representation semantics or target backend constraints.

**Recommended Fix**: split the expression into intermediate let-bindings before lowering.

### Example

```rust
// Bad input
// (Causes V033)
let bad_ir = ...;

// Corrected input
// (split the expression into intermediate let-bindings before lowering.)
let good_ir = ...;
```

## V034  -  backend `...` does not support cast target `...`

**Description**: backend `{}` does not support cast target `{target}`.

**Common Cause**: A program construct violates Vyre's intermediate representation semantics or target backend constraints.

**Recommended Fix**: choose a target type this backend supports, or validate against a backend that advertises `{target}` cast support.

### Example

```rust
// Bad input
// (Causes V034)
let bad_ir = ...;

// Corrected input
// (choose a target type this backend supports, or validate against a backend that advertises `{target}` cast support.)
let good_ir = ...;
```

## V035  -  narrowing cast from `...` to `...` may truncate high bits

**Description**: narrowing cast from `{src}` to `{target}` may truncate high bits. Source type `{src}` legal targets are {legal_targets}. Use a non-narrowing target or prove the source value fits before casting.

**Common Cause**: A program construct violates Vyre's intermediate representation semantics or target backend constraints.

**Recommended Fix**: 

### Example

```rust
// Bad input
// (Causes V035)
let bad_ir = ...;

// Corrected input
// ()
let good_ir = ...;
```

## V036  -  store index ... overflows buffer `...` with count ...

**Description**: store index {value} overflows buffer `{buffer_name}` with count {}.

**Common Cause**: A program construct violates Vyre's intermediate representation semantics or target backend constraints.

**Recommended Fix**: keep constant store indices below the declared element count.

### Example

```rust
// Bad input
// (Causes V036)
let bad_ir = ...;

// Corrected input
// (keep constant store indices below the declared element count.)
let good_ir = ...;
```

## V041  -  subgroup expressions require backend subgroup-ops support

**Description**: subgroup expressions require backend subgroup-ops support.

**Common Cause**: A program construct violates Vyre's intermediate representation semantics or target backend constraints.

**Recommended Fix**: Validate with ValidationOptions::with_backend(backend) where backend.supports_subgroup_ops() == true.

### Example

```rust
// Bad input
// (Causes V041)
let bad_ir = ...;

// Corrected input
// (Validate with ValidationOptions::with_backend(backend) where backend.supports_subgroup_ops() == true.)
let good_ir = ...;
```

## V042  -  atomic `...` on buffer `...` uses invalid memory ordering `...`

**Description**: atomic `{op:?}` on buffer `{buffer}` uses invalid memory ordering `{ordering:?}`.

**Common Cause**: A program construct violates Vyre's intermediate representation semantics or target backend constraints.

**Recommended Fix**: use Relaxed, Acquire, Release, AcqRel, or SeqCst for atomic read-modify-write operations.

### Example

```rust
// Bad input
// (Causes V042)
let bad_ir = ...;

// Corrected input
// (use Relaxed, Acquire, Release, AcqRel, or SeqCst for atomic read-modify-write operations.)
let good_ir = ...;
```

## V043  -  barrier uses memory ordering `...`, but barriers must synchronize memory

**Description**: barrier uses memory ordering `{ordering:?}`, but barriers must synchronize memory.

**Common Cause**: A program construct violates Vyre's intermediate representation semantics or target backend constraints.

**Recommended Fix**: use Acquire, Release, AcqRel, or SeqCst; use no barrier at all for Relaxed.

### Example

```rust
// Bad input
// (Causes V043)
let bad_ir = ...;

// Corrected input
// (use Acquire, Release, AcqRel, or SeqCst; use no barrier at all for Relaxed.)
let good_ir = ...;
```

## V044  -  binary operation `Mod` has a statically-zero divisor

**Description**: binary operation `Mod` has a statically-zero divisor.

**Common Cause**: A program construct violates Vyre's intermediate representation semantics or target backend constraints.

**Recommended Fix**: guard the divisor, use Select to substitute a non-zero value, or reject the input before building IR.

### Example

```rust
// Bad input
// (Causes V044)
let bad_ir = ...;

// Corrected input
// (guard the divisor, use Select to substitute a non-zero value, or reject the input before building IR.)
let good_ir = ...;
```

## V045  -  assignment to `...` has type `...` but the binding was declared as `...`

**Description**: assignment to `{name}` has type `{value_ty}` but the binding was declared as `{declared}`.

**Common Cause**: A program construct violates Vyre's intermediate representation semantics or target backend constraints.

**Recommended Fix**: cast the value to `{declared}` or introduce a new binding with the intended type.

### Example

```rust
// Bad input
// (Causes V045)
let bad_ir = ...;

// Corrected input
// (cast the value to `{declared}` or introduce a new binding with the intended type.)
let good_ir = ...;
```
