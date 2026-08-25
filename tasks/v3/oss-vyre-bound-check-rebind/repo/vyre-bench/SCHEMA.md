# vyre-bench Result Schema (v1)

> Schema identifier: `vyre-bench.result.v1`

This document specifies the JSON output schema produced by `vyre-bench run --format json`.

## Top-Level Object

| Field | Type | Description |
|---|---|---|
| `schema` | `string` | Schema version identifier. Always `"vyre-bench.result.v1"`. |
| `run_id` | `string` | Unique run identifier, e.g. `"vyre-bench.smoke"`. |
| `suite` | `string` | Suite kind: `smoke`, `release`, `deep`, `gpu`, `sweep`, `cross-backend`, `evolve`, `adversarial`, `competition`, `honest`. |
| `git` | `object` | Git context at time of run. |
| `environment` | `object` | Hardware/OS environment snapshot. |
| `features` | `string[]` | Active feature flags for this run (e.g. `["backend:cuda"]`). |
| `cases` | `CaseResult[]` | Per-case results. |
| `summary` | `Summary` | Aggregate statistics. |

## `git` Object

| Field | Type | Description |
|---|---|---|
| `commit` | `string` | Full SHA-1 of HEAD. |
| `branch` | `string` | Current branch name. |
| `dirty` | `string` | `"true"` or `"false"`  -  whether the worktree has uncommitted changes. |
| `parent` | `string` | SHA-1 of HEAD~1. |
| `timestamp` | `string` | Unix timestamp of the HEAD commit. |

## `environment` Object

| Field | Type | Description |
|---|---|---|
| `hostname` | `string` | Machine hostname. |
| `os` | `string` | Operating system identifier. |
| `cpu_model` | `string` | CPU model string. |
| `gpu_name` | `string` | GPU adapter name from NVML. |
| `gpu_driver` | `string` | GPU driver version. |
| `gpu_memory_mb` | `u64` | Total GPU memory in MiB. |
| `cuda_version` | `string` | CUDA toolkit version, if available. |
| `rust_version` | `string` | rustc version used for the build. |
| `vyre_version` | `string` | vyre crate version. |

## `CaseResult` Object

| Field | Type | Description |
|---|---|---|
| `id` | `string` | Stable case identifier (e.g. `"foundation.elementwise.add.1m"`). |
| `status` | `string` | `"passed"`, `"failed"`, `"unstable"`, `"thermal_unstable"`, `"skipped"`. |
| `wall_ns` | `MetricStats` | Host-to-host wall-clock time in nanoseconds. |
| `correctness` | `string` | `"exact"`, `"approximate"`, `"unchecked"`, `"mismatch"`. |
| `metrics` | `Map<string, MetricStats>` | All captured metrics keyed by name. |
| `artifacts` | `string[]` | Paths to generated artifacts (SVGs, traces). |
| `speedup` | `f64 \| null` | Speedup ratio vs CPU baseline, if baseline was run. |
| `contract` | `ContractResult \| null` | Performance contract evaluation, if contract is set. |
| `competitors` | `CompetitorResult[] \| null` | Competitor run results, if competitors were wired. |

## `MetricStats` Object

| Field | Type | Description |
|---|---|---|
| `samples` | `u64` | Number of measured samples. |
| `min` | `u64` | Minimum observed value. |
| `max` | `u64` | Maximum observed value. |
| `mean` | `f64` | Arithmetic mean. |
| `median` | `u64` | Median (p50). |
| `p90` | `u64` | 90th percentile. |
| `p95` | `u64` | 95th percentile. |
| `p99` | `u64` | 99th percentile. |
| `p999` | `u64` | 99.9th percentile. |
| `p9999` | `u64` | 99.99th percentile. |
| `stddev` | `f64` | Standard deviation. |
| `cv` | `f64` | Coefficient of variation (stddev/mean). |
| `determinism_cv` | `f64 \| null` | Cross-run CV from determinism gate. |

## `Summary` Object

| Field | Type | Description |
|---|---|---|
| `total_cases` | `usize` | Total number of cases attempted. |
| `passed` | `usize` | Cases with `status == "passed"`. |
| `failed` | `usize` | Cases with `status == "failed"`. |
| `total_time_ns` | `u64` | Wall-clock time for the entire suite run. |
| `cache_hit_rate` | `f64` | Fraction of dispatches that hit the pipeline cache. |

## `CompetitorResult` Object

| Field | Type | Description |
|---|---|---|
| `name` | `string` | Competitor name (e.g. `"simdjson"`). |
| `version` | `string` | Pinned version string. |
| `wall_ns_p50` | `u64` | Median wall-clock time. |
| `throughput_gb_s` | `f64` | Throughput in GB/s. |
| `output_hash` | `string` | BLAKE3 hash of competitor output. |
| `parity` | `bool` | Whether output matches Vyre's output. |

## `ContractResult` Object

| Field | Type | Description |
|---|---|---|
| `primitive` | `string` | Name of the benchmarked primitive. |
| `target_speedup` | `f64` | Contract target (e.g. 10.0 for 10×). |
| `actual_speedup` | `f64 \| null` | Measured speedup. |
| `passed` | `bool` | Whether actual >= target. |

## Example

```json
{
  "schema": "vyre-bench.result.v1",
  "run_id": "vyre-bench.smoke",
  "suite": "smoke",
  "git": {
    "commit": "44a3d6b0f8977548ef32a2f60c96e3982cccaf4b",
    "branch": "main",
    "dirty": "false"
  },
  "environment": {
    "hostname": "bench-gpu-01",
    "gpu_name": "NVIDIA GeForce RTX 5090"
  },
  "features": ["backend:cuda"],
  "cases": [
    {
      "id": "foundation.elementwise.add.1m",
      "status": "passed",
      "wall_ns": {"samples": 30, "min": 12340, "max": 15670, "mean": 13500.0, "median": 13400, "p90": 14200, "p95": 14800, "p99": 15200, "p999": 15500, "p9999": 15670, "stddev": 820.0, "cv": 0.061},
      "correctness": "exact",
      "speedup": 155.2,
      "metrics": {},
      "artifacts": []
    }
  ],
  "summary": {
    "total_cases": 1,
    "passed": 1,
    "failed": 0,
    "total_time_ns": 2040000000,
    "cache_hit_rate": 0.0
  }
}
```
