# engine-rs: Rust run-engine spike (zero crates)

A time-boxed spike that ports the deterministic path of the VulcanBench run
engine (`harness/agent/loop.py` and what it calls) to Rust with no external
crates at all, and measures how closely it reproduces the Python engine's
artifacts. The Python harness stays the engine of record. Nothing under
`harness/`, `tasks/` or `sandbox/` was changed.

The target is one command, byte for byte on its three durable artifacts:

```bash
vulcanbench run --task hello-world --model mock:synthetic --sandbox local --no-judges --output-dir <dir>
```

The Python engine writes `<dir>/<run_id>/{trace.jsonl, summary.json,
replay.html, final.patch}`. The spike matches `summary.json`, `trace.jsonl`
and `final.patch`; `replay.html` is out of scope.

## Status

Parity reached on the target command. `scripts/parity.py` (the acceptance
test) passes on all three artifacts, and it also passes with `--max-steps 2`
(an unfinished run: `finished: false`, two steps), with `--effort high
--timeout 250` (the `effort` block in the summary and on every `llm_request`
event, the CLI cap on the wall-clock budget), and with an untracked file
under `tasks/` (the dirty-tree warning line and the `source` block with its
`dirty_paths` and `-dirty` describe suffix). The error paths print the same
text with the same exit code as the Python CLI: a blocked effort (`ultra`),
an unknown task, an effort word outside the vocabulary.

`cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and
`cargo test` (41 unit tests) are clean on the pinned 1.87.0 toolchain, the
one CI already installs for Rust task validation.

Everything below the fold that did not reach parity is listed in
[What did not reach parity, and why](#what-did-not-reach-parity-and-why).

## Crate allowlist

Zero. `Cargo.toml` has an empty `[dependencies]` and an empty
`[dev-dependencies]`. What std lacks is hand-rolled under `src/`:

| Piece | File | Lines (without tests) | Notes |
| --- | --- | --- | --- |
| JSON value, parser, CPython-compatible writer | `json.rs` | 694 (616) | ordered objects, `", "` / `": "` separators, `indent=2`, ASCII escapes, `repr(float)` |
| TOML subset | `toml.rs` | 349 (308) | tables, dotted headers, strings, ints, floats, bools, arrays, comments; no inline tables or dates |
| SHA-256 | `sha256.rs` | 161 (119) | streaming, FIPS 180-4 test vectors |
| `.env` reader | `dotenv.rs` | 87 (68) | nearest file walking up, no override of shell variables |
| Secret redaction | `redaction.rs` | 309 (256) | the eight credential patterns and the bearer rule without a regex engine, plus the 20,000-character cap |
| Python spellings | `pyfmt.rs` | 228 (188) | `repr`, `round(x, n)`, `isoformat`, `splitlines`, universal newlines |
| Subprocess with timeout, `which` | `process.rs` | 140 (111) | `subprocess.run(timeout=...)` and `shutil.which` |
| Unified diff | in `local_executor.rs` | about 150 | `difflib.unified_diff` format over an LCS alignment |

Those gap-fillers are about 2,100 lines. The crate is 6,634 lines in all,
5,845 without unit tests, across 22 files. The Python it mirrors (`loop.py`,
the mock in `providers.py`, `protocol.py`, `local_executor.py`,
`collector.py`, `redaction.py`, `tasks.py`, `provenance.py`, `settings.py`,
`verifier.py`, `scorer.py`, `evaluate.py`, `langs.py`, `task_metadata.py`,
`effort.py`, `economics.py`) is about 4,100 lines, plus roughly 350 lines
for the `run` command of `cli.py` and the Python branches of `quality.py`
and `security.py`. So the port is about 1.3 times the Python line for line,
and more than a third of it is standard-library gaps rather than engine
logic.

## The deferred TLS decision

The spike has no HTTP client and no TLS on purpose: the mock provider is in
process, so the network-free path says nothing about either. A real port
needs HTTPS for every provider and for the backend write-through, and plain
HTTP for Ollama. Two options were weighed and the choice is deliberately
left open:

1. **One audited TLS crate (rustls, with `webpki-roots` or the platform
   verifier).** Keeps the engine a single static binary with no runtime
   dependency, and the TLS code is the same code that ships in widely used
   Rust HTTP stacks. The cost is the end of "zero crates": rustls brings
   `ring` or `aws-lc-rs`, `webpki`, `pki-types` and `zeroize`, so the
   allowlist becomes about six crates, and the hand-rolled HTTP/1.1 client
   (chunked encoding, keep-alive, server-sent events for the streaming
   providers) still has to be written and maintained here.
2. **Shell out to the system `curl`.** The engine already requires `git`,
   `docker` and language toolchains on the host, so one more host binary is
   consistent with how the sandbox and the manifest probes work, and it keeps
   the allowlist at zero. curl handles TLS, proxies, HTTP/2, reconnects and
   streaming (`-N`). The cost is a process per model call (negligible against
   a model round trip), request bodies through stdin or `--config` so a key
   never appears in argv, and a dependency on the host's curl and CA bundle,
   which the run manifest would have to record the way it records
   `git --version` today.

Nothing in the spike constrains either path: the provider trait takes
messages and tool schemas and returns a response, exactly as the Python
`LLMProvider.complete` does, so the transport slots in underneath.

## Layout

```
engine-rs/
  Cargo.toml              zero dependencies; clippy::all as warnings
  rust-toolchain.toml     1.87.0 with clippy and rustfmt, the CI toolchain
  scripts/parity.py       the acceptance test (parity.sh is a thin wrapper)
  src/
    main.rs               CLI: the `run` subcommand subset, exit codes, dirty-tree warning
    engine.rs             run_agent, the step loop, tool dispatch, cost and economics
    provider.rs           Provider trait and the mock policy
    local_executor.rs     host tool executor, path containment, unified diff
    protocol.rs           tool schemas and the lax argument coercion
    evaluator.rs          quality (ruff + radon), security (bandit), scorer, evaluate_run
    verifier.rs           verifier.py runner and the declarative verifier
    manifest.rs           run manifest, budgets, workspace stats
    tracer.rs             trace.jsonl and summary.json
    tasks.rs              task loading, task_hash, workspace preparation
    provenance.rs         git commit, describe and dirty paths
    settings.rs           vulcanbench.toml and the blocked-effort refusal
    effort.rs             effort vocabulary and the recorded-but-not-sent record
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

