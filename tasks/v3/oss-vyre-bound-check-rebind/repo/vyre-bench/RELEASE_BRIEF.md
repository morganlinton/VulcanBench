# Supersession notice

This brief is historical benchmark context. Active benchmark ownership and
targets are controlled by `../docs/optimization/OWNERSHIP.toml` and
`../docs/optimization/BENCH_TARGETS.toml`.

# vyre-bench release upgrade — implementation brief

**Workdir:** `/media/mukund-thiru/SanthData/Santh/libs/performance/matching/vyre/vyre-bench`
**Scope:** ONLY this crate. Do not edit any other crate. Do not invent new top-level modules. Extend the existing module tree (`api/`, `cases/`, `probes/`, `runner/`, `report/`, `cli.rs`).

## What exists today (read these first, do NOT recreate)

- `src/api/case.rs` — BenchCase trait, BenchContext, BenchMetrics, Correctness, BenchRun, performance contract types. KEEP THE TRAIT SIGNATURES. Extend.
- `src/api/metric.rs` — BenchMetrics struct with wall_ns/cpu_ns/compile_ns/validate_ns/optimize_ns/lower_ns/cache_lookup_ns/cache_hit/upload_ns/dispatch_ns/readback_ns/verify_ns/alloc_*/peak_rss/throughput_*. The fields ARE there; most are never populated. Your job is to populate them, not redesign them.
- `src/api/suite.rs` — SuiteKind enum (Smoke=10samp, Release=50, Deep=100, Gpu=20, Evolve=20, Adversarial=20, Competition=20).
- `src/runner/execute.rs` — 662 LOC. The main sample loop. Already has warmup, percentile computation (p50/p90/p95/p99), MetricStats. Already collects nvml + alloc snapshots per sample. Has `compute_stats` and `evaluate_contract`. EXTEND, DO NOT REPLACE.
- `src/probes/{nvml,cpu,environment,allocations}.rs` — basic probes. nvml uses nvidia-smi shellout (slow). cpu has rdtsc.
- `src/report/{json,sqlite}.rs` — sqlite at `.vyre_bench.db`, JSON via serde. KEEP both.
- `src/registry/mod.rs` — inventory-driven registration.
- `src/cli.rs` — Run/Compare/List/Explain/EvolveServer commands. Compare already does Welch's t-test + verdict.
- `src/cases/` — 14 cases including megakernel_latency, attention, matmul, dfa_match, etc.

**The current bench is good. It is not release. Closing the gap is the entire scope.**

## The gap — every microsecond auditable

### G1. CUDA event-based per-arm GPU timing (HARD REQUIREMENT)
Today `dispatch_ns` is `Instant::now()` around the host call — it includes API overhead, queue-submit, kernel-execute, and waits. It does NOT separate them.
- Add `src/probes/cuda_events.rs` (NEW FILE in existing module tree; do not invent a new top-level module).
- Wrap `cudarc` event-pair timing around: kernel queue-submit, kernel-execute (single fused kernel), driver synchronize.
- Populate three NEW fields on `BenchMetrics`: `kernel_queue_submit_ns`, `kernel_execute_ns`, `device_sync_ns`. Add them to the FIELDS array in `runner/execute.rs::collect_metric_fields` (17 → 20 entries) and to `metric_key`.
- For fused-multi-arm programs, also record per-arm execute time as `MetricPoint` entries in `metrics.custom` with names `arm.0.execute_ns`, `arm.1.execute_ns`, etc. Use `vyre::ir::Program::entry()` len for the arm count; the bench dispatches one program with N top-level Region nodes ≡ N arms.
- This requires extending `BenchContext::dispatch` to optionally return per-arm timing alongside the outputs. Add a new method `BenchContext::dispatch_with_events` that returns `(Vec<Vec<u8>>, KernelTimings)`. Existing `dispatch` stays unchanged for back-compat.
- Behind a `cuda_events` feature flag; degrade gracefully (return zeros + a `metrics.custom` entry `cuda_events_unavailable=1`) when not on a CUDA backend.

### G2. p99.9 / p99.99 / max tail (HARD REQUIREMENT)
Current MetricStats: `min/p50/p90/p95/p99/max`. Add `p999` (p99.9), `p9999` (p99.99) fields. Update `percentile()` in `runner/execute.rs` to handle them. Update `print_report` table.

### G3. Determinism gate (HARD REQUIREMENT)
- New field on RunConfig: `determinism_runs: u8` (default 3).
- After main measurement, re-run target_samples N times; report cross-run variance per metric in MetricStats.
- New field MetricStats: `determinism_cv: f64` (coefficient of variation across runs, expressed as fraction).
- Determinism breach (`> 0.5%` for `kernel_execute_ns` on `DeterminismClass::Deterministic` cases) sets case status to `"unstable"` and increments a new `summary.unstable` counter.

