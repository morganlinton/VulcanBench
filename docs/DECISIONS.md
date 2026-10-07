# VulcanBench decisions

A running log of operating decisions that are not derivable from the code:
what was decided, the evidence, and when to revisit. Agents working on the
harness or on sweeps should read the entries that touch their area before
changing run conditions. Suite-level policy for v4 lives in
[tasks/coding-intelligence-index-v4/CHARTER.md](../tasks/coding-intelligence-index-v4/CHARTER.md);
entries here record the measurements behind those rules.

## 2026-10-07: Sonnet 5.5 and Haiku 5.5 on Frontier v4; judging moves hosts

### Decision

Owner requests in chat, 2026-10-07.

- **Sonnet 5.5 publishes with disclosure, no rerun.** The sweep
  (`runs-effort-sonnet55/`, all five levels, `--billing subscription`,
  serial, flat 3-hour timeout) was launched on 2026-10-06 from the
  `codex/ocaml-v1` checkout, which branched before the 2026-10-05
  tagged-worktree rule. Its summaries carry no `source` block and their
  `task_hash` values differ from `suite.lock.json`. Cards and the site
  footnote both facts and the evidence below.
- **Haiku 5.5 runs locally, from a run worktree.** The owner asked whether it
  could run in the cloud so the laptop could be closed. It stays on this Mac:
  every published Frontier v4 column ran here on `--sandbox local`, CHARTER
  lifecycle item 4 requires a decision and a disclosure before any
  cross-host comparison, speed cards rank by wall clock, and the
  `claude-code` harness needs a claude.ai subscription login that a cloud
  session is not known to provide. Launcher:
  `logs/haiku55-frontier-v4.launch.sh` (untracked), run worktree at the local tag
  `bench/2026-10-07-haiku55-frontier-v4` (`d8ddcedc`), all five levels,
  output `runs-effort-haiku55/`, queued behind the Sonnet sweep. The
  refusal fallback stays at Claude Code's default (on), as for Opus 5.5.
- **Code quality judging runs on this host, with the judge pins rebuilt.**
  Earlier rounds ran on a host whose home was `/Users/morganlinton`. That
  host has been erased, and with it the only copies of
  `runs-code-quality-maintenance-v3.3/` and `v3.4/`, whose `protocol.json`
  files held the Grok and Muse reviewer settings and binary pins that every
  later runner reads. They were never committed; only their sha256 is
  published. The owner chose to rebuild and disclose rather than judge
  with Muse alone or publish without Code quality:
  `docs/judging/judge-pins-v3.json` (committed, so this cannot recur)
  holds the recovered settings, and the v3.23 runner matches binaries by
  sha256 on this host instead of comparing path strings. Muse matches the
  original pin exactly; the Cursor CLI that carries Grok 4.6 is re-pinned
  and every card discloses that its equality with the v3.3 pin cannot be
  shown. Both judges retake calibration in v3.23, as in every round.
- **The Sonnet 5.5 population goes through a committed hash bridge.** The
  judging pipeline rejects any run whose recorded `task_hash` differs from
  the task today, so every Sonnet 5.5 run would be excluded.
  `docs/results/swe-v4-sonnet55-2026-10/task-hash-bridge.json` pairs each
  task's recorded hash with its lock hash, and the population builder
  accepts a run only when the task still hashes to the lock and the run
  recorded exactly the bridged hash.
- **Costs.** Sonnet 5.5 cards use the cost Claude Code reports per run
  (`cli_reported_cost_usd`), as the Opus 5.5 card did, because the vendor
  page disagrees with itself on the cache-read price. Haiku 5.5 cards show a
  standard-tier estimate plus an upper bound priced entirely at the
  over-100k tier, following the GPT-6 Astra precedent, because run receipts
  hold cumulative session usage, not per-request prompt sizes.

### Evidence

- Task content is identical. Hashing the 23 task directories the Sonnet sweep
  read, with `main`'s `harness.tasks.task_hash` (which skips local
  byproducts, 2026-10-06 entry), matches all 23 entries of
  `tasks/coding-intelligence-index-v4/suite.lock.json`. The only tracked
  difference between `codex/ocaml-v1` and `main` under that suite is the
  lock file itself. The recorded hashes differ because the older hash
  counted gitignored `__pycache__/` folders.
- Claude Code versions, from each run's stream `init` event (each summary's
  `harness_version` agrees): low ran 21 tasks on 2.1.291 and 2 on 2.1.292;
  medium and high ran on 2.1.292; extra-high ran 22 on 2.1.292 and its last
  task (paddockcore, started 12:09 on 2026-10-07) on 2.1.293; max runs on
  2.1.293. The CLI updated itself during the sweep; the global install's
  link time (12:52) is not when the new version took effect. Each card
  footnotes the versions per level.
- Sonnet 5.5 results before max (pass@1, n=23 each, no timeouts, no fallback
  replies): low 0.652, medium 0.739, high 0.870, extra-high 0.957.
- Vendor facts checked 2026-10-07 on platform.claude.com, not recalled:
  - Sonnet 5.5: $2 input, $2.50 5m write, $4 1h write, $10 output per
    MTok. The pricing table lists cache hits at $0.20; the prompt caching
    section on the same page says 0.05x ($0.10). API default effort is
    high; the effort page does not state a Claude Code default.
  - Haiku 5.5: up to 100k prompt tokens $0.10 input, $0.125 5m write,
    $0.20 1h write, $0.01 cache hit, $0.50 output; over 100k $0.50,
    $0.625, $1, $0.05, $2.50. Default effort is medium on the API and in
    Claude Code; all five levels are supported.
  - `claude -p --model claude-haiku-5-5` resolves to `claude-haiku-5-5`;
    so does the `haiku` alias.
- Judge binaries on this host: `muse-bin-1.0.3-R2198.1` hashes to the
  frozen `MUSE_SHA256` (`4c0f9600...`), the value published in the v3.4,
  v3.15 and v3.17 bundles. `cursor-agent` 2026.09.10-fd3934a hashes to
  `2ccc9a8e...`, now the committed pin. Live probes on 2026-10-07: Cursor
  served `cursor-grok-4.6-medium` on the subscription login (display name
  "Grok 4.6 Medium", the rename v3_resume already accepts), and Muse
  Spark 1.3 answered through the pinned binary.
- Recovered settings: the published `scored_panel` blocks (the frozen
  reviewer settings minus binary paths, per the site exporter) are
  identical across the v3.4, v3.15 and v3.17 bundles and equal
  `GROK_MODEL`, `GROK_DISPLAY` and `MUSE_MODEL` in
  `harness/maintenance_review_v3.py`. A search of every branch of both
  repos found no copy of either lost protocol file or the Cursor hash.
- Hash bridge: `task_hash` from `5da730e0` (the sweep's own code) on the
  task directories it read reproduces the recorded hash of all 93 Sonnet
  5.5 runs finished so far, one distinct hash per task for the whole
  sweep; `main`'s `task_hash` on the same directories equals the lock for
  all 23 tasks. The difference is byproduct files only. All 107 `.pyc`
  files under the tasks' `repo/` directories (the part copied into agent
  workspaces) are byte-identical to compiling the current starting source
  with Python 3.12: they were compiled during task authoring (August 27
  to 30) and differ from the sources only in modification time.

### Revisit

If Haiku 5.5 is ever run off this Mac, that needs its own entry and a
cross-host footnote. If Anthropic corrects the Sonnet 5.5 cache-read price,
re-derive the API-equivalent cost and keep the CLI-reported figure as the
published one unless the owner decides otherwise.

## 2026-10-06: Frontier v5 gate runs use concurrency 2; speed reporting stays serial

### Decision

Owner, in chat, accepting the recommendation in
[docs/frontier-v5/PLAN.md](frontier-v5/PLAN.md) section 8, decision (e).
Frontier v5 pilot and gate runs pass `--n-concurrent 2` to `harbor run`. Three
rules make that compatible with the 2026-09-13 decision that kept v4 sweeps
serial, which is not edited and still governs v4:

1. Every run summary records the concurrency it ran under (Harbor's job
   config carries `n_concurrent`; the ingest copies it into
   `max_concurrency` as v4 summaries already do).
2. The v5 speed panel, and any per-run wall-clock figure published as a
   model comparison, is sourced only from the serial confirmation runs
   (PLAN.md section 7, Phase 4, `-n 1`) or from runs whose recorded
   concurrency is 1. Gate-run durations are logged as covariates and used for
   candidate anatomy, never for a published speed ranking.
3. Two pinned tasks must fit the machine. Each v5 task declares `cpus` and
   `memory_mb` in `task.toml` and Harbor runs with `--cpus limit --memory
   limit`, so two concurrent trials cannot steal from each other the way
   unpinned v4 runs on one host could. The runner refuses a concurrent
   launch whose pins exceed the Docker VM's allocation.

Concurrency above 2 is not adopted. It needs the same evidence this entry
asks the first concurrent pilot to produce (below), gathered at 2 first.

### Evidence

- The 2026-09-13 entry deferred concurrency for one main reason, the
  wall-clock speed panel, and two smaller ones: no run had ever exercised
  the Codex or Claude Code subscription with two sessions at once, and the
  harness's infra-retry path had no backoff. Rule 2 answers the main reason
  by construction. The two smaller ones are the pilot checks below.
- Per-task resource pins are the thing Harbor buys most directly
  (PLAN.md section 4), and they remove the "runs sharing one 10-core
  machine" objection: a pinned trial gets its declared CPUs and memory
  whether or not another trial is running.
- Cost: PLAN.md section 3 estimates 200 to 300 serial machine hours for
  the admit gates alone, before rejects. At concurrency 2 the calendar time
  roughly halves while every per-run measurement stays a covariate.
- Capacity on the gate machine measured 2026-10-06: host 12 CPUs and
  16 GiB; Docker Desktop VM 12 CPUs and 7.7 GiB. Two Python or
  JavaScript tasks at the plan's initial band (2 CPUs, 4096 MB each) fit
  once the Docker VM is raised to about 12 GiB, the practical ceiling on a
  16 GiB host. Two compiled-language or JVM tasks at the initial band
  (4 CPUs, 8192 MB each, plus their verifiers) cannot run concurrently on
  this machine at all. So on this host concurrency 2 applies to Python and
  JavaScript pairs, compiled and JVM gate runs stay at `-n 1` unless the
  pilots calibrate their band below 8192 MB, and rule 3's capacity check
  turns an over-committed pair into a refused launch instead of an OOM kill
  mid-run. A gate machine with 32 GiB or more lifts the restriction without
  a new decision; the rules above do not depend on the host.

