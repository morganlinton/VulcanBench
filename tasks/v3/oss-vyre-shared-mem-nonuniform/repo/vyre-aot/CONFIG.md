# vyre-aot  -  Configurability

`vyre-aot` is a library; embedding tools surface the knobs below
through their own CLIs. The library exposes both Tier A (operational)
and Tier B (community knowledge) layers so embedders do not need to
re-invent the schema.

## Tier A  -  operational config

Library callers configure the AOT pipeline through the
`AotCompileOptions` struct, optionally seeded from a TOML file the
caller picks up.

| Field / env                 | Default        | Purpose                                                                   |
|----------------------------|----------------|---------------------------------------------------------------------------|
| `target = "secondary_text"`           | `secondary_text`          | Lowering target. Resolved through the `vyre-driver` AOT emitter registry. |
| `sm = "<arch>"`            | `sm_75`        | NVIDIA SM target for PTX (e.g. `sm_75`, `sm_86`, `sm_90`).               |
| `optimize = "speed" \| "size"` | `speed`     | Target-byte optimization profile.                                         |
| `embed_launcher`           | `true`         | Emit a self-contained Rust launcher binary alongside the artifact.       |
| `compress = "none" \| "lzma" \| "brotli"` | `lzma` | On-disk artifact compression.                                             |
| env `VYRE_AOT_CACHE_DIR`   | `$CARGO_TARGET_DIR/vyre-aot` | Cache root for compiled target artifacts.                      |
| env `VYRE_AOT_VERBOSE`     | `0`            | `1` = log per-program lowering choices.                                   |

Compiled defaults < caller-supplied `*.toml` < direct field overrides.
Embedders are expected to surface `VYRE_AOT_*` to their users verbatim.

## Tier B  -  community knowledge

The AOT pipeline consumes:

- `rules/op/*.toml`  -  op lowering rules (shared with the runtime).
- `rules/aot/*.toml`  -  AOT-specific shape contracts (allowed
  workgroup sizes, register pressure budgets, target SM matrix). New
  contracts land as TOML; no Rust change required.

Schemas for both directories live at `rules/SCHEMA.md`.
