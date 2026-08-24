# Changelog

All notable changes to VulcanBench are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- **Multi-service task environments** (`harness/environment.py`): tasks can
  declare `metadata.environment` with a docker-compose file and readiness
  probes. The harness brings the stack up under a unique per-run project
  before the agent's clock starts, resolves ephemeral published ports into a
  gitignored `.vb_services.json` in the workspace, and always tears down with
  `down -v` — in `vulcanbench run` and in task validation (fresh stack per
  gold/base/determinism run). Validation rejects the isolation footguns
  (`container_name`, fixed host ports). Environment tasks require
  `--sandbox local`; the walking-skeleton template is
  `tasks/cii-v2/demo-compose-redis-smoke`.

- **Coding Intelligence Index v1** (`tasks/cii-v1/`, `--suite cii-v1`): 38 all-new
  gold-verified tasks from post-cutoff OSS PRs (May to Aug 2026) with
  complexity-scaled TB-style budgets; August 2026 frontier results published in
  `docs/results/cii-v1-2026-08/` (Opus 5 96.3% vs GPT 5.6 Sol 86.5% pass@1).
- **CII v2 frontier charter + admission gate** (`tasks/cii-v2/`,
  `scripts/frontier_gate.sh`): difficulty-gated suite requiring n=3 measurement
  against two reference frontier models before admission; candidate log with
  per-run verdicts (wave 1: 0/5 admits, all recycled into v1).
- **`codex:` CLI agent provider** (subscription-billed Codex runs) and
  resumable measurement workers/top-up scripts used for the CII sweeps.
- **CII report generator** (`scripts/cii-report/make_chart.py`): brand-styled
  results chart aggregated live from `./runs`.

- **ZCode harness (`--harness zcode`).** Runs tasks through Z.ai's GLM
  coding harness billed to a GLM Coding Plan. ZCode ships as a desktop app;
  the adapter drives the same agent runtime through the `zcode-app-cli` npm
  launcher's headless `--prompt` mode (verified on launcher 3.8.1-15 /
  runtime 0.16.3, where `--max-turns`, `--settings` and `--allowed-tools`
  print in the help but are rejected by the runtime's strict parser). Per-run
  settings travel through the workspace's `.zcode/config.json`: the model,
  permission mode `yolo`, memory off (repeat isolation), the web denies
  (`--disallowed-tools WebFetch WebSearch web_search` plus the Browser Use
  plugin disabled), and the effort level via
  `modelCatalog.overrides[...].reasoning.defaultLevel` (low/high/max, the
  GLM 5.3 API enum; the level ZCode actually ran is read back from its usage
  ledger as `reported_effort`). Headless stdout is only the final text, so
  messages, tool calls and per-request token receipts are harvested from
  ZCode's sqlite session store into `<run_dir>/zcode-session/` and folded
  into the stream log for the integrity audit; `zcode:` prices via the
  `zai:` table and `--max-run-cost` is rejected (no live usage stream).
  Preflight reads the OAuth credential store for presence only and fails
  closed when signed out or when the configured key targets the metered
  `/api/paas/v4` endpoint instead of a Coding Plan endpoint. Coding Plan
  window exhaustion (codes `[1308]` 5-hour/weekly, `[1302]` rate) is detected
  even when the CLI buries it under a generic stack (scanned from the
  harvested `model_usage.error_message`) and raised as a resumable
  `SubscriptionQuotaError` so a drained window pauses the suite instead of
  hot-retrying.
- **Report No. 18: GLM 5.3, model versus harness.** Published under
  [`docs/results/v3-glm53-2026-08/`](docs/results/v3-glm53-2026-08/model-card.md):
  GLM 5.3 on suite v3 run two ways, VulcanBench's uniform loop on the raw
  `zai` API versus Z.ai's ZCode harness on a GLM Coding Plan. Same model, a
  21.8-point pass@1 gap at `max` (65.2% API vs 87.0% ZCode), opposite effort
  curves, and every raw-API failure a wall-clock timeout while every ZCode
  failure is a wrong answer. Model card, JSON, and a branded comparison chart;
  indexed in [`docs/results/README.md`](docs/results/README.md).
