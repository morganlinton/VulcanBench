# engine-rs: Rust run-engine spike (zero crates)

A time-boxed spike that ports the VulcanBench run engine (`harness/agent/loop.py`
and what it calls) to Rust with no external crates at all, and measures how
closely it reproduces the Python engine's artifacts. The Python harness stays
the engine of record. Nothing under `harness/`, `tasks/` or `sandbox/` was
changed.

Phase 1 (PR #224) reproduced one deterministic run, the mock model on
hello-world in the local sandbox, byte for byte on `summary.json`,
`trace.jsonl` and `final.patch`. Phase 2 (this branch) adds the HTTP layer
over the system `curl`, one real provider (Anthropic Messages), the Docker
sandbox through the `docker` CLI, the price table, and `--max-run-cost`,
still with zero crates.

## Status

**Offline, verified on this machine:**

- `scripts/parity.py` passes on all three artifacts for the mock hello-world
  run in the local sandbox, and also with `--max-steps 2`, with
  `--effort extra-high --timeout 250`, and with a dirty tree.
- `scripts/parity.py --sandbox docker` passes on all three artifacts with the
  mock on `vulcanbench/sandbox:v5-base` (set through
  `VULCANBENCH_SANDBOX_IMAGE`, which both engines read). The two manifests
  agree exactly: the container's `Python 3.12.15` line as the runtime
  (the Python engine does not strip the prefix there), `Linux ... aarch64`
  from `uname -srm`, the six tool versions probed inside the container, the
  resource band and `oom_kills: 0`. No container is left behind.
- The spend gate works: a live model spec without `--approve-spend` prints
  the run it would make and exits 3 without touching the network.
- `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and
  `cargo test` (55 unit tests, including a fake `curl` on disk) are clean on
  the pinned 1.87.0 toolchain, the one CI already installs.

**Not run:** any real provider call. A live run spends API quota and needs
the owner's explicit approval; see [The proposed live run](#the-proposed-live-run).

## Crate allowlist

Zero. `Cargo.toml` has an empty `[dependencies]` and an empty
`[dev-dependencies]`. What std lacks is hand-rolled under `src/`:

| Piece | File | Lines (without tests) | Notes |
| --- | --- | --- | --- |
| JSON value, parser, CPython-compatible writer | `json.rs` | 694 (616) | ordered objects, `", "` / `": "` separators, `indent=2`, ASCII escapes, `repr(float)` |
| TOML subset | `toml.rs` | 349 (308) | tables, dotted headers, strings, ints, floats, bools, arrays, comments |
| SHA-256 | `sha256.rs` | 161 (119) | streaming, FIPS 180-4 test vectors |
| `.env` reader | `dotenv.rs` | 87 (68) | nearest file walking up, no override of shell variables |
| Secret redaction | `redaction.rs` | 309 (256) | the eight credential patterns and the bearer rule without a regex engine, plus the 20,000-character cap |
| Python spellings | `pyfmt.rs` | 228 (188) | `repr`, `round(x, n)`, `isoformat`, `splitlines`, universal newlines |
| Subprocess with timeout and stdin, `which` | `process.rs` | 171 (128) | `subprocess.run(timeout=...)`, `shutil.which` |
| Unified diff | in `local_executor.rs` | about 150 | `difflib.unified_diff` format over an LCS alignment |
| HTTP seam and retry arithmetic | `http.rs` | 315 (206) | one-method client trait, status classification, `_http_timeout`, `_budgeted_attempts`, the 1, 2, 4, 8 (cap 20) schedule |
| curl client | `curl.rs` | 279 (139) | `curl --config -`: URL, headers and body on stdin, status via `--write-out` |
| Price table | `pricing.rs` | 206 (173) | the Anthropic rows, `mock:`, `ollama:`, aliases, `VULCANBENCH_PRICING` override |

The crate is 8,890 lines in all, 7,519 without unit tests, across 28 files;
phase 2 added 2,256 lines (1,365 without tests). The engine proper
(`engine.rs`, `anthropic.rs`, `docker_executor.rs`, `local_executor.rs`,
`evaluator.rs`, `verifier.rs`, `manifest.rs`, `protocol.rs` and the rest) is
about 5,500 lines without tests against roughly 5,000 lines of Python
(the phase 1 list plus `AnthropicProvider` and its helpers in `providers.py`,
`docker_executor.py`, `images.py` and the Anthropic rows of `pricing.py`).

## The TLS decision: system curl

Decided by the owner on 2026-10-24 and recorded in
[docs/DECISIONS.md](../docs/DECISIONS.md) (entry "Rust engine spike, phase 2").
The engine already drives `git`, `docker`, the language toolchains and the
venv's analyzers through their CLIs; curl is one more host binary and keeps
the allowlist at zero, where rustls would have been about six crates plus a
hand-rolled HTTP client. The costs accepted: the host's curl and CA bundle
become part of the run environment, and secrets must never reach argv.

How the client meets that: every request is one `curl --config -` process.
The config file on stdin carries `url`, `request = "POST"`, each `header`
(the API key included), `data-binary` with the JSON body, `max-time` from the
run budget, and a `write-out` that prints the HTTP status after the body
behind a marker. The argument vector is exactly `--config -`, which the unit
tests check with a fake `curl` that records what it received. curl exit 28 is
a timeout; any other non-zero exit is a transport failure (both retryable,
like `URLError`); a 4xx other than 408, 409, 425 and 429 is final; 5xx and
those four retry. curl parses a config line of at most 10 MiB, so bodies above
8 MiB are refused with a clear error rather than truncated.

The trait is one method (`post`) over a request of URL, headers, body and
timeout. A rustls client would implement the same trait; no provider would
change.

## Layout

```
engine-rs/
  Cargo.toml              zero dependencies; clippy::all as warnings
  rust-toolchain.toml     1.87.0 with clippy and rustfmt, the CI toolchain
  scripts/parity.py       the acceptance test (parity.sh is a thin wrapper)
  src/
    main.rs               CLI: the `run` subcommand subset, exit codes, dirty-tree warning
    engine.rs             run_agent, the step loop, tool dispatch, cost cap, cost and economics
    provider.rs           Provider trait, the mock policy, provider lookup
    anthropic.rs          Anthropic Messages: payload shape, caching breakpoints, token weighting
    http.rs               HttpClient trait, post_json, retry budget arithmetic
    curl.rs               curl --config - client
    pricing.rs            price table, cost_usd, VULCANBENCH_PRICING
    executor.rs           Local or Docker, one enum
    local_executor.rs     host tool executor, path containment, unified diff
    docker_executor.rs    container lifecycle, exec, OOM accounting, resource band
    protocol.rs           tool schemas and the lax argument coercion
    evaluator.rs          quality (ruff + radon), security (bandit), scorer, evaluate_run
    verifier.rs           verifier.py runner and the declarative verifier
    manifest.rs           run manifest (host or container probes), budgets, workspace stats
    tracer.rs             trace.jsonl and summary.json
    tasks.rs              task loading, task_hash, workspace preparation
    provenance.rs         git commit, describe and dirty paths
    settings.rs           vulcanbench.toml and the blocked-effort refusal
    effort.rs             effort vocabulary; mock and Anthropic mappings
    redaction.rs          secret patterns and field caps
    deadline.rs           wall-clock budget
    json.rs, toml.rs, sha256.rs, dotenv.rs, pyfmt.rs, process.rs
```

## Running it

The parity script needs the repo's Python venv (it runs the Python engine
too, and both engines find `ruff`, `bandit`, `radon`, `python` and `pytest`
there):

```bash
uv sync --extra dev --extra test && uv pip install -e . --no-deps
```

Offline checks, from the repo root:

```bash
python3 engine-rs/scripts/parity.py
python3 engine-rs/scripts/parity.py --effort extra-high --max-steps 2
VULCANBENCH_SANDBOX_IMAGE=vulcanbench/sandbox:v5-base python3 engine-rs/scripts/parity.py --sandbox docker
```

Each builds the Rust binary, runs both engines on hello-world into a temporary
directory, normalises the volatile fields (run ids and the output paths that
contain them, timestamps, `duration_s`, pytest's elapsed time in the
`run_tests` observation) and diffs the three artifacts, key order included.
`--keep` or `--out DIR` preserves the two run directories.

A live run (`--model anthropic:<model>`) is refused unless `--approve-spend`
is passed; without it the script prints the model, task, effort and a
list-price estimate and exits 3. With it, both engines run live and the
comparison is structural (every scalar replaced by its type name, lists by
their first element's shape, trace event types with their data keys), because
model output is not deterministic. Both engines read `ANTHROPIC_API_KEY` from
the environment or the repo's `.env`.

To run the Rust engine on its own, put `.venv/bin` on `PATH` first so the
analyzers and `python` resolve the same way they do for the Python CLI:

```bash
PATH="$PWD/.venv/bin:$PATH" cargo run --manifest-path engine-rs/Cargo.toml -- \
  run --task hello-world --model mock:synthetic --sandbox docker --no-judges --output-dir /tmp/rs-runs
```

Checks that must stay clean:

```bash
cd engine-rs && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test
```

## What the spike reproduces

- The step cap, the wall-clock deadline with `budget_exceeded` events at
  every stage the Python loop checks, the per-tool timeout clamp, and the
  per-run cost cap (`--max-run-cost`, `cost_cap_exceeded` event,
  `cost_capped` in the summary) for priced models.
- Tool dispatch through the same six schemas, the "unknown tool" error for
  anything else, and the lax argument coercion of `Model.from_args` with the
  same `ValueError` texts.
- Observation truncation at 8,000 characters, and secret redaction plus the
  20,000-character field cap before any event or summary touches disk.
- The `FINISH:` marker, the workspace git repo and `.gitignore`, the patch
  and changed-file list through `git`, the legacy `verifier.py` path and the
  declarative verifier with the same infrastructure-failure classification,
  including an OOM kill at the sandbox ceiling as a verifier infrastructure
  error.
- The Docker sandbox: the same `docker run` argument vector (workdir, bind
  mount, memory and CPU ceilings, pids limit, no-new-privileges, cap-drop,
  the writable-paths environment, `--network none` unless `--network`, the
  host UID and GID, the floor flags), `docker exec` with `cd <dir> && timeout
  <n>s sh -c <cmd>` under `shlex.quote` rules, exit 137 classified as OOM
  only when the cgroup's kill counter moved, daemon failures (`125` with the
  daemon messages) as sandbox errors, file tools on the host, stop and
  force-remove on close, `--sandbox auto` with the explicit host-exec opt-in.
- The Anthropic Messages request: system prompt split out, `tool` turns as
  `tool_result` blocks, assistant tool calls as `tool_use` blocks, the two
  ephemeral cache breakpoints, `max_tokens` by effort (32K, 64K, 128K),
  `output_config.effort` when the level is supported, the tool schema
  conversion, the optional server-side refusal fallback header and payload.
  The response: text and `tool_use` blocks, refusals as retryable failures,
  `max_tokens` with no tool call as a final failure, and prompt tokens as
  `input + 0.1 * cache_read + 1.25 * cache_write` rounded half to even.
- The retry policy: per-request timeout capped at 600 s and by the remaining
  budget, attempts bounded by the budget (at most 4), the 1, 2, 4 second
  backoff, final failures surfaced at once, and the budget-exhausted
  refusal before the call.
- `summary.json` and `trace.jsonl` with CPython's `json.dumps` layout,
  `task_hash`, `source` provenance, the dirty-tree warning, `.env` loading,
  the blocked-effort refusal, and the price table for `cost_detail` and
  `economics`.

## What did not reach parity, and why

Declared out of scope and refused with a clear message rather than ported:

- `replay.html` is not written.
- Providers other than `mock` and `anthropic`. Suite runs and `--repeat`,
  the `human_like` judge ensemble, the agentic and rubric graders,
  multi-service task environments, the vendor CLI harnesses, the backend
  write-through, and the cost estimate on `--dry-run`.

Ported, but untested against a real service or with no parity run that
exercises it:

- **Every live Anthropic path.** Request shape, headers, retry, token
  weighting and refusal handling are unit-tested against canned responses and
  a fake `curl`, not against the API. Streaming is not involved (the Python
  provider does not stream either).
- **The curl client against a real TLS endpoint**, including proxy and CA
  bundle behaviour on this host.
- **Docker OOM accounting.** The 137 path and the cgroup counter parsing are
  ported from the Python code but no run triggered an OOM kill.
- **`--network`, `--sandbox auto`'s fallback, and the resource floor flags**
  were not run through the parity script (the argument vector is unit-tested).
- **`--max-run-cost`** has no deterministic parity run: the mock is free, so
  the cap can never trip without a priced model.
- **The price table** carries only the Anthropic rows and the free prefixes;
  other providers are unpriced here, which the summary records as
  `cost_usd: null`.

Still true from phase 1:

- The Rust engine still needs the Python venv on `PATH`: ruff and bandit are
  shelled out to as in Python, radon runs through a twelve-line snippet in
  the venv's `python` (its CLI does not match the harness's API calls), and
  `run_tests` plus `verifier.py` need that `python`. Under Docker the
  `run_tests` command runs inside the container, as in Python; `verifier.py`
  runs on the host in both engines.
- `edit_file` diffs on existing files come from an LCS alignment formatted
  like `difflib.unified_diff`; difflib's matcher can choose different hunks
  on larger edits. Only the create-file branch is exercised.
- `search_code`, snapshot tasks (host `tar`), setup commands and the field
  caps are ported but unexercised; OS error wording beyond the common four
  cases is approximated; run ids are eight hex characters from a hash, not
  `uuid4`.

## The proposed live run

Nothing live has been run. The cheapest meaningful check is one hello-world
run per engine on the smallest priced Anthropic model, then one on a frontier
model so the effort and cache paths are real:

```bash
python3 engine-rs/scripts/parity.py --model anthropic:claude-haiku-5-5 --effort low --approve-spend
python3 engine-rs/scripts/parity.py --model anthropic:claude-opus-5-5 --effort high --approve-spend
```

At list price, assuming 6,000 input and 1,200 output tokens per run (the
mock's hello-world transcript is about 1,200 tokens of prompt per step over
four steps), that is under a cent per engine for Haiku 5.5 and about five
cents per engine for Opus 5.5, so roughly ten cents for both pairs. The
comparison is structural, and the two `trace.jsonl` files are kept with
`--keep` so the real payloads, usage fields and cost lines can be read side
by side.

## Cost

- **Phase 1.** One working day: reading about 4,100 lines of Python, writing
  6,600 lines of Rust and the parity script, two compile-and-fix rounds;
  parity on the first full run.
- **Phase 2.** About half a working day: reading the retry, Anthropic and
  Docker code (about 900 lines of Python), writing 2,300 lines of Rust and
  the live mode of the parity script, one compile round with one type error
  and one dead function; Docker parity on the first run, local parity
  unchanged.
- **Build.** A clean debug build takes about 2 seconds, a release build about
  4 seconds; the release binary is 1.0 MB with no runtime dependency beyond
  the host binaries the Python engine already needs (`git`, `docker`, `curl`,
  and the venv's `python`, `ruff`, `bandit`, `radon`).
- **Where the risk is.** Unchanged from phase 1 in kind, larger in surface:
  every CPython spelling the artifacts depend on, every Python-only analyzer
  the scores depend on, and now every provider quirk the Python code encodes
  (effective prompt tokens, refusal handling, output ceilings) that a port
  must track release by release.