Then, from the repo root:

```bash
python3 engine-rs/scripts/parity.py
```

It builds the Rust binary, runs both engines on hello-world into a temporary
directory, normalises the volatile fields (run ids and the output paths that
contain them, timestamps, `duration_s`, pytest's elapsed time in the
`run_tests` observation) and diffs the three artifacts, key order included.
`--keep` or `--out DIR` preserves the two run directories; `--effort`,
`--max-steps` and `--timeout` are passed to both engines. The script puts
the repo root on `PYTHONPATH` for the Python engine as well as the venv on
`PATH`, so it still works on a venv whose `.pth` files the interpreter
refuses to read (seen on macOS when they carry the hidden flag).

To run the Rust engine on its own, put `.venv/bin` on `PATH` first so the
analyzers and `python` resolve the same way they do for the Python CLI:

```bash
PATH="$PWD/.venv/bin:$PATH" cargo run --manifest-path engine-rs/Cargo.toml -- \
  run --task hello-world --model mock:synthetic --sandbox local --no-judges --output-dir /tmp/rs-runs
```

Checks that must stay clean:

```bash
cd engine-rs && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test
```

## What the spike reproduces

- The step cap (`resolve_max_steps` from metadata, scale defaults and the
  CLI cap), the wall-clock deadline with `budget_exceeded` events at every
  stage the Python loop checks, and the per-tool timeout clamp.
- Tool dispatch through the same six schemas, the "unknown tool" error for
  anything else, and the lax argument coercion of `Model.from_args` with the
  same `ValueError` texts (`"120"` to `120`, `"yes"` to `true`, missing or
  mistyped arguments reported back to the model instead of crashing).
- Observation truncation at 8,000 characters with the same marker, and
  secret redaction plus the 20,000-character field cap before any event or
  summary touches disk.
- The `FINISH:` marker, the workspace git repo and `.gitignore`, the patch
  and changed-file list through `git`, the legacy `verifier.py` path and the
  declarative `fail_to_pass` / `pass_to_pass` verifier with the same
  infrastructure-failure classification.
- `summary.json` and `trace.jsonl` with CPython's `json.dumps` layout:
  key order, separators, `indent=2`, ASCII escapes and `repr(float)`
  spellings. `round(x, n)` is reproduced through correctly rounded
  formatting, so the four-decimal scores agree.
- `task_hash` (SHA-256 over the same byte stream, including Python's
  component-wise path ordering and the sorted-keys JSON of the tests spec),
  `source` provenance from git, the dirty-tree warning for `--task` runs,
  `.env` loading, and the blocked-effort refusal from `vulcanbench.toml`
  with the same message and exit code.

## What did not reach parity, and why

Declared out of scope and refused with a clear message rather than ported:

- `replay.html` is not written. The summary still records its path under
  `artifacts.replay_html`, as the Python engine does.