- **`zai:` balance exhaustion fails fast.** Code `1113` ("Insufficient
  balance or no resource package") arrives as HTTP 429 but cannot succeed on
  retry, so `ZaiProvider` now raises `NonRetryableProviderError` with a
  recharge hint rather than burning the run's retry budget against a drained
  balance.
- **Grok Build harness (`--harness grok-build`).** Runs tasks through xAI's
  `grok` CLI billed to a grok.com subscription, under a custom kernel
  sandbox profile (Seatbelt/Landlock): workspace writes plus a kernel deny
  on this checkout, so answer keys are unreadable even when the agent knows
  the path, while toolchains stay usable. Web tools are removed outright via
  `--disallowed-tools`, and `--no-memory` keeps repeats from remembering
  each other. `_subscription_env` now scrubs repo-rooted PATH entries for
  every harness (a live run extracted the repo path from `.venv/bin` and ran
  `find` over it; the kernel denied the read, and now the path never leaks).
  On grok >= 1.0 the stream reports the full token split, `num_turns`, and
  the CLI's own `total_cost_usd`, so grok-build runs carry real token
  receipts (priced via the `xai:` table) and support a live
  `--max-run-cost` cap; the session-trace harvest remains as the audit
  substrate and a guard against stream regressions. Two alpha-CLI traps are documented and coded around:
  the CLI's `--effort` flag parses but is silently ignored for reasoning
  (the adapter sends `--reasoning-effort`, and copies the session summary's
  `reasoning_effort` back into the outcome as `reported_effort` as proof),
  and the live stream carries no tool calls or usage, the adapter
  pre-assigns the session id and harvests the session trace
  (`updates.jsonl`, `summary.json`, `events.jsonl`) into the run dir so the
  integrity audit has a substrate. The trace's cumulative `totalTokens` is
  recorded as `cli_total_tokens` (no prompt/completion split, so economics
  stays unavailable). Preflight fails closed when signed out or when
  `XAI_API_KEY` is set. The audit now also recognizes Grok's snake_case web
  tool titles and `target_file`/`target_directory` path arguments.

- **Workspace containment and a filesystem integrity audit.** Blocking the web
  fixed only one leak. CLI harnesses execute on the host, so a workspace inside
  this checkout let an agent walk up into `tasks/` and read `gold_patch.diff`
  and the hidden tests: in a sweep with the web already blocked, 46 runs read
  their own task's answer key and all 46 solved. CLI-harness runs now execute
  in a workspace outside the repo (moved back under the run dir after scoring),
  and `web_audit` becomes `integrity_audit` with a second channel: filesystem
  verdicts `clean` / `out_of_workspace` / `benchmark_data_access` /
  `answer_key_access`. `vulcanbench audit-web` is now `audit-runs` and reports
  both channels. VulcanBench's own agent loop was never exposed to either
  channel: it has no web tools and its sandbox is network-off with no host
  mount.
- **Web-leakage prevention and audit for CLI harnesses.** External harnesses
  can browse, and v3 tasks derive from public merged PRs, so the fix exists at
  a known URL; in the first Cursor sweep 46% of runs fetched their task's
  exact source PR or fix commit. Three layers now: the Cursor adapter writes a
  workspace permissions file denying WebFetch/WebSearch unless `--network`,
  and switches `--force` to `--trust` for those runs -- verified against
  cursor-agent 2026.08, where `--force` approves denied queries too, while
  `--trust` honours denies but needs an explicit allow list to keep shell
  access. A `web_blocked` verdict records denied attempts; every CLI-harness run summary records a
  `web_audit` block matching captured web activity against the task's
  `metadata.upstream` provenance (verdicts no_web / web_used /
  upstream_access / solution_retrieval, with `contaminated` set on the top
  two); and `vulcanbench audit-web runs/` retro-annotates existing runs. The
  audit annotates, never rescores. Terminal-Bench, which also allows internet,
  asks users to "remain vigilant"; this automates that vigilance using
  provenance Terminal-Bench's original tasks don't have.
- **Meta Model API provider** (`meta:muse-spark-1.2`): calls Muse Spark through
  Meta's OpenAI-compatible Responses API with `MODEL_API_KEY`, while the agent's
  tools and deterministic verifier remain inside VulcanBench's Docker sandbox.
  This bypasses Muse Code's container sign-in failure without weakening task
  isolation. Supports low/medium/high effort plus `extra-high` -> `xhigh`, and
  prices both the standard and data-sharing Contributor tiers.
- **OpenRouter route for Muse Spark**: setting `META_BASE_URL=https://openrouter.ai/api/v1`
  (with `OPENROUTER_API_KEY`) reaches the same model when Meta API access is
  unavailable. The wire id is namespaced to `meta/muse-spark-1.2` while the harness
  spec stays `meta:muse-spark-1.2`, so pricing keys and `compare` output still match
  Meta-direct runs. Requests pin `provider: {order: ["meta"], allow_fallbacks: false}`
, OpenRouter's only endpoint for the model is Meta's own, and the pin keeps a
  future re-hosted endpoint from silently joining a sweep. The Contributor tier is
  rejected on this route (Meta-direct only) instead of billing at a rate OpenRouter
  does not sell. Cache-read parsing now also accepts `prompt_tokens_details`.
- **Cursor CLI harness** (`cursor:<model>` / `--harness cursor`): runs a task
  through `cursor-agent -p --output-format stream-json`, billed to a Cursor
  account (plan or promotional credits). Results measure model + Cursor
  harness, tracked as a subscription run. The CLI streams no usage or cost, so
  tokens record as zero and the economics receipt marks API-equivalent value
  unavailable instead of fabricating $0 (the loop's receipt quality now says
  `unavailable` whenever a harness reports no token basis). Preflight fails
  closed when signed out or when `CURSOR_API_KEY` is set; usage-limit errors
  raise the resumable quota error; `--effort low|medium|high` maps to Cursor's
  `model[effort=...]` bracket syntax (Cursor's Grok family instead bakes
  effort into the model id: `cursor-grok-4.6-low` ... `-xhigh`). Run with
  `--sandbox docker` so hidden-test verification uses the sandbox image.
- **xAI provider for Grok** (`xai:<model>`, e.g. `xai:grok-4.6`): OpenAI-compatible
  Chat Completions with xAI's documented `reasoning_effort` enum, 
  low/medium/high map directly, `extra-high` -> `xhigh` (Grok 4.6+ only; pre-4.6
  silently coerces xhigh to high). xAI's default is `high` and reasoning cannot
  be disabled, so an unset `--effort` is not a neutral point. Replaces the
  previous practice of running Grok through `openai:` with a base-URL swap,
  which mislabeled the lab in run records. Built-in <200K-tier pricing for
  grok-4.6/4.5/4.3; per-provider cache-read folding (0.25x/0.15x/0.16x of the
  input rate) replaces the parser's OpenAI-specific 0.1x, which under-reported
  xAI bills.
- **Ollama provider for local open-weights models** (`ollama:<model>`, e.g.
  `ollama:muse-glimmer:30b`): local inference through Ollama's OpenAI-compatible
  Chat Completions API. No API key; pricing records $0 marginal cash (hardware
  and electricity are not modeled) rather than "cost unknown", and local runs
  are excluded from the cost-estimate index. `OLLAMA_BASE_URL` overrides the
  default `http://localhost:11434/v1` and accepts any OpenAI-compatible local
  server. Request timeout ceiling is 1800s (long-context prompt processing is
  compute-bound on local hardware); 404s hint at `ollama pull`, connection
  failures hint at the server not running. Reasoning effort is recorded as
  metadata only.
- **`minimal` reasoning effort**: `--effort minimal` joins the normalized
  vocabulary, completing Meta's documented enum for Muse Spark
  (minimal/low/medium/high/xhigh) and mapping directly on OpenAI. Providers
  without a documented minimal level (Anthropic, Claude Code, Kimi, Qwen,
  DeepSeek) record it as metadata without sending it. The Claude Code CLI path
  gets its own effort map so the new label is not forwarded to a `--effort`
  flag that lacks it; its reachable set is unchanged. Verified live through the
  OpenRouter route: accepted, echoed back, and 22 reasoning tokens against
  ~400+ at `low` on the same prompt. Default sweeps stay low/medium/high;
  minimal is opt-in. Meta runs with `--effort` unset reason at an undocumented
  "model-determined level", so sweeps should always pass an explicit level.
- **Qwen reasoning effort** (`qwen:<model>`, Qwen3.8+): `--effort low/medium` maps to
  DashScope's `reasoning_effort` and `extra-high` maps to its `xhigh`; `high` is recorded
  as metadata only because Qwen's documented enum is low/medium/xhigh (no `high`) with
  `xhigh` as the default, an unset request already runs at xhigh. Built-in pricing adds
  `qwen3.8-max` ($2/$6 per 1M, flat across the 1M context).

### Changed

- **Run manifests record a `route` block** when a run did not use its provider's
  default API endpoint (base URL, wire model, pinned upstream). Meta-direct and
  OpenRouter-routed runs were previously indistinguishable in a results directory.
  Default-route manifests are unchanged.

### Fixed

- **xAI reasoning tokens are now billed.** xAI reports reasoning OUTSIDE
  `completion_tokens` (total = prompt + completion + reasoning), unlike
  OpenAI/DeepSeek/Qwen/Kimi where completion contains it. The Chat Completions
  parser recorded completion alone, so every `xai:` run under-counted output
  tokens and understated `cost_usd` by the reasoning volume -- the dominant
  output term for Grok. The parser now folds
  `completion_tokens_details.reasoning_tokens` into completion for providers
  that exclude it (xAI only). Recorded costs on prior Grok runs (Report No. 14
  and the historical `openai:grok-4.5` shim runs) are lower bounds; the raw
  reasoning counts were not persisted and must be reconciled against xAI
  console billing.
- **Patch capture and verifier output decode lossily.** The real crash site
  behind the semver decode failures was `_git_diff`: the agent's staged diff is
  whatever bytes the agent wrote, and strict decoding of a ~110MB artifact with
  one 0xa0 byte killed runs at capture time, after the model spend. `git diff`
  capture, changed-file listing, and verifier output now decode UTF-8 with
  errors="replace" alongside the CLI-agent streams.
- **CLI-agent streams decode lossily.** All three subscription adapters read
  the vendor CLI's stdout with strict platform decoding; a single non-UTF-8
  byte in a large stream (observed: 0xa0 at ~110MB of cursor-agent output)
  crashed the run after the subscription credits were already spent. The
  claude-code, codex, and cursor Popen streams now decode UTF-8 with
  errors="replace", so a stray byte costs one replacement character, not a
  paid run.
- **Cursor harness no longer forces the hidden-test verifier onto the host.**
  Requiring `--sandbox local` for `cursor:` runs (copied from the claude-code
  rule) also forced VulcanBench's verifier to the host, where toolchains don't
  match the sandbox image, every Python task in a 69-run suite failed with
  "pytest is unavailable in the verifier environment" after the agent had
  already spent subscription credits. Cursor brings its own agent sandbox, so
  like Codex it is now exempt: `--sandbox docker` runs the agent on the host
  workspace and setup/verification in Docker over the same directory.
- **Client errors are no longer retried.** Any HTTP 4xx other than 408/409/425/429
  now raises `NonRetryableProviderError`: a bad key, missing entitlement, unknown
  model, or malformed body fails identically on every attempt, so retrying only
  burned wall clock against the run's timeout budget, and then once more per task
  under a suite's infra-retry policy. Found when a 403 (`missing 18+ attestation`)
  from OpenRouter consumed its full attempt budget with backoff.
- **Tests no longer read the developer's `.env`.** Importing `harness.cli` calls
  `load_dotenv()` at module scope, so the first test to import it injected real
  provider keys and base URLs into `os.environ` for the whole session; provider
  tests then passed or failed based on the machine's configuration. A new autouse
  fixture in `tests/conftest.py` clears provider routing and credential vars.

## [0.8.0] - 2026-07-29

### Added

- **Voice Eval Suite v1** (`vulcanbench voice render|run|report`): measures the delta between a
  model answering the same held-out question set as text vs. as TTS-rendered speech, the
  "voice tax". 200 original questions (`tasks/voice-v1/`, 5 categories), audio matrix of
  3 voices x 2 rates x clean/10 dB-SNR noise with a seeded subset for the expensive conditions,
  disk-cached renders keyed by (question, voice, rate, noise), and adapters for OpenAI Realtime,
  Gemini Live, Qwen3-Omni, and xAI Grok Voice (pinned `grok-voice-think-fast-2.0`,
  speech-to-speech: own-transcript scoring with pinned STT fallback) behind a common
  `answer_text`/`answer_audio` contract. Scoring is
  modality-blind by construction (single scorer: normalize → exact/alias → pinned LLM judge,
  rubric in-repo); runs are resumable, rate-limited, and record a full manifest (models, TTS
  voices, judge, STT fallback, seed, git commit, question-file hash). See `docs/VOICE_EVAL.md`.
- New dependency: `websockets` (OpenAI Realtime + Gemini Live + Grok Voice transports).

### Added

- **DeepSeek provider** (`deepseek:<model>`): DeepSeek OpenAI-compatible Chat Completions
  API. Needs `DEEPSEEK_API_KEY`; base URL defaults to `https://api.deepseek.com`
  (override with `DEEPSEEK_BASE_URL`). `--effort low/high` maps to the API's
  `reasoning_effort` field and `extra-high` maps to its `max`; `medium` is recorded as
  metadata only, because DeepSeek's documented enum is low/high/max and the API silently
  coerces `medium` to `high`: sending it would stamp false effort metadata on runs.
  Built-in pricing covers `deepseek-v4-flash` and `deepseek-v4-pro` (public-beta list
  rates; the announced peak/off-peak 2x policy is not yet in effect and is not modeled).
- **CLI loads `./.env`**: the `vulcanbench` CLI now calls `load_dotenv(override=False)` at
  import, so provider keys and harness settings in `.env` (see `.env.example`) are picked up
  without exporting them; variables already set in the shell take precedence.

- **Subscription-billed runs via vendor agent CLIs** (`claude-code:<model>` specs): the task is
  handed to Claude Code headless (`claude -p --output-format stream-json`) in the prepared
  workspace instead of the VulcanBench agent loop, billing a Claude Pro/Max subscription instead of
  API rates. Everything downstream (git diff → verifier → evaluator → scoring) is unchanged, and
  the CLI's stream is translated into trace events so `replay.html` still works. Honesty
  guarantees: `cost_usd` is the *hypothetical* API cost of the reported token usage
  (`claude-code:` prices alias to `anthropic:`), the summary records
  `cli_agent: {harness, billing: "subscription", cli_reported_cost_usd, session_id, ...}` so
  vendor-harness results can't be silently mixed with uniform-loop columns, subscription
  usage-limit hits raise a run *error* (resumable with `--only-missing`) instead of scoring a
  starved run as 0, and `ANTHROPIC_API_KEY` is stripped from the CLI subprocess so a set key can't
  silently flip a run onto API billing. `--max-run-cost` (enforced mid-run against hypothetical
  cost), `--timeout`, and `--max-steps` (mapped to `--max-turns`) all work; judges/graders can
  also run on the subscription via `--judge-model claude-code:<model>` (single-shot, tools
  disabled). Requires `--sandbox local` (the CLI executes its own tools host-side). See
  QUICKSTART → "Run on your Claude subscription".
- **Second OSS task, and the first decontaminated one** `oss-more-itertools-iter-index` (hard,
  `bug_fix`, `medium` scale): a genuine post-cutoff fix from `more-itertools` commit `43decdd7`
  (2026-06-17). `iter_index` accepted negative `start`/`stop` for sequences (fast path) but the
  general-iterable slow path passed them to `islice`, which rejects negative indices and raises; the
  fix materializes to a tuple so both paths agree. The agent must find `iter_index` in a ~7k-LOC
  vendored package (MIT LICENSE preserved) and reason about the dual code paths. Graded by the
  commit's own test (`grader: tests`), `source: oss` / `decontaminated: true` (post-cutoff). The
  provenance dataset guard now allows decontaminated OSS tasks (was hard-coded to `false`). Validated:
  gold=1.0, pre-patch fails, deterministic over 3 runs.
- **First real OSS task, deterministically graded** `oss-click-choice-brackets` (hard, `system`,
  `bug_fix`): a genuine post-cutoff bug fix from `pallets/click` PR #3578 (optional `Choice`
  arguments rendered `[[a|b|c]]` instead of `[a|b|c]`). The repo is the real click package sliced at
  the PR's base commit via `scripts/slice_repo.py` (BSD LICENSE preserved), `source: oss` /
  `decontaminated: false`. Graded by the PR's **own tests** (`grader: tests`, `PYTHONPATH=src
  pytest`), deterministic, no LLM judge. Validated: gold=1.0, pre-patch fails, deterministic over 3
  runs. Establishes the FrontierCode-style pipeline (slice a real PR → run its tests) without the
  judge-noise/asymmetry/gold-mimicry pitfalls that LLM-rubric grading hit on subtle tasks.

- **Carbyne tier (`v1-carbyne` suite)**: 22 harder rubric-graded tasks (named for the carbon
  allotrope harder than diamond). The diamond run showed the frontier passes "obvious" mergeability
  traps and only loses cosmetic style points, so carbyne tasks use *terse prompts that do not
  telegraph the trap* over **subtly-wrong-naive** problems, with **substantive-only rubrics** (no
  cosmetic style criteria). The first run separated the frontier (Opus/GLM 1.0 > GPT-5.5 ~0.96 on
  real defects, confirmed under a conservative self-judge), so the tier was scaled with subtler
  non-telegraphed traps where the obvious solution a strong model writes is wrong a meaningful
  fraction of the time: end-of-month date clamping (Jan 31 + 1mo), truncate-fits-in-n, touching-
  interval merge, Go floor-division on negatives, leftmost insertion point, None-last total-order
  sort, snapshot-under-lock publish (deadlock-safe), LRU-touch-on-read, keep-first dedup, rotate
  edge cases, CSV newline/quote escaping, and duration-format edge cases, alongside the original
  atomic transfer, exact money split, defensive copy (`readonly` type on the same ref isn't a copy),
  get-or-create TOCTOU race, idempotent charge, transaction rollback+re-raise, even-length median,
  timezone-aware comparison (`utcnow()` is naive), N+1 batch fetch, and order-preserving dedup.
  Every task's grader-trust passes with a live judge (accuracy=1.0, false_pass=0). New `carbyne`
  key + `v1-carbyne` alias; run with a
  `--judge-model` different from the model under test.
- **Diamond tier (`v1-diamond` suite)**: the first batch of rubric-graded mergeability tasks,
  the answer to frontier saturation (GPT-5.5 hit 100% functional on the test-graded suite). Each
  ships a terse prompt over house-style code where correctness is trivial but the rubric catches
  *mergeability*: `py-validate-convention` (raise the module's error type, not ValueError/bool),
  `py-immutable-transform` (don't mutate the input), `go-counter-lock` (reuse the receiver mutex,
  no data race), `py-logging-convention` (use the module logger, not print), `py-sql-params`
  (bound parameters, not string interpolation), `ts-api-reuse` (reuse the shared request wrapper),
  plus `py-orders-rubric`. Every task's grader-trust checks pass with a live judge
  (accuracy=1.0, false_pass=0 over labeled working-but-unmergeable variants). New `diamond` key in
  `tasks/v1/suite.json` and `v1-diamond` suite alias; run with a `--judge-model` different from the
  model under test. Kept out of the default/compare suites (no self-grading there).
- **Rubric grading (`grader: "rubric"`)**: a third grading mode that scores *mergeability*,
  not just correctness, the axis that still separates frontier models once functional
  correctness saturates (motivated by Cognition's FrontierCode). A task ships a
  `metadata.rubric` with `blocking` criteria (failing any scores 0) plus `weighted` quality
  criteria; `functional` becomes continuous in `[0, 1]`. Implemented in
  `harness/evaluator/agentic_grader.grade_rubric` (blocking + weighted aggregate, per-criterion
  majority vote over `grader_samples`), wired through `evaluate_run`, the run loop, the spec
  gate (terse prompts allowed), `validate-task` (offline mock-grader wiring check), and
  `grader_eval.py` (grader-trust). First example task `py-orders-rubric` (terse "add
  `list_orders`" over a house-style API client) with a 4-case calibration set; a real judge
  scored it accuracy=1.0, false_pass=0 (gold mergeable=1.0, bypass-plumbing≈0.25,
  swallow-error≈0.625, wrong=0). Not in the default suite (kept out of self-graded compare runs).
- **Multi-file feature** `py-textdiff` (hard, `multi_file`): an LCS-based line diff plus a
  validating patch applier. `textdiff/diff.py` builds a minimal edit script of
  keep/del/ins ops whose kept lines are a *longest* common subsequence;
  `textdiff/patch.py` applies a script against a source, raising `PatchError` on mismatch or
  unconsumed input. The files share only the op-tuple contract; the defining invariant
  `apply(diff(a, b), a) == b` holds only if both halves agree. The minimality test rejects a
  naive delete-all-then-insert-all diff. Validated (gold=1.0, pre-patch fails, deterministic);
  the gold patch spans both files.
- **Bug fix** `go-slice-batches` (medium, `bug_fix`): the classic Go slice-aliasing footgun.
  `batch.Batches` returns `items[i:end]` sub-slices that keep spare capacity over the input's
  backing array, so `append(b[0], x)` overwrites the next batch and the caller's slice. The
  fix copies each batch into its own storage. Contents/size-clamp tests pass on the buggy
  version (values are correct); independence, no-alias, and zero-spare-capacity tests fail
  until fixed. Validated (gold=1.0, pre-patch fails, deterministic).
- **Behavior-preserving refactor** `py-extract-pricing` (medium, `refactor`, `multi_file`):
  `shop/checkout.py` inlines every pricing rule in one `total` function; extract them into the
  four pure helpers in `shop/pricing.py` (subtotal, discount, tax, shipping) and have `total`
  compose them without changing results. The pricing tests fail pre-refactor (stubs); the
  end-to-end total tests already pass and must stay green, so any drift in the extracted
  rules (flooring, the flat-discount cap, the free-shipping threshold) is caught. Validated
  (gold=1.0, pre-patch fails, deterministic). Broadens the `refactor` category.
- **Multi-file feature** `go-pubsub` (hard, `multi_file`): an in-memory publish/subscribe
  broker with MQTT-style wildcard topics. `pubsub/match.go` implements segment matching (`+`
  is exactly one segment, `#` matches the trailing remainder including zero segments);
  `pubsub/broker.go` is a mutex-guarded subscription registry delivering in subscription
  order, with callbacks invoked outside the lock so a callback may (un)subscribe. The
  concurrent test runs under `-race`, so a non-thread-safe solution is rejected. Validated
  (gold=1.0, pre-patch fails, deterministic); the gold patch spans both files.
