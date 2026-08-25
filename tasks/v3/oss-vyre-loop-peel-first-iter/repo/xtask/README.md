# xtask

Build task runner for the vyre workspace.

## Usage

```bash
cargo xtask <subcommand> [options]
```

## Subcommands

| Subcommand | Description |
|------------|-------------|
| `acceleration-plan-gate [--plan PATH]` | Enforces evidence-backed VX row shape in `docs/optimization/ALL_AXES_ACCELERATION_PLAN.md`. |
| `generate-tests [--op NAME \| --all]` | Wraps `vyre-gen-tests` to materialize generated conformance tests. |
| `mutation-gate [--tests PATH]` | Wraps H6 (mutation gate) to probe agent-written tests for surviving mutations. |
| `coverage-check` | Runs the workspace coverage check target. |
| `conform-verify` | End-to-end pipeline: `generate-tests` → `mutation-gate` → `coverage-check` → `cargo test`. Exits non-zero on any failure. |
| `quick-check --op NAME` | Minimal verification path for a single op (wraps the `contribute` binary). Target <10s. |

## Layout

`xtask/` is a workspace member so it shares the workspace target directory and
lockfile.
