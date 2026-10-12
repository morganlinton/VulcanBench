# vyre-conform-generate

Counterexample shrinking and witness generation for vyre
conformance testing.

When a law prover (`vyre-conform-enforce`) flags a failing tuple,
the raw witnesses are usually too large to debug directly:
`0xDEADBEEF + 0xCAFEBABE` is a failure, but the root cause is
probably `0 + <some small value>`. This crate's
`CounterexampleMinimizer` does the shrinking: a deterministic
binary-search that converges to the smallest `u32` input that still
reproduces the failure.

## Invariants

1. **Termination is `O(log n)`.** Minimisation halves the input
   every step and always makes progress; it never loops.
2. **Shrinking is monotonic.** The minimiser never returns a larger
   counterexample than the one it was given.
3. **Deterministic.** Given the same failing predicate and initial
   counterexample, the minimiser returns the same minimal value on
   every run.
4. **Side-effect free.** The minimiser invokes the predicate the
   caller provides; it does not allocate, log, or mutate external
   state.

## Boundaries

This crate owns:

- `CounterexampleMinimizer`: the shrinker.

It does NOT own:

- The law prover (`vyre-conform-enforce`): the minimiser consumes
  *already-identified* counterexamples; it does not search for new
  ones.
- Witness enumeration (`vyre-conform-spec`).
- Multi-parameter shrinking: for tuples, shrink each component in
  sequence using this minimiser; a joint shrinker would need
  coordinate-descent and lives out of scope here.

## Worked examples

### 1. Shrink a failing counterexample

```rust
use vyre_conform_generate::CounterexampleMinimizer;

let minimiser = CounterexampleMinimizer::new();
let initial = 0x1234_5678_u32;
let minimal = minimiser.shrink(initial, |v| {
    // predicate: returns true if the failure still reproduces at `v`
    v >= 0x1000 // only values >= 0x1000 trigger the bug
});
assert_eq!(minimal, 0x1000);
```

### 2. Shrink a tuple component-by-component

```rust
use vyre_conform_generate::CounterexampleMinimizer;

fn shrink_pair(mut a: u32, mut b: u32, failing: impl Fn(u32, u32) -> bool) -> (u32, u32) {
    let minimiser = CounterexampleMinimizer::new();
    a = minimiser.shrink(a, |v| failing(v, b));
    b = minimiser.shrink(b, |v| failing(a, v));
    (a, b)
}
```

### 3. Report the minimised counterexample

```rust
use vyre_conform_enforce::{LawProver, LawVerdict};
use vyre_conform_generate::CounterexampleMinimizer;
use vyre_conform_spec::{U32Witness, WitnessSet};

let prover = LawProver::new(U32Witness::enumerate());
let verdict = prover.commutative(|a, b| broken_op(a, b));
if let LawVerdict::Failed((a, b)) = verdict {
    let min = CounterexampleMinimizer::new();
    let a_min = min.shrink(a, |v| broken_op(v, b) != broken_op(b, v));
    eprintln!("smallest divergent a: {a_min:#x}");
}
# fn broken_op(a: u32, b: u32) -> u32 { a.wrapping_sub(b) }
```

## Extension guide: adding a new shrink strategy

1. If the strategy is still binary-search over a single scalar,
   extend `CounterexampleMinimizer` itself: do not create a new
   struct. One name per concept.
2. If the strategy is fundamentally different (delta-debugging,
   list shrinking, tree shrinking) add a new struct in its own
   module with its own `shrink` method; name it after the strategy
   (`DeltaDebuggingMinimizer`, not `AdvancedMinimizer`).
3. Every shrinker MUST document its termination argument in module
   docs. A shrinker without an `O(...)` termination bound is a
   bug, not a feature.
4. Every shrinker MUST have a proptest that asserts monotonicity
   (output ≤ input under the predicate's notion of size). This
   catches regression classes where "shrinking" accidentally
   enlarges a witness.
5. Re-export from `lib.rs` so consumers reach every strategy through
   `vyre_conform_generate::<Thing>`.
