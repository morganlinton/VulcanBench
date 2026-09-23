# Maintenance v3 operations log

Operator actions and observations during calibration and the full pass.
This file is not hash-bound by the protocol; the protocol document is, so
operational notes live here.

## v3.2 calibration, September 7, 2026

- Astra: passed all twenty gates on 80 calls with no allowance used.
- Opus 5, control 2, repeat 1: the Claude CLI exited non-zero with result
  subtype `error_max_structured_output_retries` after the model omitted the
  `dimensions` field in all five internal attempts. The transport recorded
  this as non-retryable. Operator review reclassified it as an invalid-schema
  response, which the protocol grants one fresh retry; the receipt is
  retained with the review note inside it and the panel resumed. The same
  rule applies to any later occurrence: one fresh attempt, receipt kept, no
  other change.
- Constrained reader: failed gate 17. Control 0 accuracy 0.933 (one read
  answered (0.25) for a balance of -125 cents) against 1.0 on the compressed
  control. The gate compares two accuracies at the ceiling, so a single slip
  decides it; that is a design weakness recorded here, not amended. L5
  carries no weight and its results are not published under v3.2.
- Opus 5, control 5, repeat 4: second occurrence of
  `error_max_structured_output_retries` (dimensions field omitted again).
  Same rule applied: receipt retained with the review note, one fresh
  attempt, panel resumed. Two occurrences in the first 29 Opus 5 reviews. If
  this rate holds, the full pass of 460 reviews would need roughly 30 such
  reviews; the fix belongs in the transport's retry classification and can
  only land with a new protocol identifier, so it is deferred to the
  boundary before the full pass and recorded here.
- Opus 5, control 2, repeat 5: third occurrence in 31 reviews. The documented
  rule is now applied by `harness/maintenance_review_v3_resume.py`, a script
  outside the frozen code set that marks exactly this CLI subtype retryable
  once per call and re-invokes the stage. Anything else still stops for a
  person. Every application is printed and recorded in the receipt.
- Opus 5: passed all twenty gates on 80 calls with no allowance used, after
  three structured-output stops resolved under the documented rule (one of
  them by the wrapper). Repeatability shortfall -0.18, the closest gate.
- v3.2 calibration verdict: Astra passed, Opus 5 passed, reader failed gate
  17. Both panels are eligible for the full pass under the frozen protocol
  (hash 47ae9135). L5 results will not be published.

## v3.2 full pass