- **Multi-file feature** `ts-state-machine` (medium, `multi_file`): implement a finite state
  machine over a transition table. `src/transitions.ts` indexes `{from, event, to}` triples
  for `nextState`/`allowedEvents`; `src/machine.ts` drives state + history and uses the table
  to validate `send`, throwing `InvalidTransitionError` and staying a no-op on an invalid
  event, and notifying listeners in registration order. First non-localized TypeScript task.
  Validated (gold=1.0, pre-patch fails, deterministic); the gold patch spans both files.
- **Async bug fix** `ts-retry-backoff` (medium, `bug_fix`): an exponential-backoff retry
  helper in `src/retry.ts` with two classic bugs, an off-by-one that runs `attempts - 1`
  tries (so an operation that would succeed on its final attempt is reported failed), and an
  uncapped delay that ignores `maxDelayMs`. An injected `sleep` recorder makes the exact
  backoff schedule (`[10, 20, 40, 50, 50]`) and the call count assertable with no real
  timers. Validated (gold=1.0, pre-patch fails, deterministic).
- **Concurrency bug fix** `go-ratelimit` (hard, `concurrency`): a token-bucket rate limiter
  for throttling an API client that over-grants and races. `bucket/bucket.go` never clamps
  refill to capacity (an idle bucket grants a huge burst) and has no locking (concurrent
  `Allow` callers race and over-grant). The fix needs both a capacity clamp and a mutex held
  across refill+consume. Tests inject a clock for deterministic timing and run under `-race`,
  asserting exactly `capacity` concurrent grants. Validated (gold=1.0, pre-patch fails,
  deterministic).
