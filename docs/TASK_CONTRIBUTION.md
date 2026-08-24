# Contributing a Task

A VulcanBench task is a small, self-contained software-engineering problem with a
hidden test suite and a reference solution. Every task must pass the validator
(`make validate-tasks`) before it is trusted.

## Directory layout

```
tasks/v1/<task-id>/
├── metadata.json        # id, category, languages, difficulty, task_complexity, source,
│                        #   decontamination_notes, and declarative tests
├── issue.md             # the problem statement shown to the agent (no solution)
├── repo/                # the STARTING repo state given to the agent
├── tests/               # HIDDEN tests, overlaid onto the workspace at scoring,
│                        #   NEVER shown to the agent
├── gold_patch.diff      # the reference fix (git diff vs repo/), used by the validator
└── expected_metrics.json
```

(`repo_snapshot.tar.gz` is also accepted instead of `repo/` for large imported
repos.)

For **medium/large** OSS slices, see [LARGE_TASK_HANDBOOK.md](LARGE_TASK_HANDBOOK.md).

## metadata.json

```json
{
  "id": "py-ttl-cache-expiry",
  "category": "bug_fix",                 // bug_fix | feature | refactor | concurrency
  "languages": ["python"],               // python | go | typescript | javascript
  "difficulty": "easy",                  // easy | medium | hard
  "task_complexity": "localized",        // localized | multi_file | system | architecture
  "created": "2026-05-30",
  "source": "hand-authored",             // or "oss" (then fill provenance below)
  "decontaminated": true,                // REQUIRED bool; hand-authored MUST be true
  "decontamination_notes": "Original scenario written 2026-05-30; not from any public repo.",
  "repo_scale": "small",
  "tests": {
    "fail_to_pass": [
      {"name": "expiry", "cmd": "python -m pytest test_expiry.py::test_x -q"}
    ],
    "pass_to_pass": [
      {"name": "basic", "cmd": "python -m pytest test_basic.py -q"}
    ]
  }
}
```

### Task complexity

`task_complexity` captures the shape of the engineering work independently from
the human-labeled expected `difficulty`:

| Value | Meaning |
|-------|---------|
| `localized` | Fix is concentrated in one source file |
| `multi_file` | Fix spans two source files |
| `system` | Fix spans three or more source files or cross-cutting behavior |
| `architecture` | Explicit design/API/ownership decision beyond a normal patch |

For existing tasks, VulcanBench infers this deterministically from
`gold_patch.diff`: one touched source file is `localized`, two is `multi_file`,
and three or more is `system`. Use `architecture` only for future tasks that are
authored to test architectural judgment.

### Optional fields (large / OSS tasks)

| Field | Purpose |
|-------|---------|
| `repo_scale` | `micro` \| `small` \| `medium` \| `large`: drives agent step/time defaults |
| `base_commit` | Required for `source: oss`: upstream SHA of the slice |
| `upstream` | `{ "url", "issue", "pr", "fix_commit" }` structured provenance |
| `test_timeout_s` | Per-task verifier wall-clock budget (seconds) |
| `agent_hints` | `{ "suggested_max_steps", "suggested_timeout_s", "entry_paths" }` |
| `allow_large_snapshot` | Set `true` to allow &gt;100MB uncompressed `repo_snapshot.tar.gz` |
| `setup` | `[{"name": str, "cmd": str}]`: commands run before the agent starts (e.g. build warm-up) |
| `setup_timeout_s` | Per-setup-command timeout in seconds (default 600) |

- **`fail_to_pass`**: commands that **fail on `repo/`** and **pass after
  `gold_patch.diff`**. This is the real signal; `functional` is the fraction that
  pass.
- **`pass_to_pass`**: commands that pass before *and* after (regression guard).
  If any fail, `functional` is gated to 0.
- Each `cmd` runs in the workspace; **exit code 0 = pass**.

### Hidden tests overlay onto the workspace root

The task's `tests/` directory is overlaid onto the workspace **root** at scoring
time, preserving relative paths, so place each test file where it must live:

| Language   | Put the test at        | Lands at                | Example `cmd` |
|------------|------------------------|-------------------------|---------------|
| Python     | `tests/test_x.py`      | `<ws>/test_x.py`        | `python -m pytest test_x.py::test_y -q` |
| Go         | `tests/pkg/x_test.go`  | `<ws>/pkg/x_test.go`    | `go test -run '^TestY$' ./...` |
| TypeScript | `tests/x.test.ts`      | `<ws>/x.test.ts`        | `node --experimental-strip-types --test --test-name-pattern='^y$' x.test.ts` |

