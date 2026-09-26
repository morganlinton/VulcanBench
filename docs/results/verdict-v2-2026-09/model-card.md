# VulcanBench Verdict v2: Jev 1.13.0

**September 25, 2026 · VulcanBench Verdict v2 · jev-1.13.0 · 4,774 test items · 20 question families · $0.58**

Verdict v2 measures TypeSafe AI's Jev across twenty kinds of judgment, in
software engineering and in general reasoning, with every answer checked
against something that cannot be argued with: a program's real output, a
type checker, a hidden test suite, a merged fix, or the generator that built
the question. Jev is a System One model. It writes no text; it returns a
typed decision (one option from a list, a level on an ordered scale, or the
probability that a statement is true) in a single fast pass.

Jev scores **45.9** on the Verdict Index (95% interval 43.1 to 48.4), where
0 means no better than the best dumb strategy for each question and 100
means perfect. GPT-6 Astra at high effort, given exactly the same inputs as a
reference row, scores **91.7** (90.1 to 93.1). Jev answers in 0.3 seconds for
about $0.12 per 1,000 questions; the reference takes 7 seconds per question
and reasons at length before answering.

## Why v2

Verdict v1 (September 22) asked Jev one kind of question, whether an agent's
patch passes its hidden tests, four different ways. Its headline could not
separate Jev from guessing: 63% of patches passed, so always answering
"passes" scored 62.7% and Jev scored 62.8%. Patch size alone predicted the
answer better than Jev did. v2 fixes all three problems:

- **Many skills, not one.** Twenty families in eight areas, each from its
  own source, so one family's quirks cannot decide the result.
- **Scores above the floor, not raw accuracy.** Items are balanced by
  construction, and each family is scored as skill:
  `100 * (accuracy - floor) / (1 - floor)`, where the floor is the best of
  always giving the most common answer, guessing, and every surface shortcut
  we could build for that family. A v1-style result reads as 0 here.
- **Shortcuts are measured, not assumed away.** Every family records what
  cheap heuristics (longest option, larger patch, first service to log an
  error, the option most similar to the others, and so on) would answer, and
  a family only ships if the best of them stays at skill 15 or below.

## How the test works

1. **A question is built with a known answer.** For code, the answer comes
   from running the code, a type checker, the hidden tests of our Frontier v4
   and earlier suites, a merged open-source fix or a security advisory. For
   general reasoning, a program generates the puzzle, table or policy and
   knows the answer by construction.
2. **Jev sees the question and nothing else.** The state is the text a
   person would need (a program, two patches, a table, logs from several
   services), plus one typed question.
3. **The answer is scored against the floor.** Jev's top answer is compared
   with the truth, and with the best trivial strategy for that family.
4. **A reference model checks that the question is fair.** GPT-6 Astra at
   high effort answers the same items with no tools. If it could not answer a
   family well, the family would say nothing about Jev and would not ship.

## Results

| | Jev 1.13.0 | GPT-6 Astra, high (reference) |
|---|---|---|
| **Verdict Index** (20 families) | **45.9** (43.1 to 48.4) | **91.7** (90.1 to 93.1) |
| Software sub-index (12) | 49.6 (45.5 to 53.1) | 86.2 (83.6 to 88.5) |
| General sub-index (8) | 40.4 (36.8 to 43.9) | 100.0 (100.0 to 100.0) |
| Calibration Index (Brier skill, 0 to 1) | 0.33 (0.30 to 0.35) | 0.87 (0.85 to 0.89) |
| Median time per answer | 0.29 s (p95 0.42 s) | 7.2 s (p95 17.8 s) |
| Cost | $0.58 for 4,774 answers | ChatGPT subscription |

Intervals are 95% bootstrap intervals that resample source units (tasks,
repositories or generator seed groups), not items. The two rows are separated
on every index.

### By family

Skill, 0 = the best trivial strategy, 100 = perfect. Ranking is AUROC for
families with a fixed set of options (no cutoff needed); a dash means the
options differ per item.