### G4. Roofline / memory traffic (HARD REQUIREMENT)
- New BenchMetrics fields: `bytes_read`, `bytes_written`, `atomic_op_count`, `peak_bandwidth_gb_s`, `achieved_bandwidth_gb_s`, `roofline_pct` (achieved/peak × 100).
- Cases populate `bytes_read`/`bytes_written` from the Program's buffer access pattern. Default impl: sum of (input bytes + output bytes); cases that know better override.
- Peak bandwidth: query NVML `clocksMem` × bus_width × 2 / 8. Hardcode RTX 5090 (1008 GB/s peak) and 4090 (1008 GB/s peak) for now if NVML clock query is unavailable; the lookup belongs in `probes/nvml.rs`.
- Roofline reporting: new column in CLI table.

### G5. Cache hit rate (HARD REQUIREMENT — currently a stub)
- `BenchMetrics::cache_hit` exists, never populated. Wire it from `vyre_driver::pipeline::compile`'s validation cache (it returns `bool` for cached vs fresh). Pipeline already has the wiring; the bench just reads it.
- New aggregate at MetricStats: per-case `cache_hit_rate: f64` = fraction of runs where `cache_hit == Some(true)`.
- Print as a CLI column.

### G6. Per-commit snapshot history (HARD REQUIREMENT)
- Today `git: BTreeMap<String, String>` is built empty in `execute_suite`. Populate it with `commit`, `branch`, `dirty`, `parent_commit`, `commit_timestamp` by shelling `git -C <bench dir> rev-parse HEAD` etc.
- Snapshot directory: `vyre-bench/snapshots/<commit>.json` (overwrite if exists). Schema = ReportSchema.
- New CLI command `Compare-snapshot --base <commit_sha>` that diffs current run against `snapshots/<commit_sha>.json`. Prints per-case delta + Welch p-value + verdict (improve/regress/flat/noisy) and exits non-zero if any case regresses by >1σ.
- Add `snapshots/` to `.gitignore`-equivalent — actually, snapshots SHOULD be committed. Just make sure the directory is created on demand and human-readable (pretty JSON).

### G7. Power & thermal normalization (currently captured, never used)
- `capture_nvml_telemetry` writes `power_draw_w`, `temperature_c`, `utilization_gpu_pct`, `utilization_mem_pct` into `metrics.gpu_counter`. They show up in samples but never gate.
- New behavior: if temperature drift across samples > 5°C OR clock frequency drift > 5%, surface `metrics.custom` entry `thermal_drift=1` and set case status to `"thermal_unstable"`.
- Capture clock frequency: extend `nvml.rs` to also query `clocks.current.graphics`, `clocks.current.memory`. Add to GpuCounter list.

