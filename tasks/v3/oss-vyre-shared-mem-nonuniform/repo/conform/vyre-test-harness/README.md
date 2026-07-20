# vyre-test-harness

Shared test infrastructure for vyre parity and conformance suites.

## Invariants

- Harness code compares backend behavior; it never repairs or normalizes outputs.
- Every surfaced failure must preserve the original backend error text and add a `Fix:` path where the harness can name one.
- Lens helpers may skip only for explicit capability gaps or declared exemptions. Silent fallthrough is a bug.

## Boundaries

- `vyre-test-harness` depends on stable driver, foundation, libs, and reference crates only.
- It does not own backend implementations, runtime orchestration, or certificate issuance.
- Backend-specific tests import this crate instead of depending on higher-layer test binaries just to reuse helpers.

## Examples

```rust
use vyre_test_harness::lens;

let entry = vyre_libs::harness::all_entries().next().unwrap();
let outcome = lens::witness(entry);
assert!(outcome.is_ok());
```

```rust
use vyre_test_harness::lens;

let entry = vyre_libs::harness::all_entries().next().unwrap();
let backend = vyre_reference::ReferenceBackend::new();
let _ = lens::cpu_vs_backend(entry, &backend);
```

## Extension guide

1. Add a helper here only if it is backend-agnostic test logic.
2. Keep backend acquisition and device lifecycle in the consuming test crate.
3. When a helper needs shared fixture metadata, add it to `vyre-libs::harness` instead of inventing a new side channel here.