- **Realistic multi-file bug fix** `py-paginate-cursor` (medium, `multi_file`, `bug_fix`):
  cursor pagination that repeats rows at page boundaries and never terminates. Two bugs in
  two files: `pagination/cursor.py` compares with `>=` (the cursor row is treated as "after"
  itself, so it repeats), and `pagination/repository.py` always advertises a `next_cursor`
  (so a walk never ends, and loops forever on the last record). Ships real buggy code, not
  stubs; tests walk the full dataset asserting no duplicates, no gaps, correct tie-breaking
  on `id`, and termination (with a loop guard so the buggy version fails instead of hanging).
  Validated (gold=1.0, pre-patch fails, deterministic).
- **Fourth multi-file task** `py-event-ledger` (hard, `multi_file`): implement an
  event-sourced bank ledger. `ledger/events.py` holds the immutable event schema and a
  pure reducer (`apply_event`/`replay`); `ledger/bank.py` is the command side that validates
  commands, emits events, and maintains live balances through the reducer. The difficulty is
  cross-file: the live state must always equal a replay of the log, a successful `transfer`
  must emit exactly two events atomically (never a half-applied transfer), rejected commands
  must emit nothing, and `Bank.from_history(history())` must reproduce balances exactly.
  Validated (gold=1.0, pre-patch=0.0, deterministic); the gold patch spans both files.
  Suite v1: 42 tasks, 14 hard, 6 non-localized.
