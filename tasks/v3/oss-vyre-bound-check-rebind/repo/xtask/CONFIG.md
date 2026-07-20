# xtask  -  Configurability

xtask is the workspace's internal build-task runner. It is intentionally
`publish = false`. Its Tier A surface is exposed as subcommands; Tier B
is the op catalog xtask emits and consumes.

## Tier A  -  operational config

| Subcommand / env             | Default | Purpose                                                                   |
|------------------------------|---------|---------------------------------------------------------------------------|
| `xtask catalog`              |  -        | Emit the op catalog TOML manifest.                                        |
| `xtask perf-inventory wave1` |  -        | Run wave-1 perf inventory.                                                |
| `xtask lego-audit`           |  -        | Walk vyre-libs primitives and audit the LEGO surface.                    |
| `xtask publish-dryrun`       |  -        | Dry-run cargo publish across every workspace member.                     |
| env `XTASK_PARALLELISM`      | `nproc` | Host-side parallelism for catalog walk / lego-audit.                      |
| env `XTASK_VERBOSE`          | `0`     | `1` = log per-crate work; `2` = log per-file.                             |
| env `XTASK_TARGET_DIR`       | `target-xtask` | Override the per-agent target dir to avoid contention with cargo-fleet. |

xtask is meant to be invoked from the workspace root; running it from a
sub-directory is a usage bug, not a configuration choice.

## Tier B  -  community knowledge

xtask both produces and consumes `rules/op/*.toml`. The generators in
`xtask/src/lego_audit.rs` and `xtask/src/perf_inventory_wave1.rs` walk
the workspace and emit Tier-B TOML; downstream gates consume the same
TOML. Adding a new audit dimension is a code change in xtask AND a
schema change under `rules/SCHEMA.md`. Both must land in the same patch.
