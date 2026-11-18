# VulcanBench Frontier v5: rollout plan to replace Frontier v4

Status: **draft, 2026-11-17, not adopted.** Nothing here changes a frozen
plan, the release 1 lock or any operating decision until the owner decides
the items in section 3 and each decision is recorded as a dated entry in
[docs/DECISIONS.md](../DECISIONS.md). Read [PLAN-v4.md](PLAN-v4.md) (the
frozen release 1 composition), the
[release 1 confirmation](../results/frontier-v5-r1-astra-2026-11.md) and
[HOW_WE_WORK.md](../HOW_WE_WORK.md) first.

Goal: VulcanBench Frontier v5 becomes the public headline coding suite, and
VulcanBench Frontier v4 is retired to an archived board.

## 1. Where things stand

**Done: the suite.**

- Release 1 is frozen (PR #258, `tasks/frontier-v5/suite.lock.json`, version
  1.0.0): six F5 tasks (one per language) plus the private F4 task
  ferrycore-crossing-opaque-parity.
- The confirmation is published with GPT-6 Astra as the only reference: 0
  complete passes in 21 attempts, 95% interval 0% to 16.1% (PR #292). The
  Claude Opus 5.5 half was not run (cost), per the DECISIONS.md entry of
  2026-11-10.
- Lock tooling (`harness/suite_lock.py`) already hashes Harbor-native tasks
  and checks the private task against its locked hash.
- The dashboard already knows the public name (`dashboard/lib/runner.ts`).

**Not started: everything that turns the suite into a leaderboard.** Section
2 lists the gaps.

## 2. What replacing Frontier v4 requires

### 2.1 Scores that separate models

Release 1 was built to be failed by frontier models, and it is. At the gate,
Astra solved none of the admitted tasks, Opus 5.5 solved 1 of 17 attempts on
the five tasks it ran (pulldown-cmark) and Fable 5.1 solved 2 of 5 on
tree-sitter. With 7 tasks
and 3 attempts per task, one solve moves a score by 4.8 points and the 95%
interval on a zero is 0% to 16.1%, so most of today's models would tie at or
near zero. Frontier v4 separated models because it had 23 tasks at a spread
of difficulty plus a combined score. A board of ties does not replace it.

### 2.2 A sweep path for any model

Release 1 runs only through `scripts/frontier-v5/run_gate.sh`, which accepts
three fixed references (`astra`, `fable`, `opus`) at medium effort with three
attempts. Frontier v4 sweeps run through the `vulcanbench` harness, which
has adapters for claude-code, codex, cursor, grok-build, zcode, muse-code,
pi, mirror, dsh, kimi-code and devin, at every effort level that is not
blocked. The harness
does not load Harbor-native tasks.

Harbor 0.24.0 ships agents for claude-code, codex, muse-code, kimi-code, pi,
cursor-cli, grok-build, devin, gemini-cli and opencode, among others. It has
none for DeepSeek's dsh, Reflection's Mirror or zcode; those need custom
Harbor agents (Harbor accepts `module.path:ClassName`).

Two owner rules apply to every run, v5 included:

- Each model runs in its lab's native harness when one exists.
- Every run writes `tool-calls.jsonl`, and public traces carry tool-call
  events (owner rule of 2026-10-29). Harbor job directories hold the raw
  agent logs but not this record.

### 2.3 A publishing pipeline

Frontier v4 results ship with a run population, the Code quality judging
panel, a speed panel, comparison cards, a public traces export, a results
write-up and site pages (report, model page, board rows). None of this reads
Harbor job directories, and the site has no Frontier v5 pages. Open decision
(d) from PLAN.md (publishing channel) has never been settled.

### 2.4 Where the gold lives

Each public v5 task carries `solution/gold_patch.diff` and its held-out
cases (`tests/hidden_cases` or equivalent) in this public repository, and
has since 2026-11-02. Frontier v4 does the same (its tasks carry
`gold_patch.diff`), with a canary GUID in every task. Open decision (f) from
PLAN.md (gold location) has never been settled. With only seven tasks, a
single contaminated task moves a headline far more than it would on a large
suite. The private task ferrycore already shows the alternative: the public
lock holds only its hash.

### 2.5 Housekeeping

- CLAUDE.md's "Suite names" and "Operating decisions" sections do not cover
  Frontier v5. Every release 1 task already declares a 3-hour agent timeout
  in `task.toml`, which matches the flat 3-hour rule, but the rule should
  name the suite.
- Sweep hosts need read access to `morganlinton/vulcanbench-frontier-private`
  for ferrycore, and ferrycore's traces can never be published.
- The lock's `pins.references` still names Opus 5.5, which was not run.

## 3. Decisions for the owner

Each decision lists the options and a recommendation. R1 comes first because
the engine work in section 4 depends on its answer.

### R1. How the headline separates models

1. **Pass rate only.** Complete passes over attempts, as the confirmation
   reported. Pure to the charter, but expect most models at 0% to 10% with
   overlapping intervals for some time.
2. **Pass rate plus a progress score.** Keep the pass rate as the headline
   and add a secondary score: the fraction of held-out checks passed,
   counted only when the tamper scan is clean and the guard wall holds (so
   breaking existing tests earns nothing). The data already exists in each
   verifier's output (JUnit XML for black, `jest.json` for prettier, test
   logs elsewhere), so it needs one extractor per task. Where a verifier
   does not record per-check results (pulldown-cmark writes only
   `build.log` and `tamper.log` itself), the verifier would need a change,
   which changes its task hash and needs a release 1.1 lock. The rule must
   be frozen before any model is scored on it.
