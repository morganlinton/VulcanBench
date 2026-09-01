# Harbor export (proof of shape)

Exports a VulcanBench Coding Intelligence Index v4 task directory into
[Harbor](https://github.com/harbor-framework/harbor) task format, the
framework behind Terminal-Bench. One task is fully converted and checked in
as the example: `exports/harbor/legacy-granarycore-binary-parity/`.

Usage (run with the repo venv; the script needs Python 3.11+ for `tomllib`):

```
.venv/bin/python scripts/harbor-export/export_task.py \
    tasks/coding-intelligence-index-v4/legacy-granarycore-binary-parity \
    exports/harbor/legacy-granarycore-binary-parity
```

The output directory is deleted and recreated on every run.

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
| `metadata.json` `agent_hints.suggested_timeout_s` (36000, the uniform 10-hour flat timeout) | `[agent].timeout_sec = 36000.0` | |
| `metadata.json` `test_timeout_s` (600, per test command) x 19 test commands | `[verifier].timeout_sec = 11400.0` | Our budget is per command; Harbor runs the whole suite once, so the equivalent upper bound is the product. Actual suite runtime is under 10 seconds. |
| grader `"tests"`: all fail_to_pass plus all pass_to_pass must pass | `test.sh` writes `1` to `/logs/verifier/reward.txt` iff the full pytest run exits 0, else `0` | All-or-nothing, matching our grading. |
| `gold_patch.diff` | omitted | Would enable Harbor's optional `solution/solve.sh` oracle, but the reference solution must not ship in the export. |
| `builder/` (secret C source, gold implementation) | omitted, and asserted absent | The exporter scans the output tree and fails if `builder`, `gold_patch.diff`, any `gold_*` file, or any `.c` file appears. |
| (no equivalent) | `[environment]` `cpus = 2`, `memory_mb = 4096`, `storage_mb = 10240`, `build_timeout_sec = 900.0` | Chosen defaults; our harness has no per-task resource declaration. Sized like the smaller terminal-bench tasks. |

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
  `python -m pytest -c /dev/null -p no:cacheprovider --rootdir=/tests -q
  /tests/reg_tests.py /tests/oss_tests.py` from `/app`, because
  `conftest.py` resolves the workspace as `Path.cwd()`; `--rootdir` pins
  conftest discovery under `-c /dev/null`.
- **Canary**: the task's canary line is stamped into `task.toml`, the
  Dockerfile, and `test.sh` (the copied test files already carry it),
  following terminal-bench's harbor-canary convention.

## Verification results (2026-09-01, Docker Desktop on this Mac)

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
  (linux/windows) but no architecture field. This task needs linux/amd64
  for the reference binary; nothing in `task.toml` can declare that, so it
  is a Dockerfile comment. Worth raising upstream or pinning via a
  prebuilt `docker_image` with an amd64-only manifest.
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
