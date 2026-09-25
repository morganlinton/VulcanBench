# Subscription harness benchmarking

VulcanBench can run a task through a product's own coding-agent CLI while
keeping task preparation, the final diff, hidden verification, and scoring in
VulcanBench. These results measure a **model plus product harness**, not a raw
model API.

Release A supports:

| Harness | Spec | Subscription authentication | Execution boundary |
|---|---|---|---|
| Claude Code | `claude-code:<model>` | Claude Pro/Max login | Claude permission auto mode; `--sandbox local` currently required |
| Codex CLI | `codex:<model>` | Sign in with ChatGPT | Codex `workspace-write`; Vulcan setup/verifier may still use Docker |
| Cursor CLI | `cursor:<model>` | `cursor-agent login` (Cursor account/credits) | Cursor sandbox enabled + force-allow; Vulcan setup/verifier may use Docker |
| Grok Build | `grok-build:<model>` | `grok login` (grok.com OIDC) | custom kernel profile: workspace writes + repo reads denied (Seatbelt/Landlock); Vulcan setup/verifier may use Docker |
| ZCode | `zcode:<model>` | `zcode login` (Z.ai OAuth, GLM Coding Plan) | ZCode permission mode `yolo` on the host workspace; web tools removed and the Browser Use plugin disabled; Vulcan setup/verifier may use Docker |
| Devin CLI | `devin:<model>` | `devin auth login` (Devin account) | Devin print mode, permission mode `dangerous`, on the host workspace with an isolated config home; web tools disabled through the per-run config; Vulcan setup/verifier may use Docker |

Cursor-specific limits: `cursor-agent` streams no token usage or cost, so
token counts are recorded as zero and the economics receipt marks the
API-equivalent value **unavailable**: Cursor's own usage dashboard is the
only ledger of what a run consumed. `--max-run-cost` is rejected (nothing to
enforce it against) and `--effort low|medium|high` travels via Cursor's
`model[effort=...]` bracket syntax, though Cursor's Grok family bakes effort
into the model id instead (`cursor-grok-4.6-low` … `-xhigh`), so sweep those by
model id without `--effort`. Run with `--sandbox docker` so hidden-test
verification uses the sandbox image toolchains; `--sandbox local` puts the
verifier on the host, where missing toolchains fail Python tasks. Preflight
fails closed when signed out or when `CURSOR_API_KEY` is set (API-key auth
bills metered usage, not the plan).

Grok Build-specific notes (verified on grok 0.2.69 and 1.0.5, both alpha, 
the surface moves fast; re-verify these on every CLI update before a sweep):

- **The effort knob.** The adapter sends `--reasoning-effort` (accepted:
  none/minimal/low/medium/high/xhigh) and proves each run's level by copying
  the session summary's `reasoning_effort` into the outcome as
  `reported_effort`. On 0.2.69 the separate `--effort` flag parsed and was
  silently ignored (every level ran at the default `high`); 1.0.5 makes it
  an alias of `--reasoning-effort`. The adapter never uses `--effort`.
- **Usage and tool calls stream on 1.0+.** `streaming-json` emits
  `tool_call`/`tool_call_update`/`usage` events plus an `end` event with the
  full token split (input/output/cache-read/reasoning; grok's
  `output_tokens` already includes reasoning, unlike the raw xAI API),
  `num_turns`, and the CLI's own `total_cost_usd` (recorded as
  `cli_reported_cost_usd`; it is far below list price and is Grok's internal
  accounting, not a bill). Token receipts and a live `--max-run-cost`
  API-equivalent cap both work; `grok-build:` prices via the `xai:` table.
  The session trace (`~/.grok/sessions/**/<id>/`) is still harvested into
  the run dir, the session id is pre-assigned with `-s` so timeouts keep
  their trace. On 0.2.69, where the stream carried none of this, the trace
  was the only source; the harvest also guards against future stream
  regressions.
- **Web denial is by tool removal.** `--disallowed-tools web_search,web_fetch`
  deletes the tools outright (with `--deny WebFetch` as a second layer), so a
  grok run shows `no_web` rather than Cursor-style `web_blocked` attempt
  counts, the model cannot reach for a tool that does not exist.
