# vyre-conform-enforce

Algebraic-law prover for vyre ops over witness sets.

Given an op's compose function and a witness enumeration, the prover
verifies commutativity, associativity, identity, and distributivity
by running the op over every witness tuple and flagging any
counterexample. Failures come back as a structured verdict carrying
the exact tuple that disproved the law, so you can reproduce the
failure byte-for-byte from the verdict alone.

## Invariants

1. **Verdicts are structural, not textual.** A `LawVerdict::Failed`
   variant carries the counterexample tuple; callers never parse a
   string to recover it.
2. **Proofs are exhaustive over the witness set.** The prover never
   early-exits on success; it runs every tuple so it finds multiple
   failures in one pass and presents the smallest by enumeration
   order.
3. **Side-effect free.** `LawProver::verify()` does not allocate per
   witness beyond what the op's compose function does; it does not
   log, print, or mutate external state.
4. **No floating-point tolerances.** Equality is byte-identical.
   Ops that legitimately have non-bit-exact outputs across rounding
   modes are Category-C (hardware intrinsics) and use a dedicated
   ULP-bounded verdict, not this crate.

## Boundaries

This crate owns:

- `LawProver`: the proof-pass orchestrator.
- `LawVerdict`: the structured outcome (`Pass`, `Failed(tuple)`,
  `Unchecked(reason)`).

It does NOT own:

- Witness enumeration (`vyre-conform-spec`).
- Counterexample minimization (`vyre-conform-generate`).
- Backend dispatch: the prover takes a `Fn(inputs) -> output` and
  is agnostic to where that function came from.

## Worked examples

### 1. Prove commutativity for `u32::wrapping_add`

```rust
use vyre_conform_enforce::{LawProver, LawVerdict};
use vyre_conform_spec::{U32Witness, WitnessSet};

let prover = LawProver::new(U32Witness::enumerate());
let verdict = prover.commutative(|a, b| a.wrapping_add(b));
assert!(matches!(verdict, LawVerdict::Pass));
```

### 2. Catch a broken associativity implementation

```rust
use vyre_conform_enforce::{LawProver, LawVerdict};
use vyre_conform_spec::{U32Witness, WitnessSet};

let prover = LawProver::new(U32Witness::enumerate());
// subtraction is not associative: prover should catch it
let verdict = prover.associative(|a, b| a.wrapping_sub(b));
match verdict {
    LawVerdict::Failed(witnesses) => {
        eprintln!("subtraction not associative at {witnesses:?}");
    }
    other => panic!("expected Failed, got {other:?}"),
}
```

### 3. Chain verdicts for a full op audit

```rust
use vyre_conform_enforce::{LawProver, LawVerdict};
use vyre_conform_spec::{U32Witness, WitnessSet};

fn audit(op: impl Fn(u32, u32) -> u32 + Copy) -> Vec<(&'static str, LawVerdict)> {
    let prover = LawProver::new(U32Witness::enumerate());
    vec![
        ("commutative", prover.commutative(op)),
        ("associative", prover.associative(op)),
        ("identity_0", prover.identity(op, 0)),
    ]
}
```

## Extension guide: adding a new law

1. Pick a well-defined algebraic property. Write its equation as a
   one-line comment at the top of the new prover method: for
   example `distributive: op(a, op(b, c)) == op(op(a, b), c)` is
   WRONG (that's associativity); `a * (b + c) == (a * b) + (a * c)`
   is distributivity.
2. Add a method on `LawProver` named after the property. Iterate
   every required tuple (pair / triple / etc.) from the witness set
   and return `LawVerdict::Failed(tuple)` on the first divergence.
3. Extend `LawVerdict` only if the existing variants don't carry
   enough structure; never add a string-only variant.
4. Add at least three unit tests: one op that passes the law, one
   op that fails with a known tuple, and one op whose inputs are
   ill-defined (the prover should return `Unchecked` with a reason,
   not panic).
5. Re-export the method on `LawProver`: consumers discover laws
   through the type's IDE completion, so keep the name-space flat.