- **Third multi-file task** `py-bytecode-vm` (hard, `multi_file`): implement a bytecode
  compiler and a stack VM that share only a documented instruction set. `vmlang/compiler.py`
  lowers an AST to a flat instruction list (with backpatched forward jumps for `if` and
  short-circuit `and`/`or`); `vmlang/vm.py` executes that list, treating jump operands as
  absolute indices. The difficulty is cross-file: the compiler's opcodes and jump offsets
  must line up exactly with the VM's stack discipline, and short-circuit correctness
  requires the two halves to cooperate so the un-selected operand is never executed.
  Validated (gold=1.0, pre-patch=0.0, deterministic); the gold patch spans both files.
  Suite v1: 41 tasks, 13 hard, 5 non-localized.
- **Second multi-file task** `py-txn-kvstore` (hard, `multi_file`): implement a
  transactional in-memory key/value store whose `Store` and `UndoJournal` collaborate
  across two files. The store computes the inverse of each mutation (prior value vs. prior
  absence) and the journal owns the frame stack, where rollback runs inverses in reverse
  and a nested commit folds its frame into the parent so an outer rollback still undoes
  inner-committed work. Validated (gold=1.0, pre-patch=0.0, deterministic); the gold patch
  spans both files. Suite v1: 40 tasks, 12 hard, 4 non-localized.
