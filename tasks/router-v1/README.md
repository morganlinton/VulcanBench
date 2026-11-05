# VulcanBench Router v1

A 16-task suite for measuring model routers: products that pick a backend
model per request and charge API-plus pricing. It answers two questions:

1. Does the served model change with language, codebase size and difficulty,
   or does the router key off surface features (prompt length) or nothing?
2. What does a router cost per task against the same tasks on a direct API at
   list price and on a flat subscription seat (Claude Max, ChatGPT Pro,
   Cursor)?

Design and budget are logged in `docs/DECISIONS.md` (2026-11-05).

## Design

A full factorial of {Python, Rust} x {small, large} x {easy, hard}, two tasks
per cell. "small" means `repo_scale` micro or small (under 1k lines); "large"
means medium and up, matching `_scale_filter` in `harness/suite.py`. Every
task's `metadata.json` carries a `router_cell` label, and `suite.json` lists
the same cells and named subsets (`python`, `rust`, `small`, `large`, `easy`,
`hard`).

| Cell | Task | Source suite | Source label |
| --- | --- | --- | --- |
| Python, small, easy | `oss-py-m2-00` | v1 | easy, small |
| Python, small, easy | `oss-py-m3-00` | v1 | easy, small |
| Python, small, hard | `py-bytecode-vm` | v1 | hard, small |
| Python, small, hard | `py-txn-kvstore` | v1 | hard, small |
| Python, large, easy | `oss-pygments-transparent-color` | cii-v1 | easy, xlarge |
| Python, large, easy | `oss-packaging-interpreter-tag-identifier` | cii-v1 | easy, medium |
| Python, large, hard | `oss-networkx-digraph-node-cuts` | cii-v1 | hard, xlarge |
| Python, large, hard | `oss-pydantic-none-discriminator` | cii-v1 | hard, xlarge |
| Rust, small, easy | `rs-borrow-split` | v1 | medium, small |
| Rust, small, easy | `rs-feature-gate` | v1 | medium, small |
| Rust, small, hard | `rs-bytecode-vm` | authored for this suite | hard, small |
| Rust, small, hard | `rs-txn-kvstore` | authored for this suite | hard, small |
| Rust, large, easy | `oss-jiff-offset-subtraction` | cii-v1 | easy, xlarge |
| Rust, large, easy | `oss-bitflags-iter-equal-names` | cii-v1 | easy, xlarge |
| Rust, large, hard | `oss-regex-leftmost-suffix-candidate` | v4 | veryhard, xlarge |
| Rust, large, hard | `oss-clap-mangen-override-usage` | cii-v1 | medium, xlarge |

The reused tasks are symlinks to their source folders (the four large Rust
repos alone are over 250 MB of tracked files, and `harness/suite.py` only
loads tasks from inside the suite folder, so links keep one copy in git).
Their scoring content is untouched; only the cosmetic `router_cell` key was
added to each source task's metadata, which `task_hash` ignores.

## Caveats

- **Difficulty is a label from the source suites.** The Rust small-easy pair
  is labelled medium at its source (no public Rust task is labelled easy
  with a small repo), and `oss-clap-mangen-override-usage` is labelled
  medium but went 0 of 3 for two of three reference models. The analysis
  also uses the measured pass rate of the cheapest leg as the empirical
  difficulty axis, so a mislabelled task does not sink the result.
- **The small cells are small on purpose.** The Python small-easy pair is a
  six-line package with a one-line fix (the floor of the ladder), and the
  small-hard pairs are near-greenfield implement-from-spec tasks whose
  starting repo is a stub. A router that treats a one-line fix and a
  from-scratch VM the same way has answered the first question already.
- **Contamination.** These are retired public tasks. That inflates pass
  rates equally across legs and does not touch the routing question, but
  no pass rate from this suite is a capability claim.
- **Rust small-hard is authored**, not mined: two single-crate tasks with no
  external dependencies, in the shape of `py-bytecode-vm` and
  `py-txn-kvstore`.

## Running

Runs go from a bench tag in a run worktree like every other suite
(`docs/HOW_WE_WORK.md`). Router legs use `router:<name>/<model>` (env
`ROUTER_<NAME>_BASE_URL` and `ROUTER_<NAME>_API_KEY`) or
`openrouter:openrouter/auto`; the served model and the gateway's own cost
land in each run's `summary.json` (`served_models`,
`router_reported_cost_usd`). Subscription legs run through `claude-code:`,
`codex:` and `cursor:`; set `VULCANBENCH_QUOTA_PROBE=1` on the Claude Code
leg to bracket every task with a usage-window probe. The single-request probe
ladder and the analysis live in `scripts/router-eval/`.
