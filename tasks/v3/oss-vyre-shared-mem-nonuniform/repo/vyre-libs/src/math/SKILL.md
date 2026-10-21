# vyre-libs::math SKILL

Linear algebra, scans, and broadcasting compositions. Every op is
pure vyre IR assembled from `vyre-ops` primitives.

## Coverage targets

- Linear algebra: `dot`, `matmul`, `matmul_tiled`. Future:
  `matmul_batched`, `outer_product`, `trace`, `transpose`.
- Scans: `scan_prefix_sum`. Future: `scan_max`, `scan_min`,
  `scan_prefix_product`, `segmented_scan`.
- Broadcast: `broadcast`. Future: `broadcast_to_shape`,
  `broadcast_add`, `broadcast_mul`.

## Witness sources

- `dot` / `matmul`: NumPy + BLAS ground truth; KAT corpus lives in
  `tests/cat_a_conform.rs`.
- `matmul_tiled`: must byte-match `matmul` for every witness (same
  semantics, tiled execution).
- `scan_prefix_sum`: Jax's `jax.numpy.cumsum` reference corpus.

## Benchmark targets (criterion)

- `dot` on 1024 elements: ≤ 100 µs CPU ref; linked dispatch backends
  stay within 1% of their checked-in baseline.
- `matmul` 256×256×256: ≤ 50 ms CPU ref; dispatch backends must stay
  sub-ms on current high-end fleet hardware.
- `scan_prefix_sum` on 4096 elements: CPU ref ≤ 1 ms; dispatch backends
  ≤ 50 µs once shared-memory cooperative scans are available.

## Backend parity contract

Every op's forward compression-function output MUST byte-match across
`vyre-reference` and every linked backend contract on the witness
corpus. Divergences are P0 conformance bugs.

## Shape-mismatch contract

All math builders reject shape mismatches at `build()` time via
`TensorRef` checks. Expected shape for each op is documented in its
builder rustdoc; the test in `tests/name_collision.rs` covers the
collision path.

## Overflow contract

`m * k`, `k * n`, `m * n`  -  any product that exceeds `u32::MAX` must
panic with `"element-count overflows u32"` at builder time. See
`tests/overflow_guards.rs` for the 7 panic-path assertions.