- **First real multi-file task** `py-reactive-sheet` (hard, `multi_file`): implement a
  reactive spreadsheet whose `Sheet` and `DependencyGraph` collaborate across two files,
  requiring transitive topological recomputation, dependency clearing on formula rebind,
  and cycle detection kept consistent across both modules. Validated; the gold patch spans
  both files. Suite v1: 39 tasks, 11 hard, 3 non-localized.
- **Grader trust + variance controls** for agentic grading: `metadata.grader_samples: N`
  grades by majority vote over N calls (ties resolve to incorrect) and reports
  `self_consistency`; `scripts/grader_eval.py` (+ `harness/grader_eval.py`) scores a
  task's grader against labeled `grader_cases.json` candidates, reporting accuracy,
  false-pass rate, and self-consistency so a grader is validated before it's trusted.
  Labeled calibration sets ship for three agentic tasks across task shapes
  (`py-slugify-terse`, `py-parse-bool-terse`, `py-chunk-terse`), each with a gold
  case plus three subtly-wrong variants, so grader trust is measured on more than
  one task.
- **Agentic grader (opt-in)**: a task can set `metadata.grader: "agentic"` with an
  `acceptance_criteria` list, and its `functional` score comes from an LLM verdict
  on the agent's diff (`harness/evaluator/agentic_grader.py`) instead of hidden
  tests, so the prompt can be terse and realistic (CursorBench-style) while the
  grader holds the spec. Runs even with `--no-judges`; uses `--judge-model` so a
  strong, independent model can grade. The spec gate is skipped for agentic tasks;
  `validate-task` checks the wiring offline (gold grades correct, an empty change
  grades incorrect). Demo: `tasks/v1/py-slugify-terse`. Test-graded tasks remain
  the deterministic default.
