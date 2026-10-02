# Harbor export

Exports VulcanBench Frontier v4 (`tasks/coding-intelligence-index-v4`) into
[Harbor](https://github.com/harbor-framework/harbor) task format, the
framework behind Terminal-Bench. All 23 tasks are exported and checked in
at `exports/harbor/frontier-v4/`, which Harbor runs as a local dataset.

Usage (Python 3.11+ for `tomllib`):

```
# Whole suite (what is checked in):
.venv/bin/python scripts/harbor-export/export_suite.py \
    tasks/coding-intelligence-index-v4 exports/harbor/frontier-v4

# One task:
.venv/bin/python scripts/harbor-export/export_task.py \
    tasks/coding-intelligence-index-v4/legacy-granarycore-binary-parity \
    /path/to/out/legacy-granarycore-binary-parity
```

Output directories are deleted and recreated on every run. Re-export
after any change under `tasks/coding-intelligence-index-v4/`.

`--with-solution` (both scripts) adds `solution/solve.sh` for Harbor's
`oracle` agent: the gold patch is applied to a scratch copy on the host and
the patched files ship under `solution/files/`. It is for local scoring
checks only. Write it outside `exports/` and never publish or commit it.

## Running with Harbor

```
uv tool install harbor
harbor run -p exports/harbor/frontier-v4 -a nop -n 1
CLAUDE_FORCE_OAUTH=1 CLAUDE_CODE_OAUTH_TOKEN=... harbor run \
    -p exports/harbor/frontier-v4 -a claude-code -m anthropic/claude-opus-5-5 \
    --effort high -n 1
```

- `-n 1`: Harbor defaults to 4 concurrent trials; our sweeps are serial
  (docs/DECISIONS.md), so pass `-n 1` for anything compared to the board.
- `--effort`: Harbor pins no default effort since 0.23, so always pass one.
  Harbor does not read `vulcanbench.toml`, so nothing there blocks
  `ultra`; never pass it.
- Subscription auth: `CLAUDE_FORCE_OAUTH=1` plus `CLAUDE_CODE_OAUTH_TOKEN`
  for `claude-code`; `CODEX_FORCE_AUTH_JSON=1` (uses `~/.codex/auth.json`)
  for `codex`.
- Harbor results are not yet comparable to the board: board runs execute
  on the host with no resource caps, while Harbor runs in a capped
  linux/amd64 container (emulated on Apple silicon). Run a parity check
  before mixing the two.

## The Harbor task format, as verified on 2026-09-01

A Harbor task is a directory:

```
<task>/
  instruction.md          # markdown instruction shown to the agent
  task.toml               # config: [task], [metadata], [agent], [verifier], [environment]
  environment/Dockerfile  # container image (build context is environment/)
  tests/test.sh           # verifier entrypoint
  solution/solve.sh       # optional oracle solution (we omit it, see below)
```

Sources for each claim (all fetched 2026-09-01):

- Directory layout, `task.toml` field inventory, reward files, network
  modes: https://harborframework.com/docs/task-format
- Reward reporting: the test script writes `/logs/verifier/reward.txt`
  (single number, 1 = solved) or `/logs/verifier/reward.json` (metric map);
  Harbor prefers `reward.json` and falls back to `reward.txt`, and errors if
  neither exists. Source: `src/harbor/verifier/verifier.py` in
  harbor-framework/harbor (`_parse_reward_json`, `_parse_reward_text`,
  `RewardFileNotFoundError`) and the reward path constants in
  `src/harbor/models/trial/paths.py` (`reward_text_path`,
  `reward_json_path`). The test script's own exit code is NOT the signal.
- Tests are hidden from the agent in shared verifier mode: the harness
  copies the task's `tests/` directory to `/tests` in the container only
  after the agent phase. Source: the `EnvironmentPaths` docstring in
  `src/harbor/models/trial/paths.py` ("tests/ ... Copied over by the
  Verifier after the agent runs") and the `upload_dir` call inside
  `Verifier.verify()` in `src/harbor/verifier/verifier.py`.
- `task.toml` schema: `TaskConfig` in `src/harbor/models/task/config.py`.
  Current `schema_version` default is `"1.4"`. `[agent].timeout_sec`
  (float, no default), `[verifier].timeout_sec` (default 600.0),
  `[verifier].environment_mode` (`"shared"` default when no
  `[verifier.environment]` is set), `[environment]` carries
  `build_timeout_sec`, `cpus`, `memory_mb`, `storage_mb`, `gpus`,
  `docker_image`, `os`, `env`, `network_mode`.
- Network policy: `network_mode` is one of `"public"` (default),
  `"no-network"`, `"allowlist"` (+ `allowed_hosts`); it exists as an
  `[environment]` baseline and as `[agent]`/`[verifier]` phase overrides.
  Source: `NetworkMode`, `PhaseNetworkPolicyConfig`,
  `BaselineNetworkPolicyConfig` in `src/harbor/models/task/config.py`, and
  https://harborframework.com/docs/task-format. So yes, Harbor supports
  phase-scoped network policy, and this export runs the verifier with the
  network off.
- Reference examples studied in github.com/harbor-framework/terminal-bench
  (branch `main`):
  - `tasks/interleaved-vigenere/{task.toml,tests/test.sh,tests/Dockerfile,environment/Dockerfile}`
    (raw URLs under
    https://raw.githubusercontent.com/harbor-framework/terminal-bench/main/tasks/...).
    Its `test.sh` is the pattern our generated entrypoint follows: run
    pytest, then `echo 1`/`echo 0` into `/logs/verifier/reward.txt`.
  - `tasks/legacy-utility-triage/{task.toml,tests/test.sh}`: shows
    `[agent] timeout_sec = 28800.0`, `[verifier] environment_mode =
    "separate"`, resource fields, and the `VERIFIER_LOG_DIR` fallback
    convention in `test.sh`.
  - `tasks/dataset.toml`: a dataset is a manifest listing
    `[[tasks]]` entries (`name`, `digest`) plus `[dataset]` info; built with
    `harbor add` / `harbor publish`.

## Format mapping

| VulcanBench source | Harbor output | Notes |
|---|---|---|
| `issue.md` | `instruction.md` | Verbatim, plus one appended line pointing at the `/app` workspace. |
| `repo/` | `environment/repo/`, COPYed to `/app` by the generated `environment/Dockerfile` | `/app` is the agent's workspace; build context is `environment/`. |
| `tests/` (pytest suite: `conftest.py`, `oss_tests.py`, `reg_tests.py`, `fixtures.json`) | `tests/` (copied unchanged) plus a generated `tests/test.sh` | Uploaded to `/tests` only after the agent phase (shared verifier mode), so fixtures and expected outputs are never agent-visible. |
| `metadata.json` `id` | `[task].name = "vulcanbench/<id>"` | |
| `metadata.json` `category`, `difficulty`, `languages`, `canary` | `[metadata]` | `decontamination_notes` and the per-test command lists are deliberately NOT exported. |
| `metadata.json` `agent_hints.suggested_timeout_s` (10800, the uniform 3-hour flat timeout since 2026-09-13) | `[agent].timeout_sec = 10800.0` | |
| `metadata.json` `test_timeout_s` (600, per test command) x the task's test command count (11 to 20) | `[verifier].timeout_sec` (6600.0 to 12000.0) | Our budget is per command; Harbor runs the whole suite once, so the equivalent upper bound is the product. Actual suite runtime is under 10 seconds. |
| grader `"tests"`: all fail_to_pass plus all pass_to_pass must pass | `test.sh` writes `1` to `/logs/verifier/reward.txt` iff the full pytest run exits 0, else `0` | All-or-nothing, matching our grading. |
| `gold_patch.diff` | omitted (only `--with-solution` uses it, for local oracle checks) | The reference solution must not ship in the export. The secret scan runs before the optional solution is added. |
| `builder/` (secret C source, gold implementation) | omitted, and asserted absent | The exporter scans the output tree and fails if `builder`, `gold_patch.diff`, any `gold_*` file, or any `.c` file appears. |
| (no equivalent) | `[environment]` `cpus = 2`, `memory_mb = 4096`, `storage_mb = 10240`, `build_timeout_sec = 900.0` | Chosen defaults, not calibrated. Our harness has no per-task resource declaration; its docker sandbox defaults to 2 CPUs and 2 GB, and board runs so far ran on the host with no caps. Sized like the smaller terminal-bench tasks. |
| (no equivalent) | `FROM --platform=linux/amd64` in `environment/Dockerfile` | `task.toml` has no architecture field, and every task ships a linux-amd64 reference binary the agent is expected to run. Without the pin, an arm64 host builds an arm64 image where neither shipped binary runs. |

Design choices:

- **Shared verifier mode** (no `[verifier].environment_mode`, no
  `[verifier.environment]`): the pytest suite must execute the agent's
  modified `/app/granarycore.py` in place, which shared mode gives us for
  free. The tradeoff versus terminal-bench's common `separate` mode: a
  malicious agent could tamper with `python`/`pytest` inside its own
  container. Listed under open questions.
- **Verifier network off**: `[verifier] network_mode = "no-network"`. The
  environment baseline stays at Harbor's default (`public`) because
  installed agents run inside the container and need their LLM API;
  runners can tighten this with runtime flags.
- **pytest invocation**: `test.sh` runs
  `PYTHONPATH=. python -m pytest -c /dev/null -p no:cacheprovider
  --rootdir=/tests -q /tests/reg_tests.py /tests/oss_tests.py` from `/app`
  (our grader's command, with every test in one run), because
  `conftest.py` resolves the workspace as `Path.cwd()`; `--rootdir` pins
  conftest discovery under `-c /dev/null`.
- **Canary**: the task's canary line is stamped into `task.toml`, the
  Dockerfile, and `test.sh` (the copied test files already carry it),
  following terminal-bench's harbor-canary convention.

## Verification results (2026-10-01, full suite)

With Harbor 0.23.0 installed, Docker not running:

1. All 23 exported tasks, and all 23 `--with-solution` copies, load
   through Harbor's own `harbor.models.task.task.Task` with the intended
   config: agent timeout 10800, verifier network off, 2 CPU / 4096 MB /
   10240 MB, `solution/solve.sh` present only in the oracle copies.
2. Host-side scoring check per task (host Python 3.14, pytest 9.0.3, the
   exact `test.sh` pytest command against the exported `tests/`): the
   unmodified repo fails for all 23, and the oracle files pass all 23,
   each passing exactly the number of tests listed in its
   `metadata.json`.

NOT yet verified for the full suite: image builds, the `oracle`/`nop`
agents under a real `harbor run`, and the container's Python 3.12 (the
earlier single-task Docker check below did cover it for granarycore).

## Verification results (2026-09-01, single task, Docker Desktop on this Mac)

What was verified locally, without Harbor installed:

1. `docker build --platform linux/amd64` of the generated
   `environment/Dockerfile` succeeds (base `python:3.12-slim`,
   `pytest==9.1.1` baked in; no network needed at verify time).
2. Simulated Harbor verify on the UNMODIFIED base repo: mounted the
   exported `tests/` read-only at `/tests`, created `/logs/verifier`, ran
   `bash /tests/test.sh` with `--network none`. Result: 5 passed
   (regression guards), 14 failed (fail_to_pass parity families),
   `reward.txt` = `0`. Exactly the expected unsolved signature; the
   harness itself runs clean.
3. Same run with `gold_patch.diff` applied to a scratch copy of the repo
   (patch applied only in a temp dir, never inside the export): 19 passed,
   `reward.txt` = `1`. Both reward directions work.
4. The legacy reference binary executes inside the amd64 container
   (`printf "" | ./legacy/run` prints the empty-session trailer,
   exit 0), so the agent-phase workflow the instruction describes is
   available under emulation on this arm64 host.

Real finding from step 2's first attempt: with the workspace COPYed into an
image layer, the test suite's `legacy/` quarantine fixture (a directory
`rename`) fails with `OSError: [Errno 18] Invalid cross-device link`,
because overlayfs cannot rename lower-layer directories. The generated
`test.sh` therefore re-roots the workspace into the container's writable
layer (`cp -a /app /app.verify && rm -rf /app && mv /app.verify /app`)
before running pytest. This would bite in real Harbor runs too, not just in
this simulation.

NOT verified (no Harbor CLI installed here):

- An actual `harbor run` end to end: instruction delivery to an agent, the
  harness's own `/tests` upload, `/logs/verifier` collection, and reward
  parsing were exercised only by simulation of the documented contract.
- Enforcement of `network_mode`, `timeout_sec`, and the `[environment]`
  resource fields by a real provider.
- `harbor add` / `harbor publish` dataset packaging (digests in
  `dataset.toml` are computed by the CLI).

## Open questions

- **Registry auth and publishing**: `harbor publish` and the hub
  (hub.harborframework.com; the default registry manifest lives at
  raw.githubusercontent.com/laude-institute/harbor/main/registry.json per
  `src/harbor/constants.py`) require claiming an org name. Who owns
  `vulcanbench/` there, and do we publish at all, or only hand datasets to
  labs directly?
- **Resource floors and ceilings**: `cpus`/`memory_mb`/`storage_mb` are
  free-form ints in the schema; provider-side minimums, maximums, and
  defaults are not documented. Our 2 cpu / 4 GB / 10 GB guess needs a check
  against a real runner.
- **CPU architecture**: `EnvironmentConfig` has an `os` field
  (linux/windows) but no architecture field. Every task needs linux/amd64
  for the reference binary; nothing in `task.toml` can declare that, so
  the Dockerfile pins `FROM --platform=linux/amd64` (since 2026-10-01).
  Cloud providers that build from the Dockerfile may ignore or reject the
  flag; a prebuilt `docker_image` with an amd64-only manifest is the
  fallback. Worth raising upstream.
- **Shared versus separate verifier**: shared mode keeps grading simple
  but grades inside a container the agent controlled. A `separate`
  verifier (tests/Dockerfile plus `artifacts = [...]` to carry
  `/app/granarycore.py` across) would be tamper-proof, at the cost of
  missing any helper modules an agent legitimately adds next to
  `granarycore.py`. Decide before exporting the full suite.
- **Oracle solution**: we omit `solution/solve.sh` because it would ship
  `gold_patch.diff`. If a private hand-off channel to a lab wants oracle
  verification, the exporter could gain a `--with-solution` flag that is
  never used for anything public.
- **Schema version**: current Harbor default is `"1.4"`; terminal-bench
  tasks in the wild still say `"1.0"`. We emit `"1.4"`. Pydantic does not
  appear to validate the value, but re-check when Harbor ships a breaking
  schema change.