| Area | Family | Test items | Floor | Jev skill (95%) | Jev ranking | Reference skill | Best shortcut skill |
|---|---|---|---|---|---|---|---|
| Reading code | What does this program print? | 233 | 26% | 39.9 (29 to 51) | 0.81 | 100.0 | 9.8 |
| Reading code | Which of two snippets type-checks? | 205 | 58% | 87.2 (72 to 97) | 0.99 | 100.0 | 0.0 |
| Reviewing changes | Which of two patches passes the tests? | 268 | 51% | 36.4 (17 to 52) | 0.76 | 57.6 | 0.8 |
| Reviewing changes | Which listed test does this patch fail? | 210 | 26% | 0.6 (-14 to 15) | - | 69.7 | 7.3 |
| Finding bugs | Which file does the fix touch? | 257 | 11% | 56.8 (50 to 63) | - | 79.9 | 1.3 |
| Finding bugs | Which function holds the planted bug? | 266 | 21% | 75.4 (66 to 84) | - | 100.0 | 5.4 |
| Security | Which version is vulnerable? | 240 | 60% | 22.9 (6 to 37) | 0.90 | 87.5 | 20.0 |
| Security | Which weakness class is this? | 240 | 23% | 60.3 (53 to 68) | - | 70.1 | 13.2 |
| Testing | Does this test catch this change? | 206 | 52% | 28.3 (7 to 46) | 0.77 | 90.9 | 2.9 |
| Testing | Is this expected value right? | 227 | 51% | 42.3 (29 to 56) | 0.84 | 100.0 | -1.8 |
| Operations | Which service caused the incident? | 240 | 14% | 81.6 (76 to 87) | - | 99.0 | 0.0 |
| Operations | Patch, minor or major version bump? | 262 | 43% | 63.3 (53 to 72) | 0.91 | 80.0 | -2.0 |
| Logic | Which assignment satisfies every rule? | 240 | 25% | 36.1 (29 to 44) | 0.78 | 100.0 | 5.6 |
| Logic | Does the conclusion follow? | 240 | 51% | 75.4 (68 to 83) | 0.95 | 100.0 | 1.7 |
| Math | Multi-step word problem | 240 | 27% | 40.9 (30 to 51) | 0.83 | 100.0 | 3.3 |
| Math | Which band holds this quantity? | 240 | 23% | 24.2 (15 to 34) | - | 100.0 | 3.1 |
| Tables | Which group answers this question? | 240 | 26% | 23.7 (16 to 31) | - | 100.0 | 0.0 |
| Tables | How many rows match? (bands) | 240 | 20% | 35.4 (29 to 42) | - | 100.0 | 0.0 |
| Rules and policy | Is this case allowed? | 240 | 51% | 44.1 (27 to 58) | 0.82 | 100.0 | 1.7 |
| Rules and policy | Which clause decides this case? | 240 | 18% | 43.4 (31 to 55) | - | 100.0 | 7.0 |

The floor is measured on the published test split. Families were balanced
over their full build, so a test split can lean one way (the type-check
pairs are 58% "A" on test), and the floor rises to match.

## Findings

### 1. Strong on recognition, weak on multi-step work

Jev's best families are ones where the answer can be recognised from the
shape of the text: whether a snippet type-checks (87), which service caused
an incident when every victim's error points upstream (82), whether a
conclusion follows from premises (75), and which function a failing
assertion implicates (75). Its weakest are the ones that need several steps
of working: a filter, group and total over a table of up to 200 rows (24),
placing a computed value in a band (24), satisfying five to eight
interlocking rules (36), predicting which hidden test a patch fails (1). The reference, which reasons before it
answers, scores 100 on nearly all of those. The gap between the two rows is
largest exactly where the work is.

### 2. Patch review is above the floor but weak

