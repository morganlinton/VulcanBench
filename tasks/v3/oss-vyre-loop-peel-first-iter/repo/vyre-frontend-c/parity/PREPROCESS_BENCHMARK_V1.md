# Differential preprocessing benchmark report v1

This report format backs release-plan item 30 for the frozen
`linux-lib-math-v6.8` parity target.

## Required command

```text
VYRE_LINUX_V68_ROOT=/path/to/linux-v6.8 \
./cargo_full test -p vyre-frontend-c --test preprocess_differential_benchmark \
  full_linux_lib_math_preprocess_benchmark_report_when_root_is_configured \
  -- --ignored --test-threads=1 --nocapture
```

`VYRE_LINUX_V68_ROOT` must point at the Linux source tree for commit
`90d1f30371ae3337beb01666b226320728d35c70`.

## Required evidence

- target id: `linux-lib-math-v6.8`
- source commit: `90d1f30371ae3337beb01666b226320728d35c70`
- target triple: `x86_64-unknown-linux-gnu`
- translation unit count: `12`
- clang wall time in nanoseconds
- vyre wall time in nanoseconds
- clang throughput in bytes/second
- vyre throughput in bytes/second
- GPU kernel launch count
- host write bytes
- host readback bytes
- non-empty clang output for every translation unit
- non-empty vyre output for every translation unit

## Gate

The report is valid release evidence only when
`PreprocessDifferentialBenchmarkReport::validate_release_evidence` accepts it
against `vyre-frontend-c/parity/linux_math_v6_8.toml`.