### G8. Counter-bench: every primitive vs CPU oracle, byte-for-byte AND timing (HARD REQUIREMENT)
- Today `BenchRun.baseline_outputs` is optional. Many cases don't run the CPU oracle. The contract says: every primitive runs CPU oracle every run, byte-for-byte parity asserted, AND CPU timing recorded.
- New trait method `BenchCase::run_cpu_baseline(&self, ctx, prepared) -> Result<BenchRun, BenchError>` with default impl that calls `ctx.reference.dispatch`. Cases that don't have a CPU oracle override to return `Err(BenchError::ExecutionFailed("no CPU baseline"))` — the runner catches that and skips the comparison gracefully but logs it.
- Store CPU timings as `baseline_kernel_execute_ns` (same machinery as G1, but on CPU it's the dispatch wall_ns).
- New summary field: `summary.gpu_slower_than_cpu_count` — how many cases had `wall_ns.p50 > baseline_wall_ns.p50`. Any non-zero value is a P0 finding; bail with non-zero exit code in `Run` command if `enforce_budgets`.

### G9. Workgroup / shape sweep matrix
- New `SuiteKind::Sweep`. When invoked, each case is run for every workgroup config in `[64,1,1] / [128,1,1] / [256,1,1] / [512,1,1] / [1024,1,1]` AND for every shape-size in case-defined `BenchCase::sweep_sizes() -> Vec<u64>` (default empty).
- Output: per (case, workgroup, size) row in the report.
- Don't break the existing case ID convention — qualify with suffix `<id>::wg256::n4096`.

### G10. Cross-backend matrix
- New `SuiteKind::CrossBackend`. Iterate `vyre_driver::backend::registered_backends_by_precedence_slice()`. For each linked dispatch backend, run every case. Report a backend-vs-backend table.
- Already have backend acquire path in `acquire_backend(Some(id))`. Just iterate.

### G11. Sub-stage attribution inside the kernel (BEST EFFORT — soft requirement)
- Use cuptiActivity / Nsight Compute API for per-PTX-block cycle counts ONLY when the env var `VYRE_BENCH_CUPTI=1` is set. Behind a `cupti` feature flag.
- Default off; emits `metrics.custom` entries `block.0.cycles`, `block.1.cycles`, etc. when available. NEVER fail the run if cupti is missing.

### G12. CLI extensions (do not break existing flags)
- `vyre-bench run` extends with `--with-cpu-baseline` (default true), `--determinism-runs N` (default 3), `--snapshot-on-pass` (write `snapshots/<commit>.json` after a clean run).
- `vyre-bench snapshot-diff --base <commit>` (NEW SUBCOMMAND).
- `vyre-bench audit <case_id>` (NEW SUBCOMMAND) — runs ONE case with maximum verbosity: every metric, every probe, every NVML counter, every per-arm timing, every cache hit. Prints a flame-graph-style breakdown:
```
case=runtime.megakernel.dispatch.256 wall=23.4ms
├─ host.queue_submit  =  87us  (0.4%)
├─ kernel.execute    = 18.2ms  (77.8%)
│  ├─ arm.0          = 12.1ms  (51.7%)
│  ├─ arm.1          =  4.8ms  (20.5%)
│  └─ arm.2          =  1.3ms  ( 5.6%)
├─ device.sync       =  2.1ms  ( 9.0%)
├─ readback          =  1.9ms  ( 8.1%)
└─ host.demux        =  1.1ms  ( 4.7%)
```
The percentages MUST sum to within 1% of wall_ns. Drift ≥ 1% means we have unattributed time — surface a `metrics.custom` entry `unattributed_ns=N` and the audit command BOLDS it red.

## Hard "do not" rules

- **Do NOT add new top-level modules at the workspace level.** Everything stays inside `vyre-bench/src/`.
- **Do NOT add Co-Authored-By or any AI attribution in commits.**
- **Do NOT use heredocs.** printf or Write tool.
- **Do NOT touch `vyre`, `vyre-foundation`, `vyre-driver*`, `vyre-runtime`, `vyre-libs` source.** Only consume their public API.
- **Do NOT split files preemptively.** Keep the existing layout. Only split if a file crosses 500 LOC due to your additions.
- **Do NOT refactor existing cases.** Add new fields with defaults; existing cases keep working.
- **Do NOT change CLI flag names or default values.** Add only.
- **Do NOT delete the sqlite path or the `compare` Welch test.** Both stay.
- **Do NOT introduce new dependencies beyond `cudarc` (already in vyre-driver-cuda) and `git2` if needed for snapshot history. Prefer shelling `git` if git2 is heavy.**
- **Do NOT skip tests.** Every new metric field has a unit test that asserts its inclusion in the report; every new CLI subcommand has a smoke test.

## Acceptance gate

The release bench is delivered when:

1. `cargo test -p vyre-bench` is green.
2. `cargo run -p vyre-bench -- audit runtime.megakernel.dispatch.256` prints the flame-graph breakdown with `unattributed_ns < 1% of wall_ns`.
3. `cargo run -p vyre-bench -- run --suite release --with-cpu-baseline` reports a CPU-vs-GPU speedup column for every case, AND fails with non-zero exit if any case has GPU slower than CPU.
4. `cargo run -p vyre-bench -- run --suite release` followed by a code change followed by `cargo run -p vyre-bench -- snapshot-diff --base <prev_sha>` prints a delta table with Welch p-values.
5. `cargo run -p vyre-bench -- run --suite release` populates `kernel_queue_submit_ns`, `kernel_execute_ns`, `device_sync_ns`, `bytes_read`, `bytes_written`, `roofline_pct`, `cache_hit_rate`, `determinism_cv` for every case (or explicit zero + `cuda_events_unavailable=1` flag).
6. `summary.unstable` and `summary.gpu_slower_than_cpu_count` are present and accurate in the report.

## Sequencing

Do these in order. Land each as a separate commit. Build green between each.
1. G2 (p999/p9999/max tail).
2. G6 (git/snapshots/Compare-snapshot subcommand).
3. G3 (determinism gate).
4. G5 (cache hit rate wiring).
5. G1 (CUDA events — biggest unknown, save for after the easier ones).
6. G4 (roofline).
7. G8 (CPU oracle gate).
8. G7 (thermal normalization).
9. G9 (workgroup/shape sweep).
10. G10 (cross-backend).
11. G12 (audit subcommand) — needs G1 done.
12. G11 (cupti) — last; soft requirement.

If any item blocks (e.g. cudarc events API doesn't expose what we need), STOP, write a `BLOCKED.md` in `vyre-bench/` describing what's missing and what would unblock, and continue with the next item.