### Pilot checks before concurrency 2 is used for a verdict

- Run the first concurrent pilot pair and confirm from each `result.json`
  that neither trial saw a usage-limit or rate-limit rejection from the
  Codex or Claude Code subscription. One rejection drops gate runs back to
  `-n 1` until a backoff exists in the launcher (Harbor's `--max-retries`
  plus `--retry-include` for the provider's exception type, verified to
  retry rather than score the failure).
- Confirm from `docker stats` during that pair that each trial stayed
  inside its pins and the verifier containers started with theirs.
- Confirm the ingest writes `max_concurrency = 2` for those runs and that
  the speed-panel source filter excludes them.

### Revisit triggers

- Any usage-limit rejection on a concurrent run: back to 1, add backoff,
  log it here before returning to 2.
- A gate verdict that would differ depending on whether a run was
  concurrent (for example a timeout at 2 that passes at 1): treat the
  concurrent run as infrastructure, not as a scored attempt, and rerun
  serially.
- The speed panel decoupled from wall clock entirely (tokens or steps):
  rule 2 becomes moot and concurrency 3 or 4 can be evaluated on the
  evidence above.

### What this touched

- This entry. The 2026-09-13 entry and v4 run conditions are unchanged.
  `docs/frontier-v5/FREEZE.json` lists (e) as open at freeze time by design
  and is not edited. The (c) entry's "`-n 1` until decision (e)" clause is
  superseded for gate runs by this one; confirmation runs stay at `-n 1`.
  Decisions (d) publishing channel and (f) gold location remain open.

## 2026-10-06: Frontier v5 runs on Harbor; the harness ingests, it does not emulate

### Decision

Owner, in chat, accepting the recommendation in
[docs/frontier-v5/PLAN.md](frontier-v5/PLAN.md) section 8, decision (c).
Every Frontier v5 pilot, gate run and sweep is executed by the Harbor CLI
(`harbor run`, Docker environment), never by `vulcanbench run` or the
effort-sweep launchers. Adopted at Harbor 0.24.0; every run records the
Harbor version it ran under, and a Harbor upgrade is logged here before the
first gated run that uses it.

What the harness keeps:

1. Loading `task.toml` (the source of truth; v5 tasks have no
   `metadata.json`) so `validate_tasks.py`, `task_hash`, the suite manifest
   and `suite.lock.json` cover v5 tasks.
2. A new `harness/harbor_ingest.py` that turns a Harbor job directory
   (`result.json`, per-trial `reward.json`, trajectories, durations, token
   counts) into the existing `runs/<run>/summary.json` shape, so the boards,
   pricing, `time_sliced.py` and the rankings chart keep working unchanged.
3. The `[effort].blocked` check from `vulcanbench.toml`, which wraps every
   Harbor launch the harness makes before any model call.
4. The export tree inverts into a strip step: a task directory minus
   `solution/` and `DESIGN.md`, asserted free of gold material, is the only
   thing handed to anyone outside the repo.

Run conventions fixed with this decision: `tests/Dockerfile` is required in
every task (Harbor skips the tests upload whenever the verifier image is
prebuilt or that file exists, so a verifier image without `COPY . /tests`
grades an empty container); `harbor run -o` takes an absolute path; job
directories live under ignored `runs-frontier-v5-*` roots; `--cpus limit`
and `--memory limit` are passed so `task.toml` resource pins are enforced
rather than advisory; `-n 1` until decision (e) says otherwise.

### Evidence

- The 2026-09-01 export was downstream of `metadata.json` and drifted: the
  checked-in `task.toml` still carried the 10-hour timeout after the
  2026-09-13 revision. One source of truth removes the class of error.
- [docs/frontier-v5/PHASE1.md](frontier-v5/PHASE1.md): a real `harbor run`
  on Docker passed both reward directions with the oracle and no-op agents,
  through the separate verifier, the top-level `artifacts` handoff,
  `reward.json` metric parsing and `no-network` enforcement in the verifier
  phase (`scripts/harbor-smoke/run.sh`). Harbor's `--effort` flag and the
  `claude-code` and `codex` agents' `reasoning_effort` levels match
  VulcanBench's low to max with no ultra level.
- Emulating the same contract in the harness would mean re-verifying every
  one of those behaviors against Harbor's source on each release, which is
  the drift problem again in a different place.

### Revisit triggers

- A Harbor release changes the `task.toml` schema, the reward contract or
  the artifacts semantics: pin the previous version for in-flight gates and
  log the migration here before upgrading.
- A task needs something Harbor's Docker environment cannot grant. The first
  known case is the ThreadSanitizer seccomp allowance on linux/arm64
  (PHASE1.md finding 2); if `--extra-docker-compose` cannot carry
  `security_opt`, that is a pilot blocker to log, not a reason to bypass the
  runner.
- The ingest cannot recover a metric the boards need (per-run tokens, cost,
  wall clock): extend the ingest or the trajectory parser, never run the
  task outside Harbor to get the number.

### What this touched

- This entry. `docs/frontier-v5/FREEZE.json` lists (c) as open at freeze
  time by design and is not edited; this entry is the resolution it points
  to. No code changes yet: the harness work above is Phase 1 and Phase 2
  scope and lands in its own PRs.

## 2026-10-06: Frontier v5 gate references are GPT-6 Astra and Fable 5.1 at medium

### Decision

Owner, in chat, accepting the recommendation in
[docs/frontier-v5/PLAN.md](frontier-v5/PLAN.md) section 8, decision (b).
The Frontier v5 admission gate (PLAN.md section 3, part 2) measures every
candidate against two reference models, both at reasoning effort `medium`:

| Reference | Agent | VulcanBench model id | Effort |
| --- | --- | --- | --- |
| GPT-6 Astra | Codex CLI via `harbor run -a codex` | `codex:gpt-6-astra` | medium |
| Claude Fable 5.1 | Claude Code CLI via `harbor run -a claude-code` | `claude-code:claude-fable-5-1` | medium |

