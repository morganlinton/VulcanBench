# vyre-frontend-c maturity gate

Current maturity: **beta / active development**.

This crate is allowed to move quickly because it is the in-repo compiler
front-end consumer that proves GPU-resident parsing, preprocessing, semantic
evidence, and object-evidence design. It is not a Vyre platform release blocker
until the promotion gate below is satisfied.

## Scope boundary

`vyre-frontend-c` owns C-source ingestion, translation-unit preparation,
GPU-reachable parsing evidence, semantic-readiness facts, and supported
compile-only object payloads.

It does not own the platform release proof for `vyre-core`, `vyre-foundation`,
`vyre-primitives`, `vyre-libs`, `vyre-runtime`, or the concrete GPU drivers.
Failures here are fixed directly, but they do not downgrade the platform crates'
release state unless they expose a platform contract violation.

## Promotion gate

Promotion out of beta requires all of these evidence classes to pass in the same
release candidate:

| Gate | Required evidence |
|---|---|
| Parser correctness | `tests/clang_*_oracle.rs`, `tests/c11_*`, and `tests/linux_grade_constructs_gpu_e2e.rs` pass against the current corpus. |
| GPU-first execution | `tests/gpu_*`, `tests/cuda_first_no_host_paths.rs`, and the real GPU directive kernels pass without silent CPU fallback. |
| Preprocessor parity | `tests/preprocess_*`, `tests/tu_host_preprocessor.rs`, and `parity/PREPROCESS_BENCHMARK_V1.md` prove token spelling, include, macro, and conditional behavior. |
| Object ABI stability | `tests/object_*`, `tests/linux_tu_object_*`, and `tests/object_version_stability.rs` prove `VYRECOB2` section stability and malformed-object rejection. |
| Clang differential evidence | `parity/PARITY_MANIFEST_V1.md` maps each supported construct to a clang-backed oracle or an explicit unsupported-feature diagnostic. |
| Adversarial inputs | Malformed syntax, malformed preprocessing, token caps, bounds checks, and object-section bounds tests pass. |
| Performance evidence | `benches/parser_pipeline.rs`, `benches/real_corpus.rs`, and `tests/preprocess_differential_benchmark.rs` publish GPU-vs-CPU measurements for compound workloads, not only primitive ops. |
| Fuzz readiness | `fuzz/README.md` documents the fuzz targets and the release candidate includes parser/preprocessor/object-format fuzz runs. |

## Production criteria

Production-ready status requires the promotion gate plus:

1. No unsupported syntax is silently accepted.
2. Every unsupported construct returns an actionable diagnostic.
3. Every hot path is GPU-reachable or explicitly classified as bootstrap/debug.
4. Every object payload section is versioned and rejected on incompatible schema.
5. The public API is frozen for the release train and covered by semver tests.
6. Clang differential coverage is measured by construct class, not by file count.
7. Benchmarks include real translation units and report throughput, latency, and
   memory movement.

## Evidence artifacts

The promotion gate is backed by these checked-in artifacts:

- `parity/PARITY_MANIFEST_V1.md`
- `parity/PREPROCESS_BENCHMARK_V1.md`
- `fuzz/README.md`
- `benches/parser_pipeline.rs`
- `benches/real_corpus.rs`
- `tests/parity_release_gate.rs`
- `tests/preprocess_differential_benchmark.rs`
- `tests/object_version_stability.rs`
- `tests/cuda_first_no_host_paths.rs`
- `tests/gpu_directive_kernels_real_gpu.rs`
- `tests/gpu_prep_kernel_libc_shape.rs`
- `tests/gpu_prepare_tu_source_e2e.rs`
- `tests/linux_grade_constructs_gpu_e2e.rs`
