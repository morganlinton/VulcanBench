# VulcanBench Frontier v5: composition v6, the grid (draft)

Planning baseline: November 21, 2026, America/Los_Angeles.
Status: **draft, not frozen.** The owner adopted the pivot and its four
framing choices in chat on 2026-11-21 (docs/DECISIONS.md, 2026-11-21, R10 to
R14). The numbers marked "pilot" below are set by the pilot cell, and this
plan is frozen with `plan-v6.json` and `FREEZE-v6.json` after the pilot.
`PLAN.md` to `PLAN-v5.md` and their freezes are not edited.

## 1. Why the grid

A Frontier v5 run under PLAN-v5 gives one pass or fail per 25 minutes of
agent time: 18 attempts took 8.5 hours on one model at one effort and left a
pass-rate interval of 9.7% to 53.5%
([measurement](../results/frontier-v5-r1-sol62-max-2026-11.md)). Stable
numbers then need three attempts per task, several effort levels and serial
runs. The grid changes the unit of work so that one run of about an hour
produces about 100 graded results:

- run time is a design input (a fixed budget), not an outcome;
- each agent session is graded on many independent items;
- an open-ended score ("how much got done") needs no difficulty band;
- setup is paid once per environment, not once per task.

What stays: held-out tests, tamper scans, `n=` and ±1 stderr on every
number, pinned repos, private gold, native harnesses, web search and apps
off.

## 2. The grid

Six languages by two repo sizes: twelve environments.

| Language | Smaller repo | Larger repo |
| --- | --- | --- |
| C | to source | to source |
| C++ | to source | to source |
| Java | to source | to source |
| JavaScript or TypeScript | to source | to source |
| Python | to source | to source |
| Rust | to source | to source |

- **Size** is lines of first-party source at the pin (tests, vendored and
  generated code excluded), measured with one tool and recorded in the lock.
  Proposed tiers: smaller under 30,000 lines, larger over 100,000 lines,
  confirmed when the candidates are measured.
- **Seeds.** The six release 1 repos (tree-sitter, black, prettier,
  javaparser, CLI11, pulldown-cmark) and the six release 2 candidate repos
  (curl, mpmath, vercel/ai, grpc-java, OpenCC, ruint) are the first
  candidates, placed by measured size. A cell with no fitting seed gets a
  newly sourced repo.

## 3. An environment

One pinned repo at one commit, one image, and a **worklist** of about 8
independent items (pilot: the exact count) that the agent receives as a
file:

- at least half are defects (upstream fixes merged after the pin, each with
  its own held-out checks);
- at least one small feature;
- where the repo supports it, one performance target under a deterministic
  cost oracle and one diagnosis item from field evidence.

The four shapes of PLAN-v5 (feature work, property oracles, performance,
diagnosis) live on as item kinds, and the release 2 candidates become items
in their repos' worklists.

Rules for items: each has a reference change and held-out checks; items are
independent (each reference change alone passes exactly its own item, and
all together pass all); the visible suite passes at base; the entry says
everything its held-out checks assert, the agent keeps the tests that
existed before each fix, and nothing in the tree gives a check away
(section 6, checks 5 to 7).

## 4. A run

- One agent session per environment with the worklist and a **fixed
  wall-clock budget** (pilot: the value, starting hypothesis 20 minutes),
  the same for every model. The agent is told the budget and may take items
  in any order. At the budget the session is stopped and the tree is graded
  as it stands.
- **One run per model, at its highest unblocked effort.** Four environments
  run at a time, in the same fixed batches for every model, so contention is
  equal across models. A model run takes about 1 to 1.5 hours.
- Runs go through Harbor (R6 stands). Engine work: stop at the budget and
  still grade, per-item scoring, four-way parallel runs, the aggregation.

## 5. Scoring and the board

- **An item is done** when all of its held-out checks pass. The tamper scan
  flags an environment to zero. How a broken guard is charged (to its item,
  or one item off the environment's count) is set by the pilot.
- **Headline:** items done over all items, with `n=` and ±1 binomial stderr.
  Beside it: the mean per size tier, the mean per language, and the
  six-by-two heatmap. Cell values are shown as counts, not ranked.
- **Also reported:** tokens and list-price cost per run. Wall-clock speed is
  not on the board (R13).
- No Code quality panel at launch (R7 stands on that point).

## 6. Admitting an environment

Correctness only; the frontier band of PLAN-v5 (R4, R5) does not apply to
the grid.

1. Base scores 0 items, the full reference scores every item, three
   deterministic repeats agree.
2. Each item's reference change alone scores exactly that item.
3. Verifier probes (exit, stealth halt, report shadowing) score 0.
4. Ceiling check: if the strongest model at its top effort finishes every
   item inside the budget in the calibration run, items are added before
   the environment is admitted.
5. Entry against assertions. Every held-out check is read against its
   worklist entry. Each assertion on exact message or output text, on file
   state, on an exact error code or exception type, or on an internal
   symbol must be stated in the entry, or be something a competent fix
   cannot avoid (it follows from behaviour the entry states, from the
   repository's existing conventions, or from the test's own fixture).
   Where neither holds, the entry is clarified or the verifier's copy of
   the check is narrowed, and the change is recorded in the environment's
   design record. Check 2 does not cover this: it shows that the checks
   need the reference, not that the entry is enough to pass them.
6. Visible guards for existing tests. A held-out test that existed before
   its item's earliest fix stays in the agent's tree, in a version that
   passes at base: the pin's test with the item's changes to it reversed
   and any assertion the fix changes cut. The verifier still runs the
   pin's version. Each visible version must pass at base and with the full
   reference. A test stays fully held out only when every version of it
   asserts what the fix changes. Tests the fix itself added stay held out.
7. No leaked test data. Nothing in the agent's image gives away a held-out
   check or describes an item as done: no held-out test or check name, no
   data file or expected output of a held-out test, no comment left behind
   by a removed test, and no changelog, README, book or example text that
   describes an item's fix or feature as present. The check lists every
   file each item's upstream change touched outside the graded source and
   compares the agent's tree with it, then searches the whole image for
   the check names and for the names a feature item adds.

Checks 5 to 7 were added on 2026-11-22, after the second pilot cell's
first runs: most of the items both models missed there failed on something
the worklist entry did not say, and the agents had lost existing
regression tests along with the new ones. They apply to every environment,
the two pilot cells included.

## 7. What this supersedes

- PLAN-v5 section 1 (the release 1.1 board): skipped (R12). Release 1.0.0
  and its results stay published as they are.
- PLAN-v5 section 2 (release 2 as twelve single tasks) and section 3 (the
  admission band): replaced by sections 2 to 6 here.
- PLAN-v5 section 4: private gold (R8), Harbor (R6) and the launch lineup
  (R9) stand; the speed column does not.
- The progress score rule (option C) applies to the release 1 measurement
  only.

## 8. Order of work

1. **Pilot cell (R14):** C++, smaller tier, on CLI11, whose six release 1
   defects already carry references and held-out checks. Add items to reach
   the worklist size, build the budgeted run, then (on the owner's
   go-ahead for the paid runs) run GPT-6.2 Sol and one cheaper model to set
   the budget and confirm the scores spread.
2. Meanwhile the other sessions measure the seed repos and source items for
   their cells; nothing is built until the pilot fixes the format.
3. Freeze this plan with the pilot's numbers, build the other eleven
   environments in parallel, admit them, lock release 2.0.0.
4. Launch sweeps for the lineup (owner's go-ahead), board, Frontier v4
   board archived the day it launches (R9).
