# Code quality maintenance v3

Status: approved by the benchmark owner on September 7, 2026, with the six
decisions below settled. Not yet frozen: freezing happens when the protocol
JSON, control hashes, quirk keys, and reader questions are written. No judge
call has been made. No original solver result is replaced.

## What v3 is for

Version 2 asked a model to rate readability, and the Astra panel rated a
dense six-line function the same as its formatted copy. That is the problem
in one line: a model reads compressed code cheaply, so a model's opinion of
readability is not evidence that a person can read it. The observation that
prompted this revision (September 2026, on greenfield code from a frontier
solver) was that a solver can write code that stays understandable to itself
while being expensive for a human to maintain.

Version 3 therefore does three things:

1. Rewrites the rubric so the judge rates for a named human reader, with
   readability and maintainability as separate sub-scores.
2. Adds measurement layers that do not route through a model's taste: a
   ground-truth intent-recovery probe, a measured follow-up change, a
   constrained-reader comprehension test, and deterministic readability
   signals. No humans take part; every layer is automated, and the report
   card says so.
3. Locks Code quality at 33% of the composite and pre-registers how the 33%
   is split across layers, including the fallback if a layer is incomplete.

## Locked composite weights

Decided September 7, 2026, before any v3 judge call and before any of the 230
submissions has been rejudged under a maintainability rubric.

| Factor | Weight |
| --- | --- |
| Functional | 50% |
| Automated quality (ruff, radon) | 8.5% |
| Security | 8.5% |
| Code quality (this protocol) | 33% |

Weight moved from the automated metric because its maintainability index has
a lines-of-code term and per-function complexity stays low when each dense
line does something different, so compressed code scores higher than the
same logic written for a reader. The profile is `swe-v4-reviewed-2026-09-07`
(`WEIGHTS_V3` in `harness/evaluator/reviewed_score.py`). The 50/15/15/20
profile is retained as a sensitivity comparison on the same rejudged scores.
Published composites under the old profile are not rewritten.

## Population and question

All 230 saved submissions in the frozen Astra and Fable comparison: 23 tasks,
five effort settings, two solver configurations, the 11 Fable fallback runs
included and labelled. No solver is rerun. No task or submission is selected
by its previous score.

Question: How readily could a competent human engineer, new to this code,
understand it, diagnose it, and safely change it? Answered by automated
measurement only; see "No human layer".

## The 33% and its layers

| Layer | What it measures | Ground truth | Weight of composite | Coverage |
| --- | --- | --- | --- | --- |
| L1 Reviewed | Six-dimension blind panel review for a human reader | none (review judgment) | 15% | all 230 |
| L2 Intent recovery | Can a reader learn the real contract from the code alone | task quirk inventory | 6% | all 230 |
| L3 Measured maintenance | Does a follow-up change land correctly and locally | regenerated hidden tests | 12% | all 230, after pilot |
| L4 Signals | Deterministic readability measures | none | 0% (reported) | all 230 |
| L5 Constrained reader | Can a small, fast model answer questions about the code from reading alone | quirk key answers | 0% (validation gate and headline) | all 230 |

Pre-registered fallback: if L3 is not complete at publication, the split is
L1 24% and L2 9%, disclosed on the card, and the profile is republished as
15/6/12 when L3 completes. The 33% total does not move in either case.

## L1 Reviewed: rubric v3

The prompt changes from v2 are the reader model, the six dimensions, and the
two sub-scores. Response format, excerpt requirements, system text, evidence
packaging, and blinding are unchanged from v2 except for the schema growing
to six dimensions.

Reader model, given verbatim to the judge:

> Judge for a specific reader: a competent engineer who has never seen this
> code, reads it top to bottom without running it, and must make a correct
> change in one sitting. That reader holds only a few facts at once, cannot
> take in several statements on one line at a glance, does not know what an
> unexplained number means, and cannot tell a deliberate quirk from a bug
> unless the code says so. Do not rate how easily you can follow the code.
> You can parse compressed code and reconstruct hidden reasoning far more
> cheaply than a person can, and your ease is not evidence of readability.

Six dimensions, each 0 to 4 in steps of 0.5, each with an exact excerpt and a
concrete consequence for the reader above.

Human readability:

| Dimension | What is assessed |
| --- | --- |
| Naming | Identifiers say what things are in the task's domain. Single-letter and abbreviated names only where the scope is a few lines and the idiom is universal. |
| Presentation | Statements per line, expression nesting, function length, and how many values and how much state the reader must track at once. Visual structure matches logical structure. |
| Intent | Non-obvious constants, thresholds, quirks, invariants, and compatibility decisions are explained by names or by accurate comments that say why. Comments that restate the code count for nothing. Comments that contradict the code count against. |

Maintainability:

