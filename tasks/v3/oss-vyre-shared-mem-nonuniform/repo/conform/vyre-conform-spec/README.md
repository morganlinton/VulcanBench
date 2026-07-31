# vyre-conform-spec

Witness sets and composition laws for vyre conformance testing:
deterministic, stable, fingerprintable.

This crate holds the *contract* side of conformance: the canonical
enumeration of input values every backend must produce identical
output for, plus the trait that new `DataType`s plug into when they
join the conformance matrix.

## Invariants

1. **Enumeration is deterministic.** `WitnessSet::enumerate()`
   returns the same values in the same order on every call, every
   build, every machine. The order is part of the public contract
   and may not change without a major-version bump.
2. **Enumeration is finite for bounded types.** `U32Witness`
   enumerates a fixed set of u32 values chosen to cover boundary
   conditions (0, 1, MAX, MAX-1, powers of two, known-hazardous
   values for hashes/bit-ops). Unbounded types use a sampled
   strategy documented on the impl.
3. **Fingerprints are stable.** Callers can hash the witness
   sequence to produce a cache key; the hash does not change across
   builds because the sequence does not change.
4. **Zero allocation in the hot path.** `enumerate` returns an
   iterator; no intermediate `Vec` is materialised inside the prover
   loop.

## Boundaries

This crate owns:

- The `WitnessSet` trait.
- The `U32Witness` canonical enumeration.

It does NOT own:

- The law prover (`vyre-conform-enforce`): that's the *mechanism*
  side.
- The counterexample shrinker (`vyre-conform-generate`).
- The CI runner (`vyre-conform-runner`).
- Any `DataType` definition: those live in `vyre-foundation`.

## Worked examples

### 1. Enumerate the canonical u32 witnesses

```rust
use vyre_conform_spec::{U32Witness, WitnessSet};

for value in U32Witness::enumerate() {
    println!("witness: {value:#x}");
}
```

### 2. Drive a backend-parity test

```rust
use vyre_conform_spec::{U32Witness, WitnessSet};

fn check_parity<F: Fn(u32) -> u32>(backend: F, reference: F) {
    for value in U32Witness::enumerate() {
        assert_eq!(
            backend(value),
            reference(value),
            "divergence at witness {value:#x}"
        );
    }
}
```

### 3. Fingerprint the witness sequence for a cache key

```rust
use blake3::Hasher;
use vyre_conform_spec::{U32Witness, WitnessSet};

let mut hasher = Hasher::new();
for v in U32Witness::enumerate() {
    hasher.update(&v.to_le_bytes());
}
let cache_key = hasher.finalize();
```

## Extension guide: adding a new DataType witness

1. Create `src/witness/<type>.rs`. Implement `WitnessSet` for the
   type you're adding (for example `I32Witness`, `F32Witness`,
   `VecU32Witness`).
2. Pick the enumeration order *once* and document the rationale in
   module docs. Remember: order is part of the public contract.
3. Add boundary values: zero, one, type MAX / MIN, subnormals (for
   floats), known-hazardous bit patterns (for hashes and bit ops).
4. Add at least one property test that asserts the sequence length
   and the first ten values are stable; that test catches accidental
   reordering during future refactors.
5. Re-export from `lib.rs` so consumers can `use
   vyre_conform_spec::<Your>Witness;`.
6. Add a matrix row in `vyre-conform-runner` so every registered
   backend is diffed against the CPU reference on your new type from
   day one.
