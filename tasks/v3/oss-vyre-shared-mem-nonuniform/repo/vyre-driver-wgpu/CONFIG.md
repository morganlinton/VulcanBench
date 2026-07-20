# vyre-driver-wgpu  -  Configurability

The `vyre-wgpu` binary and the wgpu backend library expose a small,
explicit Tier A surface and consume the workspace Tier-B op corpus.

## Tier A  -  operational config

CLI flags + environment variables. Compiled defaults < env < CLI.

| Flag / env                       | Default | Purpose                                                                  |
|----------------------------------|---------|--------------------------------------------------------------------------|
| `--bench-only`                   | off     | Run the latency / throughput micro-benches only; skip parity checks.    |
| `--adapter <name>`               | auto    | Force a specific wgpu adapter (substring match against `Adapter::info`).|
| `--features <list>`              | runtime | Override the `wgpu::Features` mask (subgroup ops, timestamp queries…). |
| env `VYRE_PIPELINE_CACHE_ENTRIES`| `4096`  | In-memory pipeline-cache entry budget (audit item #18).                 |
| env `VYRE_PIPELINE_CACHE_BYTES`  | `512 MiB` | In-memory pipeline-cache byte budget (audit item #17).                  |
| env `VYRE_DISK_CACHE_DIR`        | `~/.cache/vyre/wgpu` | Override the on-disk pipeline-cache root.                                |
| env `VYRE_DISK_CACHE_MAX_BYTES`  | `4 GiB` | Hard ceiling for the disk pipeline cache.                              |
| env `VYRE_DISK_CACHE_TTL_DAYS`   | `30`    | Disk-cache eviction age. `0` disables time-based eviction.              |
| env `VYRE_TRACE_DISPATCH`        | `0`     | `1` = print one line per dispatch (timing, buffer count, hit/miss).     |

Every env var has a documented default in `vyre-driver-wgpu/src/runtime/`
and an integration test that round-trips parsing through the public
`WgpuBackendStats` surface.

## Tier B  -  community knowledge

The wgpu backend consumes the workspace op corpus at `rules/op/*.toml`.
Each op rule names a backend lowering (wgpu / spirv / cuda / reference)
and an emit contract. New backend coverage lands as a TOML rule; no Rust
change is needed to register an emitter when the rule references an
op already present in the inventory registry.

The schema for op rules lives at `rules/SCHEMA.md`. The wgpu backend
fails fast with a structured `BackendError::InvalidProgram` if an op
appears in a Program without a matching `rules/op/*.toml` entry.