- **Discrimination report**: `vulcanbench report` now includes a model-separation
  section, per task whether the models split, and per model pair how many tasks
  tell them apart (McNemar discordant counts), plus retirement candidates that
  every model passes or fails. Surfaces ties that aggregate pass@1 hides (e.g.
  two models posting an identical 0.7885 with zero tasks separating them).
- **Hard, discriminating tasks** to raise the suite's ceiling (52 -> 43 -> 31 ->
  35): `py-expr-eval` (a recursive-descent arithmetic evaluator with a subtle
  operator-precedence/associativity bug, `2 + 3 * 4`, `2 ** 3 ** 2`, `-2 ** 2`),
  `go-parallel-map` (a bounded-concurrency ordered map that must preserve input
  order, return the lowest-input-index error, and pass `go test -race`),
  `py-sliding-window-max` (an O(n) deque solution gated by a performance test, so
  a correct naive O(n*k) answer is too slow), `go-ttl-lru-cache` (a
  thread-safe LRU cache with per-entry TTL, verified under `go test -race`, where
  a correct single-threaded implementation that omits the mutex fails), and two
  edge-dense correctness tasks designed to pull frontier models below 100%:
  `py-url-normalize` (RFC 3986 path normalization, percent decoding before
  dot-segment removal, with malformed-input handling) and `py-semver-compare`
  (Semantic Versioning precedence, including the prerelease identifier rules that
  trip strong models). Their hidden tests are split into many independent
  edge-case groups, so missing any one leaves the task unsolved. Suite v1 is now
  37 tasks; the hard floor in the dataset guards rose from 5 to 9.
- **Specification gate (`harness/spec_check.py`)**: task validation now flags
  issues that state a defect or location but never describe the expected
  behavior, the failure mode where a hidden test asserts an output the agent
  cannot infer. `vulcanbench validate-task` / `make validate-tasks` downgrade
  such tasks `PASS -> WARN` (warnings do not fail the run); `scripts/check_spec.py`
  scans the suite offline. A reference-model `solvability_verdict` fails
  trivially small, localized fixes that a capable model still cannot solve.

### Changed

- **Curation discipline**: added composition guards (`tests/test_dataset.py`) for
  a hard-task floor, a medium-or-hard majority, and non-localized coverage so the
  suite can't silently regress to easy filler; documented the discipline
  (specify behavior, aim above the floor, calibrate-then-retire) in
  `CONTRIBUTING.md`; and reconciled the README's task-corpus description with the
  actual composition (predominantly `localized` today, with broader
  `multi_file`/`system`/`architecture` and larger `repo_scale` coverage tracked
  as active work rather than claimed as shipped).