On pairs of real agent patches for the same issue, matched on size so that
size gives nothing away, Jev picks the one that passes the hidden tests 68.7%
of the time: skill 36 (17 to 52), ranking 0.76. That is above the floor,
unlike v1, but well behind the reference (58, ranking 0.90). Asked which listed test a failing patch breaks,
with the source of every listed test in front of it, Jev scores 0.6
(-14 to 15): no better than choosing the longest test. The reference scores
70 on the same items, so the question is answerable.

### 3. A lean towards yes on yes or no questions

Every yes or no family is built at a 50% base rate. Jev's stated probability
ranges from about 0.01 to 0.99 on all four, so the v1 failure, a model whose
probabilities never reached the decision boundary, does not recur. But it
says "true" more often than it should: 73% of the time on whether an expected
value is right, 72% on whether a test catches a change, 60% on whether a
policy allows a case. Its ranking on those families (0.84, 0.77, 0.82) shows
more signal than its yes or no answers do.

| Yes or no family | Lowest p(true) | Median | Highest | Share answered yes | True rate |
|---|---|---|---|---|---|
| Is this expected value right? | 0.02 | 0.74 | 0.98 | 73% | 51% |
| Does this test catch this change? | 0.10 | 0.65 | 0.97 | 72% | 50% |
| Is this case allowed? | 0.03 | 0.62 | 0.98 | 60% | 50% |
| Does the conclusion follow? | 0.01 | 0.57 | 0.99 | 55% | 50% |

### 4. Calibration

The Calibration Index is the mean Brier skill score against forecasting
each family's answer frequencies: 0 is no better than that, 1 is perfect.
Jev scores 0.33, the reference 0.87. Jev is best calibrated where it is most
accurate (type checking 0.83, incidents 0.73, bug location 0.68) and close to
uninformative on code output (0.10), vulnerable versions (0.09) and failing
tests (0.06).

### 5. Headroom

The reference answered all 1,920 general test items correctly. These
families separate a fast decision model from a reasoning model very clearly,
but they cannot rank two strong reasoning models against each other. The
real-world software families still have headroom for the reference:
patch pairs 58, failing tests 70, weakness class 70, which file 80, version
bump 80, vulnerable version 88.

## The gate change

Families were admitted by a gate written down before any results. On a
30-item development pilot, the reference was to score between 40 and 95, the
best shortcut 15 or less, and each family needed at least 200 test items
from at least 20 independent sources. The pilot broke the upper bound: the
reference scored 100 on 14 of 20 families. We inspected items by hand for
leaks and found none; the reference simply solves generated puzzles by
reasoning through them. Hardening those families until the reference fell
below 95 would have pushed Jev onto the floor on most of them.

So the gate was amended after seeing the pilot, on the owner's decision,
and logged in the project's decision record on September 25:

- The reference only has to show a family is answerable (skill 40 or more);
  it has no upper bound.
- A family is too easy when **Jev** reaches skill 90, because then it
  measures nothing about Jev.
- The shortcut rule is checked on the full build (about 300 items per
  family), not on the 30 pilot items, where chance alone moves a shortcut by
  about 18 points.

Under the amended gate, four families needed work before they shipped, and
all four were rebuilt, not waved through:

- **Program output** (Jev 95 on the pilot): the wrong options were all one
  edit away from the true output, so the right answer was the option most
  similar to the others, 88.7% of the time. No builder had recorded that
  shortcut. Wrong options now form clusters and chains, and the gate checks
  this "most similar option" shortcut on every multiple-choice family.
- **Planted bug** (Jev 96): crash messages named the buggy function. Only the
  test's own assertion is shown now, the options are the functions on the
  test's call path, and 15 modules come from outside networkx.
- **Failing test** (reference 39): test names alone were barely answerable.
  Each item now shows the source of every listed test.
- **Weakness class** (keyword shortcut 15.2): rebalanced to 13.2 on the test
  split.

After the rebuild, all 20 families passed, and the item set was frozen
(5,926 items, seed 20260924, SHA-256 recorded) before the test split was
run.