- Astra: all 230 primary reviews and 10 repeats complete; 2 of 10 pairwise
  calls complete. Stopped on the Codex subscription usage limit ("try again
  at Sep 12th, 2026 8:50 AM"). Per protocol: receipts preserved, no paid API
  fallback, resume with identical frozen inputs after the reset. Remaining
  for Astra: 8 pairwise, 230 probe, 230 match calls.
- Opus 5: stopped at primary review 101 after the retry rule fired four
  times in 100 reviews. Both attempts on submission 101 failed only the
  excerpt rule: the intent excerpt joined a hard-wrapped Markdown line into
  one line. Content verbatim, scores unaffected. Second operator rule added
  to the wrapper: when both attempts fail only on excerpts and every
  rejected excerpt matches the source once whitespace and line breaks are
  collapsed, attempt 1 is selected with those excerpts re-wrapped to the
  source's line breaks, scores untouched, original excerpts recorded in
  the selected receipt under operator_recovery. Quotes that do not match
  after collapsing are not recovered and stop for a person.
- Opus 5, primary submission 133 (evening of September 7): both attempts were
  answered by claude-opus-4-8 although the session requested and reported
  claude-opus-5, with no model_fallback event and no refusal text in the
  stream. This is the silent refusal fallback previously documented in the
  Fable panel comparison. The v3 protocol text accepts no reviewer fallback,
  so the identity guard rejected both attempts and the chain stopped. Not
  recoverable by either wrapper rule; awaiting the benchmark owner's policy
  decision. Opus 5 stands at 132 of 230 primary reviews. No other Opus 5
  stream in this run contains a fallback.
- Owner decision, evening of September 7, 2026: retain and disclose
  reviewer fallbacks, as in the earlier Fable panel comparison. This amends
  the protocol text's "no reviewer fallback is accepted" for the v3.2 full
  pass; no gate, weight, prompt, or hash changes. Mechanism: third wrapper
  rule. An attempt that failed only the identity guard, whose session
  requested and reported claude-opus-5, whose every assistant message came
  from claude-opus-4-8, that used no tool, and whose response validates, is
  selected with a reviewer_fallback record in the receipt. The card and
  summary must report the count of fallback-served reviews per stage.
- Opus 5, primary submission 136 (overnight, September 7 to 8): both attempts
  quoted two verbatim fragments joined by an inline ellipsis on one line.
  The re-wrap rule now splits excerpt lines on inline ellipsis markers and
  requires each fragment to be verbatim; fragments are rejoined with a
  dots-only line. Any fragment absent from the source still stops. The chain
  sat idle overnight on this stop.

## v3.3 calibration, September 8, 2026

- Two v3.3 freezes were superseded before any counted call (argparse panel
  names; then inline-ellipsis and punctuation-fragment excerpt rules). Both
  directories retained. Live freeze hash b82da598.
- GLM 5.3: on control 0, repeat 2, both attempts contained a fabricated
  excerpt: attempt 1 quoted `CARD_FEE_FIXED_CENTS = 25` (the file says 30);
  attempt 2 quoted a dictionary named METRIC_TO_AXIOM that does not exist in
  any file. These are not quoting variants; no rule recovers an invented
  quote. Under gate 1 (validity) after the single permitted retry, GLM 5.3
  fails calibration. Two other GLM reviews were recovered by the excerpt
  re-wrap rule (inline ellipsis joins, minimal spans, original recorded).
  Awaiting the owner's acknowledgement before recording the panel as
  failed; nothing further runs for GLM meanwhile.
- Grok 4.6: running clean, 7 reviews in, about 50 seconds per review.
- GLM 5.3 recorded as failed calibration (gate 1) on the owner's decision
  to select a different second judge. `calibration-glm.json` carries the
  operator record; receipts retained.
- Grok 4.6: passed all twenty gates on 80 calls, no allowance used, no
  operator rule applied at any point. Closest gate: formatting, margin 0.20.
  Full pass started under v3.3 (primary, repeat, pairwise, probe, match).
- Frozen-code execution: the v3.3 protocol binds the runner's hash, and the
  runner changed for v3.4, so Grok's full pass runs from a git worktree at
  commit 6040f28a (`/Users/morganlinton/dev/VulcanBench-frozen-v33`), whose
  runner and protocol document match the v3.3 hashes. The worktree holds
  copies of the untracked judging modules (unchanged, hashes verified by
  `verify_frozen`), the latest operator wrapper (not hash-bound), and
  symlinks to the v3.3 and v3.2 run directories and the comparison file.
  `verify_frozen` passed from the worktree before the first call. Launcher:
  `logs/cii-v4-maint-v33-fullpass-grok.sh`.
- Opus 5 sensitivity chain (v3.2) died at primary review 189: after a
  structured-output stop the wrapper re-invoked the runner, which by then
  had been amended for v3.3 and pointed at that directory, so the frozen
  check could not even find its protocol. No receipt was affected. The v3.2
  panels now run from a worktree at commit e42e69f0
  (`/Users/morganlinton/dev/VulcanBench-frozen-v32`), whose runner and
  document match the v3.2 hashes; `verify_frozen` passed from it before the
  relaunch. Astra's launcher for September 12 points at the same worktree.
- Muse Spark 1.3: passed all twenty gates on 80 calls, no allowance used,
  no operator rule applied. Closest gate: repeatability, margin 0.02. Both
  neutral judges are now calibrated (Grok under v3.3, Muse under v3.4).
  Muse full pass started under v3.4 from the main tree at the freeze commit;
  the runner must not be edited while it runs, or the pass moves to a
  worktree at that commit like the older panels.
- Owner decision, September 8, 2026, 10:30 PDT: the scored panel is Muse
  Spark 1.3 and Grok 4.6 only. The Astra and Opus 5 sensitivity panels are
  retired: their existing receipts (Astra 230 primaries and 10 repeats, Opus
  230 primaries, 10 repeats, 10 pairwise, 109 probes, 108 matches) are
  retained as raw diagnostics but are not resumed, not waited for, and not
  shown on the card. The self-preference estimate is therefore not
  published. Combined results no longer depend on the Codex quota reset.
- Grok, primary 27: both attempts quoted a documentation line with its
  Markdown code marks and leading bullet stripped. Recovery now matches
  ignoring code marks and list bullets and returns the verbatim source span.
  Opus 5, match 109: quirk ids decorated with descriptions under the frozen
  v3.2 validator; a match-id recovery rule was added but is moot for Opus
  under the decision above. Both recoveries stay available to the live panels.
- Muse, primary 168: both attempts quoted `history["score"] += 2` where the
  source line is `self.history[parts[1]]["score"] += 2`; the judge dropped
  the receiver and index without an elision marker. New recovery rule,
  omission-only: an excerpt fragment of at least four tokens whose tokens all
  appear, in order, within one source line (nothing added) is replaced by
  that verbatim line, original recorded. A wrong value or an absent name
  still fails; both GLM fabrications were re-checked and still fail. Final
  source lines are preferred over patch lines.
- Cursor leaves one detached helper process per call (parent pid 1); they
  are idle and do not hold the runner. Grok's effective pace is about two
  minutes per review including CLI start-up.
- Grok, primary 140: both attempts quoted hard-wrapped documentation prose
  verbatim, but the quotes began and ended mid-line, which the span matcher
  did not accept. Recovery generalized: a fragment may start at a
  word-aligned point inside a source line and span following lines; the
  whole verbatim lines are selected and the original recorded. Fabricated
  text still fails; both GLM cases were re-checked.
- Muse, probe 39: both attempts quoted the line containing backslash-0 with
  the escape decoded to a real NUL character inside the JSON string.
  Recovery re-escapes control characters in the common source spellings
  (backslash-x hex, backslash-octal, backslash-u) before matching and
  selects the verbatim line. Excerpt recovery now also covers probe
  responses, which it had not.
- Muse, probe 47: the judge process exited 143 after "received SIGTERM";
  no response was produced. The Contributor-tier solver sweep's status
  file updated four seconds later, consistent with a new Muse session on
  the shared machine terminating the other. Wrapper rule: an outside kill
  with no terminal event is a transport fault and gets the protocol's one
  fresh attempt; a second kill on the same call stops for a person. The
  sweep was not modified.
- Grok, probe 208 (September 9, 00:39 PDT): Cursor returned a transport
  error, RetriableError resource_exhausted, with no response. Treated as a
  quota stop under the protocol: the attempt's files are archived inside the
  call folder under quota-stops/ with a note, the wrapper waits with backoff
  (3 minutes doubling to 30), and the same call resumes with identical
  inputs. Twelve archived stops on one call leave it for a person. Muse had
  already completed its full pass.
- September 9, 2026, 02:16 PDT: Grok's full pass complete after six
  rate-limit resumes. Summary written (230 of 230 published, no L2
  redistribution, no reviewer fallbacks on either scored panel) and the
  final card generated by the finisher. Preliminary card artifacts removed.

## v3.5, September 14 to 15, 2026: GPT-5.5 and GPT-5.6 Luna

- Population frozen September 14, 03:50 PDT after the Luna sweep finished
  and the paddockcore GPT-5.5 extra-high re-run completed under the fixed
  process-group kill (functional 0.53, 44 minutes). 207 rows, no exclusions.
  The capped first attempt is set aside under
  `runs-effort-gpt55/extra-high-superseded/`.
- Both judges retook the calibration exam under v3.5. Grok 4.6 passed every
  gate. Muse Spark 1.3 passed using the pre-registered allowance: gate 11
  (repeatability) short by 0.02 on one control, within the half-point
  allowance; every other gate passed.
- Grok, primary submission-055 (September 14, 14:55 PDT): the Cursor CLI
  exited with `getaddrinfo ENOTFOUND api2.cursor.sh` and no response. The
  wrapper stopped for a person because the failure did not match any
  retry rule. New wrapper rule, `retry_network_fault`: a judge CLI exit
  whose only output is a network resolution or connection error is a
  transport fault and gets the protocol's one fresh attempt, receipt
  retained; a second fault on the same call stops for a person. The call
  was retried under the rule and completed.
- Second attempts under the existing rules: Muse five (two unsupported
  excerpts, two malformed JSON responses, one missing maintenance
  consequence in calibration); Grok six including the fault above (four
  unsupported excerpts, one malformed response). Neither judge produced a
  reviewer fallback.
- September 15, 02:50 PDT: both passes complete, 718 calls per judge.
  Summary written: 207 of 207 published, ten L2 redistributions (nine Luna
  low, one Luna medium, each a submission that passed no quirk family), no
  fallbacks, both panels passing.

## v3.6, September 15 to 16, 2026: GPT-5.6 Terra

- Population frozen September 15, 19:25 PDT: 114 rows, no exclusions, one
  missing (paddockcore at max, refused by the Codex API's usage limit until
  September 19; one refused attempt kept under
  `runs-effort-terra/max-quota-refused/`). Judged on the owner's decision to
  publish max at 22 and top up later.
- Both judges retook the calibration exam under v3.6. Grok 4.6 passed every
  gate. Muse Spark 1.3 passed on the pre-registered allowance: gate 11
  (repeatability) short by 0.02 on one control, as under v3.5.
- Muse, primary submission-112 (22:10 PDT): both attempts failed on
  "Unsupported evidence excerpt"; the wrapper's excerpt recovery crashed
  because, when driving a later protocol module, it looked up read and the
  validators on that module instead of the frozen v3 implementation. The
  wrapper was fixed (helpers taken from the v3 module, which also holds the
  rebound directories; PR #119), the chain relaunched from the review stage,
  and the recovery applied under the existing re-wrap rule (one excerpt on
  presentation).
- Grok: two Cursor DNS transport faults (`getaddrinfo ENOTFOUND
  api2.cursor.sh`), both on match calls, both retried under the
  retry_network_fault rule without stopping.
- Second attempts under the existing rules: Muse six (four primary, one
  repeat, one probe, all unsupported excerpts); Grok eight (five unsupported
  excerpts on primaries, one match that cited an unlisted departure, the two
  transport faults). Neither judge produced a reviewer fallback.
- September 16, 09:36 PDT: both passes complete. Summary written: 114 of
  114 published, two L2 redistributions (granarycore at low, schedcore at
  high), both panels passing.

## v3.7, September 18, 2026: GPT-5.6 Sol

- Population frozen September 18, 13:09 PDT: 115 rows (23 tasks at each of
  low, medium, high, extra-high and max), no exclusions, none missing.
- Both judges retook the calibration exam under v3.7 and passed every gate;
  neither used the pre-registered allowance.
- Grok, calibration probe-0-r4 (14:30 PDT): Cursor returned
  `ActionRequiredError: Request blocked ... under the model provider's
  usage guidelines` before any model output. The identical control prompt
  served under v3.5 and v3.6, so the block is a transport fault, not a
  judgment. New wrapper rule retry_provider_block: one fresh attempt when
  the provider refuses with no assistant output; a second block on the same
  call stops for a person. The retry served and the gate run completed.
- Grok, probe submission-023 (23:35 PDT, sol at max on
  legacy-codeccore-binary-parity): both attempts quoted
  `memo = record[31:46].rstrip(".",")` where the source line is
  `memo = record[31:46].rstrip(".,")`. The excerpt has a character inserted
  inside the string literal; it is neither a re-wrap, an omission, nor an
  escape spelling, so no recovery rule accepts it and, as with the GLM
  fabrications, none was added. The probe is invalid. The frozen summary
  already defines the outcome: a submission without a valid match from a
  passing panel is left unpublished, so submission-023 keeps its two L1
  reviews and its Muse probe but carries no published code quality score;
  sol at max is therefore published on 22 of 23 tasks. New wrapper rule
  invalidate_unrecoverable_probe records the finding in the call folder
  (`operator-invalid.json`) and drives the rest of the probe stage
  in-process with that row skipped, since the frozen `probe` command would
  stop on it every time. Nothing in the judge's response was altered or
  filled in. The owner can reverse this by deleting the marker and
  rerunning the probe under an amendment; the receipts are retained.
- Second attempts over the whole pass: Muse nine (four unsupported
  excerpts, two on primaries and two on probes; five malformed JSON, four on
  primaries and one on a probe); Grok nine (five unsupported excerpts: one
  calibration control, two primaries, two probes including submission-023;
  one malformed JSON on a probe; two DNS transport faults; the provider
  block above). Neither judge produced a reviewer fallback.
- September 19, 03:35 PDT: both passes complete. Summary written: 114 of
  115 published, no L2 redistributions, both panels passing.

## v3.8, September 21 to 22, 2026: the private Routine v1 suite

- Population frozen September 21, 17:48 PDT, in the private VulcanRoutine
  repository (`judging/code-quality-maintenance-v3.8`): 384 rows, twelve
  tasks at every level each of seven models offers (Astra, Terra, Luna, Sol
  and Fable 5.1 at five levels, GPT-5.5 at four, SWE-2 at medium, high and
  max), no exclusions, none missing. Every routine task freezes an empty
  quirk key, so no probe or match call is made and Code quality is the L1
  reviewed score under v3's pre-registered zero-denominator rule.
- First freeze superseded. The first v3.8 freeze (September 21, 16:28 PDT)
  pinned the hash of the live protocol document, as every earlier amendment
  did. While Grok was 48 calls into calibration (Muse had already passed),
  the Devin SWE-2 amendments (v3.9, then v3.10) were written into that same
  file, and `verify_frozen` stopped the chain with "Frozen input changed".
  The pinned text cannot be reconstructed: the v3.9 section was edited in
  place before v3.10 was added and none of it was committed. Harness PR #136
  makes v3.8 freeze its own copy of the document inside the run directory
  and pin that copy, with the source hash kept for provenance. The first
  freeze is retained as `code-quality-maintenance-v3.8-superseded-freeze1`
  with its 128 calibration receipts; no counted call had been made. Both
  judges retook calibration under the second freeze, as receipts are bound
  to the protocol hash.
- Both judges passed every gate under the second freeze; neither used the
  allowance. Muse's closest gate was g11 repeatability (shortfall 0.02);
  Grok's was g11 (0.14).
- Cursor display rename, every Grok call: Cursor now reports the pinned
  model `cursor-grok-4.6-medium` as "Grok 4.6 Medium" where the frozen v3.3
  settings record "Cursor Grok 4.6 Medium". The wrapper rule
  accept_display_rename (added for v3.9 the same day) accepted the label on
  489 calls; the model id requested was unchanged throughout.
- Grok, repeat diagnostic submission-001 (September 22, 06:07 PDT): both
  attempts quoted a `"rows": [...]` excerpt absent from the code; no
  recovery rule accepts it and none was added. The call is a diagnostic
  (repeats and pairs feed the operations record, not the published score),
  so it is marked invalid in its folder (`operator-invalid.json`, rule
  invalidate_unrecoverable_diagnostic) and the remaining 11 repeats and 24
  pairwise calls were driven in-process under the wrapper's rules
  (`logs/routine-v38-finish-wrapped.py`). All 384 primary reviews from both
  judges are selected; nothing in any judge's response was altered.
- Second attempts over the whole pass: Muse nine, Grok fifty (excluding the
  rename retries), every one an unsupported evidence excerpt; the excerpt
  re-wrap rule resolved all but the diagnostic above.
- Summary: 384 of 384 published, both panels passing, no fallback reviews.
  Combined score (50/8.5/8.5/33) at Low, by model: Astra 94.8, Terra 93.1,
  Luna 92.5, Sol 94.4, GPT-5.5 94.9, Fable 5.1 94.2; SWE-2 93.4 at Medium.
  Every model passed all twelve tickets at every level, so the routine
  question is answered by cost and time: the cheapest adequate level is Low
  for every priced model and Medium for SWE-2. Fable 5.1 is the one model
  whose Code quality climbs steadily with effort (85.5 at Low to 93.8 at
  Max). Routine Code quality is L1 alone and is not comparable with
  Frontier v4 Code quality. Public aggregates: VulcanRoutine
  `results/routine-v1-aggregates.{json,csv}`; board section: VulcanBenchCOM
  PR #61.

## v3.9, September 21, 2026: Devin SWE-2

- Population frozen September 21, 10:08 PDT: 68 rows (23 at medium and max,
  22 at high), one excluded (high, cellarcore: the run reached the 3-hour
  task budget before verification), none missing. Two submissions carry the
  solver's own scratch directories (.work, .probe: disassembly dumps and
  fuzz outputs) in their final files, making 370k and 200k character
  prompts; v3.4 judged a 636k character prompt, so the evidence stands as
  the run left it.
- Muse Spark 1.3 passed the calibration exam under v3.9 on the first pass.
- Grok, calibration control-7-r1 (11:07 PDT): Cursor reported the display
  label "Grok 4.6 Medium" for the pinned model id cursor-grok-4.6-medium;
  the frozen v3.3 settings record "Cursor Grok 4.6 Medium", and the
  requested-only identity guard failed on the label. The binary is the
  pinned one (sha checked at prepare) and the model id in the request is
  unchanged, so this is a provider-side rename of the label, not a served
  model change. New wrapper rule accept_display_rename: an attempt whose
  only failure is that label, where the reported label equals the frozen
  display name minus the "Cursor " prefix, is selected unchanged with the
  finding recorded under operator_review. Any other label still stops. The
  rule applies to every Grok call in this pass and is reported here rather
  than per call.
- Grok 4.6 failed the calibration exam under v3.9 (September 21, 12:02
  PDT) on gate 16, probe recovers documented intent: control 0 (the clear
  control, no documented departure) must draw an empty probe on at least
  four of five repeats, and Grok reported invented departures on two
  repeats (three empty). Every other gate passed. Gate 16 is boolean, so
  the one-gate 0.5 allowance cannot excuse it. Under every earlier pass
  Grok drew exactly one non-empty repeat on control 0 (v3.5, v3.6, v3.7),
  so this gate was always its margin. Observation, recorded and not used
  as an excuse: the same morning Cursor relabelled the pinned model id and
  its usage accounting changed (fewer input tokens, more cache reads on the
  identical prompt), consistent with a serving change on the provider's
  side. Under the pre-registered single-panel rule and the GLM precedent,
  nothing further runs for Grok; Devin SWE-2 is judged by Muse Spark 1.3
  alone, the failed calibration is published beside it, and the card and
  report say so. The chain resumed 12:04 PDT with Muse reviews and probes
  (logs/cii-v4-maint-v39-muse.sh).
- Muse, primary submission-004 (12:10 PDT): the CLI's stream ended with
  "transport error [net-timeout]: timed out waiting for response data" and
  no assistant output; stderr carried only a session-registry warning, so
  the receipt's error text did not match the network-fault markers. The
  retry_network_fault rule now also reads the stream's terminal record for
  that Muse transport marker; the call took its one fresh attempt.
- September 21, 15:15 PDT: the Muse pass under v3.9 completed (68 reviews,
  5 repeats, 6 pairwise, 68 probes and matches; one transport retry).
  Summary written with Muse as the only passing panel.
- Owner decision, September 21, 2026: rather than publish Devin from one
  judge, fill the second seat with GPT-5.6 Sol through Codex for this
  population only (Sol is neutral for a Cognition model and is not admitted
  as a neutral judge of OpenAI submissions). Amendment v3.10: identical
  population (manifest and signals byte-identical to v3.9, checked at
  freeze), Muse scored from its v3.9 directory as a scored sibling, Sol
  takes the full calibration exam and pass. The Codex CLI file is pinned by
  hash; transport, config and read-only sandbox are the ones v3 uses for
  its Astra sensitivity panel. Grok's v3.9 calibration failure stays
  published and is named in the v3.10 protocol record.

## v3.10, September 21, 2026: Devin SWE-2, the Sol seat

- Frozen 17:44 PDT (protocol 0c8068b1). Sol calibration started at once,
  about 35 seconds per review through Codex on the second ChatGPT account,
  while the routine sweeps use the same account for solver runs; the
  wrapper's quota rule now also recognises Codex's "usage limit" and
  "limit reached" phrasing.
- GPT-5.6 Sol failed the calibration exam under v3.10 (September 21, 18:47
  PDT) on the same gate 16: it reported invented departures on the clear
  control in four of five repeats (one empty; four needed), while
  recovering the documented quirk on control 7 every time and passing every
  other gate, repeatability included. As with Grok, the gate is boolean and
  the allowance cannot excuse it. Under the pre-registered rule nothing
  further runs for Sol. The v3.10 summary was written with Muse Spark 1.3 as
  the only passing panel and both Grok (v3.9) and Sol (v3.10) disclosed as
  failed; Devin SWE-2's published Code quality is Muse's L1 plus L2 alone.
  Observation for the record: both judges that failed here failed on the
  clear control's probe, the one call that rewards saying nothing; Muse has
  drawn an empty probe on that control in every pass since v3.4.
- The v3.9 and v3.10 summaries could not be written: three Devin runs
  (snapcore and vaultcore at high, freightcore at max) changed no
  recognized source file, only file modes on the legacy binaries, so the
  sweep's automated quality and security metrics are None and the frozen
  composite has no value for them. The runs scored 0 functionally, and Muse
  rated the untouched original module in each. These are non-submissions
  that the population builder should have excluded beside the unfinished
  run; it now does, with the reason recorded. Owner decision, September 21,
  2026: re-freeze as v3.11 on the 65 judged submissions with Muse Spark 1.3
  alone (no retake for Grok or Sol), reusing Muse's v3.9 calibration verdict
  on the v3.6.1 precedent. The Muse pass repeats because the new freeze
  changes every call's binding; the v3.9 calls stay archived.

## v3.11, September 21, 2026: Devin SWE-2, judged population

- Frozen September 21, 19:11 PDT: 65 rows (23 medium, 20 high, 22 max),
  four excluded (one unfinished, three with no source change), Muse's v3.9
  calibration verdict checked and reused. Muse reviews started at once.
- September 21, 21:48 PDT: the Muse pass complete (68 reviews including 3
  repeats, 6 pairwise, 65 probes and matches), no second attempt and no
  operator rule needed. Summary written: 65 of 65 published, one L2
  redistribution (a high submission with no passed quirk family), Muse the
  only panel. Devin SWE-2 combined score medium 82.43, high 81.42, max
  86.14; Code quality 65.1, 66.3, 69.2.

## v3.12, September 22, 2026: Muse Spark 1.3 judged by Claude Opus 5

- Owner decision, September 22: Muse's sweep needs a Code quality score for
  the four-model cards. Muse cannot judge itself; Grok 4.6 (v3.9) and
  GPT-5.6 Sol (v3.10) failed gate 16, and the owner declined to retake Sol.
  Claude Opus 5, which passed every gate under v3.2, takes the seat alone
  under the v3.2 settings, CLI pin and guards.
- Population: 99 of 115 sweep runs. Excluded, with reasons in the record:
  14 unfinished, one that changed no source file (paddockcore at high),
  and one whose patch cannot be reconstructed (lodgecore at medium: the
  solver committed a binary scratch file under .tmp/ without an index
  line, so the evidence pipeline cannot rebuild its final files). The
  population builder now tries the reconstruction at build time and
  excludes on failure. Paddockcore has no judged run at any level; the
  freeze check now requires judged and excluded tasks together to cover
  the suite. Two earlier freeze attempts stopped before any call (task
  coverage, then the unreconstructible patch); their directories were
  discarded, nothing counted.
- Frozen 08:14 PDT, 99 rows (21, 18, 19, 20, 21 from minimal to extra-high).
  Opus 5 calibration started at once, about 16 seconds per review through
  Claude Code on the Max subscription.
- Opus 5 passed the calibration exam under v3.12 with no allowance used
  (80 calls). Reviews, repeats and pairwise diagnostics completed with two
  structured-output retry stops handled by the existing rule.
- Opus 5, probe submission-099: the CLI recorded the answer as a
  StructuredOutput call with unparsed input followed by pseudo tool calls
  named after JSON fields ("excerpt"); no tool ran (no tool_result in the
  stream), but the frozen guard read the blocks as tool use and stopped
  with a non-retryable receipt. New wrapper rule
  retry_garbled_structured_output: when the only tool blocks are that
  unparsed emission and no tool result exists, the response is a malformed
  answer under the protocol text and takes the single fresh attempt.
- September 22, 10:20 PDT: the pass complete (99 reviews, 5 repeats, 6
  pairwise, 99 probes and matches; three operator rule applications, no
  reviewer fallback). Summary written: 99 of 99 published, Opus 5 the only
  panel. Muse Spark 1.3 combined score minimal 74.84, low 79.33, medium
  78.96, high 80.71, extra-high 77.44; Code quality 62.0, 64.8, 63.6, 66.2,
  61.2.

## v3.13, September 22, 2026: Devin SWE-2, the second seat attempt

- Occasion: v3.11 published Devin from one judge after Grok 4.6 and GPT-5.6
  Sol failed the exam. Claude Opus 5 had passed the identical exam under
  v3.12 hours earlier with no allowance used, so v3.13 offered it the second
  seat on the byte-identical v3.11 population, with Muse scored as a sibling.
- Opus 5 failed the exam under v3.13 on two gates, so the one-gate allowance
  does not apply: gate 4 (formatting is presentation) missed by 0.10, and
  gate 14 on the pair control 0 against control 3 failed outright. It scored
  the clean control 100 in the forward order and 100 again in the reverse
  order, where a consistent judge scores the reverse 0. That is the
  order-consistency failure the gate exists to catch.
- The calibration exam uses the fixed control set and is identical whatever
  population follows it, so this is run-to-run variance in the judge, not
  anything about Devin's evidence. Opus 5 answered the same pair
  consistently under v3.12 (100 forward, 0 reverse) and inconsistently here.
- No counted call was made. Under the pre-registered rule nothing further
  runs for Opus 5 on this population and v3.11's single-judge publication
  stands. The exam was not retaken: repeating a draw until a judge passes
  selects on the outcome, which is what the no-retake rule prevents.
- Recorded limitation, not acted on here: admission rests on one draw of a
  stochastic exam. Three of the four judges offered a seat on this
  population (Grok, Sol, Opus 5) failed a single draw, and Muse's own
  admission is a single draw too. Whether admission should require a
  repeated or aggregated exam is a protocol question for a future revision,
  to be decided before a panel is under consideration rather than after.
