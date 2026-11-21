# VulcanBench Frontier v5: plan to replace Frontier v4 as the headline coding suite

Status: **draft for the owner, 2026-11-19; R1 to R9 and the fast path (section 4a) adopted 2026-11-19; composition frozen in PLAN-v5.md. On 2026-11-21 the owner pivoted the suite to a grid of worklist environments (section 4b, R10 to R14, draft [PLAN-v6.md](PLAN-v6.md)), which supersedes the release 1.1 board and the release 2 composition below.** It replaces the
rollout draft of 2026-11-17 (PR #302, closed unmerged) and updates its gaps
and decisions for the owner's new emphasis: a real spread of task shapes and
languages, and not another suite that mostly measures reverse-engineering.
Read [STATUS-2026-11-19.md](STATUS-2026-11-19.md) first.

Rules this plan keeps: frozen plans and freezes are never edited, so a
composition change is a new `PLAN-v5.md` with `FREEZE-v5.json`. Each decision
the owner adopts becomes its own dated entry in
[docs/DECISIONS.md](../DECISIONS.md). No task is authored, no gate is run and
no paid model call is made before the owner's yes. Engine changes and
results go in separate PRs.

## 1. What changed since the 2026-11-17 draft

- **The owner's emphasis.** Frontier v4's 23 tasks are all legacy binary
  parity tasks, and the frontier now solves them by disassembly: GPT-6.2 Sol
  disassembled the binary in all 115 of its runs and solved 115; Fable 5.2
  solved 115 and disassembled in 114. Frontier v5 must not repeat that
  thesis, which also argues against keeping F4 (R2).
- **DailyWork v1 exists.** It is the everyday-work suite (cost and solve
  rate, no single accuracy rank, D2), it takes the easy and mid-band work,
  and its engine work (D5: container execution for every adapter in the
  `vulcanbench` harness) is the sweep path Frontier v5 lacks (R6). D9 paused
  Frontier v5; resuming amends it (R1).
- **Claude spend is stopped** (DECISIONS.md 2026-11-10), which bears on the
  second gate reference (R5).

## 2. The target: release 2

Release 1 stays frozen and published as it is. The headline suite is
**release 2**, a new suite version with its own plan, freeze and headline,
built to these rules (decided in R4):

- **About 20 tasks**, at least **five shapes** and **six languages**.
- **No shape above 40%** of the tasks, and no shape reported as the
  headline on its own.
- **Every language in at least two shapes**, so a language view is not a
  view of one task.
- **No opaque-component or binary-reading task** in the headline (R2).
- **A difficulty band, not a wall** (R3, R4): tasks the strongest model can
  sometimes solve, so the board separates models.

Proposed mix (counts are targets, not slots to fill at any cost):

| Shape | Target | Admitted now | Source of the next candidates |
| --- | --- | --- | --- |
| F5 concentrated multi-bug correctness | 6 | 6 | done; the six release 1 tasks carry over |
| F3 long-horizon feature work (redefined, section 3) | 4 to 5 | 0 | upstream feature series with a design issue |
| F1 tool oracles (property-first) | 4 to 5 | 0 | upstream property and fuzz regressions that ship a counterexample |
| F6 performance under a deterministic cost oracle (new) | 2 to 3 | 0 | upstream performance fixes with a benchmark |
| F7 diagnosis from field evidence (new) | 2 to 3 | 0 | upstream bugs reported with logs, traces or a core file |
| F2 differential parity | 0 to 2 | 0 | only if a pilot holds (section 3) |
| F4 opaque-component parity | 0 | 1 (ferrycore, release 1 only) | none (R2) |

**Count today: 6 of 20 target tasks admitted.**

## 3. Shape diversity: why each shape failed so far and what to change

Every lesson below is from [GATE-LOG.md](GATE-LOG.md). The common thread:
a shape collapses whenever the sandbox hands the agent an oracle it can
grade itself against (a runnable or translatable reference, an RFC, a
symptom list that maps one to one onto tests). The shapes that survive make
the agent decide what correct means from evidence that does not run.

### F5 concentrated multi-bug correctness (6 admitted)

What held: upstream guard tests that the obvious fix breaks (floor 8). What
fell: candidates whose causes reading finds, and candidates with a public
specification the agent can turn into its own differential check
(Boost.URL and RFC 3986). F5 needs no new candidates for release 2; reserves
(sqlparser-rs, mruby) stay on the shelf in case a release 1 task is pruned.

One audit, no model calls: in four of the six tasks every Astra attempt
missed the same held-out case (black, cli11, pulldown-cmark, tree-sitter).
Before release 2 freezes, check for each such case that the instruction and
the repository specify the behavior. A case that is not derivable is an
underspecification, not difficulty, and gets fixed in a new task version.

### F1 tool-provided oracles (never sourced)

Why it failed: the two pilots fuzzed for the defect behind a security fix
and found no report on the base. The sanitizer track also pushes authoring
sessions onto the cybersecurity fallback model (AUTHORING-TRACKS.md).

Change: **lead with property oracles in managed languages**, where upstream
already committed the counterexample. Hypothesis `@example` regressions in
Python, fast-check regressions in JavaScript and TypeScript, jqwik in Java:
the base fails the stored counterexample corpus, the gold passes it and the
full upstream suite. Sanitizer tasks in C, C++ and Rust follow only from
defects that ship a reproducer (OSS-Fuzz regression test cases, a committed
fuzz corpus entry), authored in the dedicated Track B session. Floor to add
from the F5 lesson: the hidden corpus contains siblings of the visible
counterexample that a symptom-level fix does not reach, shown by a
naive-fix control.

### F2 differential parity (0 of 3)

Why it failed, three ways: the reference was runnable (comrak); a port
already existed and the agent ported from the Java (phonenumbers); and with
B1 holding, both references translated the whole reference mechanically
(jsts, Opus 5.5 matched 11,822 of 11,822 in all three attempts).

B1 is necessary and not enough: a readable reference is an oracle once an
agent can translate it. **Recommendation: F2 gets at most a two-task pilot
in release 2, and none if the pilot fails.** A pilot candidate must have a
reference whose semantics do not translate line by line into the target
(laziness, a garbage-collected graph rewritten for an ownership model, a
macro-heavy or logic-language reference), a reconstruction gap well above
9,000 logic lines, and no complete target public anywhere. If F2 is dropped,
its weight goes to F3 and F1, not to parity in another form.

### F3 long-horizon volume (0 of 1)

Why it failed: apollo's 1,042 lines were one sitting, and the instruction
listed each symptom, so the agent wrote one contract test per symptom and
fixed each in turn.

Change: **redefine F3 as long-horizon feature work.** Source a feature that
upstream built over a series of PRs from one design issue or RFC, composed
at the earliest base: several thousand changed lines across at least three
subsystems that must change together, the instruction written from the
design issue (what the feature is for and its contract), never from a list
of symptoms, and upstream's own tests for the feature held out, with the
full upstream suite as the guard wall. The derivability audit from F5
applies to every held-out test. This is the shape most like the hardest real
engineering work, and the one least exposed to a self-made oracle.

### F6 performance under a deterministic cost oracle (new)

Make a real operation faster without changing behavior. Oracle: instruction
counts (callgrind or cachegrind), allocation counts, or an asymptotic
scaling test on a hidden input family, measured in the verifier, never
wall-clock time, so grading is deterministic on a shared host. Guard wall:
the full upstream suite. Sourced from upstream performance PRs that came
with a benchmark. Risk to measure in the pilot: an agent that finds the one
hot loop in minutes, the same failure as F5's easy candidates, so floor 8's
guard idea carries over (the obvious optimization breaks a guarded edge
case).

### F7 diagnosis from field evidence (new)

The agent gets the bug report as users filed it (logs, a stack trace, a
core file or a recorded trace) and no reproducer. It must reproduce, fix
and keep the suite green; held-out tests are upstream's regression tests.
It measures debugging without a provided oracle, which is the skill F5 and
F1 both hand the agent for free. Risk: underspecified reports. Every report
must be sufficient for a human maintainer, which is what the upstream fix
shows.

### Admission for every shape

The gate's correctness part (PLAN.md section 3, Part 1) is unchanged. The
frontier part changes in R4. Gate references run with web search, connected
apps and plugins off, as today. Each new shape gets one pilot candidate
gated Astra-half first before any batch is sourced, as the original plan
did for F1 to F3.

## 4. Decisions for the owner, one at a time

Each decision has options and a recommendation. They are asked in this
order because each later one depends on the earlier answers. Mapping to the
2026-11-17 draft: R3 was R1, R6 was R2, R7 merges R3 and R4, R8 was R5, R9
merges R6 and R7, R5 updates R8; R1, R2 and R4 are new.

### R1. Does Frontier v5 resume now, alongside DailyWork v1?

1. **Resume alongside DailyWork.** Frontier v5 becomes the public ceiling
   suite with a single accuracy rank (the role Frontier v4 had); DailyWork
   stays the everyday-work suite with its cost and solve-rate views.
   DailyWork's engine work comes first because Frontier v5's sweep path
   reuses it (R6). Frontier v5 planning, sourcing and validation run in
   parallel without paid calls; gate runs are approved per wave; no heavy
   Docker validation while a sweep runs on the same Mac without asking.
   Amends D9.
2. **Keep Frontier v5 paused until DailyWork launches,** then resume.
3. **Resume Frontier v5 first** and pause DailyWork after its pilots.

**Recommendation: option 1.** The two suites answer different questions
(which model is strongest; which configuration a team needs), they share one
engine, and Frontier v5's gate wait times leave room for DailyWork work.

**Adopted 2026-11-19: option 1** (DECISIONS.md, 2026-11-19).

### R2. What happens to F4 opaque-component parity?

1. **Drop F4 from release 2.** ferrycore stays in release 1's frozen record
   and confirmation, and is not carried into the headline.
2. **Cap F4 at one task** reported on its own line, outside the headline.
3. **Keep F4 at two slots** as PLAN.md set it.

**Recommendation: option 1.** It is the Frontier v4 lineage; all three F4
candidates fell or held only after a redesign against the gate; it needs a
private repo on every sweep host; and its traces can never be published.

**Adopted 2026-11-19: option 1** (DECISIONS.md, 2026-11-19).

### R3. How does the headline separate models?

Release 1 gave GPT-6 Astra 0 of 21, so on its own it cannot separate models.

1. **Pass rate plus a progress score.** Pass rate (complete passes over
   attempts) stays the headline; a secondary score is the fraction of
   held-out checks passed, counted only when the tamper scan is clean and
   the guard wall holds. It needs per-check verifier output for every task
   (pulldown-cmark writes none today, so its verifier changes in a new task
   version). The rule is frozen before any model is scored.
2. **Pass rate only,** with release 2's difficulty band (R4) doing the work.
3. **Both, plus a measurement first:** before engine work, run the
   strongest available model at its highest unblocked effort on release 1
   (21 attempts, quota), to learn whether release 1 already separates the
   top.

**Recommendation: option 3.** The progress score uses data the verifiers
already produce, and one top-effort run says whether the wall is a wall or
an artifact of medium effort. Its result decides R4's band.

**Adopted 2026-11-19: option 3** (DECISIONS.md, 2026-11-19). The
measurement's model, conditions and run need their own go-ahead.

### R4. Release 2 composition and admission band

1. **Adopt section 2's target** (about 20 tasks, five or more shapes, six
   languages, no shape above 40%, F4 out) in a new PLAN-v5 with FREEZE-v5,
   and change the frontier gate from "each reference solves at most 1 of 3"
   to a band: the stronger reference at its highest unblocked effort solves
   between 1 and 3 of 5, and the weaker at medium solves at most 1 of 3.
