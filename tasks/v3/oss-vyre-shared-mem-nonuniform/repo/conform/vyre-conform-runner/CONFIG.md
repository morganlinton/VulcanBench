# vyre-conform  -  Configurability

Tier A and Tier B knobs for the `vyre-conform` binary. Both tiers are
required; together they form the entire surface that operators and the
community use to extend conformance without touching Rust.

## Tier A  -  operational config

CLI flags + environment variables + per-run TOML manifest. Tier A
controls *how* a run executes.

| Flag / env                  | Default          | Purpose                                                                    |
|----------------------------|------------------|----------------------------------------------------------------------------|
| `--backends <list>`        | `wgpu,reference` | Comma list of backends to dispatch.                                       |
| `--ops <regex>`            | `.*`             | Restrict run to op IDs matching the regex.                                |
| `--corpus <dir>`           | `rules/kat`      | Tier-B corpus root (override the default location).                       |
| `--certificates <dir>`     | `certs/`         | Where signed conformance certificates land.                               |
| `--strict-fp`              | off              | Enable the `strict-fp` feature gate; blocks float ULP drift.              |
| `--seed <u64>`             | nondet           | Deterministic seed for shrinking + witness generation.                   |
| `--features <list>`        | runtime-detected | Override capability detection (`subgroup-ops`, `gpu`, `strict-fp`, …).    |
| env `VYRE_CONFORM_VERBOSE` | `0`              | `1` = log per-witness; `2` = log per-pair; `3` = full backend traces.     |
| env `VYRE_CONFORM_TIMEOUT` | `300`            | Per-op deadline in seconds; the runner aborts the offending op only.      |

Compiled defaults < `vyre-conform.toml` < CLI/env. CLI wins.

## Tier B  -  community knowledge

The KAT corpus under `rules/kat/` is the canonical Tier-B layer for
conformance:

```
rules/kat/
├── primitive/        # tier-2 primitive ops
│   ├── logical/*.toml
│   ├── bitwise/*.toml
│   └── math/*.toml
├── composite/        # tier-3 composite ops
└── ...
```

A new conformance witness pair lands as a new `*.toml` file. The runner
auto-loads anything under the corpus root that matches `SCHEMA.md`. No
Rust change is required to extend the corpus.

`rules/SCHEMA.md` is the schema-of-truth; conformance KATs use the same
schema as the rest of the workspace's Tier-B layer.
