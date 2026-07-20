# vyre-bench

Canonical benchmarking and performance-regression infrastructure for the [Vyre](../README.md) GPU compiler.

## Architecture

```
vyre-bench
├── api/           # Core types: BenchCase trait, metrics, suites, competitor API
├── cases/         # Benchmark implementations (one file per workload)
├── runner/        # Execution engine, snapshot diffing, execute_suite()
├── report/        # JSON schema, scorecard generation
├── evolve/        # OpenEvolve-style evolutionary optimization server
├── probes/        # NVML environment probing, thermal normalization
├── registry/      # inventory-based case collection
├── cli.rs         # CLI: run, list, explain, snapshot-diff, compare, dashboard
└── main.rs        # Entry point
```

## Quick Start

```bash
# List all registered benchmarks
cargo_full run -p vyre-bench -- list

# Run the smoke suite (30 measured samples, GPU required)
cargo_full run -p vyre-bench --release -- run --suite smoke --measured-samples 30

# Run honest workload suite
cargo_full run -p vyre-bench --release -- run --suite honest --measured-samples 30

# Generate CUDA release evidence
cargo_full run -p vyre-bench --release -- run --backend cuda --suite release --measured-samples 30 --warmup-samples 3 --enforce-budgets

# Generate WGPU fallback evidence
cargo_full run -p vyre-bench --release -- run --backend wgpu --suite release --measured-samples 30 --warmup-samples 3 --enforce-budgets

# Compare two runs
cargo_full run -p vyre-bench -- compare --baseline baseline.json --candidate candidate.json

# Generate HTML dashboard from latest snapshot
cargo_full run -p vyre-bench -- dashboard --output dashboard/
```

## Suite Kinds

| Suite | Purpose | Min Samples |
|---|---|---|
| `smoke` | Fast CI gate, foundation primitives | 30 |
| `release` | Full coverage pre-release | 30 |
| `deep` | Extended analysis, tail latencies | 100 |
| `gpu` | GPU-specific capabilities | 30 |
| `honest` | Real-world workloads with CPU baselines | 30 |
| `sweep` | Workgroup × size parameter grid | 5 |
| `cross-backend` | Same program across CUDA/SPIR-V/WGPU | 30 |
| `evolve` | Evolutionary optimization search | 50 |
| `adversarial` | Pathological inputs, register exhaustion | 30 |
| `competition` | Parameter golf scoring | 30 |

## Honest Workloads

These benchmarks use CPU baselines that run the same algorithm or contract shape, enabling defensible speedup claims. The release suite contains both real algorithm workloads and synthetic contract workloads: synthetic cases are allowed only when they model a named release contract with exact CPU-output parity and an explicit SOTA baseline class.

| Workload | Description | Contract |
|---|---|---|
| `hashtable.openaddr.probe.10m` | Open-addressing hash table: 1M probes against a prebuilt 10M-key table | 10× vs hashbrown |
| `interpreter.bytecode.dispatch.10m` | Bytecode VM: 4096 instances × 2500 instructions | 3× vs interpreted |
| `crypto.aes_ctr.encrypt.10mb` | AES-128-CTR over 10MB | 3× vs OpenSSL EVP AES-NI |
| `regex.backtracking.adversarial` | `(a+)+b` pattern on hostile inputs (4096 instances) | 100× vs PCRE2 |
| `bigint.modexp.4096` | 1024 instances of modular exponentiation | 3× vs rug/GMP |

## Release Workloads

The `release` suite must cover at least 12 workload families before Vyre `0.4.2` / Weir `0.0.1` can ship. CUDA is the preferred release backend; WGPU is the portable GPU fallback. Every row below has exact output parity against a CPU baseline and a performance contract against a serious CPU baseline class.

