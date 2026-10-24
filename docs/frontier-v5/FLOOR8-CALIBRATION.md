# F5 floor 8 calibration (2026-10-24)

The question PLAN-v3.md section 2 asks before floor 8 is adopted: does "the
obvious fix is wrong in a way the project's own tests detect" separate the
one admitted F5 task from the rejected ones, when measured with the real
verifiers rather than argued? Two points were measured on the Mac Studio on
2026-10-24, with `scripts/frontier-v5/run_controls.py` and local Docker only.
No model was called. PLAN-v3.md's stopping rule applied: if a reject passed
floor 8, the floor is not the discriminator and the proposal is wrong.

## Floor 8 as measured

A `naive:<name>` control is a patch from base. It meets floor 8 iff, graded
by the task's own verifier, it builds, clears at least one fail-to-pass check
(so it is a real partial fix, not a strawman), and regresses at least one
guard: a check that passes at base and at gold. Guards are the
`pass_to_pass` ids plus any other test the verifier's whole-suite guard wall
reports; the held-out ids themselves are excluded, because a partial fix
leaves some of them failing by definition (see "Tooling" below).

## Positive point: treesitter-query-anchor-matching (admitted F5-c)

The naive controls are the six attempts the frontier models themselves made
and lost at the gate: three GPT-6 Astra trials of 2026-10-18 (from the
first-wave archive in the private backup repo) and three Claude Fable 5.1
trials of 2026-10-22, each diffed against the task base, `.c` and `.h` files
only, as `tests/controls/naive-*.diff`. They are the shortcuts the floor has
to reject, taken from the record rather than invented.

| Control | Agent minutes at the gate | Held-out checks | Guards | Floor 8 |
| --- | --- | --- | --- | --- |
| gold-from-parts | | 11 of 11 | hold | reference |
| naive:astra1-5HSaDCw | 9.9 | 10 of 11 | regressed: anchors_on_both_sides_of_zero_quantifier | **met** |
| naive:astra2-9oZyKar | 12.2 | 10 of 11 | regressed: same | **met** |
| naive:astra3-NnEPBuh | 7.6 | 10 of 11 | regressed: same | **met** |
| naive:fable-MSBrnJS | 33.0 | 10 of 11 | regressed: anchor_after_nested_zero_quantifier | **met** |
| naive:fable-L3VGnaw | 23.0 | 10 of 11 | regressed: same | **met** |
| naive:fable-R2Eua83 | 79.6 | 10 of 11 | hold | not met |

Every shortcut misses the same held-out check,
`test_query_matches_with_leading_anchor_before_zero_quantifier` (unit D).
Five of six also break a guard that passes at base, so the project's own
suite rejects them: floor 8 is met by the task, through five independent
witnesses from two models. The sixth is Fable's longest attempt: it learned
to keep the guards green and still could not reach the held-out case. On
this task the guards stop the fast shortcut and the held-out case stops the
careful one; floor 8 describes the first mechanism only.

Run: `~/vulcanbench-gate/controls/treesitter-floor8-20261024-0738`.

## Negative point: commons-lang-fraction-lowest-terms (rejected, Astra 2 of 2 in 4.3 and 4.4 minutes)

No failed frontier attempt exists for a reject, so the naive controls are
the shortcuts the task's own DESIGN.md predicted ("A per-symptom patch that
reduces in one method, or special-cases MIN_VALUE, leaves a neighbouring
held-out test failing or trips the existing overflow tests"), written from
the base source as `tests/controls/naive-*.diff`:

- `reduce-multiply-only`: reduce both operands at the top of `multiplyBy`,
  nothing else;
- `widen-reduced-factory`: rewrite `getReducedFraction` with `long`
  arithmetic and casts, so `MIN_VALUE` over an even or negative partner
  reduces but `MIN_VALUE / -1` silently wraps instead of throwing;