- **The sandbox is a custom kernel profile, not `strict`.** The adapter
  writes `<workspace>/.grok/sandbox.toml` (`extends = "workspace"`, `deny =
  [<repo root>]`) and runs `--sandbox vulcanbench`: toolchains stay usable
  (`strict` also kernel-denied `~/.cargo` and homebrew, crippling non-Python
  tasks, observed live) while this checkout's answer keys are read-denied
  by the kernel even if the agent learns the path. That path can leak:
  a live run extracted the repo location from `PATH`'s `.venv/bin` entry and
  ran `find` over it (Seatbelt denied it), so `_subscription_env` now scrubs
  repo-rooted PATH entries for every harness. Grok fails closed if the
  profile cannot be applied. Note the sandbox does not block child-process
  network on macOS, `curl` in a shell works; web tool removal plus the
  audit's command scan remain the check on that.
- **Session hygiene.** `GROK_MEMORY=0` is always set (1.0 dropped the
  `--no-memory` flag): Grok's cross-session memory would let repeat N+1
  remember repeat N's task. `grok trace` uploads remotely by default, 
  anything touching it must pass `--local`.
- Preflight fails closed when signed out or when `XAI_API_KEY` is set
  (API-key auth bills console.x.ai metered usage, not the plan).

ZCode-specific notes (verified on `zcode-app-cli` 3.8.1-15 wrapping
`zcode-runtime` 0.16.3; ZCode itself is Z.ai's Electron desktop app, and the
`zcode` command is the npm launcher around the same agent runtime, so
re-verify these on every runtime update before a sweep):

- **Install and sign in.** `npm install -g zcode-app-cli@latest` needs Node
  22.19 or newer on `PATH` (the launcher exits under older Node; `nvm use 24`
  or set `ZCODE_NODE`). `zcode login` opens the Z.ai OAuth flow (macOS only)
  and writes the shared credential store; the preflight reads that store for
  presence only, never the token. A Coding Plan API key configured through
  `/login` in the TUI also counts as a plan run (it lands in
  `~/.zcode/cli/config.json` against a plan endpoint). A key against the
  general pay-as-you-go endpoint (`/api/paas/v4`) fails closed, the metered
  analogue of `XAI_API_KEY` on Grok Build.