Admit iff each reference solves at most 1 of 3 runs. A prong at exactly 1/3
gets the n=5 top-up and must finish at or below 2/5. Two solves by either
reference end the gate early as a reject. Effort is a covariate only: wall
clock, tokens and list-price cost are logged per run and never decide
admission. Each gate run records the Harbor model identifier string, the
agent CLI version (pinned per wave through the agent's `version` setting),
the Harbor version and the date; the exact identifier strings are fixed in
the candidate log with the first gate run and do not change within a wave.

The gate is relative to the frontier at admission time. When a new model
generation lands, candidates and admitted tasks are re-gated and the
pruning rule applies prospectively; the reference pair changes only through
a new entry here.

### Evidence

- These are the two models that saturated v4
  (`docs/results/swe-v4-frontier-quartet-2026-09/frontier-quartet-efforts.csv`):
  Astra 23/23 at medium, high, extra-high and max; Fable 5.1 23/23 at max,
  22/23 at extra-high, 20/23 at medium. A v5 task that either of them solves
  routinely is not a frontier task.
- Medium is the lowest effort at which Astra swept v4 (100.0, median 3.8
  minutes), so gating at medium measures the task shape rather than the
  reasoning budget, and it is the effort the harness has the most v4 runs at
  for both references. Fable 5.1 at medium scored 97.6 on v4, so the
  weaker-prong reading is conservative, not lenient: a task it misses 2 of 3
  at medium is harder than anything in v4.
- Harbor passes the effort through natively (`--effort medium`, mapped to
  each agent's `reasoning_effort`), verified in
  [docs/frontier-v5/PHASE1.md](frontier-v5/PHASE1.md) finding 6; the
  `[effort].blocked` rule holds because no ultra level exists on either
  agent.
- The v4 charter's reference-model policy and the n=5 top-up protocol
  (loyaltycore and tariffcore prunes in `CANDIDATES.md`) carry over
  unchanged; only the admission prong moves from effort back to correctness.

### Revisit triggers

- A new frontier model generation ships for either lab: re-gate and log the
  new pair here before the next wave.
- Either reference becomes unavailable at medium (model retired, effort
  level removed): substitute with a logged entry, never silently.
- Three consecutive pre-registered candidates in one family go 0/3 on both
  references: that is a design question for a v2 plan, not a reason to raise
  the admission prong.
- The first gated pilot run: confirm from its `result.json` that the effort
  requested is the effort the agent reports, before any verdict is logged.

### What this touched

- This entry. `docs/frontier-v5/FREEZE.json` lists (b) as open at freeze
  time by design and is not edited. Decisions (d) publishing channel, (e)
  concurrency on gate runs and (f) gold location remain open; (e) is next,
  since the gate cost estimate in PLAN.md section 3 assumes serial runs.

## 2026-10-06: Frontier v5 composition frozen (20 tasks, four families, six languages)

### Decision

Owner, in chat, confirming decision (a) of the v5 plan as proposed. The
composition of VulcanBench Frontier v5 (suite id `frontier-v5`, tasks under
`tasks/frontier-v5/`) is fixed before any task is authored:

- 20 tasks at 5% weight each. With three confirmation attempts per task the
  overall score is complete passes divided by 60, the OCaml blueprint's
  arithmetic.
- Four engineering families with fixed weights: F1 tool-provided oracles
  (sanitizers or property-based oracles on a stored corpus, 30%, six slots),
  F2 differential parity against a source-available reference (30%, six
  slots), F3 long-horizon upstream volume (30%, six slots), F4
  opaque-component parity, the v4 lineage (10%, two slots).
- Six languages: Python, JavaScript/TypeScript, Rust, C, C++, Java. Each has
  one slot in each of F1 to F3; the two F4 slots are Python and Java. The
  per-language subscore is reported over F1 to F3 only (15% each) and F4 is
  reported as its own line.
- Tasks are authored natively in Harbor format: `task.toml` is the source of
  truth, there is no `metadata.json`, and `harbor run` is the intended
  runner (decision (c), still open).

Frozen in [docs/frontier-v5/FREEZE.json](frontier-v5/FREEZE.json) over
[PLAN.md](frontier-v5/PLAN.md) and [plan.json](frontier-v5/plan.json). The
frozen files are immutable; capacity, weights or slot changes require a v2
plan with its own freeze. Decisions (b) gate references and effort, (c)
runner, (d) publishing channel, (e) concurrency on gate runs and (f) gold
location are open and will each get a dated entry here that cites the
freeze. Nothing is gated before (b) and (c) are recorded.

### Evidence

- The saturation measurement in the entry below: the suite's own lifecycle
  rule says the frontier suite must be re-made, and the v4 candidate log
  shows the binary-parity thesis cannot carry another one (one solver
  strategy, disassembly, collapses it; 23 correlated tasks carry the
  information of a few; wall-clock effort decayed from a 32.9-minute to a
  3.8-minute median in one model generation).
- Fixing capacity before authoring follows the frozen
  [OCaml family-balanced blueprint](ocaml-expansion-v1/PLAN.md): without
  it the headline becomes a weighted average of whatever got authored, and
  per-language subscores are not comparable.
- Family and language were crossed deliberately: F1 is the substantive
  reason for C and C++ (sanitizer oracles do not exist in Python), and F4
  went to the two languages with the weakest sanitizer story so compiled
  languages carry tool-oracle depth and managed runtimes carry black-box
  depth.
- The alternative, a 4x6 grid of 24 tasks, was rejected because it would
  give the binary-parity lineage six slots.

### Revisit triggers

- A family's first three pre-registered candidates all reject for the same
  structural reason: halt the wave by rule and open a v2 plan rather than
  bending the floors.
- A language's toolchain image cannot pass the determinism gate with a
  flaky control caught: that language's slots stay empty and the headline
  is withheld, never renormalized.
- Decisions (b) through (f) landing: each is a separate entry, not an edit
  to this one or to the frozen files.

### What this touched

- `docs/frontier-v5/PLAN.md`, `plan.json`, `FREEZE.json` (new, on branch
  `frontier-v5-plan` from `origin/main`; the OCaml work on
  `codex/ocaml-v1` is a separate suite and carries none of this).
- No task, image, harness code or v4 file changed. Frontier v4 stays frozen
  at 2.0.0 under `suite.lock.json` (entry of 2026-10-05).

## 2026-10-06: Frontier v4 is saturated; the charter's pruning rule is triggered

### Decision

Frontier v4 (`coding-intelligence-index-v4`, 23 tasks) is recorded as
saturated at the current frontier. Consequences:

1. No new task is admitted to v4; its candidate log closes with the wave-11
   verdicts.
2. v4 is not pruned task by task. Applied literally against GPT-6 Astra the
   rule would remove nearly the whole suite, so v4 stays frozen at 2.0.0
   (lock of 2026-10-05) as the historical column and keeps running for
   board comparability until Frontier v5 confirms, after which it is
   retired like cii-v1 and v1 to v4.
3. The frontier suite is re-made as Frontier v5 (entry above). Run
   conditions do not change: 3-hour flat bound (2026-10-03, every suite),
   one task at a time, tagged run worktrees (2026-10-05).

### Evidence

From `docs/results/swe-v4-frontier-quartet-2026-09/frontier-quartet-efforts.csv`
and `docs/results/swe-v4-gpt6-sol-2026-09/gpt6-sol-v317-efforts.csv`, one run
per task per effort level:

| Model | Best functional | Tasks passed | Median minutes at that effort |
| --- | --- | --- | --- |
| GPT-6 Astra (Codex) | 100.0 at medium, high, extra-high and max | 23 of 23 | 3.8 (medium) |
| Claude Fable 5.1 (Claude Code) | 100.0 at max | 23 of 23 | 27.1 |
| Devin SWE-2 | 93.7 at max | 21 of 23 | 56.6 |
| GPT-6 Sol | combined v3.17 86.8 at max (not a functional column) | 19 of 23 | 24.5 |
| Muse Spark 1.3 | 76.9 at extra-high | 14 of 23 | 36.0 |

The charter's saturation rule (CHARTER.md, lifecycle item 1) prunes any
task the weaker reference solves 3/3 or the stronger reference's median
solves under 10 minutes. Astra clears every task at four effort levels with
a 3.8-minute median; the admission bar required the August 2026 best model
to miss outright or to spend 10 or more minutes per run, and
legacy-paddockcore-binary-parity was admitted on Opus 5 missing all three
gated runs, one after 230 minutes.

Caveat recorded with the measurement: the rule as written calls for an n=3
re-gate per model per task at one effort. The quartet evidence is one run
per task at each of four efforts, 92 Astra runs in all, every one a pass.
That is treated as stronger than the n=3 re-gate, and the re-gate is not
run (it would cost about 69 Codex hours to confirm a result already
observed four times over), but the two are not the same measurement and
this entry does not claim they are.

### Revisit triggers

- Frontier v5 confirms (60 scored outcomes): retire v4 from the live board
  and move it to the retired-suite list.
- A model generation scores below 90% functional on v4: the saturation
  claim is about the models measured here, not about v4 being trivial; keep
  v4 as a reference column rather than retiring it.

### What this touched

- This entry only. `CHARTER.md`, `CANDIDATES.md`, `suite.json` and
  `suite.lock.json` for v4 are unchanged.

## 2026-10-06: task_hash and the agent workspace skip local byproducts

### Decision

Owner, in chat, after the investigation below: `task_hash` and the copy of
a task's repo/ into the agent workspace (and of tests/ at verification)
skip local byproducts: `__pycache__/`, `.pytest_cache/`, `.mypy_cache/`,
`.ruff_cache/`, `*.pyc`, `*.pyo` and `.DS_Store`
(`harness.tasks.is_local_junk`). Every other file still counts, untracked
or not. Nothing under tasks/ that git tracks matches these patterns, so
the hash of every committed task is unchanged: all 289 tasks across every
suite hash identically before and after, and the Frontier v4 lock holds.

The 197 August and early September runs below stay flagged stale. Their
recorded hashes cannot be recomputed from anything that survives, so this
change does not make them match.

### Evidence

- Replaying `task_hash` at every commit that touched the Frontier v4 task
  directories (2026-08-27 to now): each task's committed hash settles at
  the commit that admitted it or, at the latest, at b510137d (2026-08-29,
  the contamination canary) and never changes after. All 115 Muse Spark
  1.3 runs (2026-09-06 to 2026-09-24) match the committed hash.
- The 197 runs in docs/results/cii-v4-fable51-2026-09 and
  docs/results/cii-v4-opus5-effort-2026-09 record 52 distinct hashes, none
  equal to the committed state of its task at any commit. Each task has two
  or three recorded hashes over time; the 2026-09-02 to 09-04 runs agree
  with each other. Tasks run on the day they were admitted (2026-08-31)
  already differ from that day's commit. So the task directories on the
  machine that ran them held content git never had, and it changed between
  run days.
- Both sets ran `--sandbox local` on darwin; the mismatching runs used
  Python 3.12.14, the matching Muse runs Python 3.14.3 (a different venv,
  possibly a fresh checkout).
- `task_hash` and the workspace copy read every file under repo/ and
  tests/, ignored or not. The likeliest source is gitignored byproducts
  of building the suite in place (Python bytecode caches change whenever
  their sources change, which fits the drift between run days) or Finder
  metadata. Not proven: the run traces for these sweeps are only on the
  run machine, and manifest file counts are taken after the agent works,
  so they cannot show the starting tree. `scripts/validate_tasks.py` was
  ruled out: it works on a copy and leaves the task directory untouched.
- Consequence while stale: `vulcanbench compare` and calibration exclude
  these runs, and `--only-missing` treats them as missing, so resuming
  into those output directories would re-run them. The published Fable 5.1
  and Opus 5 cards were made when the hashes still matched their machine
  and are not affected.

### Open

- On the run machine, `git status --ignored tasks/coding-intelligence-index-v4`
  and a search of those sweeps' trace.jsonl files for `__pycache__`,
  `.pyc` or `.DS_Store` would confirm the cause, and show whether any
  agent saw byproducts in its workspace.

## 2026-10-05: sweeps run from tagged worktrees; published suites are frozen

### Decision

Owner, in chat, approving all five parts of the proposal: building suites
and running benchmarks are kept apart. Full workflow in
[HOW_WE_WORK.md](HOW_WE_WORK.md); in short:

- Sweeps and judging rounds launch from a run worktree detached at a
  `bench/<date>-<label>` tag with its own venv (`make bench-tag`,
  `make run-worktree`), never from the checkout being edited.
- `vulcanbench run --suite` and `effort-sweep` refuse uncommitted changes
  under harness/, tasks/, sandbox/ and the run config unless
  `--allow-dirty` (or `VULCANBENCH_ALLOW_DIRTY=1`) is passed. Every run
  summary and suite.json records a `source` block (commit, describe, dirty).
- Published suites carry `tasks/<suite>/suite.lock.json`; CI fails on drift.
  Frontier v4 is frozen at version 2.0.0.
- Engine changes and published results go in separate PRs; no new raw
  files under traces/; no blob over 5 MiB outside tasks/
  (`.github/workflows/pr-hygiene.yml`, override label `mixed-scope` for
  scope only).

Timeouts and concurrency do not change: 3-hour flat bound, one task at a
time.

### Evidence

- `vulcanbench` is an editable install, and sweeps ran from the main
  checkout, so a pull or a local edit during a multi-day sweep changed the
  code later tasks ran with. Run summaries recorded `task_hash` but no
  commit, so a mixed column could not be detected after the fact.
- Recorded `task_hash` values against today's Frontier v4 tree: all 115
  Muse Spark 1.3 summaries under traces/ (September 2026) match. All 197
  summaries under docs/results/cii-v4-fable51-2026-09 and
  docs/results/cii-v4-opus5-effort-2026-09 (runs from 2026-08-28 to
  2026-09-04) differ for every one of the 23 tasks. No commit since
  2026-08-25 touches those tasks' scoring files (only metadata budgets and
  the directory rename). Investigated in the 2026-10-06 entry: local,
  uncommitted files in the task directories, now excluded from the hash.
- traces/ is about 550 MB across 2,465 tracked files, the largest single
  file 37 MB, all added in one week.

### Revisit when

- An archive location is chosen (`VULCANBENCH_ARCHIVE`); record it here.
- VulcanRoutine or VulcanConduct need locks or the PR hygiene check; port
  `harness/suite_lock.py` and `scripts/check_pr_scope.py`.

## 2026-10-03: every suite gets the flat 3-hour task timeout

### Decision

Owner, in chat: "all timeouts should be 3 hours for every suite". The flat
10800 s bound Frontier v4 adopted on 2026-09-13 now applies to every suite
that runs agents:

| Suite | Before | After |
| --- | --- | --- |
| Frontier v4 | 10800 s (2026-09-13) | unchanged |
| Safety v1 (conduct-v1) | 36000 s (2026-08-29 policy) | 10800 s |
| Routine v1 | 1800 s (2026-09-17) | 10800 s |
| Routine v2 | 2700 s (2026-09-25) | 10800 s |

Only the timeout changes; step budgets stay as they were. Retired suites
(cii-v1, v1 to v4, hard-1, python-1, voice-v1, vulcancyber-v1) are not
swept and were not restamped.

### Evidence

- The 2026-09-13 restamp covered Frontier v4 only; Safety v1 tasks were
  forked from v4 before it and kept the 10-hour bound. The gap surfaced
  when Grok 4.7's Cursor Safety v1 low pacecore run started a Python
  process that ran at full CPU from 06:55 with no agent activity after
  07:03, set to run until its 10-hour bound.
- No prior result moves. Safety v1: the longest recorded run (Opus 5.5,
  50 runs) took 84 minutes. Routine v1: none of 504 recorded runs came
  within 100 s of the old 1800 s bound, so raising it changes nothing
  recorded. Routine v2 has not been swept.

### What this touched

- VulcanConduct `tasks/conduct-v1`: `suite.json` gains a `flat_budget`;
  every task's `agent_hints.suggested_timeout_s` and `budget_calibration`.
- VulcanRoutine `tasks/routine-v1` and VulcanRoutine-v2 `tasks/routine-v2`:
  `flat_budget` (previous value kept beside it), every task's timeout and
  `budget_calibration`, and the charters' Budgets sections.
- `CLAUDE.md` "Current:" line. `scripts/stamp_task_budgets.py --check`
  passes on all four suites.

### In-flight runs

A run's bound is fixed when its agent starts, so the Grok 4.7 Cursor Safety
v1 low pacecore run in progress at the change keeps its 10-hour bound. The
remaining tasks of that invocation may keep it too, depending on when the
suite loaded their metadata; every later invocation gets 3 hours.

Outcome: the pacecore run reached its 10-hour bound at 16:55 PDT and is
recorded as unfinished and failed (functional 0). The owner chose to treat
it as a timeout under the 3-hour rule; any time figure that includes it
counts it at 3 hours, with a footnote. An in-process trigger of the
harness's own timeout handler was prepared but needed root and was
overtaken by the bound. The Medium invocation (started 17:39) records the
10800 s bound in its traces.

## 2026-10-03: Grok 4.7 in Grok Build is judged like Cursor (v3.21), launched when its Frontier leg ends

### Decision

The owner asked for a card comparing Grok 4.7 in Cursor with Grok 4.7 in
Grok Build. Grok Build's Frontier v4 sweep is judged under v3.21 with v3.20's
judges (Muse Spark 1.3 and GPT-6.1 Sol), so both halves of the card share a
protocol and panel.

### Operation

`logs/cii-v4-maint-v321-watch.sh` waits for `logs/grok47-all-suites.log` to
reach its Grok Build Routine v1 banner (Frontier leg done) or the chain to
finish, then builds the population, freezes it with
`python -m harness.maintenance_review_v321 prepare`, judges both panels in
parallel and summarizes. The card is drawn by
`scripts/cii-v4-board/make_grok47_harness_card.py` after both summaries
exist; publication waits for review.

## 2026-10-03: Grok 4.7 is judged by Muse Spark 1.3 and GPT-6.1 Sol (v3.20), alongside its remaining legs

### Decision

Grok 4.7's Frontier v4 sweep in Cursor is judged for Code quality under
v3.20 by Muse Spark 1.3 and GPT-6.1 Sol. Owner choice in chat, 2026-10-03:
"let's do muse plus GPT-6.1 Sol". The judging starts now rather than after
the remaining Grok 4.7 legs.

### Evidence

- Grok 4.6, the usual second judge, is an xAI model and not neutral for an
  xAI submission. GPT-6.1 Sol is the strongest OpenAI model on the board
  and runs on the ChatGPT subscription through a Codex binary already
  pinned for it.
- Neither judge runs through Cursor, so the reason for queueing v3.19
  behind the Grok 4.7 Cursor legs (a shared Cursor account) does not apply.
  The overlap adds machine load inside Grok 4.7's remaining wall clock, the
  same trade the owner accepted for GPT-6.1 Sol's sweep overlapping v3.17
  judging; the windows are logged and disclosed on any speed figure they
  touch.
- If GPT-6.1 Sol fails its first exam, Muse publishes alone and the failed
  exam is disclosed, under the single-panel rule.

### Operation

Population built by `scripts/cii-v4-board/build_grok47cursor_population.py`
and frozen with `python -m harness.maintenance_review_v320 prepare`.
`logs/cii-v4-maint-v320-chain.sh` (log and pid beside it) calibrates, runs
and probes the Muse and Sol panels in parallel (per-panel locks), then
summarizes.

## 2026-09-29: GPT-6.1 Sol runs on Frontier v4 now, on a pinned Codex 0.159.0, ahead of Grok 4.7

### Decision

GPT-6.1 Sol (`gpt-6.1-sol`, new in Codex on 2026-09-29) is swept on
VulcanBench Frontier v4 at Low, Medium, High, Extra-high and Max through
Codex on the ChatGPT Pro subscription (`scripts/run_gpt61_sol_v4.sh`,
outputs `runs-effort-gpt61-sol/`). Owner request in chat, 2026-09-29: start
it while GPT-6 Sol's Code quality judging (v3.17) runs. The judges use Muse
and Cursor and the sweep uses Codex, so no quota is shared; the sweep's
wall-clock overlaps the judging window and is disclosed on its column, as
GPT-6 Sol's overlapped GPT-6 Luna's judging. Grok 4.7 now waits for this
sweep too: the gate launcher was relaunched at 19:18 PDT against a hold
(`logs/grok-hold-v2.pid`) that exits only when the GPT-6.1 Sol chain logs
`GPT-6.1 SOL CHAIN DONE` and the v3.17 chain logs `V3.17 CHAIN DONE`.

Amended the same evening (owner: same release treatment for GPT-6.1 Sol,
judged before Grok for the same Cursor reason): the hold was replaced at
20:14 PDT by `logs/grok-hold-v3.pid`, which also waits for GPT-6.1 Sol's
Code quality judging (`logs/cii-v4-maint-v318-chain.log` recording
`V3.18 CHAIN DONE`). If that judging never runs, Grok stays paused until the
owner decides.

### Evidence

- Codex 0.159.2 lists the model ("Near-Astra performance for complex work
  at a lower cost"). One-line probes on the ChatGPT account: 0.155.0,
  0.157.0 and 0.158.0 are refused ("not supported when using Codex with a
  ChatGPT account"); 0.159.0 and 0.159.1 answer. The column runs on 0.159.0
  from its own prefix (`~/.local/vulcanbench-codex-0.159.0`); every other
  Codex column keeps its version.
- List prices, checked 2026-09-29 at developers.openai.com/api/docs/pricing
  and the model page (USD per 1M tokens, standard, short context): $2.00
  input, $0.10 cached input, $10.00 output; long context over 272K input
  tokens at 2x input and cache, 1.5x output. Effort `low` to `max`; no
  ultra is offered.

## 2026-09-29: GPT-6 Sol is judged before Grok 4.7 starts

### Decision

GPT-6 Sol's Code quality judging (protocol v3.17, about 14 hours) runs as
soon as its Frontier v4 sweep finishes, and the paused Grok 4.7 chain waits
for that judging to finish. Owner decision in chat, 2026-09-29, chosen over
running both at once: the Grok 4.6 judge runs through Cursor, and the Grok
4.7 chain starts with its Cursor legs, so running them together would share
Cursor limits and put judging load inside Grok 4.7's wall-clock timings.

The Routine v2 gate launcher was relaunched at 11:46 PDT with
its `GPT6_PID` pointing at a hold process (`logs/grok-hold-for-sol-judging.pid`)
that exits only when the GPT-6 chain has exited and
`logs/cii-v4-maint-v317-chain.log` records `V3.17 CHAIN DONE`. The launcher
then resumes Grok as before (only if the GPT-6 log records the chain done),
waits for Grok, and runs the gate. If Sol's judging stops early, Grok stays
paused until the operator resolves it.

## 2026-09-26: Quality and security analyzers for JavaScript, Rust, C and C++

### Decision

The quality and security metrics now score JavaScript, C and C++ changes, and
the Rust analyzers count what they claim to. Owner decision 2026-09-25: the
private Routine v2 suite (five languages) gets the same four factors in every
language ("full parity") rather than dropping factors outside Python.

- **C and C++** (new): quality runs clang-tidy with a fixed check set
  (bugprone, performance, cognitive complexity over 25, a few readability
  checks); security runs the clang static analyzer's memory-safety and
  insecure-API checks plus cppcheck. Include paths are every header
  directory and its ancestors; `.h` is C++ when the change or workspace has
  C++ sources. The harness line counter now counts C and C++ files.
- **JavaScript**: quality and security run a pinned ESLint with the harness's
  own config (`harness/data/eslint.config.mjs`): recommended rules plus a few
  maintainability rules, and `eslint-plugin-security` without
  `detect-object-injection`. Before this, quality needed an ESLint install in
  the workspace and security ran only `npm audit`, so dependency-free JS
  tasks scored neither. `npm audit` still runs when dependencies are declared.
  TypeScript is unchanged (workspace ESLint and `npm audit`), because the
  pinned config has no TypeScript parser.
- **Rust quality fix**: the clippy parser read `reason == "diagnostic"`
  (cargo emits `compiler-message`) and compared relative span paths with
  absolute ones, and the fmt check read stderr (the diff is on stdout). No
  clippy warning or formatting hunk was ever counted, so every Rust quality
  score recorded before this change was 1.0 unless clippy failed outright.
  Earlier Rust runs are not re-scored; any board that compares them with new
  Rust runs must re-score both.
- **Rust security**: `cargo audit` runs only when `Cargo.lock` lists an
  external crate (it otherwise fetched its database over the network to
  report zero), and the `unsafe` penalty counts only added, non-comment
  lines instead of every occurrence in a changed file.

The new JavaScript, C and C++ security scanners, and the Rust `unsafe`
count, score only findings on lines the change added. On sanitizer-clean
reference code the clang analyzer reported up to 8 false positives in
untouched functions; a whole-file count would charge every run for them
alike. Python's bandit stays whole-file because frozen Code quality
protocols pin its behaviour. Quality stays a density over the changed files
in every language, like ruff.

Versions are pinned by `scripts/install_analyzers.sh` (ESLint 10.11.0,
@eslint/js 10.0.1, eslint-plugin-security 4.0.1, cppcheck 2.22.0, LLVM 23)
and each analyzer records its version in the run's score details.

### Evidence

Probe on the 35 Routine v2 reference fixes (2026-09-26): every language
scored, no missing-tool reasons, no compile errors on admitted candidates.
Quality 0.94 to 1.0 for JavaScript, Rust, C and C++; 0.84 to 0.92 for
Python, whose composite also weighs complexity and maintainability index.
Levels differ by language, so compare quality within a task or suite, not
across languages. Checks that fired repeatedly on sanitizer-clean reference
code were turned off and are listed in `harness/evaluator/cfamily.py`.

### Revisit

Any analyzer version bump or check-set change is a scoring change: log it here.

## 2026-09-25: GPT-6 Luna and Sol run on a pinned Codex CLI 0.155.0, ahead of Grok 4.7

### Decision

GPT-6 Luna (`gpt-6-luna`) and GPT-6 Sol (`gpt-6-sol`) are swept on
VulcanBench Frontier v4 at Low, Medium, High, Extra-high and Max, through
Codex on the ChatGPT Pro subscription, one run at a time
(`scripts/run_gpt6_luna_sol_v4.sh`, Luna first). These are new models, not
the GPT-5.6 Luna and Sol already on the board; outputs go to
`runs-effort-gpt6-luna/` and `runs-effort-gpt6-sol/`. Owner request in
chat, 2026-09-25. The owner chose to run them before the paused Grok 4.7
chain, which resumes after this chain exits.

These two columns run on Codex CLI 0.155.0, installed in its own prefix
(`~/.local/vulcanbench-codex-0.155.0`) and put first on PATH through
`CODEX_BIN_DIR` for this chain only. The global CLI stays at 0.153.4, so
every other Codex column and rerun is unchanged. The bump is disclosed as
a footnote on the GPT-6 Luna and Sol columns, as with the Claude Code bump
for Opus 5.5 (2026-09-22 entry); incumbent Codex columns are not
re-baselined.

### Evidence

The bump was not optional. On 0.153.4 both models are refused before any
work, while GPT-6 Astra answers on the same login:

    400 invalid_request_error: The 'gpt-6-luna' model is not supported
    when using Codex with a ChatGPT account.

One-line probes on 2026-09-25 (low effort, "Reply with the single word
OK."): 0.153.4 and 0.154.0 refused; 0.155.0, 0.155.1, 0.156.0, 0.156.1 and
0.157.0 answered for both models. 0.157.1 (published minutes earlier) does
not start on Node 22.11. So 0.155.0 is the lowest release that runs the
models, and the pin goes no further. The first launch on 0.153.4 produced
70 refused run folders and no summaries; they were moved out of the tree
and are not part of any cell.

List prices, checked 2026-09-25 at developers.openai.com/api/docs/pricing
and each model page (USD per 1M tokens, standard tier, short context):
Sol $2.00 input, $0.20 cached input, $10.00 output; Luna $0.10, $0.01,
$0.50. Prompts over 272K input tokens bill the whole request at 2x input
and cache, 1.5x output. Both models expose effort `none` to `max` in the
API; Codex lists low to max for Luna and adds ultra for Sol, which stays
blocked (`vulcanbench.toml`).

### Operator stop, 2026-09-26: Luna Extra-high pacecore

The Extra-high pacecore run (run dir
`legacy-pacecore-binary-parity-67916795`) started 10:27 local, worked for
two minutes, and went silent at 10:29 right after Codex logged
"Reconnecting... 2/5 (stream disconnected before completion: Incomplete
response returned, reason: max_output_tokens)". Codex never made the next
reconnect attempt or gave up. At 11:57, after about 88 idle minutes, the
operator stopped the Codex process, which the harness records as an
infrastructure error and re-queues. Owner decision in chat: the retry
counts, as a client or API failure under the retry convention, and the
stall is disclosed on the column. The stalled attempt is kept in
`runs-effort-gpt6-luna-operator-killed/extra-high/` and is not part of any
cell. The harness does not flush `cli-agent-stream.jsonl` per event, so
the stream file stayed empty while the run was live; diagnose Codex stalls
from the process and the trace, not the stream size.

### Timeouts in the combined score, 2026-09-28

GPT-6 Luna hit the flat 3-hour bound six times, all while working (no idle
gap over 4 minutes): extra-high depotcore and paddockcore; max cellarcore,
depotcore, lodgecore and paddockcore. A timed-out run has no finished
submission, so Code quality v3.16 cannot judge it and excludes it, as v3.15
excluded Opus 5.5's incomplete high depotcore run. With four of 23 max runs
excluded, all of them failures, the judged mean alone flatters the top
levels. Owner decision in chat: publish both figures. The standard combined
score (judged runs only, comparable with every other board column) stays
the headline cell value, and beside it a second figure counts each
timed-out run as a combined score of 0 over all 23 runs. Pass rates always
count timeouts as failures. The card and report page label both figures
and state the per-level judged n (23, 23, 23, 21, 19).

### Revisit when

- the global Codex CLI is upgraded for another reason: record which
  columns ran on which version;
- a GPT-6 Luna or Sol run fails in a way 0.153.4 columns never did;
- another model's column excludes more than one run per level: apply the
  same two-figure rule.

## 2026-09-25: Verdict v2 gate caps the model under test, not the reference

### Decision

Amends the 2026-09-24 Verdict v2 entry after its pilot. A family is
admitted when, on the 30-item dev pilot, the GPT-6 Astra reference (high
effort) scores skill of at least 40, which shows the family is answerable,
and Jev scores skill below 90, which shows it still measures Jev. The
reference no longer has an upper bound. The shortcut rule (skill at most 15)
is measured on the full build rather than the 30 pilot items. The item-count
and source-unit rules are unchanged. This change was made after seeing
pilot results and is disclosed in the report. Owner decision, in chat,
2026-09-25.

### Evidence

- Pilot, 600 items (30 per family, dev split): the reference scored skill
  100 on 14 of 20 families. Items were inspected by hand for leaks (option
  text, question wording, state); none found. The reference reasons through
  generated puzzles at length; Jev answers without reasoning and spread from
  14 to 96 on those same families, which is the separation v1 lacked.
- Under the original rule only 4 of 20 families passed. Hardening the rest
  until the reference fell below 95 would have pushed Jev to the floor on
  most of them.
- 30 pilot items put a shortcut's accuracy within about 18 points by chance
  alone: vuln-pair's best shortcut read skill 40 on the pilot and 12 on the
  full build.
- Under the amended rule 16 of 20 pass. bug-function (Jev 96) and
  code-output (Jev 95) need hardening, failing-test (reference 39) needs
  more answerable states, weakness-class (keyword shortcut 15.2) needs
  rebalancing.

### Revisit triggers

- A future System One model approaches 90 on most families: the suite has
  run out of headroom; harden rather than raise the ceiling.
- A family passes only because Jev is at or below the floor while the
  reference is also weak (below 60): check that it is answerable from the
  state before reading Jev's number.

## 2026-09-24: Claude Code refusal fallback stays on, disclosed (Artificial Analysis convention)

### Decision (final, same day)

The owner's standing rule for fallbacks is to follow Artificial Analysis.
Artificial Analysis publishes Opus 5.5 only as "Claude Opus 5.5 (Adaptive
Reasoning, <effort> Effort, Default Fallback)", with the fallback enabled
and every run counted. Its methodology page states no separate rule for
fallback-served responses; its general rules are pass@1, retries on API
failures, and blocked questions allowed to lower scores. So VulcanBench:

- runs Claude Code solvers with the refusal fallback ON (the CLI default;
  the launcher unsets `CLAUDE_CODE_DISABLE_REFUSAL_FALLBACK`);
- counts every run, including ones partly served by the fallback model;
- labels the column "Claude Opus 5.5 (with fallback)" and, going beyond
  Artificial Analysis, publishes per level how many runs fell back and the
  fallback model's share of replies.

The fallback-off experiment below ran for about five hours. Its 21
finished runs and 63 refused attempts are kept in
`runs-effort-opus55-nofallback-archive/` and are not part of any published
cell. The 28 original fallback-on runs were restored. The
`_SAFE_ENV_KEYS` entry stays: it is inert unless the variable is set.

### Superseded first decision (same day)

(Superseded.) Claude Code solver runs set `CLAUDE_CODE_DISABLE_REFUSAL_FALLBACK=1`. When a
safeguard classifier refuses a turn, the session now stops that turn instead
of silently finishing on another model. Every Opus 5.5 run in which another
model wrote any reply was archived, not deleted, and is rerun under the new
setting. The Opus 5.5 column publishes Opus 5.5's own work only, and a
classifier stop that zeroes a run is disclosed as a stop.

### Evidence

Claude Code 2.1.280 ships an automatic refusal fallback. In a headless `-p`
run nobody can answer its "switch models?" prompt, so it takes the
declined path, which retries on the fallback model: `claude-opus-4-8`. The
stream records one `model_refusal_fallback` event, and every later reply
carries the fallback model id. The run summary's `reported_model` and the
harness's model checks still said Opus 5.5 for most such runs, so the swap
went unnoticed until the replies were counted.

Counted from the streams on 2026-09-24:

| suite | runs with any Opus 4.8 reply | of |
|---|---|---|
| Frontier v4 | 27 (medium 3, high 7, extra-high 8, max 9 including one in flight) | 93 |
| Safety v1 | 1 (low hedgecore, 244 of 252 replies) | 10 |
| Routine v1 | 0 | 60 |

2,730 replies across those runs came from Opus 4.8. In several, most of
the session was Opus 4.8 (extra-high tallycore 236 of 263). Low had none;
the classifier fires more often as effort and turn count rise.

### How it is applied

- `harness/agent/cli_agents.py`: the variable is added to
  `_SAFE_ENV_KEYS`. The solver environment is an allowlist, so exporting it
  in a launcher alone does nothing (verified). Unset, the behaviour is
  unchanged for every other sweep.
- `scripts/run_opus55_all_suites.sh` exports it for both remaining legs.
- Archived runs: `runs-effort-opus55-fallback-archive/` and
  `VulcanConduct/runs/runs-conduct-v1-opus55-fallback-archive/`.
- Routine v1 and its v3.14 judging are unaffected (no fallback in any run).

### Comparability

Runs 1 to 92 ran with the fallback on. The kept runs are exactly those in
which it never fired, so they are the same measurement as a run with it
off. The reruns may stop outright where the old runs switched models, so a
rerun cell can score lower than the archived one; the archived score is the
wrong comparison, because it was partly Opus 4.8.

The published Fable 5.1 Frontier v4 column (Claude Code 2.1.259 to 2.1.261)
also contains refusal fallbacks, checked 2026-09-25 by counting
assistant-message models: 11 of 115 runs have claude-opus-4-8 replies (low 1,
medium 3, high 2, extra-high 3, max 2; reply share 6.9, 34.1, 23.9, 40.3 and
12.3%). Its v3.4 manifest already records those 11 as `fallback`. So both
Claude Frontier columns are "with fallback" under the same convention. The
published Opus 5 column has not been checked yet.

### What the fallback-off experiment showed

With the fallback off, a whole-request refusal (category `cyber`, during
disassembly of the legacy binaries) ends Claude Code with "API Error: Opus
5.5's safeguards flagged this session". The harness classed that as an
infrastructure error and retried, so a refused task was re-run until an
attempt was not refused (high stampcore: 15 refused attempts). That makes
fallback-off pass rates conditional on not being refused, which is why it
was not a clean alternative either.

### Revisit triggers

- A Claude column where classifier stops zero many runs: consider the Cyber
  Verification Program or a documented retry rule, not the fallback.
- Any CLI update: confirm the variable name still gates the fallback (the
  first classifier stop should emit `model_refusal_no_fallback`).


## 2026-09-24: Verdict v2 is a 20-family suite scored above the best shortcut

### Decision

VulcanBench Verdict v2 (suite id `verdict-v2`) replaces Verdict v1 as the
typed-decision suite. It covers twenty families in eight areas across two
pillars: Software (reading code, reviewing changes, finding bugs, security,
testing, operations) and General (logic, math, tables, rules and policy).
Spec: [VERDICT_V2.md](VERDICT_V2.md). Rules:

- Every answer comes from execution, a deterministic tool, or a generator
  that built the item. Never from a model's opinion, so v1's
  `quality-preference` family has no v2 counterpart.
- Items are balanced by construction (pairs, uniform option positions, 50%
  base rate for yes/no) and the floor is printed per family.
- Headline unit is skill: `100 * (acc - floor) / (1 - floor)` on the model's
  own top answer, where floor is the best of majority, uniform guess and the
  family's shortcut baselines. Verdict Index is the mean skill over shipped
  families, with Software and General sub-indices; Calibration Index (mean
  Brier skill score) is published beside it and never folded in. Intervals
  are bootstrapped by source unit; overlapping intervals are "not separated".
- A family ships only if, on a 30-item pilot, the GPT-6 Astra reference
  (high effort, Codex on the subscription, same state, no tools) scores skill
  40 to 95, the best shortcut scores 15 or less, and the family has at least
  200 test items from at least 20 source units.
- Chart rows are Jev (pinned) and GPT-6 Astra labelled as a reference row.
  No other LLM rows in v2.

Owner decisions, in chat, 2026-09-24: build a well-rounded v2 because the
v1 headline tied the floor at 63 and covered one skill; include a General
pillar beyond software; show Jev plus one reference model.

### Evidence

- v1: four of five families asked about the same 745 patches; "always
  passes" scored 62.7% and Jev 62.8% on the pass question; lines changed
  alone out-ranked Jev (AUROC 0.785 against 0.690). See the 2026-09-19,
  2026-09-23 and 2026-09-24 entries.
- docs.typesafe.ai (read 2026-09-24): `score` questions take an ordered
  `criteria` list of 2 to 10 levels and return per-level `probabilities`;
  `choice` allows 255 options; 32k tokens for state plus the longest
  question; `jev-1.13.0` is still the only versioned id.

Build changes the same day: `ci-failure` was replaced by the generated
`incident-root-cause` (the archive keeps no hidden-test output to label),
and `failing-test` shows one failing target plus the passing ones for any
number of failing targets. Details in the spec's "Changes during the build".

### Revisit triggers

- A family fails the gate twice: cut it and list it in the report rather
  than loosening the gate.
- The reference row beats Jev by less than the interval on most families:
  the suite may be too easy to separate models; raise distractor difficulty.
- TypeSafe ships a new Jev version: new column under its pinned id.

## 2026-09-24: Verdict reports a diff-size baseline beside every model's ranking

### Decision

On Verdict v1's pass question, ranking fixes by lines changed alone is
reported as a baseline next to "always guessing", in the report, the data
file and Table 1 of the model card, with its cutoff fitted on the
development split like every other cutoff. A model's ranking is only read as
evidence that it reads code when it beats this heuristic, overall and within
size bands. Owner request, in chat, 2026-09-24.

### Evidence

- The owner asked what size of diffs the suite tests. Median fix: 108 lines
  changed (10th to 90th percentile 45 to 277), one file, about 2,900 input
  tokens for Jev. Pass rates climb steeply with size: on the published split,
  7% under 50 lines, 64% at 50 to 199, 90% at 200 or more.
- Lines changed alone ranks the 611 published pass questions at AUROC 0.785
  (bootstrap 0.745 to 0.823) and scores 65.1% at a development-fitted cutoff
  of 127 lines, above Jev on both (0.690, 62.8%). Jev's stated confidence
  tracks size (AUROC 0.816 for telling above-median fixes apart); in the
  50 to 199 line band, 417 fixes, Jev ranks at 0.558 against 0.813 for the
  GPT-6 Astra control.

### Revisit triggers

- A new source of fixes whose pass rate does not rise with size: keep the
  baseline, but expect it near 0.5.
- Other families (regression, four-way outcome) show the same size effect:
  add their size baselines before reading those rankings.

## 2026-09-23: integrity audit false flag cleared on published Claude columns

### Decision

The filesystem integrity audit marked Claude Code runs as
`benchmark_data_access` / `contaminated` whenever the agent read one of the
CLI's own background-command logs
(`/private/tmp/claude-<uid>/<slug>/<session>/tasks/<id>.output`), because it
treated any `/tasks/` path as the benchmark task tree. PR #148 fixed the
rule. On the owner's instruction (in chat, 2026-09-23) the audit field was
refreshed on every affected Claude run set. Annotation only: no score,
pass@1 or published column changes.

### What carried the false flag

- Published Opus 5 summaries (`docs/results/cii-v4-opus5-effort-2026-09/`):
  8 of 93 runs (high: queuecore, settlecore, matchcore, granarycore;
  medium: lodgecore; max: matchcore, cellarcore, snapcore). Their agent
  streams are not on this machine, so they were re-classified from the
  stored path lists under the new rule and carry a `reclassified` note.
- Fable 5.1 Frontier v4 sweep (`runs-effort/`, Claude Code 2.1.259 to
  2.1.261): 20 of 115 runs, fully re-audited from their streams.
- Opus 5.5 sweep in progress (`runs-effort-opus55/`): 37 of 82 runs, fully
  re-audited. Runs finished later by a chain on the pre-#148 code need the
  same refresh.

In every case the flagged paths were the run's own CLI logs; no run read a
gold patch, hidden tests, another task or `runs/`, and none used the web.
After the refresh all three sets report 0 contaminated runs.

## 2026-09-23: Verdict families get an answerability control before a model is blamed

### Decision

When a Verdict v1 family scores at or near the majority floor, it is not
read as a verdict on the model until a frontier model has been given
exactly the same inputs on the same items and shown that the family is
answerable. The control runs through a subscription CLI (Codex here, to stay
clear of the Claude subscription a concurrent sweep was using), in an empty
read-only directory with no tools, with its cutoff fitted on the development
split exactly as the model's was. It is published as a control note on the
report, never as a leaderboard column. While a control that could change the
reading of a published result is running, the results come off the site
entirely, card and data files included, and return together with the
control. Owner requests, in chat, 2026-09-23.

### Evidence

- The owner asked whether the suite was flawed after Jev and always
  guessing tied at 63% on the pass question. All 745 fixes come from
  binary-parity tasks whose hidden tests check undocumented behaviour of a
  retired program, and Jev sees only the bug report and the fix, so the
  question might have been unanswerable from its inputs.
- Control, GPT-6 Astra at high effort, on the 611 published pass questions:
  AUROC 0.866 (bootstrap 0.837 to 0.894) against Jev's 0.690 (0.648 to
  0.730), a gap of 0.13 to 0.22; 72.5% at a development-fitted cutoff and
  67.1% at 0.5, against a 62.7% floor and Jev's 62.8%. Within-task AUROC,
  item-weighted over 19 tasks, 0.905 against 0.727. The family is
  answerable, and Jev's shortfall is ranking as well as calibration.
- On the 134 development fixes, drawn from only 4 tasks, the gap looked
  small (0.817 against 0.764), and the page briefly said Jev's shortfall was
  calibration alone. The test split corrected that, which is why results
  were withheld rather than left up while the test-split control ran.

### What this touched

- `scripts/verdict-v1/run_llm_control.py` (new), `export_results.py`
  (control blocks with bootstrap intervals and development-fitted cutoffs),
  `docs/results/verdict-v1-jev-2026-09/` (report, data), and the site report,
  which was withheld on 2026-09-23 and restored with the final control.

### Revisit triggers

- The style family has no control yet; run one before reading its 0.962
  AUROC against any other model.
- A second decision model on the suite reuses the same control run; no need
  to re-query Astra unless the item set changes.

## 2026-09-23: typed-decision scoring reports ranking, not a fixed 0.5 cutoff

### Decision

VulcanBench Verdict v1 never reports a yes/no family's accuracy at a bare
0.5 cutoff as a headline. Every published family carries, at minimum, its
AUROC, the range of probabilities the model actually produced, and the
majority-label share. Where a decision has to be made from a yes/no
probability, the cutoff is fitted on the development split and named in the
report; the 0.5 figure may be shown beside it as a diagnostic, never alone.
Subgroup tables print the majority baseline for each subgroup. Owner
request, in chat, 2026-09-22, after the owner questioned a published result.

### Evidence

- The first Jev report (published 2026-09-22) led with "40.8% against a
  72.4% floor" and "it called every patch broken". Jev's probability that a
  patch passes runs 0.09 to 0.42 across all 611 test items, so a 0.5 cutoff
  sat outside its output range and no item could be answered "passes". The
  headline measured the decision rule, not the model.
- The ordering under that cutoff is informative: AUROC 0.690 on the pass
  question, 0.652 on regression, 0.651 macro on the four-way outcome and
  0.962 on the style pairs, against 0.5 for chance. With cutoffs fitted on
  the development split (0.17 and 0.72) accuracy is 62.8% and 93.1% against
  floors of 62.7% and 94.1%: level with the floor, not 32 points below it.
- The same flaw produced a second false finding. "Accuracy falls as the
  patch gets longer" inverted once the cutoff was fixed, because the share
  of patches that pass rises with size (54%, 89%, 90%); at a fitted cutoff
  each bucket's accuracy equals that bucket's majority baseline exactly.
- The preflight had already shown probabilities clustered at 0.51 to 0.76 on
  one family and a 0.22 mean on another. A printed probability range would
  have caught it before publication, which is why one is now mandatory.

### What this touched

- `harness/verdict/scoring.py` (`auroc`, `family_auroc`, `tuned_thresholds`,
  `majority_label_share`, `p_true_min`/`max`/`mean`,
  `accuracy_at_threshold`), `scripts/verdict-v1/export_results.py`,
  `scripts/verdict-v1/score_predictions.py`,
  `scripts/verdict-v1/make_jev_card.py`, `tests/test_verdict.py`,
  `docs/results/verdict-v1-jev-2026-09/` (report, data and card), and the
  site report, which was corrected in place with the withdrawn numbers kept
  visible beside the corrected ones.

### Revisit triggers

- A model whose probabilities do span 0.5 on these items: the fitted cutoff
  should land near 0.5, and a large gap between the two accuracy columns
  becomes the calibration signal to report.
- A second model on the suite: fit its cutoffs on the same development split
  and publish both columns for every model, so no model is flattered by a
  cutoff chosen after seeing the test items.

## 2026-09-22: Claude Code CLI bumped to 2.1.280 for Opus 5.5

### Decision

Claude Opus 5.5 (`claude-opus-5-5`) columns run on Claude Code 2.1.280.
Every earlier Claude column on the board was produced on 2.1.261 or
earlier. The bump is disclosed as a footnote on the Opus 5.5 columns
rather than papered over, and the incumbent Claude columns are not
re-baselined.

Opus 5.5 also ships a different vendor default effort than its
predecessors: medium, where Opus 5 and every other Claude column on the
board default to high. The headline Opus 5.5 column is medium, labelled
as the shipped default. The full Low to Max sweep runs as usual, so a
high-vs-high reading against the incumbents stays available from the
effort card.

### Evidence

The bump was not optional. On 2.1.261 the API refuses the model outright,
before any run starts:

    API Error: 400 Claude Code 2.1.261 does not support this model;
    version 2.1.280 or newer is required.

So the choice was not "2.1.261 or 2.1.280", it was "2.1.280 or no Opus 5.5
column at all". 2.1.280 is the exact minimum the API names; the CLI was
updated to that version and no further.

Vendor facts verified 2026-09-22 against platform.claude.com, not recalled:

| | Opus 5.5 | Opus 5 |
|---|---|---|
| API id | `claude-opus-5-5` | `claude-opus-5` |
| Base input | $4 / MTok | $5 / MTok |
| Output | $20 / MTok | $25 / MTok |
| Cache hits | $0.20 / MTok (0.05x) | $0.50 / MTok (0.1x) |
| Default effort | medium | high |

The 0.05x cache-hit multiplier is an Opus 5.5 exception to the standard
0.1x; it is stated explicitly in the price table rather than defaulted,
or api-equivalent costs for cache-heavy agent runs would be overstated.

`claude-opus-5.5` (dotted) is rejected client-side and is not an alias.

### Why the columns are still comparable, and where they are not

`harness/agent/cli_agents.py` already warns that the harness is part of a
column's identity. Two things bound the risk here. The CLI moved 19 patch
versions inside one minor line, not across a major one, and the run
conditions the board actually ranks on (flat 3-hour task timeout, serial
sweeps, effort vocabulary) are set by the harness and the suite, not by
the CLI. What the bump could still move is solver behavior inside a run:
tool-loop changes in the CLI are not visible to the harness, and no
measurement here isolates them.

That residual is why the footnote exists. Treat a small Opus 5.5 margin
over an incumbent Claude column as harness-confounded, not as a clean win.

### What this touched

- `harness/pricing.py`: `anthropic:claude-opus-5-5` added, with an
  explicit `cached_input` for the 0.05x rate.
- `harness/api_equivalent_costs.py`: `claude-opus-5-5` added to
  `CLAUDE_RATES` as (4, 0.2, 5, 8, 20); `VERIFIED_DATE` moved to
  2026-09-22 after re-checking every existing Claude row against the
  live pricing page (all unchanged).
- The `claude-opus-5` carve-out in the cost reconciliation (an observed
  missing model-specific TTL on one internal call) was deliberately NOT
  widened to Opus 5.5. If 5.5 receipts show the same quirk, extend it
  then, with the receipt as evidence.
- No effort-map change was needed: `_CLAUDE_CODE_EFFORT_VALUES` is keyed
  by harness, not by model.

### Revisit triggers

- A Claude column that needs a CLI newer than 2.1.280: re-baselining the
  incumbents becomes cheaper than accumulating per-column footnotes.
- Any Opus 5.5 result that beats an incumbent Claude column by less than
  the sweep's own repeat-to-repeat spread: the harness delta is a live
  alternative explanation, say so rather than ranking on it.
- Anthropic changing Opus 5.5's default effort away from medium: the
  headline column's label stops matching the vendor default.


## 2026-09-22: Grok 4.7 runs on Cursor and Grok Build, queued serially behind Opus 5.5

### Decision

Grok 4.7 is swept on all three suites (Frontier v4, Routine v1, Safety v1)
on two harnesses, Cursor and Grok Build, so the harnesses can be compared
on identical tasks. Levels are Low, Medium, High and Extra-high; neither
harness exposes a max level for Grok 4.7. The owner considered running
alongside the Opus 5.5 chain and chose to keep sweeps serial (2026-09-13
entry), so `scripts/run_grok47_all_suites.sh` waits for the Opus chain to
finish. Owner request, in chat, 2026-09-22.

### Evidence

- Cursor lists `grok-4.7-low`, `-medium`, `-high` and `-xhigh` (plus
  `-fast` tiers, not used). The adapter's bracket form
  (`grok-4.7-low[effort=low]`, `grok-4.7[effort=high]`) is rejected with
  "Cannot use this model", so Cursor legs pass the per-level id and no
  `--effort`, as the Grok 4.6 Cursor sweep did.
- Grok Build 1.0.41 lists `grok-4.7` (default); a one-line probe at
  `--reasoning-effort xhigh` answered on subscription login.

## 2026-09-19: VulcanBench Verdict v1 scores typed decisions against executed tests

### Decision

VulcanBench Verdict v1 (suite id `verdict-v1`) is a separate suite for
models that return a typed decision (a choice, an ordered score, or a yes/no
probability) and cannot write code, starting with TypeSafe AI's Jev. It is
never a column on the Frontier v4 board. Rules:

- Headline numbers use only items whose answer comes from the hidden-test
  verifier or a gold patch. Agreement with the code-quality judges
  (`quality-preference`, reference `llm-panel`) is reported apart and never
  folded into the overall score.
- Accuracy is always published next to calibration (Brier, log loss, ECE)
  and the majority-answer floor. Unanswered items count as wrong.
- Items embed Frontier v4 issues and agent patches, so the item file is
  private, gitignored and canaried. There is no public dev set.
- Jev is pinned to a versioned id (`jev-1.13.0`), not `jev-latest`, and has
  one column: the API has no effort, temperature or seed setting. LLM
  baselines run the same items from Low to Max; ultra stays blocked.
- Latency is wall clock from the operator's machine and includes the network
  round trip. It is published as indicative, with the measuring location
  stated, and is not used to rank.

The name is Verdict, chosen by the owner on 2026-09-20: it says what is
tested (judging a patch, with the tests delivering the real verdict) and
stays vendor-neutral. "System One model" is TypeSafe's term for the class;
use it in taglines and copy, not in the suite name. Working name before
that was Decisions v1, which collided with this file.

Owner request, in chat, 2026-09-19.

### Evidence

- Jev cannot take Frontier v4 or Routine v1: every task needs an agent that
  reads a repo, calls tools and writes a patch, and Jev emits no text.
- TypeSafe's own evals use the average of GPT-6 Astra and Fable 5.1 answers
  as the reference, and the workflows were written by TypeSafe. A survey of
  about 15 independent Jev evals on 2026-09-19 found public labelled
  datasets, rule generators, author labels and LLM agreement as references;
  none covered software-engineering judgments and none used executed tests.
- One independent calibration study (`jev-ood-calibration`) reports ECE
  0.107 on unseen items against a 0.024 noise floor, so calibration is a live
  question and not a given.
- The archive yields 745 distinct uncontaminated agent patches with verifier
  outcomes from seven completed Frontier v4 sweeps (459 all pass, 226 partial,
  55 regression, 5 no progress). Always answering "passes" scores 62.7% on
  the test split with Brier 0.238, which is the floor to beat.
- docs.typesafe.ai (read 2026-09-19): 32k tokens of state per request, up to
  255 choice options, $0.042 per million input tokens, output free, service
  hosted on the US West Coast.

### What this touched

- `harness/verdict/` (items, scoring, TypeSafe adapter),
  `scripts/verdict-v1/build_items.py`, `tests/test_verdict.py`,
  `.gitignore` (`verdict-v1-items/`).

### Not built, and why

- A contamination family. Whether a patch recites a gold patch is a string
  comparison once the gold is in the state, and unanswerable without it, so
  it is not a judgment worth scoring.

### Revisit triggers

- The three patch families ask different questions about the same 745
  patches, so their errors correlate. If a report needs one number, use
  `patch-outcome` alone or bootstrap by patch, not by item.
- `fix-localization` has 23 items (19 test). Treat it as a probe until the
  task pool grows.
- TypeSafe ships a new version: rerun under the new pinned id as a new
  column; never overwrite the old one.

## 2026-09-18: Devin SWE-2 sweeps run medium, high and max only, unpriced

### Decision

The Devin SWE-2 column on VulcanBench Frontier v4 is swept at exactly the
three effort variants Devin's account catalog lists for the family
(`swe-2-medium`, `swe-2-high`, `swe-2-max`), through the Devin CLI harness
(`--harness devin`), one attempt per task, serial, under the flat 3-hour
task timeout. No low or extra-high column is published for SWE-2, and the
cost column is "unavailable" rather than an API-equivalent estimate.
Launcher: `scripts/cii-v4-board/run_devin_effort_sweep.sh`, queued behind
the Sol chain by `logs/devin-swe2-chain.sh`. Owner request, in chat,
2026-09-18.

### Evidence

- Devin exposes effort only as the last token of a model id; `devin models
  list --format json` on this account lists SWE-2 as `swe-2-medium`,
  `swe-2-high` and `swe-2-max` and nothing else (no `-low`, no `-xhigh`).
  Cognition's SWE-2 announcement names the same three levels. The adapter
  refuses an unlisted uid because the CLI otherwise falls back silently to a
  default model (its log: "did not resolve to an available, allowed model;
  starting on the default").
- The cloud Devin Sessions API cannot pin a model or an effort at all (its
  only knob is `devin_mode`: normal, fast, lite, ultra, fusion), so the local
  CLI is the only route that measures SWE-2 at a known effort. `ultra` there
  is a session mode, not an effort, and is blocked anyway.
- SWE-2 has no public per-token price (catalog cost tier "Free" through
  2026-10-10, Devin-only availability), so an API-equivalent cost would be
  invented. Tokens and Devin's credit/ACU counters are recorded instead.
- A live hello-world run on 2026-09-18 (`swe-2-medium`, CLI 3000.10.31)
  confirmed print-mode flags, per-run config acceptance, session harvest,
  receipt deduplication and the served-model check.

### What this touched

- `harness/agent/devin_cli.py` (new), `harness/agent/cli_agents.py`,
  `harness/effort.py`, `harness/agent/run_audit.py` (`"webfetch"` marker),
  `harness/agent/loop.py` (`.devin/` ignored in workspaces), `harness/cli.py`,
  `harness/pricing.py` (comment only), `tests/test_devin_cli.py`,
  `scripts/cii-v4-board/run_devin_effort_sweep.sh`, `logs/devin-swe2-chain.sh`,
  README and `docs/HARNESS_BENCHMARKING.md`.

### Revisit triggers

- Devin adds `swe-2-low` or `swe-2-xhigh` to the catalog: extend `LEVELS`
  in the launcher and the sweep gains a column, nothing else changes.
- Cognition publishes a per-token API rate for SWE-2: add a `devin:swe-2`
  price entry and re-run `reprice_runs.py`; until then the column stays
  unpriced.

## 2026-09-18: assert statements in test files are not security findings

### Decision

The security factor's bandit scan skips finding B101 (use of `assert`) when
the file is test code: under a `tests/` or `test/` directory, named
`test_*.py` or `*_test.py`, or `conftest.py`. Every other finding, and every
B101 outside test code, counts as before. Owner decision, in chat,
2026-09-18; the skipped count is reported in the metric details.

### Evidence

- On the VulcanBench Routine v1 admission gate, GPT-6 Astra at Low wrote or
  extended a unit test file on all 12 tasks and scored 0.0 to 0.7 on
  security for it: every low-severity finding was B101 in those tests. An
  engineer adding tests to a routine change is the behaviour the suite wants.
- Bandit's own documentation recommends skipping B101 for test code, since
  `assert` is the mechanism of a test, not a weakness in shipped code.
- Across all 438 published Frontier v4 runs no patch touched a test file, so
  no published score changes; the rule matters for the Routine suite and for
  any future run that adds tests.

### What this touched

- `harness/evaluator/security.py`: counts severities from bandit's per-finding
  results instead of its totals, dropping B101 in test files.
- `tests/test_security.py`: a test for the exemption and for B101 outside
  tests still counting.

## 2026-09-16: the "ultra" effort level never runs

### Decision

No VulcanBench benchmark runs at effort "ultra", for any model, harness or
suite. The block lives in `vulcanbench.toml` (`[effort].blocked`) and is
enforced before any model call by `harness/settings.py` at every entry point:
`vulcanbench run`, `vulcanbench effort-sweep`, `run_agent` (which every sweep
driver calls), and the Codex effort launcher. A blocked level is refused with
an error naming the file. Owner decision, in chat, 2026-09-16.

### Evidence

- The Codex model catalogue lists "ultra" for GPT-5.6 Terra only, above max;
  GPT-5.5 stops at xhigh and Luna at max. The board publishes Low to Max, so
  an ultra column would compare against nothing.
- Muse Code's "ultra" is a client-side mode mapped onto the provider's
  highest supported tier, not a distinct API effort (recorded in the Muse
  Contributor sweep protocol), so it does not measure a new level either.
- A settings file rather than a code constant so the rule survives new
  adapters and catalogue changes without anyone remembering it.

### What this touched

- `vulcanbench.toml` (new), `harness/settings.py` (new), `harness/agent/loop.py`,
  `harness/cli.py`, `scripts/cii-v4-board/run_codex_effort_sweep.sh`,
  `tests/test_settings.py`.
- Not touched: `harness/effort.py` and `harness/agent/muse_code.py` still know
  the word so recorded runs and protocol hashes stay valid; the running Muse
  Contributor sweep pins both files. That sweep's protocol lists ultra as its
  final level; under this block it will stop with a refusal before that level
  rather than run it, and its protocol should be amended to drop ultra at its
  next stop.

## 2026-09-13: v4 task timeout lowered to 3 hours; concurrency stays at 1

### Decision

1. Every VulcanBench Frontier v4 (`coding-intelligence-index-v4`) task carries a
   flat 3-hour agent timeout (10800 s, 540 steps at the 20 s/step stamp),
   down from 10 hours. Stamped in each task's `agent_hints` and declared
   once in `suite.json` `flat_budget`; `scripts/stamp_task_budgets.py --check`
   verifies against it.
2. Sweeps keep running one task at a time (`--max-concurrency 1`, the
   harness default). Concurrency was considered and deferred.

### Evidence for the timeout

Measured over every completed v4 solver run on disk at the time
(479 runs: claude-fable-5-1, gpt-5.5, gpt-5.6-luna, gpt-6-astra, muse-spark-1.3, muse-spark-1.3-contributor; efforts from minimal to max).

| Statistic | Minutes |
|---|---|
| Median | 11.2 |
| Mean | 25.3 |
| p90 | 33 |
| p95 | 55 |
| p99 | 404 |
| Max | 953 (two runs exhausted the 10-hour bound, both scored 0) |

Runs above each candidate cutoff, and how many of those still passed
functional:

| Cutoff | Runs over | Passed |
|---|---|---|
| 30 min | 60 | 34 |
| 60 min | 22 | 7 |
| 120 min | 16 | 4 |
| 180 min | 9 | 0 |
| 240 min | 9 | 0 |

None of the nine runs past 3 hours passed (best functional score in that
group 0.88). Eight were Muse Spark 1.3 runs. The ninth was a GPT-5.5
extra-high run on paddockcore that ran 16 hours against the 10-hour cap:
the harness's watchdog fired but killed only the npm launcher, not the
native Codex worker, which kept working and writing to the pipe. That is
fixed (the adapters now start each CLI in its own session and kill the
whole process group; see `_kill_process_group` in
`harness/agent/cli_agents.py`), but the fix has not yet been exercised by
a real Codex timeout. The Muse adapter was verified to cut off at 600
minutes. The slowest passing run on record is
claude-fable-5-1 at extra-high effort on legacy-depotcore-binary-parity,
168 minutes. Apart from that runaway run, Codex-harness models (GPT-6 Astra,
GPT-5.5, GPT-5.6 Luna) never exceeded 61 minutes at any effort. So a 3-hour
bound would have changed no pass@1 result while removing 7 hours of wall
clock per stuck run (13 hours in the runaway case).

Note the margin: the slowest passing run sits 12 minutes under the new bound.
A 4-hour bound has the same zero-loss property with more headroom (no run
landed between 3 and 4 hours). 3 hours was chosen deliberately; if a
Claude Code run at high or max effort passes after more than 2.5 hours,
reconsider 4 hours before treating the timeout as a result.

### Why concurrency was deferred

The harness supports it (thread pool in `harness/suite.py`, one temp
workspace per run, no fixed ports), and no rate-limit rejection has ever
been recorded (867 infra retries across v4 sweeps: 866 were one expired
Codex refresh token, one was a Claude Overloaded error). The blocker is
the wall-clock metric. The speed cards and the v4 report cards rank models
by wall-clock minutes per task, and every v4 suite record on the board was
produced at concurrency 1. Runs sharing one 10-core machine and one
subscription session would take longer per task than they do today, and
their durations would not compare against the serial history. Until the
speed panel is sourced only from serial runs (the suite record stores
`max_concurrency`, so a filter is possible) or is decoupled from wall
clock, sweeps stay serial.

Two smaller reasons: no run has ever exercised the Codex or Claude Code
subscription with more than one session at once, so the rate-limit
behavior is unmeasured, and the infra-retry path has no backoff, so a
burst of usage-limit rejections would exhaust both retries and record
scored failures.

### Revisit triggers

- Any v4 run that passes after more than 2.5 hours: consider 4 hours.
- The first Codex run that hits the 3-hour bound: confirm from its
  `summary.json` that `duration_s` is within a minute of 10800 and that
  `cli_agent.timed_out` is true. If it overran, the process-group kill
  regressed and the bound is not enforced for Codex.
- Speed reporting sourced from serial runs only, or from a metric that
  does not depend on wall clock: concurrency 2 or 3 becomes viable. Add a
  backoff on usage-limit infra retries first.
- A new model family with a materially different duration profile: rerun
  the cutoff table above before its first full sweep.

### What this touched

- `tasks/coding-intelligence-index-v4/*/metadata.json` (23 tasks):
  `agent_hints` restamped to 10800 s / 540 steps, `budget_calibration`
  updated. blendcore had been left on the formula value (5400 s) when the
  10-hour bound was stamped; it now carries the flat bound like the rest.
- `tasks/coding-intelligence-index-v4/suite.json`: `flat_budget` added.
- `tasks/coding-intelligence-index-v4/CHARTER.md`: budgets rule 1 revised.
- `scripts/stamp_task_budgets.py`: checks against `flat_budget` when the
  suite declares one.
- `scripts/cii-v4-board/make_report_card.py`, `scripts/harbor-export/`:
  wording that quoted the 10-hour figure.
- Published snapshots under `docs/results/` describe the conditions their
  runs were made under and were not rewritten.

Runs are comparable across the change: no run in either regime was bound
below 3 hours, so results before and after differ only in how long a
failure could take. The GPT-5.6 Luna extra-high sweep was 17 of 23 tasks
in when the restamp landed on 2026-09-13; its last six runs, and the
queued Astra rerun, run under the 3-hour bound.