- Only the `mock` provider exists. No HTTP providers, no TLS, no retry and
  backoff loop, no `--max-run-cost` (it needs the price table; only `mock:`
  is priced here, so `cost_capped` is always false), and `--dry-run` prints
  the plan line but not the cost estimate the Python CLI adds for priced
  models.
- `--sandbox docker` and `auto`, suite runs, `--repeat`, the `human_like`
  judge ensemble (`--judges`), the agentic and rubric graders, multi-service
  task environments, the vendor CLI harnesses, and the backend write-through
  (`VULCANBENCH_API_BASE`) all exit with a message naming the gap. The
  suite-run refusal on a dirty tree is therefore not ported either; the
  `--task` warning is.

Ported, but with a known difference or no parity run that exercises it:

- **The Rust engine still needs the Python venv.** ruff and bandit are
  shelled out to exactly as in Python, but radon is an in-process Python
  library with no CLI mode that matches the harness's calls (`radon cc`
  flattens methods out of classes, which changes the average), so quality's
  `avg_complexity` and `maintainability_index` come from a twelve-line
  snippet run in the venv's `python`. `run_tests` (`python -m pytest`) and
  `verifier.py` need that `python` too. A real port either keeps this
  dependency or re-implements radon's metrics, which is not small.
- **`edit_file` on an existing file** returns a unified diff from an LCS
  alignment formatted like `difflib.unified_diff`. difflib's SequenceMatcher
  (longest-matching-block recursion, autojunk above 200 lines) can pick
  different hunks on larger edits, so that observation text (trace only,
  never a score) can differ. The mock only exercises the create-file branch,
  which matches.
- **`search_code`** is ported (ripgrep JSON and the pure-Python fallback)
  but no parity run reaches it; rg's presence and the fallback's filesystem
  order are environment-dependent in Python as well.
- **OS error text.** Python's `[Errno N] ...: 'path'` strings are reproduced
  for not-found, permission, is-a-directory and not-a-directory; other OS
  errors and the exact `UnicodeDecodeError` wording are approximated. These
  reach the trace as `tool_observation.error` only.
- **Snapshot tasks** (`repo_snapshot.tar.gz`) extract through the host `tar`
  rather than Python's `tarfile` with its traversal check; unexercised.
- **`manifest.runtime.python`** and the interpreter for `verifier.py` are
  the `python` found on `PATH` (parsed from `python --version`); the Python
  engine reports its own interpreter. They agree whenever `.venv/bin` leads
  `PATH`, which the parity script enforces.
- **Run ids** are eight hex characters from a SHA-256 of time, pid and a
  stack address, not `uuid4`: same shape, less entropy. The parity script
  normalises them anyway.
- **Timestamps** mirror `datetime.isoformat()` including its quirk of
  dropping the fraction when the microsecond is zero; normalised anyway.
- **Setup commands** (`metadata.setup`), the 8,000-character observation cap
  and the 20,000-character field cap are unit-tested but no task on the
  deterministic path reaches them.
- **`ProviderError`** (exit code 2) has no producer, since the mock cannot
  fail; the other exit codes (1 and 3) are mapped.

## Cost

- **Effort.** One working day of a single session: reading about 4,100 lines
  of Python, writing the 6,600 lines of Rust and the 200-line parity script,
  then two compile-and-fix rounds (two type errors, three clippy findings).
  Parity passed on the first full run of the script; no artifact needed a
  fix after compilation. The Python engine was run twice up front as the
  oracle, which is what made the volatile fields obvious before any Rust was
  written.
- **Build.** A clean debug build takes about 1.3 seconds and a clean release
  build about 3.5 seconds on this machine; the release binary is 1.0 MB with
  no runtime dependencies beyond the host binaries the Python engine already
  needs (`git`, and the venv's `python`, `ruff`, `bandit`, `radon`).
- **Run time.** The run itself is dominated by subprocesses (git, pytest,
  the verifier, ruff, bandit, radon), so the two engines land close together
  on hello-world: three wall-clock runs each on this machine took 2.6, 3.5
  and 2.6 seconds for the Python engine and 2.4, 2.5 and 2.6 seconds for the
  release Rust binary. The measurable difference is the Python interpreter's
  own start-up and imports, not the loop.
- **Where the risk is.** Not in the loop, the trace or the scoring, which
  port mechanically. It is in the long tail of CPython behaviours the
  artifacts depend on (`repr(float)`, `round`, `json.dumps` escapes, path
  ordering in `sorted(Path)`, `splitlines` boundaries, `OSError` text,
  difflib's matcher) and in the Python-only analyzers the scores depend on
  (radon today; the judge and grader prompts tomorrow). Each of those is
  small, but each is a place where a Rust engine of record would silently
  diverge from four years of Python results unless the parity script grows
  with it.
