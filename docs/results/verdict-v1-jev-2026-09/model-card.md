# VulcanBench Verdict v1: Jev, the model that will not say a patch works

**September 22, 2026 · VulcanBench Verdict v1 · jev-1.13.0 · 2,163 test items · 745 agent patches · 23 tasks · $0.45**

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

## Results

Test split, one query per item, model pinned to `jev-1.13.0`.

| Question | Items | Majority floor | Jev | Brier (Jev / floor) | ECE |
|---|---|---|---|---|---|
| Does this patch pass every hidden test? | 611 | 62.7% | **37.3%** | 0.383 / 0.238 | 0.409 |
| Does it break a test that passed before? | 611 | 94.1% | **33.4%** | 0.284 / 0.063 | 0.268 |
| Which of four outcomes did the tests report? | 611 | 62.7% | **49.8%** | 0.650 / 0.522 | 0.186 |
| **Overall, ground truth only** | **1,852** | **72.4%** | **40.8%** | **0.434 / 0.280** | **0.278** |
| Which of two patches is more maintainable? * | 311 | 57.9% | **89.7%** | 0.162 / 0.500 | 0.082 |

\* Reference is agreement with the Muse and Grok code-quality panel, which is
opinion, not ground truth, so this line is reported apart and excluded from
the overall row.

The majority floor is the score for always answering a family's most common
label, fitted on the dev split. Unanswered items would count as wrong; none
were unanswered. Brier is binary for yes/no families and multiclass for
choice families, so compare it within a row, never down a column.

**Operations.** 2,641 queries (dev and test), zero failures, zero retries,
$0.447 total, $0.15 per 1,000 decisions. Latency 328 ms median and 471 ms at
p95, measured as wall clock from a machine in California, including the
network round trip to the service. Latency is published as indicative and is
not used to rank anything.

## Findings

### 1. Jev said no to every patch

On all 611 patch-verdict items, Jev's probability that the patch passes came
out below 0.5. Its mean probability was 0.22. In reality 383 of those 611
patches (62.7%) pass every hidden test. The model is not guessing and it is
not noisy: it holds a confident prior that agent patches are broken and
nothing in the diff moves it.

The regression question shows the same bias from the other side. Jev answered
"this breaks an existing test" on 435 of 611 patches. Thirty-six of those
really do regress. That is 403 false alarms against 32 true ones.

This is what pushes Jev below the majority floor on every ground-truth
family. A model that always answered "passes" would score 62.7%. Jev scores
37.3%.

### 2. It is consistently wrong, not randomly wrong

We asked the same 134 dev-split patches the verdict question four ways: the
original wording, "this patch is a correct and complete fix", "a careful
maintainer would merge this", and an inverted form, "this patch is broken".

| Wording | Says the patch passes | Mean probability of passing |
|---|---|---|
| Original | 0% | 0.22 |
| Correct and complete fix | 1% | 0.21 |
| A maintainer would merge it | 4% | 0.28 |
| Inverted (patch is broken) | 0% | 0.22 |

The four wordings agree with each other on 96% to 100% of patches, and the
inverted form mirrors the original almost exactly (0.22 passing against 0.78
broken). TypeSafe's consistency claim holds up cleanly. The pessimism is a
property of the model, not an artifact of how the question was phrased.

### 3. Accuracy falls away as the patch gets longer

Patch-verdict accuracy by input size:

| Input tokens | Items | Accuracy |
|---|---|---|
| 0k to 4k | 462 | 45.7% |
| 4k to 8k | 130 | 11.5% |
| 8k and above | 19 | 10.5% |

Short patches leave room for the prior to be right by accident. Long ones do
not. Reading a large diff against an issue and deciding whether the change is
complete is System Two work, and this is a model that has given that up by
design.

### 4. It reads style well

On the 311 pairs where both code-quality judges preferred the same patch by
at least 10 points, Jev picked the same patch 89.7% of the time, with a
well-calibrated 0.082 expected calibration error. The two obvious shortcuts
do not explain it: picking the longer patch agrees with the judges only 44.7%
of the time, and Jev's A and B split (183 to 128) tracks the true split (180
to 131), so there is no position bias.

Judges are opinion, so this is not a correctness result. It is still the
clearest positive signal in the suite: Jev can read the surface of code
(naming, structure, comments, duplication) even though it cannot evaluate
behaviour.

### 5. The localization probe is not a result

Jev picked the right file on 19 of 19 items. Then we checked the items: all
23 issues name the answer file's stem in their text, so the question is
string matching. It is kept as a sanity probe that the adapter is wired
correctly, and it is excluded from any claim about ability.

## What this means

Two conclusions, and they point in opposite directions.

For anyone thinking of using a System One model as a merge gate or a CI
pre-filter, the answer from this suite is no. At these numbers Jev would
block nearly every correct patch, and its confidence would give no warning,
because it is confidently wrong rather than uncertain.

For anyone using it where the judgment is about the shape of the code rather
than its behaviour, the 89.7% agreement at 328 ms and $0.15 per 1,000
decisions is worth a look. That is roughly the cost and latency of a database
query, for a judgment that currently costs an LLM call.

The wider point is about how this model class gets evaluated. Jev is
self-consistent, fast, cheap, and cannot produce a malformed answer. None of
those properties is correctness, and the first of them can look like
correctness on a benchmark that scores agreement rather than truth. A
reference the model cannot argue with, in this case a test suite, separates
the two.

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
- **Scoring.** Accuracy, Brier, log loss and expected calibration error on
  10 bins, plus latency and cost per 1,000 decisions. Items a model declines
  to answer count as wrong.
- **Privacy.** The item file embeds Frontier v4 issues and agent patches. It
  stays private and canaried; this directory carries metrics only.

## Limits

- The three patch families ask different questions about the same 745
  patches, so their errors are correlated. Any aggregate over them should be
  bootstrapped by patch, not by item.
- No other model has been run on these items yet, so the reference points
  here are the majority floor and the judge panel. In particular, 89.7%
  agreement on style is not yet known to be better or worse than what a
  frontier LLM would score on the same pairs.
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
recorded in [docs/DECISIONS.md](../../DECISIONS.md) under 2026-09-19.
