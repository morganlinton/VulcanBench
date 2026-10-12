# Authoring a Cat-A op in vyre-libs

This document is the canonical recipe for landing a new Category-A
composition inside `vyre-libs`. A Cat-A op produces a `Program`
assembled from vyre primitives  -  no raw shader source, no
backend-specific code. Every backend that implements `VyreBackend`
runs the same Program with byte-identical output.

Follow the 5-step recipe below. Every step has a test or a lint
protecting it; skipping a step fails CI.

---

## 1. Pick primitives + wrap in `Region`

Every composition is a slice of vyre IR nodes ending in a
[`Node::Region`](https://docs.rs/vyre-foundation) wrapper. The wrapper
is a **debug marker**: it carries the generator name and optional
source-region metadata so conformance certificates and tracing spans
can name where the IR came from. Region is semantically transparent
(see `docs/ir-semantics.md`), so the wrapper never affects execution.

Skeleton:

```rust
use vyre_libs::prelude::*;

fn my_op(input: TensorRef, output: TensorRef) -> Result<Program, TensorRefError> {
    check_tensors(
        "vyre-libs::dialect::my_op",
        &[(&input, DataType::F32), (&output, DataType::F32)],
    )?;
    // ... build `body: Vec<Node>` using the primitives in vyre-ops ...
    let body = vec![/* ... */];

    Ok(Program::wrapped(
        vec![
            BufferDecl::storage(input.name_str(), 0, BufferAccess::ReadOnly, DataType::F32)
                .with_count(input.element_count().expect("checked above")),
            BufferDecl::storage(output.name_str(), 1, BufferAccess::ReadWrite, DataType::F32)
                .with_count(output.element_count().expect("checked above")),
        ],
        [64, 1, 1],
        vec![wrap("vyre-libs::dialect::my_op", body, None)],
    ))
}
```

**Rules enforced by CI:**
- Buffer count must be declared via `BufferDecl::with_count(n)`
  when the shape is known at build time (P1.5).
- Product overflows (m·k, s·d, etc.) must be guarded with
  `checked_mul` or routed through `TensorRef::element_count()`.
- No raw WGSL, WGSL-like strings, or any backend-specific code
  (`scripts/check_no_wgsl_in_libs.sh`).

## 2. Ship a typed builder

Every non-trivial Cat-A op exposes a chainable builder so future
options (workgroup size, region generator override, tenant id) land
as options without breaking call sites:

```rust
pub struct MyOp { input: TensorRef, output: TensorRef, options: BuildOptions }

impl MyOp {
    pub fn new(input: TensorRef, output: TensorRef) -> Self {
        Self { input, output, options: BuildOptions::default() }
    }
    pub fn with_workgroup_size(mut self, s: [u32; 3]) -> Self {
        self.options = self.options.with_workgroup_size(s); self
    }
    pub fn with_region_generator(mut self, n: &'static str) -> Self {
        self.options = self.options.with_region_generator(n); self
    }
    pub fn with_tenant_id(mut self, t: u32) -> Self {
        self.options = self.options.with_tenant_id(t); self
    }
    pub fn build(self) -> Result<Program, TensorRefError> { /* ... */ }
}

/// Back-compat free function.
pub fn my_op(input: &str, output: &str, n: u32) -> Program {
    MyOp::new(TensorRef::f32_1d(input, n), TensorRef::f32_1d(output, n))
        .build()
        .unwrap_or_else(|err| panic!("Fix: my_op build failed: {err}"))
}
```

The free function is back-compat only. New callers use the builder.

## 3. Write a CPU reference

Every Cat-A op needs a plain-Rust reference that computes the same
thing the IR claims to. This is the oracle. Example for a 1-D
reduce-sum:

```rust
fn cpu_reduce_sum(input: &[u32]) -> u32 {
    input.iter().copied().sum()
}
```

Keep it trivially correct. Performance doesn't matter here; the
reference's only job is to be obviously right.

## 4. Add a `cat_a_conform` witness test

Every op ships a byte-identity assertion in
`vyre-libs/tests/cat_a_conform.rs`. The test runs the Program
through the reference interpreter and compares byte-for-byte against
the CPU reference on a corpus of inputs:

```rust
#[test]
fn cat_a_my_op_matches_cpu_reference() {
    use vyre_libs::my_op;
    let witnesses: &[(Vec<u32>, u32)] = &[
        (vec![1, 2, 3, 4], 10),
        (vec![0; 16], 0),
    ];
    for (input, expected) in witnesses {
        let program = my_op("in", "out", input.len() as u32);
        let outputs = run_program(
            &program,
            vec![Value::from(u32_bytes(input)), Value::from(vec![0u8; 4])],
        );
        let got = decode_u32_words(&outputs[0])[0];
        assert_eq!(got, *expected, "diverged on {input:?}");
    }
}
```

External pattern sources (BLAKE3 KATs, Aho-Corasick 1975 paper
corpus, Jax/TF reference outputs) are preferred over hand-rolled
witnesses.

## 4b. Declare your op's algebraic laws (optional but recommended)

If your op satisfies any algebraic laws (commutativity, associativity,
identity-element, idempotence, self-inverse), declare them so the
canonical-form + CSE passes can optimize through your op:

```rust
use vyre_foundation::{AlgebraicLaw, AlgebraicLawRegistration};

inventory::submit! {
    AlgebraicLawRegistration::new(
        "vyre-libs::dialect::my_op",
        AlgebraicLaw::Commutative,
    )
}
inventory::submit! {
    AlgebraicLawRegistration::new(
        "vyre-libs::dialect::my_op",
        AlgebraicLaw::Identity { element: 0 },
    )
}
```

The registration is pure metadata  -  CI does not verify the law
holds at this step; that's what the conformance harness (P5.4) does
against a witness corpus. Mis-declaring is a P0 correctness bug:
the optimizer will apply rewrites that produce wrong bytes.

## 5. Register an `OpEntry` for the universal harness

At the bottom of your op's source file:

```rust
inventory::submit! {
    crate::harness::OpEntry {
        id: "vyre-libs::dialect::my_op",
        build: || my_op("input", "output", 4),
        test_inputs: None,        // or Some(|| vec![...])
        expected_output: None,    // or Some(|| vec![...])
    }
}
```

The harness fires validation, wire round-trip, and CSE stability on
every registered entry  -  no per-op test file needed for those gates.

---

## Where the op lives

| Dialect | Path | Feature flag |
| --- | --- | --- |
| Linear algebra | `src/math/*.rs` | `math-linalg` |
| Scans + reductions | `src/math/scan.rs` | `math-scan` |
| Broadcasting | `src/math/broadcast.rs` | `math-broadcast` |
| NN activation | `src/nn/*.rs` | `nn-activation` |
| NN linear layers | `src/nn/linear.rs` | `nn-linear` |
| NN normalization | `src/nn/layer_norm.rs` | `nn-norm` |
| NN attention | `src/nn/attention.rs`, `src/nn/softmax.rs` | `nn-attention` |
| Substring search | `src/matching/substring.rs` | `matching-substring` |
| DFA / Aho-Corasick | `src/matching/aho_corasick.rs`, `src/matching/dfa_compile.rs` | `matching-dfa` |
| FNV hash | `src/crypto/fnv1a.rs` | `crypto-fnv` |
| BLAKE3 | `src/crypto/blake3.rs` | `crypto-blake3` |

Add a new dialect by creating `src/<dialect>/mod.rs` + registering a
feature flag in `Cargo.toml`. Re-export the op from `src/lib.rs`'s
`prelude` module so `use vyre_libs::prelude::*;` still works.

## What gets checked before merge

- `cargo check --workspace --all-features` (IR compiles).
- `cargo clippy --workspace --all-features --all-targets -- -D warnings`.
- `cargo test --workspace --all-features` (every op's conformance
  test, plus name-collision / overflow / assign-semantics / region
  lifetime regressions).
- `cargo doc --workspace --all-features --no-deps` (all intra-doc
  links resolve).
- `scripts/check_parity_testing_not_leaked.sh`.
- (WIP P3.7) op fingerprint unchanged vs. last release unless a
  CHANGELOG entry documents the break.

Any gate failing reverts the PR. The frozen core doesn't churn
for new ops; new ops conform to the frozen core.