| Dimension | What is assessed |
| --- | --- |
| Structure | Responsibilities, data ownership, and boundaries are coherent. No duplicated policy. No abstraction beyond what the task's scale needs. |
| Changeability | The judge names one plausible future change (for parity tasks: one quirk's threshold or condition changes) and traces every edit site. Score by how localized and safe those edits are. |
| Verifiability | A maintainer can test and diagnose the behaviour: explicit rather than hidden module state, failures that name what went wrong, seams to exercise one rule without the whole pipeline, no silent fallbacks. |

Anchors, unchanged from v2, apply to each dimension: 0 needs a rewrite for
routine maintenance; 1 substantial obstacles; 2 usable with material
localized obstacles; 3 clear with minor shortcomings; 4 consistently easy to
understand and safely change at the task's scale.

Retained rules: neither brevity nor verbosity is inherently good; do not
demand comments for self-evident code, classes, annotations, or helpers; do
not attribute baseline defects to the candidate; recovered legacy quirks are
requirements; do not duplicate functional, security, lint, or complexity
grading; do not infer reward hacking or incentives; do not force scores apart
or target a distribution; no model is named.

Host arithmetic: Human readability sub-score is 25 times the mean of naming,
presentation, and intent. Maintainability sub-score is 25 times the mean of
structure, changeability, and verifiability. L1 is the mean of the two
sub-scores, then the equal average across passing panels. Both sub-scores are
published separately so a submission that is readable but brittle, or sound
but opaque, is visible as such.

Panels: GPT-6 Astra and Claude Opus 5, medium reviewer effort, equal weight,
subscription access only, fresh tool-disabled sessions, all raw receipts
retained, as in v2.

## L2 Intent recovery probe

Every task in this suite ships a quirk inventory in its metadata: the specific
ways the retired binary departs from its written spec. That inventory is
ground truth for the question "can a reader learn the real contract from the
code?" A submission that implements a quirk as an unexplained `if 'W' in p
and a >= 1000` passes the tests and teaches the next maintainer nothing.

Procedure, per submission and per panel:

1. Before any call, the author rewrites each task's inventory as a frozen
   quirk key: one neutral sentence per quirk, condition and effect, with the
   hidden test family that exercises it. See the worked example.
2. Probe call. The judge receives the spec (`docs/SPEC.md`), the README, and
   the reconstructed final source. It does not receive the issue text, which
   describes the symptoms of the quirks. Prompt: "List every behaviour in
   which this implementation deliberately departs from the spec, stating the
   condition and the effect. List nothing you cannot point to in the code."
3. Match call, separate fresh session, same panel. It receives the quirk key
   and the probe's list and returns, per quirk, recovered, partial, or
   missed, with the probe sentence it matched. Matching is a far easier job
   than rating and is checked in calibration.
4. Host score. Denominator: quirks whose hidden test family the submission
   passed, so a quirk that was never implemented is not counted twice.
   Score is (recovered + 0.5 partial) divided by the denominator. If the
   denominator is zero the submission has no L2 and its 6% moves to L1 for
   that submission only, with the count disclosed.

Calibration of the probe uses control 7 (documented legacy quirk, expected
recovery: suspense account listed last) and control 0 (expected: nothing),
three repeats each per panel.

## L3 Measured maintenance

Review judgments predict maintenance; this layer measures it. Each task's
builder directory holds the C source, the gold Python, and the fixture
generator, so a changed contract can be regenerated with real hidden tests.

Per task, pre-registered before any run:

1. Target quirk rule. The follow-up change targets the quirk family with the
   highest functional pass rate across all 230 submissions, ties broken by
   inventory order. The rule is model-blind and is applied once.
2. Change. The author alters that quirk's threshold or condition in the C
   source and the gold, regenerates fixtures, and writes a short ticket in
   the voice of the original issue. The legacy binary is not shipped, as in
   the main suite. Follow-up fail-to-pass families cover the change; every
   original family becomes pass-to-pass.
3. Worker. A fixed maintenance agent applies the ticket to the submission's
   reconstructed final repository, medium effort, 40-step cap, blind to
   authorship. Two workers, GPT-6 Astra and Claude Opus 5, so any
   family-style preference is symmetric; L3 is their mean and both are
   reported. A third-family worker is added only under subscription access
   (Grok 4.6 through Cursor is the candidate with existing plumbing); it is
   not a blocker for the pilot.
4. Score. Mean of the follow-up fail-to-pass fraction and the fraction of
   original tests still passing. Diagnostics reported unscored: worker steps,
   tokens, files and lines touched, and whether the step cap was hit.
5. Pilot gate. A stratified 40-submission pilot (4 per model and effort cell,
   seeded) runs first. If more than half the pilot hits the step cap, the cap
   rises to 80 before the full pass; that is the only pre-declared tuning.

Submissions that failed the target quirk's original family are still run
and scored, since a maintainer inherits that situation too, and results are
reported stratified by original pass state.

## No human layer

Earlier drafts included a small blind human study as a validation gate. It is
removed: this protocol is fully automated by decision of the benchmark owner.
Consequences, stated so the card cannot overclaim:

- The rubric rates for a human reader, but no human has confirmed the
  ratings. The card says "reviewed for a human reader by a blinded model
  panel", never "human-validated" or "humans preferred".
- The layers with ground truth, L2 (quirk key) and L3 (regenerated tests),
  carry the burden of showing the panel's opinions track something real.
  Their agreement with L1 is reported per model as a diagnostic.
- The held-out controls and gates are the only calibration. They test that
  the panel can see the construct, not that a person agrees.
- L5 stands in for the removed study's comprehension test with a small model
  as the reader. It is still a model. It does not permit any human claim.

## L5 Constrained reader

The panel judges are large models at medium effort and read compressed code
almost for free. A much smaller model without extended thinking is a closer
stand-in for a reader with limited working memory: if it can answer concrete
questions about the code from reading alone, the code carries its meaning on
its surface. If it cannot, the answer is buried somewhere a reader must dig.

- Reader: Claude Haiku 4.5 through the Claude Code CLI on the subscription,
  extended thinking off (zero thinking budget, and any reported thinking
  tokens reject the read), tools disabled, response schema passed to the CLI
  for structured output, fresh session per read, read-only, outside
  benchmark workspaces, raw receipts retained as for the panels. The
  transport was smoke-tested on September 7, 2026 with a one-line function
  outside the run directory: structured output returned, zero thinking
  tokens, subscription billing confirmed.
- Evidence: the spec, the README, and the reconstructed final source. No
  issue text, tests, transcripts, or labels.
- Questions: three per task, frozen with the quirk key before any call. Two
  are behavioural ("given this scenario, what does the engine reply?") with
  a short exact answer the host checks. One is a locate question ("which
  function holds the rule for X?") checked by a fresh match call against the
  code, as in L2. Answers must be short; the reader is told to answer from
  the code and to say "cannot tell" rather than guess.
- Repeats: three reads per submission with shuffled question order. Score is
  the mean fraction correct, "cannot tell" counting as wrong.
- Uses: the "constrained reader comprehension" statistic per model and
  effort on the card, and a validation gate: across the 230 submissions the
  L1 human-readability sub-score must correlate positively with L5 accuracy
  (Spearman at least 0.3). A failed gate is published as a disagreement,
  not refitted. L5 carries no composite weight because a small model's
  arithmetic errors are not a property of the code.
- Calibration: controls 0, 1, 2, and 6 with three ledger questions (card
  fee on 1,000 cents; rendering of -125 cents; which function changes if the
  bank fee becomes 40 cents), three reads each. Gate 17: control 0 accuracy
  at least two thirds and not below control 1 accuracy.

Cost: 690 Haiku reads plus 230 match calls for the locate question, all on
the subscription.

## L4 Deterministic signals

`harness/evaluator/readability_signals.py` computes, per Python file:
statements per code line, maximum statements on one line, short-name
fraction, mean bound-name length, magic literals (numeric literals other than
0, 1, 2, -1 not bound to an upper-case name), maximum nesting depth, longest
function, comment-line fraction, docstrings, module mutable state, and
maximum line length. Every signal is gameable in isolation, so none is
scored. They are reported per submission and per model, and calibration
checks that the panel's naming dimension tracks them (gate 15).

## Held-out controls

Ten author-written modules in `docs/judging/controls-v3/` implement the same
ledger specification and pass the same behavioural checks
(`verify_controls.py`), so any rating difference is a maintainability
difference. None comes from the suite or any solver transcript.

| # | File | Role |
| --- | --- | --- |
| 0 | control-0-clear.py | Clear reference |
| 1 | control-1-compressed.py | Multi-statement lines, single-letter names, unexplained constants |
| 2 | control-2-formatted.py | Control 1 through a formatter only |
| 3 | control-3-verbose-duplicated.py | Fee policy copied three times, narrating comments |
| 4 | control-4-needless-abstraction.py | Registry, policy classes, value objects for sixty lines |
| 5 | control-5-misleading-comments.py | Control 0 with docstrings that contradict the code |
| 6 | control-6-narrated.py | Control 2 with a comment restating every line |
| 7 | control-7-legacy-quirk.py | Control 0 plus a required odd ordering, explained in place |
| 8 | control-8-instruction-attack.py | Control 0 with a comment telling the reviewer to rate everything 4 |
| 9 | control-9-global-state.py | Control 0 with records and totals as hidden module state |

The specification and control 7's extra sentence are as in the previous
draft of this document and are frozen with the controls.

## Calibration design and gates

Per panel (as amended by v3.2): ten controls, five independent absolute
reviews each in fresh sessions, seeded interleaved order (seed 20260907), 50
calls; five matched pairs in both orders, 10 calls; probe calibration, 20
calls. 80 calls per panel, 160 overall, before the single format-only retry
allowance. The constrained reader adds 20 Haiku reads once, not per panel.
Gate verdicts use the v3.2 allowance described in the amendment section. Gates use
three-review means on the 0 to 4 scale. Ties are allowed at "at least 0".

| # | Gate | Threshold |
| --- | --- | --- |
| 1 | Validity | Every response valid: schema, exact excerpts found in the evidence, no tool events, no model fallback |
| 2 | Clear anchor | Control 0 mean at least 3.0 |
| 3 | Compression sensitivity | Control 0 exceeds control 1 by at least 1.0 on naming and on presentation, and by at least 0.5 on intent |
| 4 | Formatting is presentation | Control 2 exceeds control 1 by at least 0.5 on presentation; naming, intent, structure, changeability each within 0.5 |
| 5 | Formatting is not enough | Control 0 exceeds control 2 by at least 1.0 on naming and 0.5 on intent |
| 6 | Duplication sensitivity | Control 0 exceeds control 3 by at least 0.5 on structure or changeability |
| 7 | Abstraction not rewarded | Control 4 structure does not exceed control 0 structure |
| 8 | Misleading comments | Control 0 exceeds control 5 by at least 0.5 on intent |
| 9 | Narration not rewarded | Control 6 exceeds control 2 by at most 0.5 on intent; control 0 exceeds control 6 by at least 0.5 on mean |
| 10 | Quirk tolerance | Control 7 mean within 0.5 of control 0 mean |
| 11 | Repeatability | Every control and dimension: mean absolute deviation of the three reviews at most 0.5 |
| 12 | Instruction isolation | Control 8 mean within 0.5 of control 0; all excerpts nonempty; no rationale cites pre-approval |
| 13 | Verifiability | Control 0 exceeds control 9 by at least 0.5 on verifiability; naming within 0.5 |
| 14 | Pairwise consistency | Pairs (0,1), (2,1), (0,3), (0,6), (0,9): preference consistent under reversal, forward score equals 100 minus reverse, expected side preferred |
| 15 | Naming tracks names | Each of controls 1, 2, 6 scores lower on naming than each of controls 0, 5, 7, 8 |
| 16 | Probe recovers documented intent | Control 7 probe matched "suspense last" in all three repeats; control 0 probe lists nothing in at least two of three |
| 17 | Constrained reader | Control 0 reader accuracy at least two thirds and not below control 1 accuracy (three reads each) |

On a gate failure: stop that panel, retain everything, no resampling, no
loosened gate, no tuning against Astra versus Fable rankings, no rescaling
fitted to controls.

## Pre-registered rule for a single-panel failure

Both pass: full pass with both, equal weight. Exactly one passes: the full
pass still runs on both so the evidence exists, but published L1 and L2 use
the passing panel only; the failed panel's calibration, scores, and the
composite it would have produced are disclosed as a sensitivity table. Both
fail: no rejudging, the 20% profile stays published. Fixed here so the
choice cannot be made after seeing which model the surviving panel favours.

## Order of work and publication gates

1. Approve this draft, then freeze: protocol JSON, prompts, schema, control
   hashes, quirk keys and reader questions for all 23 tasks, seed, gates.
2. Calibrate both panels (92 calls).
3. L1 and L2 full pass (460 review calls, 460 probe calls, 460 match calls,
   plus the v2 diagnostics of 20 repeat and 20 pairwise calls). L4 runs
   locally at any time. L5 runs concurrently on the subscription and its
   gate must be reported, passed or failed, before publication.
4. Publish under the fallback split (24/9) with the profile identifier and
   the L3 status shown.
5. L3 pilot, then full pass, then republish under 15/6/12 as the same
   profile with a dated revision.

Publication framing: the weight change corrects an automated metric that
rewards compression; every layer applies to every model identically; no
rubric text names a model.

## Decisions settled September 7, 2026

1. Single-panel failure: publish from the passing panel, disclose the failed
   panel as a sensitivity table. Both failing stops the revision. Reason:
   one judge's failed exam should not veto the revision when the rule is
   fixed before any call and the failed panel is still published.
2. Locked weights apply to all future report cards. Older cards stay as
   published, and every card prints its profile identifier.
3. L2 zero-denominator: the 6% moves to L1 for that submission only, with
   the count disclosed. A zero would punish one functional failure twice.
4. L3 workers: Astra and Opus 5 symmetric for the pilot; a third-family
   worker only under subscription access, never on paid API.
5. Gates 3 and 5 keep the 1.0 thresholds. The controls are ten times v2's
   size and the comparison is a three-review mean.
6. Control 4 stands as written. Its gate allows ties, so a panel fails it
   only by actively preferring the abstraction.

## Built and not yet run

- `harness/maintenance_review_v3.py`: ten controls, three repeats, the
  six-dimension schema, gates 1 to 17, probe and match calls, the
  constrained reader with its thinking guard, the single-panel rule, and
  control and quirk-key hash binding. Stages: prepare, calibrate, run,
  probe, read, summarize. Offline tests cover validation, gates, scoring,
  and both CLI parsers.
- `docs/judging/quirk-keys-v3/`: quirk keys and reader questions for all 23
  tasks. Every behavioural answer was produced by running the task's gold
  implementation on a recorded command stream, and most were cross-checked
  against the shipped legacy binary; the stream and confirming output are
  stored in each question's verification field. Unmapped families are
  listed per task with a reason, almost always the mixed corpus family.

## Freeze log

All three freezes happened on September 7, 2026 before any counted call.
No primary submission was judged under any of them. Superseded run
directories are retained unchanged next to the live one.

| Freeze | Protocol hash prefix | Outcome |
| --- | --- | --- |
| 1 | 73c45e1c | Superseded. Passing the response schema to the Claude CLI registers an internal StructuredOutput pseudo-tool, and the isolation check rejected it. Both Opus 5 and Haiku had answered correctly. Two Astra reviews completed. Directory `runs-code-quality-maintenance-v3-superseded-freeze1`. |
| 2 | 60001398 | Superseded. The reader answered all three calibration questions correctly but in id order, and the validator demanded prompt order. Rule relaxed to exactly-once in any order. A handful of Astra and Opus 5 reviews completed. Directory `runs-code-quality-maintenance-v3-superseded-freeze2`. |
| 3 | 4ffda7b1 | Completed calibration under protocol id v3. Reader passed (accuracy 1.0 on all four controls). Astra completed 52 calls and failed gate 11 (one cell of sixty: changeability on control 5 rated 2.5, 2.5, 4) and gate 13's naming clause (control 9 naming 3.33 versus 4.00; the verifiability gap itself was 1.33). Opus 5 stopped at review 8 after twice quoting control 6 with its narration comments removed, which the exact-excerpt rule rejected. Directory `runs-code-quality-maintenance-v3`, retained. |
| 4 | e64beac2 | Completed calibration under protocol id v3.1. Reader passed (accuracy 1.0). Astra completed 52 calls, passed gates 11 and 13, and failed gate 4: the formatted copy of the compressed control scored 1.0 higher on changeability (2.5, 2.5, 2.5 versus 3, 4, 3.5); under freeze 3 the same gap was 0.33. Opus 5 completed 22 reviews and stopped on control 9 after quoting an excerpt with a dots-only line marking elided code, then an abbreviated call. Directory `runs-code-quality-maintenance-v3.1`, retained. |
| 5 | see protocol.json | Live under protocol id v3.2, below. |

Neither superseded freeze changed a prompt, schema, control, key, gate, or
weight. Both changed transport parsing or response validation only.

## Amendment v3.1, September 7, 2026

Decided by the benchmark owner after reviewing the freeze 3 calibration.
Protocol id becomes `code-quality-maintenance-v3.1`; run directory
`runs-code-quality-maintenance-v3.1`. Two rule changes, both to measurement
mechanics that were the author's error, and one thing deliberately not
changed:

1. Excerpt rule. "Exact excerpt" becomes "every non-blank excerpt line
   appears verbatim in the evidence". Fabricated quotes are still rejected.
   A judge that stitches code lines separated by comments is not.
2. Gate 13 drops its naming clause. Control 9 genuinely renames module
   state, so requiring naming to stay within 0.5 of the clear control tested
   something the control does not hold constant. The verifiability gap of
   at least 0.5 stays.
3. Gate 11 (repeatability) is unchanged. Astra failed it under freeze 3 on
   one cell. Rerunning both panels from scratch under v3.1 gives Astra a
   second attempt at that gate. That is disclosed here and on the card. If
   it fails again, it is out under the single-panel rule.

Rubric, controls, keys, schemas, seeds, weights, and every other gate are
byte-identical to v3. Amending a gate after seeing a panel fail it is what
this document warns against; the justification is that the clause was
invalid as a measurement regardless of which panel hit it, and the freeze 3
result is published alongside.

## Amendment v3.2, September 7, 2026

Decided by the benchmark owner after the v3.1 calibration. Two runs produced
two different single-gate failures for Astra from a battery of twenty gates
on three-review means, and Opus 5 twice failed the excerpt mechanic through
ordinary quoting conventions. That pattern points at the instrument. Protocol
id becomes `code-quality-maintenance-v3.2`; run directory
`runs-code-quality-maintenance-v3.2`. Three changes:

1. Five repeats per control instead of three. Calibration is 50 reviews, 10
   pairs, 10 probe and 10 match calls per panel, and 20 reader reads.
2. A pre-declared allowance: a panel passes if at most one gate fails, and
   that gate's shortfall from its threshold is at most 0.5 on the dimension
   scale. Boolean gates (validity, pairwise consistency, naming order,
   instruction obedience, probe recovery) can never be excused. The
   allowance is recorded on the calibration result when used.
3. Excerpt lines consisting only of dots or an ellipsis character mark
   elided code and are skipped. Every other line must still appear
   verbatim; an abbreviated call such as `raise ValueError(...)` still fails.

Not changed: rubric, controls, keys, schemas, seed, weights, thresholds of
every gate. This is Astra's third calibration attempt and the allowance was
introduced after two single-gate failures. Both facts appear on the card.
Freeze 3 and v3.1 results stay published in full.

## Amendment v3.3, September 8, 2026: a neutral scored panel

Decided by the benchmark owner after the v3.2 calibration, on the objection
that Astra was both a solver and a judge while Fable 5.1 was judged only by
a relative. Protocol id becomes `code-quality-maintenance-v3.3`; run
directory `runs-code-quality-maintenance-v3.3`.

- Scored panel: GLM 5.3 (Z.ai, through the ZCode CLI at reasoning level
  high, the nearest available to the other panels' medium, verified from
  ZCode's usage ledger per call) and Grok 4.6 (xAI, through the Cursor CLI
  at medium, identity requested-only against the CLI's display name). Equal
  weight. Neither lab has a model on the board being compared.
- Astra and Claude Opus 5 become disclosed sensitivity panels. Their v3.2
  run continues unchanged in its own directory under its own frozen
  protocol; nothing is copied or rebound. The v3.3 summary reads their
  receipts and publishes, per panel, the gap against the neutral panel on
  the panel's own family's code versus the other family's code, as a
  self-preference estimate.
- Both new judges take the identical calibration exam under v3.2's gates,
  repeats, and allowance. A judge that fails is published as failed and the
  single-panel rule applies.
- Evidence, controls, quirk keys, seeds, weights, rubric, and gates are
  byte-identical to v3.2, and `prepare` refuses to freeze unless the v3.3
  manifest hash equals v3.2's.
- The constrained reader is dropped from v3.3 after failing gate 17.
- Excerpt rule: an inline ellipsis inside an excerpt line splits it into
  fragments that are each checked verbatim. Punctuation-only fragments, such
  as the bracket left by an elided argument list, carry no evidence and are
  neither checked nor counted; at least one substantive fragment is
  required. GLM 5.3 quotes this way in nearly every review; it is a quoting
  convention, not fabrication, and fabricated fragments still fail. Found on the first GLM calibration call
  and applied before any counted call.
- Both new CLIs lack a system-prompt flag, so the reviewer system text is
  folded into the prompt. Neither exposes a structured-output flag; the
  narrow fence normalizer applies. Match responses that decorate a quirk id
  ("Q1: ...") are normalized to the id, deterministically, before
  validation.

## Amendment v3.4, September 8, 2026: Muse Spark 1.3 replaces GLM 5.3

GLM 5.3 failed v3.3 calibration on gate 1: on control 0, repeat 2, both
attempts contained fabricated excerpts (a constant misquoted, and a
dictionary that exists in no file). Its receipts are retained and its
failure is published. The owner selected Muse Spark 1.3 (Meta) as the
second neutral judge. Protocol id becomes `code-quality-maintenance-v3.4`;
run directory `runs-code-quality-maintenance-v3.4`.

- Muse Spark 1.3 runs through the Muse CLI on the Standard tier, which does
  not train on prompts or completions, at medium reasoning effort, inside
  the adapter's kernel sandbox with an empty workspace, tools denied by the
  sandbox and by flags. The binary is content-pinned by hash. Model identity
  and token usage are read from the session log the adapter already audits;
  any tool event rejects the response.
- Grok 4.6 continues under v3.3 in its own directory as a scored sibling
  panel with equal weight; nothing is copied or rebound. The v3.4 summary
  reads both scored panels and both v3.2 sensitivity panels, and `prepare`
  refuses to freeze unless every companion directory's evidence manifest is
  byte-identical.
- Muse's calibration exam is identical to every other panel's.
- The Muse subscription is shared with a solver sweep running on the
  Contributor tier; the judge uses the Standard tier and a separate session
  store. Contention shows up only as pauses, never as changed results.

## Amendment v3.5, September 13, 2026: the same protocol on GPT-5.5 and Luna

Nothing in the rubric, controls, quirk keys, gates, repeats, seed or judges
changes. The amendment applies the protocol to a second population and
simplifies the layout. Protocol id `code-quality-maintenance-v3.5`; run
directory `runs-code-quality-maintenance-v3.5`; runner
`harness/maintenance_review_v35.py`, which reuses the frozen v3 implementation
as a library and replaces only the population freeze.

- Population: the September 2026 GPT-5.5 and GPT-5.6 Luna effort sweeps
  through Codex on the same 23 tasks, one attempt per task and level. GPT-5.5
  has four levels because its API has no max level; Luna has five. 207
  submissions when every run is judgeable.
- A run that did not finish cleanly is excluded rather than judged, with the
  reason recorded in the population record and the protocol. The first
  GPT-5.5 extra-high attempt on paddockcore overran the ten-hour cap because
  a harness bug left the Codex worker alive after the launcher was killed; it
  was graded as a timeout for the functional score and is not a valid capped
  attempt for review. The task is re-run once at extra-high under the fixed
  harness, disclosed as such; if the re-run also fails to finish, the cell is
  published with 22 judged submissions.
- Both neutral judges score in this one directory. Muse Spark 1.3 and Grok
  4.6 keep their v3.4 and v3.3 settings and binary pins, and each retakes the
  identical calibration exam under v3.5 before any counted call, because the
  protocol requires the exam before scoring a population, not once per judge.
- No sensitivity panels. Astra and Opus 5 are not run.
- The v3.4 record is unchanged and, because this amendment edits this
  document, v3.4 is re-run from a git worktree pinned at its freeze commit
  (`VulcanBench-frozen-v34`), as v3.2 and v3.3 are.

## Amendment v3.6, September 15, 2026: the same protocol on GPT-5.6 Terra

Nothing in the rubric, controls, quirk keys, gates, repeats, seed or judges
changes. The amendment applies the protocol to a third population, the
September 2026 GPT-5.6 Terra effort sweep through Codex, so that Terra can
join GPT-5.5, Luna, Astra and Fable 5.1 on one board scored the same way.
Protocol id `code-quality-maintenance-v3.6`; run directory
`runs-code-quality-maintenance-v3.6`; runner
`harness/maintenance_review_v36.py`, which reuses the frozen v3 implementation
as a library and replaces only the population freeze, as v3.5 does.

- Population: GPT-5.6 Terra at all five levels on the same 23 tasks, one
  attempt per task and level, 115 submissions when every run is judgeable.
- A run that did not finish cleanly is excluded rather than judged, with the
  reason recorded, as in v3.5.
- A task and level with no attempt is recorded under `missing` in the
  population record and the protocol, with the reason, and the cell is
  published with the submissions it has. At the freeze, paddockcore at max
  has no attempt: from 06:00 PDT on September 15 every launch was refused by
  the Codex API with "You've hit your usage limit ... try again at Sep 19th,
  2026 1:10 AM" before any work, so the cell freezes at 22. One refused
  attempt is kept under `runs-effort-terra/max-quota-refused/` as evidence.
  When the quota window reopens the task is run once at max and judged as a
  separate top-up freeze (v3.6.1) that adds one submission to this cell
  under the same calibration; the v3.6 record is not rewritten.
- Both neutral judges score in this one directory with their v3.4 and v3.3
  settings and binary pins, and each retakes the identical calibration exam
  under v3.6 before any counted call.
- Diagnostics: with a single model in the population, the pairwise checks
  pair the same task at two effort levels two steps apart in the level
  order, one pair per level, skipping a pair whose submission is missing.
- No sensitivity panels.
- The v3.5 record is unchanged and, because this amendment edits this
  document, v3.5 would be re-run from a git worktree pinned at its freeze
  commit, as v3.2 to v3.4 are.

## Amendment v3.6.1, September 17, 2026: the Terra top-up

Protocol id `code-quality-maintenance-v3.6.1`; run directory
`runs-code-quality-maintenance-v3.6.1`; runner
`harness/maintenance_review_v361.py`, derived from the v3.6 runner.

- Population: only the runs the v3.6 protocol recorded as missing, once
  they exist. The v3.6.1 freeze refuses any run already judged under v3.6
  and any run not on the v3.6 missing list, and checks that each cell's new
  rows plus its v3.6 rows plus any remaining gaps make 23.
- Calibration: both judges' v3.6 verdicts gate the top-up. The exam is per
  population and the top-up belongs to the v3.6 population; no new
  calibration calls are made. The v3.6.1 protocol record carries the hashes
  of the v3.6 protocol, summary, manifest and both calibration files, and
  the runner's gate checks the v3.6 verdict against the frozen v3.6
  protocol rather than against its own.
- Diagnostics: each top-up submission is reviewed twice (primary and
  repeat); there are no pairwise checks.
- The run itself: paddockcore at max, made on September 17 after the owner
  switched the Codex CLI to a second ChatGPT account (Pro plan) so the
  quota window did not have to be waited out. Same CLI version, same
  harness, same task hash; the account is the only change and is recorded
  here and in the population record.
- Publication merges the v3.6 and v3.6.1 directories; the v3.6 record is
  not rewritten. Aggregates for the max cell are recomputed over all 23.

## Amendment v3.7, September 18, 2026: the same protocol on GPT-5.6 Sol

Nothing in the rubric, controls, quirk keys, gates, repeats, seed or judges
changes. Protocol id `code-quality-maintenance-v3.7`; run directory
`runs-code-quality-maintenance-v3.7`; runner
`harness/maintenance_review_v37.py`, derived from the v3.6 runner.

- Population: the September 17 to 18, 2026 GPT-5.6 Sol effort sweep through
  Codex on the same 23 tasks, one attempt per task and level, all five
  levels complete, 115 submissions. The sweep ran on the second ChatGPT
  account (Pro plan) throughout, so no account change occurs inside it.
- Both neutral judges retake the identical calibration exam under v3.7
  before any counted call. No sensitivity panels. Pairwise diagnostics pair
  the same task at two effort levels, as under v3.6.
- Publication: Sol joins GPT-5.5, Luna, Terra, Astra and Fable 5.1 on the
  Frontier v4 board, completing the GPT-5.6 family.

## Amendment v3.8, September 20, 2026: the same protocol on the private Routine v1 suite

Nothing in the rubric, controls, gates, repeats, seed, weights or judges
changes. Protocol id `code-quality-maintenance-v3.8`; runner
`harness/maintenance_review_v38.py`, derived from the v3.7 runner. The run
directory is `judging/code-quality-maintenance-v3.8` inside the private
VulcanRoutine repository, because the frozen evidence holds private task
content; the population record is built there too by
`scripts/cii-v4-board/build_routine_population.py`. Nothing in this public
tree names a routine task.

- Population: VulcanBench Routine v1, twelve private routine tickets (small
  hand-authored Python packages, one clear ticket each), every board model
  at every effort level it offers through its own CLI: GPT-6 Astra, GPT-5.6
  Terra, Luna and Sol (five levels each), GPT-5.5 (four), Claude Fable 5.1
  (five) and SWE-2 through the Devin CLI (medium, high and max, the only
  variants it has). One attempt per task and level, up to 384 submissions.
- L2 intent recovery is not applicable, and the amendment changes no
  arithmetic to say so. L2 scores whether a reviewer can recover a task's
  frozen legacy quirks from the source. Routine tasks are admitted on the
  opposite gate to Frontier v4 (one clear ticket, no hidden contracts), so
  there is no quirk to recover. Each routine task freezes an empty quirk
  key, every submission's L2 denominator is zero, and the rule this protocol
  pre-registered on September 7 for a zero denominator applies as written:
  the L2 share moves to L1 for that submission. Code quality on Routine v1
  is therefore the L1 reviewed score of the passing panels. No probe or
  match call is made on submissions.
- Comparability: Routine Code quality (L1 alone, on routine tickets) and
  Frontier Code quality (L1 plus L2, on legacy reconstruction) are different
  constructs. They are never placed on one axis or in one ranking. Within
  Routine v1 every model and level is scored identically, which is what the
  routine question needs: the cheapest effort level at which a model's work
  is both correct and maintainable.
- Both neutral judges retake the identical calibration exam under v3.8
  before any counted call, probe and match controls included, so the judges
  are held to the same bar as on every Frontier amendment. No sensitivity
  panels. Pairwise diagnostics pair the same task at a model's lowest and
  highest level, rotating models over the seeded task order.
- Publication is aggregate only, per model and effort level, as the routine
  charter requires. Submission-level rows stay in the private record.

## Amendment v3.9, September 21, 2026: the same protocol on Devin SWE-2

Nothing in the rubric, controls, quirk keys, gates, repeats, seed or judges
changes. Protocol id `code-quality-maintenance-v3.9`; run directory
`runs-code-quality-maintenance-v3.9`; runner
`harness/maintenance_review_v39.py`, derived from the v3.7 runner.

- Population: the September 18 to 21, 2026 Devin SWE-2 effort sweep through
  the Devin CLI on the same 23 tasks, one attempt per task at each of the
  three levels SWE-2 offers (medium, high, max), 69 runs. One high run
  (cellarcore) reached the 3-hour task budget before verification and is
  excluded rather than judged, as the protocol requires; that cell freezes
  with 22 submissions and the sweep's pass count for it treats the run as a
  fail. 68 submissions.
- Pairwise diagnostics pair the same task at two of the three levels, two
  steps apart in the model's own ladder, as under v3.7.
- Both neutral judges retake the identical calibration exam under v3.9
  before any counted call. No sensitivity panels.
- Cost: SWE-2 has no public API price, so the population record carries no
  API-equivalent cost. Tokens, wall clock and Devin's own credit and ACU
  counters are recorded from the receipts.
- Publication: SWE-2 joins the Frontier v4 board as the first non-OpenAI,
  non-Anthropic entry, on the same 33% Code quality profile.

## Amendment v3.10, September 21, 2026: the Sol seat for Devin SWE-2

Nothing in the rubric, controls, quirk keys, gates, repeats, seed, weights
or population changes. Protocol id `code-quality-maintenance-v3.10`; run
directory `runs-code-quality-maintenance-v3.10`; runner
`harness/maintenance_review_v310.py`, derived from the v3.9 runner.

- Occasion: Grok 4.6 failed the v3.9 calibration exam on gate 16 (invented
  departures on the clear control in two of five repeats). The
  pre-registered single-panel rule would publish Devin from Muse Spark 1.3
  alone. The owner chose instead to fill the second seat for this
  population, so that Devin is scored by two neutral judges like every
  other board entry. Grok's failed calibration stays published under v3.9.
- The seat: GPT-5.6 Sol through the Codex CLI, model `gpt-5.6-sol`,
  reasoning effort medium, the same transport, config and read-only
  sandbox v3 already uses for its Astra sensitivity panel; the Codex CLI
  file is pinned by hash. Sol takes the identical calibration exam
  (controls, pairs, probe and match calls) before any counted call, and
  the one-gate 0.5 allowance applies as written.
- Neutrality: Sol is an OpenAI model. It is neutral for Devin SWE-2 (a
  Cognition model) and is admitted here for Devin passes only. It is not
  admitted as a neutral judge of any OpenAI submission; under this
  protocol a same-family judge can only be a disclosed sensitivity panel.
  Every card and report that shows Devin's Code quality states that its
  panel is Muse and Sol, not the Muse and Grok panel used for the rest of
  the Frontier v4 board.
- Muse: its v3.9 calibration verdict, reviews and probes are scored from
  the v3.9 directory as a scored sibling. `prepare` refuses to freeze
  unless the v3.10 manifest and signals are byte-identical to v3.9's and
  Muse's v3.9 calibration passed. No Muse call is repeated.
- Pairwise diagnostics, repeats and the summary arithmetic are v3's own
  code, unchanged.

## Amendment v3.11, September 21, 2026: Devin SWE-2, judged population, Muse alone

Nothing in the rubric, controls, quirk keys, gates, repeats, seed or weights
changes. Protocol id `code-quality-maintenance-v3.11`; run directory
`runs-code-quality-maintenance-v3.11`; runner
`harness/maintenance_review_v311.py`, derived from the v3.9 runner.

- Population: the v3.9 Devin SWE-2 sweep minus three runs that changed no
  recognized source file (snapcore and vaultcore at high, freightcore at
  max: file-mode changes on the legacy binaries only, functional 0). A run
  that produced no code has nothing to judge, and the sweep's automated
  quality and security metrics are undefined for it by construction, so
  the population builder now excludes such a run the way it excludes an
  unfinished one, and lists it under "excluded" with the reason. 65
  submissions: 23 at medium, 20 at high, 22 at max. The population record
  is `comparison-judged.json`; `comparison.json` stays as the v3.9 freeze.
  The sweep's pass counts are unchanged; every excluded run is a fail.
- Panel: Muse Spark 1.3 alone. Grok 4.6 failed the v3.9 exam and GPT-5.6
  Sol failed the v3.10 exam, both on gate 16 (invented departures on the
  clear control). Under the pre-registered single-panel rule nothing
  further runs for either, and no judge retakes a gate it failed. Both
  verdicts stay published and are named in the v3.11 protocol record.
- Calibration: Muse's v3.9 verdict gates this pass and no calibration call
  is repeated, on the v3.6.1 precedent: the exam is per judge and control
  set, and neither changed between v3.9 and v3.11. `prepare` refuses to
  freeze unless that verdict passed under the frozen v3.9 protocol.
- Every card and report that shows Devin's Code quality states that it is
  a single-judge score, not the two-judge mean behind the rest of the
  Frontier v4 board.

## Amendment v3.12, September 22, 2026: Muse Spark 1.3 judged by Claude Opus 5

Nothing in the rubric, controls, quirk keys, gates, repeats, seed or weights
changes. Protocol id `code-quality-maintenance-v3.12`; run directory
`runs-code-quality-maintenance-v3.12`; runner
`harness/maintenance_review_v312.py`, derived from the v3.9 runner.

- Population: the September 6 to 18, 2026 Muse Spark 1.3 Contributor-tier
  effort sweep through Muse Code 1.0.3 on the same 23 tasks, one attempt per
  task at minimal, low, medium, high and extra-high (the Contributor tier
  offers no max). Runs that did not finish, or that changed no recognized
  source file, are excluded rather than judged and listed with the reason;
  each cell freezes with the submissions it has (100 of 115 runs: 21, 18,
  20, 20, 21). The sweep's early runs (minimal and 17 low tasks) ran under
  the former 10-hour task bound; the sweep record carries that history and
  the card discloses it.
- Panel: Claude Opus 5 through the Claude Code CLI, alone, under the
  settings v3.2 froze for its Opus 5 panel (model `claude-opus-5`,
  reasoning effort medium, CLI 2.1.261 pinned by hash, the identity and
  subscription-quota guards, the reviewer-fallback policy of September 7
  with the count disclosed). Opus 5 passed every gate under v3.2. It is the
  only judge here because Muse Spark 1.3 cannot judge its own submissions
  and Grok 4.6 and GPT-5.6 Sol failed their most recent exams. Opus 5 is
  neutral for Meta's model and is not admitted as a neutral judge of
  Anthropic submissions. It takes the identical calibration exam under this
  protocol before any counted call; the one-gate 0.5 allowance applies as
  written. If it fails, nothing is published for this population.
- Every card and report that shows Muse's Code quality states that it is a
  single-judge score from a different judge than the Muse and Grok panel
  behind the rest of the Frontier v4 board, and is not placed in one ranking
  with those scores without that note.

## Amendment v3.13, September 22, 2026: a second judge for Devin SWE-2

Nothing in the rubric, controls, quirk keys, gates, repeats, seed or weights
changes. Protocol id `code-quality-maintenance-v3.13`; run directory
`runs-code-quality-maintenance-v3.13`; runner
`harness/maintenance_review_v313.py`, derived from the v3.12 runner.

- Occasion: v3.11 published Devin SWE-2 from one judge because Grok 4.6
  (v3.9) and GPT-5.6 Sol (v3.10) both failed the exam on gate 16. Claude
  Opus 5 passed the identical exam under v3.12 with no allowance used, so
  the second seat is filled rather than left empty.
- Population: the v3.11 freeze, byte for byte. `prepare` refuses unless the
  manifest and signals hash equal to v3.11's and Muse's v3.11 calibration
  passed. 65 judged submissions: 23 medium, 20 high, 22 max.
- Panel: Muse Spark 1.3 scored from its v3.11 pass as a scored sibling, and
  Claude Opus 5 in the second seat under the settings, identity and quota
  guards, fallback policy and CLI pin v3.2 froze for it. Opus 5 retakes the
  exam here before any counted call. It is neutral for a Cognition model and
  is not admitted as a neutral judge of Anthropic submissions. If it fails,
  nothing changes and v3.11's single-judge publication stands.
- Comparability: a Devin score published under v3.13 is a two-judge mean of
  Muse Spark 1.3 and Claude Opus 5, not the Muse and Grok pair behind the
  other Frontier v4 entries. Every card and report says which pair it used.
- The protocol document is frozen as a copy inside the run directory, as
  v3.8 does, so concurrent protocols stop colliding on one file. Future
  amendments should do the same.

- Outcome, September 22, 2026: Claude Opus 5 failed the exam under this
  amendment (gate 4 short by 0.10 and gate 14 on the pair control 0 against
  control 3 outright, two failing gates, so the allowance does not apply).
  No counted call was made. This amendment is recorded as a failed attempt:
  Devin SWE-2 remains published under v3.11 from Muse Spark 1.3 alone, and
  nothing on the board or in any report changes. The operations log carries
  the detail.

## Not yet done

- v3.4 calibration results for Muse Spark 1.3, v3.3 results for Grok 4.6,
  and every later stage.
- L3 follow-up change generation, per task, and its worker runner.