2. **Same composition, keep the current gate** (both references at most 1
   of 3 at medium).
3. **Smaller release 2** (12 tasks: the six F5 plus two each of F1, F3 and
   one new shape), then grow.

**Recommendation: option 1.** A gate that admits only what the frontier
fails produces a board of zeros; a band keeps the hardest tasks reachable,
and DailyWork already covers the work below it.

**Adopted 2026-11-19: as recommended** (DECISIONS.md, 2026-11-19; PLAN-v5.md).

### R5. Gate references for release 2

1. **GPT-6.2 Sol (Codex) and Claude Fable 5.2 (Claude Code),** the two
   strongest models measured on Frontier v4, if the owner funds the Claude
   half.
2. **GPT-6.2 Sol and a non-Claude second reference** (Gemini 4 Neon once its
   native harness runs, or DeepSeek V4.1 Flash, which is cheap), with no
   Claude spend.
3. **Keep GPT-6 Astra and Claude Opus 5.5,** as in release 1.

**Recommendation: option 1, else option 2.** References from two labs keep
the gate honest; the release 1 references are a generation behind.

**Adopted 2026-11-19: as recommended** (DECISIONS.md, 2026-11-19; PLAN-v5.md).

### R6. Sweep path for any model

1. **The `vulcanbench` harness with DailyWork's container engine (D5).**
   Convert Frontier v5's task environments to the harness task format once
   D5 lands; every adapter and effort level, `tool-calls.jsonl`, economics
   and audits come for free. Before any score, check under the harness that
   every task still grades base 0, gold 1 and every probe 0, and that the
   infrastructure guard and fail-closed usage pre-flight from `run_gate.sh`
   (PR #288) carry over.
2. **A Harbor sweep launcher** generalized from `run_gate.sh` (any agent,
   model, effort, attempts) plus an importer from Harbor jobs into run
   records, and custom Harbor agents for dsh, Mirror and zcode.

**Recommendation: option 1.** One engine for both public coding suites;
Harbor's agents lag the pinned CLIs and lack three adapters. The cost is
comparability with the release 1 gate record, which the base, gold and probe
check and a re-gate of release 2 under the harness answer.

**Adopted 2026-11-19: as recommended** (DECISIONS.md, 2026-11-19; PLAN-v5.md).

### R7. Published score, judging and publishing channel (open decision (d))

1. **Launch with pass rate, progress score and speed** (speed from serial
   runs only), no Code quality panel; results write-up, per-task table,
   `n=` and ±1 stderr on every column, comparison cards, public traces for
   public tasks. Channel: a Frontier v5 board on vulcanbench.com; datasets
   handed to labs on request from the stripped tree; no Harbor hub
   publication.
2. **The same, with the Code quality panel at launch** (Frontier v4's
   combined score).
3. **Option 1, plus publishing the task set on the Harbor hub.**

**Recommendation: option 1.** With few solves a panel mostly judges failed
patches; add it once a column has enough solves to judge. A public hub
listing invites contamination of a 20-task suite.

**Adopted 2026-11-19: as recommended** (DECISIONS.md, 2026-11-19; PLAN-v5.md).

### R8. Gold location for release 2 (open decision (f))

Decided for private material on 2026-10-31; this question is about the rest.

1. **Private golds and held-out tests for every release 2 task,** in
   `vulcanbench-frontier-private`, with hashes in the public lock, as
   DailyWork D8 does. Release 1 stays as published.
2. **Keep the 2026-10-31 rule:** tasks whose golds are public upstream stay
   in this repo with their held-out cases.

**Recommendation: option 1.** Upstream golds are public anyway, but the
held-out cases and guards are ours and are what holds a task; at 20 tasks a
scraped held-out case moves the headline by five points.

**Adopted 2026-11-19: as recommended** (DECISIONS.md, 2026-11-19; PLAN-v5.md).

### R9. Launch lineup, effort levels and Frontier v4 retirement

1. **Launch with four to six models** each in its native harness (for
   example GPT-6.2 Sol, Fable 5.2, GPT-6.1 Terra, Sonnet 5.5, DeepSeek V4.1
   Flash, Gemini 4 Neon), at the model's highest unblocked effort, three
   attempts per task; medium for the two references only; full effort
   curves after launch. Frontier v4 runs only on the owner's request from
   R1, and its board is archived with a dated note the day Frontier v5
   launches.
2. **Full effort curves at launch,** fewer models.

**Recommendation: option 1.** It gets a board up with separation before the
long tail of effort levels.

**Adopted 2026-11-19: as recommended** (DECISIONS.md, 2026-11-19; PLAN-v5.md).

## 4a. Fast path (adopted 2026-11-19)

The owner found the one-to-two-month estimate too slow and chose, on
2026-11-19, to put a board up first and add shapes next:

1. **Board first, about one week.** If the release 1 measurement (R3)
   shows the progress score separates models, Frontier v5 launches a
   public board on release 1's six public F5 tasks (one shape, six
   languages, no reverse-engineering), scored on pass rate plus the
   progress score, and says plainly that it is one shape. If the
   measurement shows no spread, this step is skipped.