| Workload family | Case id | Contract |
|---|---|---|
| Conditional rule evaluation | `release.condition_eval.1m` | 100× vs optimized CPU rule-condition evaluator |
| String bitmap scatter | `release.string_bitmap_scatter.1m` | 100× vs Hyperscan/ripgrep-class CPU bitmap materialization |
| Offset/count aggregation | `release.offset_count_aggregation.1m` | 100× vs SIMD CPU aggregation over match streams |
| PE/header metadata predicates | `metadata.condition.filesize_header.1m` | 50× vs optimized CPU PE-header predicate evaluator |
| Entropy/window predicates | `release.entropy_window.1m` | 100× vs SIMD CPU rolling entropy baseline |
| Quantified condition loops | `release.quantified_condition_loops.1m` | 100× vs optimized CPU quantified-condition evaluator |
| Alias/reaching-definition predicates | `release.alias_reaching_def.1m` | 25× vs LLVM-style sparse dataflow and alias baseline |
| IFDS witness predicates | `release.ifds_witness.1m` | 25× vs optimized CPU graph reachability/witness baseline |
| C AST traversal predicates | `release.c_ast_traversal.1m` | 25× vs tree-sitter/libclang-class CPU AST traversal |
| Persistent megakernel queued batches | `release.megakernel_queue.1m` | 100× vs optimized CPU batched condition evaluator |
| E-graph saturation predicates | `release.egraph_saturation.1m` | 10× vs egg/egraph CPU saturation baseline |
| Sparse fired-rule readback | `sparse.compaction.count.1m` | 100× vs optimized CPU fired-rule collection |
| Callgraph reachability | `callgraph.reachability.step.262k` | 25× vs optimized CPU graph reachability |

## Verification Gates

Every benchmark run enforces these quality gates:

- **G1**: CUDA event timing populates `kernel_queue_submit_ns`, `kernel_execute_ns`, `device_sync_ns`
- **G2**: Tail latencies are monotonic: min ≤ p50 ≤ p90 ≤ p95 ≤ p99 ≤ p999 ≤ p9999 ≤ max
- **G3**: Determinism gate: `CV < 0.005` for stable cases across 10 runs
- **G4**: Roofline metrics: `bytes_read`, `bytes_written`, `peak_bandwidth_gb_s`
- **G5**: Pipeline cache hit rate: second-run cache hit > 95%
- **G6**: Per-commit snapshots: `snapshots/<commit>.json` written automatically
- **G7**: Thermal normalization: NVML temperature monitoring, `thermal_unstable` detection
- **G9**: Sweep matrix: workgroup × size parameter grid
- **G10**: Cross-backend: CUDA/SPIR-V/WGPU parity verification
- **G12**: CLI verification: all subcommands produce correct output

## CI Integration

The `bench-regression.yml` workflow runs on every PR and push to `main`:
1. Builds `vyre-bench` in release mode on a self-hosted GPU runner
2. Runs smoke and honest suites with 30 measured samples
3. Compares against the baseline snapshot (if available)
4. Comments the comparison on the PR
5. Fails if any case regresses by > 1σ

## Schema

Result JSON follows the `vyre-bench.result.v1` schema. See [SCHEMA.md](SCHEMA.md) for full documentation.

## Competitor Matrix

Competitors are declared in `competitors.toml` with pinned versions:

```toml
[[competitor]]
name = "hashbrown"
crate = "hashbrown"
version = "=0.16.1"
workloads = ["hashtable.openaddr.build_probe.10m"]
```

The `CompetitorRun` trait in `api/competitor.rs` enables side-by-side A/B comparisons.

## Dashboard

`vyre-bench dashboard --output dashboard/` generates:
- `index.html`: interactive scorecard with dark-mode UI
- Per-case SVG bar charts (p50/p99/max)
- `cross-backend.svg`: cross-backend comparison
- `scorecard.md`: markdown summary
- `data/results.json`: raw data

## Adding a New Benchmark

1. Create `src/cases/my_workload.rs` implementing `BenchCase`
2. Add `inventory::submit! { &MyWorkload as &'static dyn BenchCase }` at the bottom
3. Register in `src/cases/mod.rs`
4. Run `cargo_full test -p vyre-bench` to verify integration
5. Add the competitor entry to `competitors.toml` if applicable

## Release evidence

Release readiness for this document is proven through the Vyre/Weir evidence manifest and generated artifacts under `release/evidence/`. Claims here must map to concrete gate output, benchmark output, conformance output, parser corpus output, or documentation proof files before the release requirement can be closed.

Concrete evidence anchors:

- `release/evidence/benchmarks/release-workload-matrix.json`
- `release/evidence/benchmarks/cuda-release-suite.json`
- `release/evidence/benchmarks/wgpu-fallback-suite.json`
- `release/evidence/benchmarks/bench-release-axes.json`