3. **Add a core tier.** Twelve more tasks are already built and validated
   in `tasks/frontier-v5/` on main (among them nx-digraph, geos, ioverlay,
   tseslint and commons-lang, which Astra solved in 4 to 12 minutes, plus
   fmt, yyjson, comrak, luxon, petgraph, sympy and nx-group). A frontier
   tier (release 1) and a core tier (these, re-validated and gated for
   soundness, not difficulty) would give the board a gradient. This
   contradicts PLAN.md's "no easy anchors" rule and the plan to recycle
   solved candidates into Routine v1, so it needs a new plan with its own
   freeze.

**Recommendation: option 2 at launch, with a separation check.** After the
first four models are scored, check whether the top and bottom models are
separated beyond their error bars on either score. If not, take up option 3
as release 1.1 rather than launching a board of ties.

### R2. Sweep path

1. **Harbor-native sweep launcher.** Generalize `run_gate.sh` into a sweep
   launcher that takes any Harbor agent, model, effort and attempt count,
   keeps its infrastructure guard, usage pre-flight, Docker lock and bench
   tag check, and add custom Harbor agents for dsh, Mirror and zcode. A
   separate importer converts each Harbor job into the run record the
   publishing tools read (patch, timings, cost, `tool-calls.jsonl`).
2. **Teach the `vulcanbench` harness to run Harbor tasks.** Every existing
   adapter works unchanged, but the harness takes on Harbor's task contract
   (separate verifier image, no-network verifier, artifacts list), which is
   the part Harbor already does correctly.

**Recommendation: option 1.** Release 1's admission and confirmation ran
under Harbor, so results stay comparable with the gate record. Pin each
Harbor agent's CLI version with `--ak version=...`, the way the gate pins
Codex 0.159.0, and check each pin against the version used on Frontier v4
for that model.

### R3. What the published score includes

1. **Pass rate (and progress score, if R1 option 2) plus speed.** No Code
   quality panel at launch.
2. **Combined score as on Frontier v4,** with the Code quality judging panel.

**Recommendation: option 1 at launch.** With few solves, judging code
quality mostly scores failed patches, and each panel costs a judging pass
per run. Revisit when the board has enough solves to judge.

### R4. Publishing channel (open decision (d))

**Recommendation:** a new Frontier v5 board on vulcanbench.com as the
headline, with a suite page that states the reference result, the admission
caveats (CLI11 held only by held-out guards, pulldown-cmark's floor-8 miss,
prettier's tamper-scan fix, tree-sitter's earlier image build) and the
private task. The Frontier v4 board stays online, frozen and labeled
"retired" with its date. Public traces for the six public tasks only.

### R5. Gold location (open decision (f))

1. **Accept public gold for release 1,** as Frontier v4 did, and disclose it
   on the suite page with the canary GUID.
2. **Move gold and held-out cases private** for release 1. This does not
   undo what git history has published since 2026-11-02, so it only helps
   against future scrapes.

**Recommendation: option 1 for release 1, and keep gold and held-out cases
private from release 2 onward,** in the private repo the way ferrycore
works.

### R6. Launch lineup and effort levels