2. **Shapes next, about three weeks.** Six new tasks in four shapes (F3
   feature work x2, F1 property oracles x2, F6 performance x1, F7
   diagnosis x1) are built in parallel from now and join the board as
   release 2.
3. **How the time is cut:** parallel authoring sessions (no paid calls),
   sweeps through Harbor with `run_gate.sh` extended to more models instead
   of waiting for DailyWork's container engine, gating on GPT-6.2 Sol at max
   first (the second reference only when Sol lands in the band), R4 to R9
   decided in one pass, and a second host for runs if available.

## 4b. Pivot to the grid (adopted 2026-11-21)

The R3 measurement finished on 2026-11-19: GPT-6.2 Sol at max passed 5 of 18
in 8.5 hours ([results](../results/frontier-v5-r1-sol62-max-2026-11.md)).
The owner then asked for a benchmark that is faster and easier to run and
adopted, on 2026-11-21:

- **R10:** Frontier v5 becomes a grid of worklist environments: pinned
  repos, about 8 independently graded items each, one budgeted agent session
  per repo, headline items done over all items.
- **R11:** six languages by two repo sizes, twelve environments.
- **R12:** the release 1.1 board is skipped; release 1.0.0 stays published.
- **R13:** one run per model at its top effort, four environments at a
  time, a fixed time budget, speed off the board (Frontier v5 only).