- **Suite v1 pruned to 31 tasks** (from 52) to raise discriminating power. First,
  9 placeholder Python scaffolds (`oss-py-cache-evict`, `oss-py-m2-03/06/09`,
  `oss-py-m3-03/06/09/12/15`) whose issues asked to "correct `run`" with no
  statement of intended behavior, unsolvable by design, so all three benchmarked
  models scored 0. Then 12 zero-discrimination `Double`/`x*2` one-liners across Go
  and TypeScript (`oss-go-m2-04/07`, `oss-go-m3-01/07/10/13`, `oss-ty-m2-05/08`,
  `oss-ty-m3-02/08/11/14`) that every model solved. Two easy anchors per language
  are kept; `oss-py-m2-00` and `oss-py-m3-00` were re-specified as honest anchors.

## [0.7.0] - 2026-07-24

### Added

- **Qwen provider** (`qwen:<model>`): Alibaba Cloud DashScope OpenAI-compatible Chat
  Completions API. Needs `DASHSCOPE_API_KEY`. Default base URL is the international
  endpoint (`https://dashscope-intl.aliyuncs.com/compatible-mode/v1`); override with
  `DASHSCOPE_BASE_URL` for China (`https://dashscope.aliyuncs.com/compatible-mode/v1`)
  or another region. Reasoning effort is recorded as metadata only (DashScope's
  `enable_thinking` path is not used yet, it requires streaming on some models).
  Built-in pricing covers `qwen3.7-plus`, `qwen3.7-max`, `qwen3.6-flash`, `qwen3-max`,
  `qwen3.5-plus`, and `qwen-plus` (international list rates for the default context
  tier).

## [0.5.1] - 2026-06-23

### Fixed

- **Rust tasks in Docker**: auto-select `vulcanbench/sandbox:rust` for Rust tasks;
  writable `CARGO_HOME` in non-root sandboxes; `make sandbox-image-rust` builds the
  Rust toolchain image

## [0.5.0] - 2026-06-22

### Added

- **`validate_tasks --sandbox docker`**: run gold-patch verification inside the
  Docker sandbox (same environment as `vulcanbench run`); `make validate-tasks-docker`

## [0.4.0] - 2026-06-22

### Added

- **Bundled cost priors**: `vulcanbench estimate` and `run --dry-run` use shipped
  benchmark cost data when local `./runs` history is missing (cold-start installs)
- **`--no-priors`**: disable bundled priors and fall back to legacy defaults only
- **`scripts/export_cost_priors.py`**: regenerate `harness/data/cost_priors.json`
  from local reference runs

## [0.3.0] - 2026-06-22

### Added

- **`vulcanbench estimate`**: pre-run USD cost ranges per provider/model from local
  `./runs` history, with recommended credit to load before a benchmark
- **`run --dry-run`** now prints a cost estimate for priced models
- **`v1-compare` suite**: 12-task trimmed head-to-head set (Go / Python / TS / Rust)

### Fixed

- **Docker sandbox Go verification**: set writable `HOME` / `GOCACHE` so `go test`
  scoring works for non-root containers (fixes false `functional=0.0` on Go tasks)
- **GPT-5 Chat Completions**: omit `temperature` for GPT-5 / o-series models that
  only accept the API default

## [0.2.0] - 2026-06-21

### Added

- **Z.ai provider** (`zai:<model>`): first-class support for GLM models via
  `ZAI_API_KEY` and OpenAI-compatible Chat Completions (`glm-5.2`, etc.)
- Built-in token pricing for `zai:glm-5.2`, `zai:glm-5.1`, `zai:glm-5`, and
  `zai:glm-5-turbo`

## [0.1.0] - 2026-06-19

### Added

- **v1 MVP harness**: `vulcanbench run` with mock, OpenAI, and Anthropic providers
- **Docker sandbox** (default): isolated, non-root, network-off command execution
- **52-task benchmark suite** across Python, Go, TypeScript, and Rust with gold-patch validation
- **Five-metric scoring**: functional, quality, security, efficiency, human_like (3-judge ensemble)
- **Suite tooling**: `--repeat`, `--max-concurrency`, `--max-cost`, `--fail-under`, effort sweeps
- **Artifacts**: JSONL trace, `summary.json`, `final.patch`, self-contained `replay.html`
- **Leaderboard, report, and calibration** CLI commands
- **FastAPI backend** with filesystem or Postgres storage
- **Next.js dashboard**: leaderboard, tasks, run viewer, submission flow
- **Optional DB write-through** from CLI when `VULCANBENCH_API_BASE` is configured
- **Alembic migrations** for database schema evolution
- **Production Docker stack** (`docker-compose.prod.yml`, backend/dashboard Dockerfiles)
- **Documentation**: METRICS.md, REPRODUCIBILITY.md, DEPLOYMENT.md
- **CI**: lint, typecheck, ≥80% harness coverage, task validation, dashboard build, sandbox image build
- **Release workflows** for GitHub Releases and optional PyPI publish

### Notes

- Install from source: `pip install -e ".[dev,test]"` (task corpus and sandbox Dockerfiles ship in the repo clone, not the PyPI wheel)
- Hosted deployment: see [docs/DEPLOYMENT.md](docs/DEPLOYMENT.md)

[0.3.0]: https://github.com/morganlinton/VulcanBench/releases/tag/v0.3.0
[0.2.0]: https://github.com/morganlinton/VulcanBench/releases/tag/v0.2.0
[0.1.0]: https://github.com/morganlinton/VulcanBench/releases/tag/v0.1.0
