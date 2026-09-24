# VulcanBench Verdict v1: Jev 1.13.0

**September 22, 2026, corrected September 23 · VulcanBench Verdict v1 · jev-1.13.0 · 2,163 test items · 745 agent patches · 23 tasks · $0.45**

First measurement of TypeSafe AI's Jev on software engineering. Jev is a
"System One" model: it does not generate text, it returns a typed decision
(one of a fixed set of options, a score on an ordered scale, or the
probability that a statement is true) in a single parallel pass. It cannot
write code, so it cannot take VulcanBench Frontier v4. It can judge code,
which is what this suite asks it to do.

Every headline answer here was produced by running tests, not by asking
another model. The items are real patches written by frontier coding agents
during Frontier v4 sweeps, and the answer to "does this patch work" is what
the hidden test suite reported when that patch was graded.

> **Correction, September 23, 2026.** The first version of this report scored
> yes/no questions at a 0.5 cutoff. Jev's probability that a patch passes
> never exceeds 0.42, so that cutoff sat outside its output range and no item
> could be answered "passes". The published figures were an artifact of the
> decision rule, not of the model. This version reports the ranking (AUROC),
> which needs no cutoff, and accuracy at a cutoff fitted on the development
> split. Two claims did not survive and are withdrawn; see
> [What changed in the correction](#what-changed-in-the-correction).

> **Is the pass question answerable at all? A control, added September 23,
> 2026.** Every fix here comes from a task where an agent rebuilt a retired
> program, and the hidden tests check agreement with that program's real
> behaviour, including quirks the written specification gets wrong. Jev sees
> only the bug report and the fix. To check that the question can be answered
> from that alone, GPT-6 Astra at high effort, through Codex, was given
> exactly the same inputs on all 611 published pass questions, with its
> cutoff chosen on the 134 development ones, as Jev's was. See
> [The control](#the-control) for the numbers. In short: the question is
> answerable, and Jev's shortfall on it is real. Astra ranks the fixes far
> better (AUROC 0.87 against 0.69) and beats always guessing by about 10
> points, where Jev draws level with it.

## Results

Test split, one query per item, model pinned to `jev-1.13.0`.

| Question | Items | Majority floor | Jev, fitted cutoff | AUROC | Brier | ECE |
|---|---|---|---|---|---|---|
| Does this patch pass every hidden test? | 611 | 62.7% | **62.8%** | 0.690 | 0.383 | 0.409 |
| Does it break a test that passed before? | 611 | 94.1% | **93.1%** | 0.652 | 0.284 | 0.268 |
| Which of four outcomes did the tests report? | 611 | 62.7% | **49.8%** | 0.651 | 0.650 | 0.186 |
| **Overall, ground truth only** | **1,852** | **72.4%** | **68.9%** | | **0.434** | **0.278** |
| Which of two patches is more maintainable? * | 311 | 57.9% | **89.7%** | 0.962 | 0.162 | 0.082 |

\* Reference is agreement with the Muse and Grok code-quality panel, which is
opinion, not ground truth, so this line is reported apart and excluded from
the overall row.

The majority floor is the score for always answering a family's most common
label. Cutoffs for the two yes/no families were fitted on the development
split and are 0.17 and 0.72; the four-way and two-way choice families are
decided by top-ranked option, which needs no cutoff. AUROC is the chance
that the model ranks a true item above a false one, so it does not depend on
where the cutoff sits, and 0.5 is chance. Brier and ECE score the
probabilities as stated, which is where Jev does badly. Compare Brier within
a row, never down the column: it is binary for the yes/no families and
multiclass for the choice families.

**Operations.** 2,641 queries (dev and test), zero failures, zero retries,
$0.447 total, $0.15 per 1,000 decisions. Latency 328 ms median and 471 ms at
p95, measured as wall clock from a machine in California, including the
network round trip to the service. Latency is published as indicative and is
not used to rank anything.

## Findings

### 1. Jev's probabilities never reach the decision boundary

Across all 611 patch-verdict items, the probability Jev gives for "this
patch passes" runs from 0.09 to 0.42, with a mean of 0.22. In reality 383 of
those patches, 62.7% of them, pass every hidden test. There is no input in
this suite for which Jev states that a patch is more likely to pass than
not.

The same compression shows on the regression question from the other side:
the probability that a patch breaks something runs from 0.24 to 0.80 with a
mean of 0.53, on a population where the true rate is 5.9%.

This is a calibration failure, and a large one. A model whose stated
probabilities are meant to be believed, asked about a class that occurs 63%
of the time, never assigns that class more than 0.42. The expected
calibration error of 0.409 on patch-verdict is the largest number in this
report.

### 2. Underneath the miscalibration, the ordering is informative

The ranking survives what the cutoff destroys:

| Family | AUROC |
|---|---|
| Does this patch pass every hidden test? | 0.690 |
| Does it break a test that passed before? | 0.652 |
| Which of four outcomes (macro, one against the rest) | 0.651 |
| Which of two patches is more maintainable? | 0.962 |

0.5 would be chance. So Jev does rank passing patches above failing ones,
modestly but well clear of noise, and it separates the style pairs almost
perfectly.

Ordering is not the same as usefulness. With a cutoff fitted on held-out
items, accuracy on the two yes/no families lands at 62.8% and 93.1%, against
floors of 62.7% and 94.1%. The ordering is real, but the classes are
imbalanced enough that acting on it gains nothing over answering the most
common label.

Measured against a frontier model given the same inputs, though, Jev's
ranking on the pass question is weak: GPT-6 Astra reaches an AUROC of 0.87
on the same 611 fixes (see [The control](#the-control)). It is also weaker
than the simplest possible heuristic. In this suite bigger fixes pass more
often (7% of the published fixes under 50 lines pass, 90% of those over 200), so ranking
fixes by lines changed alone scores an AUROC of 0.78, above Jev's 0.69.
Jev's stated confidence follows fix size closely, and among the 417 fixes of
50 to 199 lines, where size says little, its ranking falls to 0.56, near
chance, while Astra's holds at 0.81.

### 3. The four wordings agree with each other, and all of them are compressed

The same 134 development-split patches were asked the verdict question four
ways: the original wording, "this patch is a correct and complete fix", "a
careful maintainer would merge this", and an inverted form, "this patch is
broken".

| Wording | Mean probability of passing |
|---|---|
| Original | 0.220 |
| Correct and complete fix | 0.208 |
| A maintainer would merge it | 0.276 |
| Inverted (patch is broken) | 0.221 |

The four agree with each other on 96% to 100% of patches, and the inverted
form mirrors the original almost exactly. TypeSafe's consistency claim holds
up cleanly. Rewording moves the mean by at most 0.06 and never lifts the
distribution across 0.5, so the compression is a property of the model on
this task rather than an artifact of how the question was put.

### 4. It reads style well

On the 311 pairs where both code-quality judges preferred the same patch by
at least 10 points, Jev picked the same patch 89.7% of the time, with an
AUROC of 0.962 and a well-calibrated 0.082 expected calibration error. The
two obvious shortcuts do not explain it: picking the longer patch agrees
with the judges only 44.7% of the time, and Jev's A and B split (183 to 128)
tracks the true split (180 to 131), so there is no position bias.

Judges are opinion, so this is not a correctness result. It is still the
clearest positive signal in the suite, and it is the one place where Jev's
probabilities are both discriminating and honestly scaled. On the pass question,
by contrast, a frontier model ranks fixes far better than Jev does (see
[The control](#the-control)).

### 5. The localization probe is not a result

Jev picked the right file on 19 of 19 items. Then we checked the items: all
23 issues name the answer file's stem in their text, so the question is
string matching. It is kept as a sanity probe that the adapter is wired
correctly, and it is excluded from any claim about ability.

## The control

Every fix in this suite comes from a Frontier v4 task in which an agent
rebuilt a retired program, and the hidden tests check byte-for-byte
agreement with that program's real behaviour, including quirks the written
specification gets wrong. Jev is shown only the bug report and the fix, not
the specification, the retired program or its output. That raised a fair
question: can "does this fix pass every test?" be answered from those inputs
at all, or would any reader sit at the floor?

To find out, GPT-6 Astra at high effort was given exactly Jev's inputs
through Codex on a ChatGPT subscription, in an empty read-only directory with
no tools, and asked for a probability on every pass question. Its cutoff was
chosen on the 134 development-split questions and applied to the 611
published ones, the same procedure as Jev's. It made no tool calls and took a
median of 22 seconds per answer, against 0.3 seconds for Jev.

| Pass question, 611 published fixes | GPT-6 Astra | Jev 1.13.0 | Lines changed alone |
|---|---|---|---|
| Always guessing | 62.7% | 62.7% | 62.7% |
| Ranking (AUROC, 95% interval) | **0.87** (0.84 to 0.89) | 0.69 (0.65 to 0.73) | 0.78 (0.75 to 0.82) |
| Accuracy at a development-fitted cutoff | **72.5%** (cutoff 0.20) | 62.8% (cutoff 0.17) | 65.1% (127 lines) |
| Accuracy at the plain 50% line | 67.1% | 37.3% | |
| Stated probability, lowest to highest | 0.00 to 0.95 | 0.09 to 0.42 | |
| Mean stated probability (true rate 62.7%) | 0.34 | 0.22 | |

Ranking AUROC within each fix size:

| Fix size | Fixes | Share that pass | GPT-6 Astra | Jev | Lines changed alone |
|---|---|---|---|---|---|
| Under 50 lines | 68 | 7% | 0.80 | 0.45 | 0.89 |
| 50 to 199 lines | 417 | 64% | 0.81 | 0.56 | 0.65 |
| 200 lines or more | 126 | 90% | 0.96 | 0.88 | 0.64 |

Four things follow.

- **The question is answerable.** A frontier model beats always guessing by
  about 10 points from the same text. The hidden quirks limit how well any
  reader can do, but they do not make the task impossible.
- **Jev's shortfall is real, and it is ranking as well as calibration.** The
  ranking gap, 0.18 AUROC, has a bootstrap interval of 0.13 to 0.22. Within
  each of the 19 tasks the gap holds (item-weighted AUROC 0.91 for Astra
  against 0.73 for Jev), so it is not an artifact of some tasks being easier.
- **Fix size explains most of Jev's ranking, and none of Astra's.** Ranking
  by lines changed alone beats Jev overall (0.78 against 0.69). In the middle
  size band, which holds two thirds of the fixes, Jev is near chance (0.56)
  and Astra is not (0.81). Jev does rank the largest fixes well (0.88), but
  only 13 of those 126 fail, so that figure rests on few cases.
- **Astra is under-confident too, but usable.** Its average stated chance
  that a fix passes is 34% where 63% do, yet its answers do cross 50%, and at
  that plain line it still beats guessing, 67.1% against 62.7%. Jev's never
  do.

On the 134 development fixes the gap looked smaller (AUROC 0.82 against
0.76). The development split covers only 4 tasks; the published split covers
19, and it is the one reported here.

## What this means

Jev cannot be dropped in as a merge gate or a continuous-integration
pre-filter on the strength of its own probabilities. Read literally, it says
every patch is more likely to fail than pass, which would block everything.
Read as a ranking, with a cutoff fitted on your own labelled data, it comes
out about level with answering "it passes" every time. A frontier model given
the same inputs beats guessing by about 10 points and ranks fixes far better
(see [The control](#the-control)), so the shortfall is Jev's, not the test's.
Even the modest ranking Jev has is mostly available for free: sorting fixes
by how many lines they change does better.

Where the judgment is about the shape of the code rather than its behaviour,
the picture changes: 89.7% agreement with a calibrated judge panel, AUROC
0.962, at 328 ms and $0.15 per 1,000 decisions. That is roughly the cost and
latency of a database query, for a judgment that currently costs a model
call.

The methodological point is the one this report had to learn the hard way. A
model that returns probabilities can be self-consistent, well ordered and
badly scaled all at once, and which of those you see depends entirely on the
metric. Accuracy at a fixed cutoff measures the scaling and hides the
ordering. AUROC measures the ordering and hides the scaling. Reporting one
without the other produces a confident, wrong headline, which is what the
first version of this report did.

## What changed in the correction

Withdrawn:

- **"Jev scores 40.8% against a 72.4% floor."** That was accuracy at a 0.5
  cutoff the model never crosses. At a fitted cutoff it is 68.9% against the
  same floor. Both numbers are in the data file; the fitted one is the fair
  comparison, and Jev is still below the floor, by 3.5 points rather than 32.
- **"Accuracy falls as the patch gets longer."** It does not. The share of
  patches that pass rises with size (54%, 89%, 90% across the three buckets),
  and at a fitted cutoff Jev's accuracy in each bucket equals that bucket's
  majority baseline exactly. The original finding was the class balance
  moving, seen through a broken decision rule.

Retained, with the framing corrected:

- The compression of the probabilities, now reported as the calibration
  failure it is rather than as "it called every patch broken".
- The four-wording consistency result.
- The style-pair result and the localization probe.

Changed in the suite itself: the scorer now reports AUROC and the range of
probabilities the model actually produced, fits yes/no cutoffs on the
development split, and prints the majority baseline beside every subgroup.
Everything was recomputed from the stored predictions, so the correction
cost no further queries.

## Method

- **Items.** 2,641 items mined from 745 distinct agent patches produced
  during seven completed Frontier v4 sweeps, across 23 tasks. Contaminated
  runs, empty patches and duplicate patches are excluded. Splits are by task,
  so no task appears in both dev and test.
- **Answers.** The verifier's own record for that patch (which target tests
  passed, whether anything regressed) for the three patch families, the gold
  patch for localization, and the code-quality panel for the style pairs.
- **Model.** `jev-1.13.0`, pinned rather than the `jev-latest` alias, because
  the alias moves and calibration is per version. Jev exposes no effort,
  temperature or seed setting, so it gets one column. A future version is a
  new column, never an overwrite of this one.
- **Scoring.** AUROC, accuracy at the top-ranked answer, accuracy at a
  development-fitted cutoff, the majority-label share, Brier, log loss,
  expected calibration error on 10 bins, the range of probabilities produced,
  plus latency and cost per 1,000 decisions. Items a model declines to answer
  count as wrong.
- **Privacy.** The item file embeds Frontier v4 issues and agent patches. It
  stays private and canaried; this directory carries metrics only.

## Limits

- The three patch families ask different questions about the same 745
  patches, so their errors are correlated. Any aggregate over them should be
  bootstrapped by patch, not by item.
- The fitted cutoffs come from 134 development items per family. They are
  held out from the published split, but they are not many, and a different
  development sample would move them.
- Apart from the control on the pass question, no other model has been run
  on these items. In particular, an AUROC of 0.962 on style is not yet known
  to be better or worse than what a frontier model would score on the same
  pairs.
- The control is one frontier model at one effort level on one question. It
  shows the pass question is answerable from the inputs and gives a
  reference point for Jev; it is not a leaderboard entry for Astra.
- Every item comes from one benchmark suite, whose tasks skew toward
  binary-parity reimplementation work in Python. The patch population is not
  a sample of open-source pull requests in general.
- Latency was measured from California against a service hosted on the US
  West Coast. Anyone further away should expect worse.

## Reproducing

```bash
python scripts/verdict-v1/build_items.py          # mine items from local runs
python scripts/verdict-v1/preflight.py            # 10 items, checks the wiring
python scripts/verdict-v1/run_typesafe.py         # the full pass, resumable
python scripts/verdict-v1/probe_phrasing.py       # the four-wording probe
python scripts/verdict-v1/export_results.py       # the JSON beside this file
python scripts/verdict-v1/make_jev_card.py        # the card
```

Needs `TYPESAFE_API_KEY` in `.env`. The rules this suite runs under are
recorded in [docs/DECISIONS.md](../../DECISIONS.md), under 2026-09-19 and the
scoring correction under 2026-09-23.