- **R14:** one pilot cell first (C++, smaller tier, CLI11).

The draft composition is [PLAN-v6.md](PLAN-v6.md), frozen after the pilot.
Sections 2, 4a, 5 and 6 describe the plan before the pivot and are kept as
the record.

## 5. Work order

1. Decisions R1 to R9, each a DECISIONS.md entry. R4 becomes PLAN-v5.md
   with FREEZE-v5.json.
2. No model calls: the derivability audit on release 1 (section 3); confirm
   the Mac Studio evidence is backed up; per-check verifier output where
   missing (a new task version, not an edit of a frozen one).
3. R3's top-effort measurement on release 1 (quota, owner's go-ahead).
4. Pilots: one candidate each for redefined F3, property-first F1, F6 and
   F7 (and F2 only if section 3's pilot rule keeps it), each validated, then gated
   Astra-half first under per-wave approval.
5. Sourcing waves of five for the shapes whose pilot held.
6. Engine, after DailyWork's D5 lands: Frontier v5 tasks in the harness
   format, progress score extractor, aggregation (pass rate, progress, `n=`,
   ±1 stderr, per-task table), results template, card generator, traces
   exporter. Each its own PR.
7. Freeze release 2, run its confirmation, then the launch sweeps (R9),
   then the site (board, suite page, Frontier v4 archive).

## 6. Rough cost and time (estimates)

- **Planning, audit, engine:** no model calls. Engine work after D5 is
  about a week of sessions.
- **R3 measurement:** 21 attempts on one model at its top effort; 5 to 20
  serial hours on subscription quota.
- **Authoring:** 14 more admits. At F5's yield (5 of 6 in wave 1 after
  floor 8, 6 of 13 overall) and the other shapes' record so far (1 of 7,
  after a redesign), plan on 30 to
  45 candidates at one to two session-days each, so 4 to 8 weeks at the
  October pace.