> Go: a `-run` pattern that matches **no** test exits 0 (a false pass), make sure
> your pattern matches a real `func TestX`. The validator's pre-patch check
> catches this (a fail-to-pass that passes pre-patch fails validation).

## Generating `gold_patch.diff`

Author `repo/` in its broken/incomplete state, then generate the patch
mechanically (don't hand-write it):

```bash
T=tasks/v1/<id>; G=/tmp/gen-<id>
rm -rf "$G" && mkdir -p "$G" && cp -r "$T/repo/." "$G/"
cd "$G" && git init -q && git add -A && git -c user.email=b@b -c user.name=b commit -qm base
# ...apply your fix to the files in $G...
git diff > "$OLDPWD/$T/gold_patch.diff"
```

## Validate before opening a PR

```bash
make validate-tasks                       # or:
python scripts/validate_tasks.py tasks/v1/<id>
```

The validator checks: schema, that the gold patch **applies and solves**
(`functional == 1.0`), that `fail_to_pass` **genuinely fails pre-patch**, and that
scoring is **deterministic** across runs. A task whose language toolchain isn't
installed is **skipped** (CI installs Python, Go, and Node).

## Provenance & decontamination (honesty)

Every task carries an explicit `decontaminated` boolean. The validator enforces
these rules (it **fails** a task that violates them):

- `source: hand-authored`: original problems written for VulcanBench. Written
  now, so they post-date all model cutoffs: set `decontaminated: true` and say so
  in `decontamination_notes`. (`decontaminated: false` is rejected for
  hand-authored tasks, if it's original, it's decontaminated.)
- `source: oss`: derived from a real upstream issue. It is almost always
  **`decontaminated: false`**, because the upstream fix predates the cutoffs of
  the models you evaluate (a model may reproduce it from memory). That's allowed,
  but you must *prove* the provenance:
  - `decontamination_notes` must include the **source URL** (`http…`) **and** a
    **commit/issue/PR reference** (a commit hash, `#123`, or an `/issues/…`
    link), and
  - the vendored `repo/` must **preserve the upstream LICENSE/NOTICE** file
    (only permissive licenses, MIT/BSD/Apache, should be vendored).

  If the task vendors its dependencies (`go mod vendor` / `cargo vendor`) so the
  offline sandbox needs no installs, the `vendor/` tree **must be committed with
  the task**, verify with `git ls-files <task>/repo/vendor | wc -l` after
  committing. Upstream Go repos routinely carry a `vendor` rule in `.gitignore`
  that would silently drop it (a clean clone then can't build the task);
  `slice_repo.py` strips such rules from the slice root, but double-check when
  assembling a task by hand.

  Scaffold the structure with
  `python scripts/slice_repo.py` then
  `python scripts/import_oss_issues.py --id <id> --repo <path> --issue <file>`,
  then fill in the tests, gold patch, and provenance. See
  `tasks/v1/oss-inflection-titleize/` and [LARGE_TASK_HANDBOOK.md](LARGE_TASK_HANDBOOK.md).

Runs scored against a `decontaminated: false` task are flagged in the
`vulcanbench report` **integrity** section, so their scores are never presented
as if they were contamination-free.

Never label a task with provenance you can't stand behind, the benchmark's value
is its transparency.


## Multi-service environments (`metadata.environment`)

A task that needs live services (databases, caches, load generators) ships a
compose file and declares:

```json
"environment": {
  "compose": "compose.yaml",
  "ready": [{"service": "redis", "cmd": "redis-cli ping", "timeout_s": 60}],
  "up_timeout_s": 300
}
```

Rules the validator enforces:

- publish ports **ephemerally** (`ports: ["6379"]`) — never `"6379:6379"`;
  the harness resolves the real host ports per run,
- no `container_name:` — per-run compose projects must not collide,
- every `ready` probe needs `service` and `cmd` (run via `docker compose
  exec` until it exits 0).

The harness writes `.vb_services.json` (project name + `{service: {container
port: host port}}`) into the workspace and gitignores it; the issue text
should tell the agent to read it, and hidden tests reach services the same
way. Environment tasks run with `--sandbox local` (the docker sandbox is
network-disabled) and validate with `--sandbox local` plus host toolchains.
Template: [`tasks/cii-v2/demo-compose-redis-smoke`](../tasks/cii-v2/demo-compose-redis-smoke).
