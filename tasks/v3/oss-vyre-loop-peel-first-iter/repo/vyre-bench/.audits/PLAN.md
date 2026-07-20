# Supersession notice

This plan is historical benchmark context. Active benchmark ownership and
targets are controlled by `../docs/optimization/OWNERSHIP.toml` and
`../docs/optimization/BENCH_TARGETS.toml`.

# vyre-bench release upgrade plan

I will implement the release upgrade for `vyre-bench` following the G1..G12 requirements in the mandated sequence.

## Implementation Sequence

1.  **G2: p99.9 / p99.99 / max tail** - Update `print_report` to include `p99.99`. Core fields and calculation are already present but need validation.
2.  **G6: Per-commit snapshot history** - Implement git info capture, snapshot storage in `snapshots/<commit>.json`, and `snapshot-diff` subcommand.
3.  **G3: Determinism gate** - Add `determinism_runs` to `RunConfig`, implement cross-run variance check, and update case status to "unstable" if it exceeds threshold.
4.  **G5: Cache hit rate** - Wire `cache_hit` from `vyre_driver::pipeline::compile`, compute `cache_hit_rate` in `MetricStats`, and show in CLI.
5.  **G1: CUDA event-based timing** - Add `src/probes/cuda_events.rs`, implement `dispatch_with_events`, and populate detailed GPU timing fields.
6.  **G4: Roofline / memory traffic** - Populate `bytes_read`/`bytes_written`, query peak bandwidth from NVML, and compute `roofline_pct`.
7.  **G8: Counter-bench / CPU oracle** - Implement `run_cpu_baseline`, enforce GPU > CPU performance, and update summary.
8.  **G7: Power & thermal normalization** - Extend NVML probe to capture clocks, detect thermal/clock drift, and mark "thermal_unstable".
9.  **G9: Workgroup / shape sweep matrix** - Implement `SuiteKind::Sweep` with workgroup and size variations.
10. **G10: Cross-backend matrix** - Implement `SuiteKind::CrossBackend` to iterate over all registered backends.
11. **G12: CLI extensions & Audit subcommand** - Add CLI flags and implement the `audit` subcommand with flame-graph breakdown.
12. **G11: CUPTI attribution** - Implement optional sub-stage attribution using CUPTI behind a feature flag.

## G1..G12 Checklist

- [x] G1: CUDA events
- [x] G2: Tail latencies (p999, p9999)
- [x] G3: Determinism gate
- [x] G4: Roofline
- [x] G5: Cache hit rate
- [x] G6: Commit snapshots
- [x] G7: Thermal normalization
- [x] G8: CPU baseline
- [x] G9: Sweep matrix
- [x] G10: Cross-backend
- [x] G11: CUPTI (Best effort)
- [x] G12: CLI + Audit