- **Gate runs:** a reject costs 2 to 3 runs with the early exit, an admit 6
  to 8 (two references, top-ups). That is about 120 to 200 runs at 10 to 60
  agent minutes each: 60 to 200 serial hours, plus Claude spend if R5 picks
  option 1.
- **Release 2 confirmation and launch sweeps:** 20 tasks x 3 attempts = 60
  runs per model and level, 10 to 60 serial hours each. Six models at one
  level is 60 to 360 serial hours, one task at a time.
- **Money:** API-equivalent cost per Frontier v5 run has not been measured
  (the gate ran on subscriptions). The R3 measurement records it and turns
  these ranges into a budget for the owner before the launch sweeps.

## 7. Risks

- **Ties at launch.** R3 and R4 exist for this; the separation check is
  that the top and bottom launch models are separated beyond their error
  bars.
- **Underspecified held-out cases** passing as difficulty (the audit in
  section 3).
- **New shapes collapse like F2 and F3.** Pilots before batches, and the
  self-made-oracle test in every DESIGN.md: can the agent grade itself from
  something in the sandbox?
- **Hidden infrastructure failures** (the 2026-11-03 fake zeros) on a new
  sweep path: the guard and pre-flight must carry over, and every sweep's
  first attempt is checked within minutes.
- **Contamination** of a small suite (R8).
- **Competing work.** The Frontier v5.1 build in
  `../VulcanBench-frontier-v5p1` is separate and is not touched by this plan.