## Method

- **Items.** 5,926 items, 4,774 in the published test split. Items are split
  into development and test by source unit (task, repository or seed group),
  so no task, repository or template appears in both. The development split
  was used only for the pilot.
- **Sources.** Software families come from executed programs (27 Python and
  8 JavaScript templates for program output), mypy 2.1.0 in strict mode,
  planted mutations in real library modules labelled by running each
  library's own tests, the hidden-test verdicts of 3,821 distinct archived
  agent patches over 129 tasks (VulcanBench v1, v3, the Python pool and
  Frontier v4), merged single-file fixes from 107 repositories, and reviewed
  security advisories whose fix commits date from June 1, 2026 or later.
  General families are generated fresh by code, with nonsense names, so no
  item exists anywhere else.
- **Ground truth audit.** Before the freeze, a separate script re-derived
  the answer to a fixed 5% sample of every family (298 items) without using
  the builders' code: 298 of 298 matched.
- **Jev.** `jev-1.13.0` through TypeSafe's System One API, pinned (not
  `jev-latest`). The API has no effort, temperature or seed setting, so Jev
  has one column. Latency is wall clock from the operator's machine in
  California, including the network round trip, and is indicative only.
- **Reference.** GPT-6 Astra at high effort through `codex exec` on a
  ChatGPT subscription, in an empty read-only directory, with the identical
  state, question and options and a request for a probability on every
  option. No answer used a tool.
- **Scoring.** Skill uses each model's top answer. Ties break to the first
  option, whose position is randomised. Missing answers count as wrong
  (there were none). Intervals resample source units 2,000 times, keeping
  each family's floor strategy fixed at the one chosen on the real data.

## Limits

- **The gate was changed after the pilot.** The change and its reasons are
  above and in the decision record. It removed the reference ceiling, added
  the Jev ceiling, and moved the shortcut check from the 30 pilot items to
  the full build. On the pilot items alone, the "shorter version" shortcut
  on vulnerable versions read skill 40 and would have failed the original
  rule.
- **One shortcut is stronger on the test split than the gate saw.** On
  "which version is vulnerable", choosing the shorter version scores skill
  20 on the test split (11.6 on the full build that the gate checked). Jev's
  skill of 22.9 is measured above it, but only just.
- **Some families lean on one source.** 58% of planted-bug items come from
  networkx. 199 of 242 failing-test items and 184 of 300 patch pairs come
  from Frontier v4's binary-parity tasks. Most general families
  are one generator each.
- **Labels by convention.** The version-bump family is labelled by our own
  public-API differ for Python (names without a leading underscore are
  public), not by maintainers' intent. The weakness family collapses
  advisory CWE ids into ten classes by a published mapping.
- **Memorisation.** Generated items cannot have been seen before. Mined items
  are from June 2026 or later, but the libraries around them (networkx
  especially) are old and well known.
- **Pilot numbers were noisy.** Each pilot family had 30 items; patch pairs
  read -45 on the pilot and 36 on the 268-item test split. Only the test
  split is published.
- **Verdict and Frontier scores measure different things** and are not
  comparable.

## Reproducing

Item files are private and gitignored, because software items embed
benchmark tasks and hidden tests. The code, the freeze manifest, the pilot
gate table and every published number are public:

- Suite: `harness/verdict/v2/` (items, registry, scoring, gate, twenty family
  builders); spec in `docs/VERDICT_V2.md`; decisions in `docs/DECISIONS.md`
  (September 24 and 25, 2026).
- Build: `scripts/verdict-v2/build_items.py --per-family 300` (seed
  20260924); the manifest in `freeze-manifest.json` records the commit and
  the item file's SHA-256.
- Run: `scripts/verdict-v2/run_jev.py` and `run_reference.py`; score and
  export with `scripts/verdict-v2/export_results.py`, which writes
  `verdict-v2-jev.json` beside this report.
- Audit: `scripts/verdict-v2/audit_ground_truth.py`.
