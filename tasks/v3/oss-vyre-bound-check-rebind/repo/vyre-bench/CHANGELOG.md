# vyre-bench Changelog

## 2026-04-30  -  Release Infrastructure Upgrade

### §3: Benchmark Bugs Fixed
- **B-1**: Throughput consistency  -  fixed metric collection so `wall_ns` is always populated.
- **B-2**: Baseline determinism  -  CPU baseline now uses deterministic seed and pinned thread count.
- **B-3**: Suite completeness  -  `SuiteKind::Smoke` now includes all foundation cases.
- **B-4**: Min samples gate  -  `measured_samples < 30` panics with clear message. Override via `VYRE_ALLOW_FEW_SAMPLES`.
- **B-5**: Result schema  -  `wall_ns` surfaced as top-level metric in every case.
- **B-6**: DFA match  -  documented that grid IS auto-inferred correctly (65536 threads), added `bytes_read`/`bytes_written` telemetry.

### §5: G1–G5 Verification
- **G1**: CUDA event timing  -  verified `kernel_queue_submit_ns`, `kernel_execute_ns`, `device_sync_ns` populated.
- **G2**: Tail latencies  -  verified p999/p9999/max monotonicity.
- **G3**: Determinism gate  -  verified `determinism_cv < 0.005` for stable cases, flaky case gets `"unstable"`.
- **G5**: Cache hit rate  -  verified second-run cache hit > 95%.

### §6: G4/G6–G12 Completion
- **G4**: Roofline  -  `bytes_read`, `bytes_written`, `peak_bandwidth_gb_s` populated from NVML. `roofline_pct` column added.
- **G6**: Per-commit snapshots  -  `execute_suite` now writes `snapshots/<commit>.json` automatically.
- **G7**: Thermal normalization  -  NVML captures temperature drift, `thermal_unstable` metric populated.
- **G9**: Sweep matrix  -  `SuiteKind::Sweep` iterates workgroup × size grid via `execute_run_matrix`.
- **G10**: Cross-backend matrix  -  `SuiteKind::CrossBackend` runs all dispatch-capable backends.
- **G12**: CLI  -  `list`, `run`, `snapshot-diff`, `compare` subcommands verified.

### §4: Honest Workload Suite (3 of 14)
- `hashtable.openaddr.probe.10m`  -  1M random probes against a prebuilt 10M-key open-addressing hash table.
- `interpreter.bytecode.dispatch.10m`  -  4096-instance stack-based bytecode VM, 2500 instructions each.
- `crypto.aes_ctr.encrypt.10mb`  -  AES-128-CTR encryption over 10MB with an OpenSSL EVP AES-NI baseline.

### §8: Competitor Matrix
- Added `CompetitorRun` trait + `CompetitorMetrics` / `CompetitorResult` structs.
- Created `competitors.toml` with pinned versions for all planned honest workload competitors.
- `BenchLayer::Honest` and `WorkloadClass::Honest` enum variants added.
- `PerformanceContract::cpu_sota_10x()` and `cpu_sota_3x()` constructors added.

### §9: CI Regression Gate
- `.github/workflows/bench-regression.yml`  -  runs on PR + push to main, self-hosted GPU runner.

### §11: LFS Setup
- `.gitattributes`  -  tracks `corpus/honest/**/*` via git-lfs.
- `scripts/fetch_honest_corpus.sh`  -  idempotent corpus downloader (currently no-op since all data is synthesized).

### §12: Documentation
- `SCHEMA.md`  -  complete JSON schema for result format.
- `CHANGELOG.md`  -  this file.

### Infrastructure
- Removed redundant `.into()` on `Arc` in `evaluate_candidate_headless`.
- Fixed borrow in `CudaCompiledPipeline::dispatch_borrowed` and `dispatch_borrowed_timed`.
- `SuiteKind::Honest` added with full `FromStr` + `as_str` support.

### Test Coverage
- 14 test files, 17+ tests, all green.
- New tests: `g6_snapshot`, `g7_thermal`, `g9_sweep`, `g10_cross_backend`, `g12_cli`, `min_samples_gate`, `result_schema`.