- **The real headless surface is narrower than the help text.** The runtime's
  strict parser accepts `--prompt`/`-p`, `--cwd`, `--mode`,
  `--disallowed-tools`, `--verbose`, `--no-color`, `--resume`, `--continue`.
  `--max-turns`, `--settings` and `--allowed-tools` print in `--help` but
  are rejected with "Unknown option", so `max_turns` cannot be forwarded (the
  wall-clock `--timeout` bounds a run) and per-run settings travel through
  the project-level `<workspace>/.zcode/config.json`, which the runtime
  merges over the user config. The adapter writes: `model.main`,
  `permission.mode: yolo` (plus `--mode yolo`), `features.memory: false` and
  `memory.use/write: false` (cross-session memory would let repeat N+1
  remember repeat N's task), the web denies, and the effort override below.
- **The effort knob is the "thought level".** ZCode's catalog lists exactly
  `low` / `high` / `max` for glm-5.3 (default `max`), the same enum as the
  API. The adapter pins it through
  `modelCatalog.overrides["zai/<model>"].reasoning.defaultLevel` and proves
  the level each run actually used by reading it back from ZCode's usage
  ledger (`model_usage.variant`) into `reported_effort`. `medium` does not
  exist and stays recorded-but-not-sent.
- **Headless stdout is just the final text; the record lives in sqlite.**
  Every message, tool call (`part` rows with `callID` / `tool` /
  `state.input`) and per-request token receipt (`model_usage`: input,
  output, reasoning, cache read/write, model id, thought level) is in
  `~/.zcode/cli/db/db.sqlite`. After the run the adapter locates the session
  by workspace directory, copies the rows into `<run_dir>/zcode-session/`
  (`messages.jsonl`, `model_usage.jsonl`, `turn_usage.jsonl`,
  `tool_usage.jsonl`, `log.jsonl` from ZCode's daily log), appends the
  message and tool-call records to `cli-agent-stream.jsonl` so the integrity
  audit has a substrate, and sums usage into the outcome. Subagent child
  sessions (`parent_id`) are included in the totals. `zcode:` prices via the
  `zai:` table (GLM 5.3: $1.40 / $0.26 cached / $4.40 per M). Because usage
  is only visible after the run, `--max-run-cost` is rejected rather than
  pretended; `--timeout` is the hard boundary.
- **Web denial is by tool removal plus plugin disable.** `--disallowed-tools
  WebFetch WebSearch web_search` removes the web tools (mirrored into the
  project config's `permission.disallowedTools` so subagents inherit it), and
  the Browser Use plugin, a headless Chromium the launcher enables by default
  for `--prompt` sessions, is disabled via
  `plugins.enabledPlugins["browser-use@zcode-plugins-official"] = false`.
  `--network` skips both. ZCode has no kernel sandbox: shell runs on the
  host, so the filesystem channel is the containment perimeter (workspace
  outside the repo) plus the audit, as for Cursor.
- **Session hygiene.** The runtime walks up from the workspace looking for a
  `.env` to load; CLI-harness workspaces live in a VulcanBench-created tmp
  perimeter with nothing above them, so no checkout `.env` is ever read.
  `_subscription_env` passes `ZCODE_NODE`, `ZCODE_HOME`,
  `ZCODE_DATA_BASE_DIR` and `ZCODE_STORAGE_DIR` through (a relocated state
  root must still be found) and never `ZCODE_API_KEY` / `ZCODE_BASE_URL`.

## Leakage: two channels, both real

External harnesses execute on the host with broad tool access, and a benchmark
built from public merged PRs leaks through two independent channels. Both were
observed in live sweeps, and blocking one does nothing about the other.

**The web.** Every v3 task derives from a public PR, so the fix sits at a known
URL. In the first Cursor sweep 46% of runs fetched their task's exact source PR
or fix commit, and the median patch of those runs matched the gold patch
exactly. (Terminal-Bench, which allows internet by design, asks users to
"remain vigilant" about this; VulcanBench automates the check using provenance
its own tasks record.)

**The filesystem.** Harness agents run on the host, so a workspace inside this
checkout lets an agent walk up into `tasks/` and read `gold_patch.diff` and the
hidden tests -- the grader's answer key, not merely the upstream fix. In a
sweep with the web already blocked, 46 runs read their own task's answer key
and all 46 solved.

Three layers now stand against both:

1. **Containment.** CLI-harness runs get a workspace outside the repo
   (`tempfile.mkdtemp`), so no benchmark data exists anywhere above the
   agent's cwd; the tree is moved back under the run dir after scoring. This
   is the load-bearing defence: enumerating forbidden paths is the same losing
   game as enumerating forbidden URLs.
2. **Prevention.** The Cursor adapter writes a workspace permissions file
   denying `WebFetch(*)` and `WebSearch` (with an explicit allow list for the
   work tools) and runs with `--trust` unless `--network` is passed. The
   mechanism is fussy and was verified live: `--force` approves *denied*
   queries too, so a deny list under `--force` is silently useless; `--trust`
   honours denies but rejects shell calls without an allow list. Claude Code
   gets `--disallowedTools WebSearch,WebFetch`.
3. **Detection.** Every CLI-harness run summary carries an `integrity_audit`
   with both channels. Web verdicts: `no_web`, `web_blocked` (attempts made,
   all denied -- clean, but recorded), `web_used`, `upstream_access`,
   `solution_retrieval`. Filesystem verdicts: `clean`, `out_of_workspace`,
   `benchmark_data_access`, `answer_key_access`. A run is `contaminated` if
   either channel says so. `vulcanbench audit-runs runs/` re-annotates
   existing runs. The audit annotates; it never rescores.

Note what the audit is not: a rejected call is not access, and the audit must
correlate a tool call's `started` and `completed` events before scoring it. An
earlier version scored the `started` event alone and flagged four clean runs as
contaminated, one of them as solution retrieval.

## Preflight

Check installation, CLI version, authentication source, and non-secret plan
metadata without starting a paid model run:

```bash
vulcanbench harness list
vulcanbench harness doctor
vulcanbench harness doctor codex --json
```

Doctor fails closed when the CLI is signed out or authenticated with API
billing. VulcanBench never copies login tokens into run artifacts. External CLI
processes receive a minimal environment rather than the caller's entire shell
environment, and provider API keys are not inherited.

Devin-specific notes (verified on `devin` 3000.10.31, Homebrew cask
`devin-cli`, 2026-09-18; the CLI is Cognition's local terminal agent, the
same runtime Devin Desktop drives over ACP, so re-verify on every CLI update
before a sweep):

- **Install and sign in.** `brew install --cask devin-cli`, then
  `devin auth login`. The May 2026 builds (`2026.5.x`, installed by the
  older curl installer into `~/.local/bin`) have a dead `devin update`
  channel, no `devin models` command and no per-run `disabled_tools`; the
  preflight refuses anything older than 3000.10.21 and names the cask. Keep
  `/opt/homebrew/bin` ahead of `~/.local/bin` on `PATH` (the sweep launcher
  does). The login is `credentials.toml` under the data home
  (`~/.local/share/devin`); the preflight checks it for presence only and
  confirms with `devin auth status`, which makes no model call.
- **Effort is the model id's last token, not a flag.** The account catalog
  (`devin models list --format json`) lists one `model_uid` per effort
  variant: `swe-2-medium`, `swe-2-high` and `swe-2-max` for SWE-2 (no low,
  no xhigh), `claude-fable-5-1-xhigh` and so on for other families, with
  `-fast` / `-priority` speed suffixes after the effort token. The adapter
  takes the family (`--model swe-2`) plus `--effort` and composes the uid;
  any uid the catalog does not carry is refused before a model call. This
  matters because the CLI otherwise falls back silently: its log records
  "configured `agent.model` did not resolve to an available, allowed model;
  starting on the default" and carries on. A bare family without `--effort`
  is refused for the same reason (an exact uid such as `swe-2-high` is
  accepted). The served model is proven from every response's
  `generation_model` receipt and a mismatch fails the run; the effort token
  of that served uid is `reported_effort`.
- **Print mode prints only the final text; the record lives in sqlite.**
  `devin --config=<run config> --model <uid> --permission-mode dangerous
  --respect-workspace-trust=false --export=<run_dir>/devin-session/export.json
  --prompt-file <prompt> -p`, stdin closed, in the workspace (print mode
  refuses an untrusted directory by default, and the workspace is a fresh
  tmp perimeter, so trust is asserted per run on the command line and as
  `respect_workspace_trust: false` in the run config). Every message, tool call (`tool_calls`
  with `id`, `name`, `arguments`) and per-request receipt (`metadata.metrics`:
  input, output, cache read and cache creation tokens, plus `request_id` and
  `generation_model`) is in `~/.local/share/devin/cli/sessions.db`
  (`sessions`, `message_nodes`, `tool_call_state`). After the run the adapter
  finds the session by workspace directory and start time, copies it into
  `<run_dir>/devin-session/` (`session.json` with Devin's own
  `total_credit_cost` / `total_acu_cost`, `messages.jsonl`, `tool_calls.jsonl`,
  `cli-log.txt` with any model-resolution warnings), appends normalized
  `tool_call` / `tool_result` lines to `cli-agent-stream.jsonl` for the
  integrity audit (Devin's `read` takes `file_path` and `exec` takes
  `command`; the normalized line carries `path` and `command` so the
  filesystem audit sees them), and sums usage. The CLI stores each assistant
  message twice with identical receipts, so receipts are deduplicated by
  `request_id` and a conflicting copy is an error, never a double count.
  `max_turns` cannot be forwarded (no step cap in print mode); `--timeout`
  is the hard boundary and `--max-run-cost` is rejected.
- **No API-equivalent price.** SWE models exist only inside Devin; the
  catalog lists SWE-2 at cost tier "Free" (through 2026-10-10) with no
  per-token rate, so `devin:` specs are unpriced: `cost_usd` is `None`, the
  economics receipt reports `api_equivalent_cost_usd` as unavailable, and the
  ledger of what a run consumed is Devin's credit and ACU counters in
  `session.json` plus the token receipts. Do not add a `devin:` pricing
  alias to make the number appear.
- **Web denial and context hygiene are per run, never the user's config.**
  The run gets its own `XDG_CONFIG_HOME` holding a generated `config.json`
  (`agent.model` pinned to the uid, `auto_update: false`, and unless
  `--network` is passed `disabled_tools: ["webfetch", "web_search",
  "browser_preview", "close_browser_preview"]` plus a permission deny on
  those tools and `Fetch(*)`); the same file is passed as `--config`. The
  denial is proven per run, not assumed: the ATIF export the CLI writes
  (`devin-session/export.json`) lists the agent's `tool_definitions`, the
  adapter records those names in the trace (`devin_usage.tool_names`) and a
  non-network run that still exposes any of the disabled tools fails closed.
  Verified live on 2026-09-18: the agent listed 23 tools, none of them web,
  and reported both a fetch and a search as impossible. That keeps the user's rules, skills and MCP servers out of
  the benchmark context (skills under `~/.agents/skills` and
  `~/.claude/skills` are HOME-based and still load; recorded as a boundary
  caveat). `DEVIN_MODEL` and `DEVIN_PERMISSION_MODE` are not inherited.
  Devin has no kernel sandbox in this configuration: shell runs on the host,
  so the filesystem channel is the workspace perimeter plus the audit, as
  for ZCode and Cursor.

## Run with an explicit harness

```bash
# Claude Code through a Claude subscription
vulcanbench run --task hello-world \
  --harness claude-code \
  --billing subscription \
  --model claude-sonnet-5 \
  --sandbox local \
  --no-judges

# Codex through a ChatGPT subscription
vulcanbench run --task hello-world \
  --harness codex \
  --billing subscription \
  --model gpt-5.6-sol \
  --no-judges

# ZCode (Z.ai's GLM harness) through a GLM Coding Plan
vulcanbench run --task hello-world \
  --harness zcode \
  --billing subscription \
  --model glm-5.3 \
  --effort extra-high \
  --no-judges

# Devin CLI (Cognition's SWE-2) through a Devin account; effort selects the
# catalog variant swe-2-high
vulcanbench run --task hello-world \
  --harness devin \
  --billing subscription \
  --model swe-2 \
  --effort high \
  --sandbox local \
  --no-judges
```

The old `--model claude-code:<model>` form remains supported. For publication,
use `--no-judges` during execution and grade saved patches with the same fixed,
independent judge. Deterministic task verifiers are unchanged.

## Economics receipt

A subscription run does not claim that included usage cost zero. Each
`summary.json` contains an `economics` object with independent fields:

- `marginal_cash_usd`: cash caused by this run; unknown until the product
  provides an overage receipt.
- `overage_cash_usd`: paid usage beyond the plan allowance, when measurable.
- `allocated_plan_cost_usd`: a modeled share of the plan fee, when supplied.
- `grading_cash_usd`: metered cash for an independent API judge; unknown when
  grading also uses an included subscription.
- `grading_api_equivalent_usd`: counterfactual API value of grading usage.
- `api_equivalent_cost_usd`: counterfactual API cost derived from reported
  tokens.
- `quota`: provider-reported usage-window consumption, when available.
- `measurement_quality`: whether each value is exact, provider-reported,
  estimated, or unavailable.

The legacy top-level `cost_usd` remains an API-equivalent compatibility field
for existing tools. New reports and leaderboards label marginal cash and API
equivalent separately.

When the CLI reports cache reads and the price table has a `cached_input`
rate, VulcanBench subtracts those tokens from full-price input and applies the
cache-read rate separately. The receipt's `measurement_quality` states whether
cache pricing was applied. Cache writes and model-specific long-context tiers
remain unknown unless the CLI exposes enough per-request detail to price them.

```bash
vulcanbench leaderboard --track subscription
vulcanbench leaderboard --track api
```

Never add the two tracks to one model-performance claim. Subscription results
include the product's system prompt, context management, safety layers, tools,
and possible model routing.

## Reproducibility receipt

External-harness summaries record:

- Harness and CLI version
- Authentication mode and non-secret plan label
- Requested and CLI-reported model, when exposed
- Model-identity confidence
- Requested effort and the actual value sent
- Raw, redacted CLI event stream
- Token and cache usage exposed by the CLI
- Economics measurement basis
- Task hash, environment manifest, final patch, and verifier result

If a CLI cannot report a field, VulcanBench records it as unknown rather than
inferring it.

## Limits and recovery

Subscription quota exhaustion is a non-retryable infrastructure outcome, not a
task failure. The suite stops hot-looping that unit and records an error; resume
after the usage window resets:

```bash
vulcanbench run --suite v3 --harness codex --billing subscription \
  --model gpt-5.6-sol --repeat 3 --only-missing --no-judges
```

Codex currently reports token usage when a turn completes, so VulcanBench
rejects `--max-run-cost` for Codex rather than pretending it can enforce a live
cap. Use `--timeout` for a hard per-run boundary. Claude Code streams usage and
can enforce the API-equivalent cap during a run.

## Publication protocol

1. Freeze the task set and CLI versions.
2. Run `harness doctor` and save its non-secret JSON receipt.
3. Run one cheap smoke task.
4. Run a stratified pilot across easy, medium, and difficult tasks.
5. Use concurrency one for the baseline subscription comparison.
6. Complete every task/repeat cell or label the column incomplete.
7. Publish pass@1 with uncertainty, latency, quota/cost basis, and harness
   version.
8. Keep a small raw-API control subset to estimate the harness delta.

Use Docker verification for publication runs. A missing verifier dependency is
an infrastructure error, not a model failure; VulcanBench now surfaces and
retries that condition instead of recording a functional zero.