Proposed lineup for discussion (owner's pick): GPT-6.2 Sol, Fable 5.2, Opus
5.5, GPT-6.1 Terra, Sonnet 5.5, Muse Wonder and DeepSeek V4.1 Flash, each in
its native harness.

**Recommendation:** launch at two effort levels per model (medium, for
comparability with the references, and the model's highest level that is
not blocked), then fill in full effort curves after launch. Three attempts
per task, as in the confirmation.

### R7. Frontier v4 retirement timing

**Recommendation:** sweeps already running on Frontier v4 (GPT-6.2 Sol as of
today) finish and publish there. From the Frontier v5 launch date, new
models run on Frontier v5 only; Frontier v4 runs only on the owner's
explicit request.

### R8. The missing Opus 5.5 reference

**Recommendation:** leave release 1 as published (Astra only). If Opus 5.5
is in the launch lineup (R6), its medium-effort run is the reference run in
practice; say so on the suite page rather than amending the lock.

## 4. Workstreams and order

Engine changes and published results go in separate PRs (CI enforces it).
Every sweep runs from a run worktree pinned to a `bench/...` tag.

1. **Decisions.** The owner decides R1 to R8; each goes into DECISIONS.md in
   the usual format (Decision, Evidence, Revisit triggers, What this
   touched). If R1 picks option 3, write and freeze a release 1.1 plan
   before any engine work that depends on it.
2. **Engine (one PR each):**
   - the sweep launcher and the Harbor job importer (R2);
   - custom Harbor agents for dsh, Mirror and zcode, each smoke-tested on
     one task before any sweep;
   - the progress-score extractors and, if needed, verifier changes with a
     new lock version (R1 option 2);
   - aggregation: pass rate, progress score, `n=` per column, ±1 stderr,
     per-task table, speed from serial runs only.
3. **Dry run.** One model, one effort level, all seven tasks, from a bench
   tag, through import and aggregation. Diff-review any solve against the
   base, as for gate runs. This is the first check that the whole pipeline
   works end to end.
4. **Publishing tools:** the results write-up template, the comparison card
   generator for Frontier v5, and the public traces exporter (public tasks
   only).
5. **Launch sweeps** for the R6 lineup, one task at a time, with the
   separation check from R1 after the first four models.
6. **Site:** the Frontier v5 board and suite page, model pages updated, the
   Frontier v4 board archived (R4). Site PRs go to VulcanBenchCOM.
7. **Housekeeping:** CLAUDE.md suite names and operating decisions, a
   DECISIONS.md entry retiring Frontier v4, and access to the private repo
   on every sweep host.

## 5. Cost and time (estimates)

- Engine work (step 2): no model calls.
- Per model and effort level: 21 attempts. The Astra confirmation took about
  3.6 hours of agent time in total (ferrycore alone took 1.6 hours of it).
  Slower models took longer at the gate (Opus 5.5 spent 13 to 38 minutes
  per black attempt against Astra's 5 to 6), so plan on 4 to 10 hours of
  serial machine time per model and level, plus image builds and verifiers.
- Launch (R6 as proposed): 7 models at 2 levels is 14 sweeps, roughly 55 to
  140 serial hours, one task at a time.
- Model quota: Claude, ChatGPT and API spend for each sweep needs the
  owner's go-ahead before that sweep, as today.

## 6. Risks

- **Ties at launch.** The main risk; R1's separation check exists to catch
  it before the board goes live.
- **Harness drift.** Harbor's bundled agents may lag the pinned CLIs used on
  Frontier v4. Pin versions and record them in each run record.
- **Hidden infrastructure failures.** The 2026-11-03 Opus run scored 21 fake
  zeros. The sweep launcher must keep `run_gate.sh`'s infrastructure guard
  and fail-closed usage pre-flight, and the first attempt of every sweep is
  checked within minutes for real model turns and tool calls.
- **Provider-side tools.** Web search and connected apps were two ways to
  the internet the gate had to close (DECISIONS.md 2026-10-30 and
  2026-10-31). Every new Harbor agent needs the same check before its first
  scored run.
- **Small-suite contamination.** See R5.

## 7. Launch criteria

Frontier v5 replaces Frontier v4 on the site when all of these hold:

- R1 to R8 are decided and recorded in DECISIONS.md.
- The dry run has passed end to end.
- At least four models are scored, and the R1 separation check has passed.
- Each published result carries `n=`, ±1 stderr, per-task results and the
  admission caveats.
- The Frontier v4 board is archived with a dated retirement note.