- `zero-shortcut-reduce`: return the reduced operand from the zero shortcuts
  of `addSub`, negating with a plain minus;
- `string-wrap`: one try/catch around the string factory, rethrowing
  `ArithmeticException` as `NumberFormatException` with the cause attached.

| Control | Held-out checks | Guards | Floor 8 |
| --- | --- | --- | --- |
| gold-from-parts | 6 of 6 | hold | reference |
| naive:reduce-multiply-only | 2 of 6 | hold | not met |
| naive:string-wrap | 1 of 6 | hold | not met |
| naive:zero-shortcut-reduce | 0 of 6 | hold | not met |
| naive:widen-reduced-factory | 0 of 6 | regressed: testReducedFactory_int_int | not met |

The two shortcuts that fix something regress nothing: the 89,196-test suite
accepts them and only the remaining held-out checks say the job is unfinished,
which is exactly the signal a fast agent climbs. The one shortcut that trips
a real guard (the widened factory fails the existing `testReducedFactory_int_int`
overflow case, as DESIGN.md predicted) fixes nothing, so it is a wrong fix
rather than a partial one and the "clears some fail-to-pass" clause excludes
it. No control meets floor 8.

Runs: `~/vulcanbench-gate/controls/commons-lang-floor8-20261024-0740` (first
run, table re-derived from its saved verifier output after the parser fix
below) and `commons-lang-floor8-rerun-*` (the same four controls through the
corrected runner, identical result).

## Reading

- Floor 8 separates the admitted task from the reject on the data available:
  tree-sitter meets it through five frontier shortcuts, commons-lang through
  none of four predicted shortcuts. The stopping rule was not triggered.
- The mechanism is as PLAN-v3 argued: a reject's partial fixes keep the whole
  suite green, so every cheap check the agent can run agrees with its wrong
  premise; the admit's partial fixes turn an existing test red, which is the
  one signal that contradicts the premise.
- Limits. One admit and one reject were calibrated. GEOS, the other reject
  named in PLAN-v3, has no `controls.json` yet (its F5 controls were run by
  hand) and its images rebuild the library with a full test run, so it was
  left for a second negative point. The commons-lang shortcuts were written
  by the author role, as floor 8 intends; the tree-sitter shortcuts were the
  models' own, which is stronger evidence than an author's guess but is only
  available for a task that has already been gated.
- Floor 8 measures the fast-shortcut mechanism. Fable's 80-minute attempt
  shows a second mechanism, a held-out case no shortcut reaches, which the
  floor does not name; PLAN-v3's suggested second route (composition or dual
  held-out checks) is the place to capture it.

## Tooling

`scripts/frontier-v5/run_controls.py` gained, in this change:

- a `naive` list in `tests/controls.json`, run in addition to the generated
  or explicit variants, each a patch from base;
- per-variant reporting of the pass-to-pass checks regressed, read from the
  checker's `pass_to_pass: <id>: <status>` lines and, where a verifier has a
  whole-suite guard wall, its `guard_suite: unexpected failure: <id>` lines,
  with the held-out ids removed;
- a floor-8 verdict line per `naive:` variant and a `floor8` block in
  `controls-report.json`.

The held-out exclusion matters: the commons-lang verifier runs the held-out
`FractionTest` methods inside the pristine suite, so its guard wall lists
them as unexpected failures whenever a partial fix leaves them failing. The
first commons-lang run was parsed without the exclusion and showed two
shortcuts as "regressing a guard"; the re-derivation from the same saved
output and the re-run with the corrected runner both show the table above.
Authors of verifiers with a whole-suite guard wall should expect the same
conflation and never read `guard_suite` alone as a guard regression.

## Recommendation

Adopt floor 8 (PLAN-v3 decision (g)) as calibrated here, with the
`run_controls.py` additions as its measurement, and source the first F5 wave
under it. A second negative point on GEOS is cheap in effort and expensive in
build time; run it when the GEOS images are next built for another reason.
