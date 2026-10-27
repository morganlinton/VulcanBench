# VulcanBench Frontier v5: composition v3 proposal (F5 guard floor, F2 reference shape)

Planning baseline: October 23, 2026, America/Los_Angeles.
Status: **proposal for the owner's decision, not frozen.** If adopted, it is
frozen by `docs/frontier-v5/FREEZE-v3.json` together with a machine-readable
`plan-v3.json`, and a dated `docs/DECISIONS.md` entry records the decision
citing the 2026-10-10 revisit clause. `PLAN.md`, `PLAN-v2.md` and their
freezes are not edited, as v2's own policy requires ("Capacity, weights or
slot changes require a v3 plan with its own freeze"). Until the owner decides,
nothing in this file governs a task, a gate run or a slot, and no model
invocation is requested by it.

Scope. This document amends exactly two things:

1. the F5 admission floors (PLAN-v2.md section 2) gain floor 8, met by a
   guarded fix (shown by a naive-fix control) or by combination coverage
   (shown by unit controls), as amended on 2026-10-26 (section 2);
2. the F2 definition (PLAN.md section 2) gains a reference-shape floor with a
   build probe, with two alternatives laid out for the owner.

Everything else is unchanged: 26 tasks at equal weight 1/26, the family and
language capacity tables of v2, the admission gate (PLAN.md section 3, both
parts), the Harbor-native task contract, the determinism checklists, the
sequencing and the resolved owner decisions. Admissions and rejections
already recorded in GATE-LOG.md stand: treesitter-query-anchor-matching
keeps F5-c, and the six rejected candidates stay rejected.

## 1. What the gate measured (2026-10-18 to 2026-10-22)

Seven candidates have been through the gate (docs/frontier-v5/GATE-LOG.md).
Agent minutes are covariates only.

| Candidate | Slot | GPT-6 Astra | Claude Fable 5.1 | Outcome | Anatomy |
| --- | --- | --- | --- | --- | --- |
| nx-digraph-node-connectivity | F5-python | 2 of 2 (about 6, 6) | not run | REJECT | every cause found from symptoms |
| geos-curved-overlay-arc-noding | F5-cpp | 2 of 2 (4.5, 4.2) | not run | REJECT | same |
| ioverlay-hole-binding-extraction | F5-rust | 2 of 2 (6.2, 7.3) | not run | REJECT | same |
| tseslint-type-params-mapped-types | F5-javascript | 2 of 2 (12.2, 10.8) | not run | REJECT | same |
| commons-lang-fraction-lowest-terms | F5-java | 2 of 2 (4.3, 4.4) | not run | REJECT | same |
| treesitter-query-anchor-matching | F5-c | 0 of 3 (9.9, 12.2, 7.6) | 2 of 5 (33.0, 27.3, 23.0, 79.6, 27.2) | **ADMIT** | all six failures miss unit D; five also break a guard |
| comrak-gfm-tables-autolinks-parity | F2-rust | 2 of 2 (11.1, 8.2) | not run | REJECT | reference compiled in the sandbox, used as an oracle |

Three findings carry the proposal.

**F5's floors are met by every candidate and discriminate nothing.** All six
met floors 1 to 7 (several causes, concentration, interaction shown by
controls, leave-one-out necessity, no dominant fix, enough signal, the
contract), measured with the real verifiers before the gate. Five fell to one
reference in 4 to 12 minutes against a 3-hour budget. Time to solve did not
track cause count or interaction: geos (five causes, two coupled files) fell
in 4.5 minutes, tseslint (four causes) in 12. Interaction depth is not the
lever the 2026-10-10 decision hoped for.

**The one that held has guard tests that punish the obvious fix.** tree-sitter
has two upstream tests (added by dfcf7392 and 42f33fe2) that pass at base and
at gold but fail under the symptom-level fix "an anchor after a zero-matched
quantifier is vacuous". DESIGN.md named that as the predicted miss before the
gate. At the gate, all six failed attempts across both references (three
Astra, three Fable) missed the same held-out case, unit D (a leading anchor
before a zero-matched quantifier), and five of the six also regressed one or
both guards. The two Fable solves took 27 minutes each and are two different
designs from each other and from upstream; Fable's failures took 23 to 80
minutes. Nothing else distinguishes tree-sitter from the five rejects: it
has more units (eight) but a smaller gold than some of them.

**F2's premise fails when the reference can be run.** PLAN.md defines F2 as
"reading is allowed; reconstructing at volume is the work" and ships "no
reference binary". Both Astra solves of comrak read the two C extensions,
wrote the Rust modules, and then built the reference themselves: `cmake` was
not installed (exit 127), so each generated `config.h` from `config.h.in`
with a Python one-liner and compiled cmark-gfm with the `gcc` that every
sandbox image inherits from `Dockerfile.base` (`build-essential`). From there
each ran thousands of random inputs through both binaries and fixed every
mismatch. The ports are genuine (4 to 7 percent of lines in common with
upstream's files), and the whole thing took 8 and 11 minutes for an 830-line
gap with a 5,410-case corpus. Differential parity became "diff until
identical" the moment the reference was executable.

**On the revisit clause.** The 2026-10-10 decision says to revisit "if F5
candidates are solved 3/3 by both references". That literal condition is not
met: the early exit stops at 2 of 2, and the Fable half never ran on the five
rejects. The spirit is met: five of six F5 candidates fell to one reference
in minutes, and the sixth held only on a property the floors do not name.
The owner can decide on this evidence, or first run the Fable half on the
five (about ten attempts with early exits; it also decides their Routine v1
recycling under PLAN.md section 3). This proposal is written so either path
leads to the same decision points.

## 2. Proposal A: F5 floor 8, a guarded fix or combination coverage

### Definition

Add to the F5 floors of PLAN-v2.md section 2:

8. **Guarded fix or combination coverage.** A candidate meets floor 8 by
   either route below; both are measured before the gate with the task's real
   verifier, and DESIGN.md records which route it meets (or both).

   **Route (a), guarded fix.** The obvious fix is wrong in a way the project's
   own tests detect. Before the gate, the author writes the *naive fix*: the
   symptom-level patch a competent reader of the instruction and the failing
   behaviour would write first, which addresses the reported symptoms where
   they surface without the structural change the upstream fix made. It is
   pre-registered in DESIGN.md as the predicted shortcut (one patch per
   predicted shortcut when there are several) and stored under
   `tests/controls/naive-<name>.diff`. Measured with the task's real
   verifier, the naive fix must
   - build (`build_ok` 1), so it is a real patch and not a strawman;
   - clear at least one fail-to-pass check, so it is a real partial fix of
     the reported symptoms;
   - fail at least one pass-to-pass check that passes at base and at gold
     (the guard), so the task's own tests reject it.

   A task with several predicted shortcuts needs at least one shortcut that
   trips a guard; the others are recorded. The guard checks that catch the
   naive fix are named in DESIGN.md, and they stay in `pass_to_pass`, never
   in the visible tests.

   **Route (b), combination coverage.** At least one fail-to-pass check that
   no single repair unit clears and no pre-registered naive fix clears. A
   repair unit is one cause's part, or a source-coupled group of parts (the
   task's `coupled` groups, which cannot apply apart) applied together, each
   alone on base. Such a check passes only when two or more independent
   causes are fixed together, so no partial repair the visible suite could
   confirm reaches it. The checks are named in DESIGN.md and stay out of the
   visible tests, like every held-out case.

   Route (b) counts units, not single parts: counted by single parts and
   naive fixes only, the rejected commons-lang task has one uncovered check
   (`testAddSubtractZeroOperand`), but its coupled pair #1784 and #1787
   clears it, so it is one cause's repair, not a combination.

### Amendment of 2026-10-26 and its measurement

The owner directed this amendment on 2026-10-26 after two wave-1 tasks
missed route (a): pulldown-cmark held GPT-6 Astra at 0 of 3 (GATE-LOG.md,
Astra half only) with no naive fix tripping a guard, and the sqlparser-rs
reserve missed it the same way. Route (b) was added so a task can qualify on
held-out coverage instead of a guard. It was then measured on every F5 task
with a gate record or a full control set, with `run_controls.py`'s `unit:`
variants (one per coupled group) and local Docker only, no model calls:

| Task | Gate record | Route (a): naive fix trips a guard | Route (b): checks no unit or naive fix clears | Floor 8 |
| --- | --- | --- | --- | --- |
| treesitter-query-anchor-matching | ADMIT (Astra 0 of 3, Fable 2 of 5) | met, five of six model shortcuts | not measured (no per-cause unit run) | met |
| black-fmt-skip-region-handling | ADMIT (Astra 0 of 3, Opus 5.5 0 of 3) | met (`climb-any-parent`) | 1 of 7 (`fmtskip_opening_bracket_conditional`) | met, both routes |
| commons-lang-fraction-lowest-terms | REJECT (Astra 2 of 2) | not met | 0 of 6 | not met |
| pulldown-cmark-html-block-scanning | Astra 0 of 3, Opus 5.5 half not run | not met | 0 of 24 (the three checks no single part clears are cleared by the coupled pair #1163 and #1180) | not met |
| sqlparser-display-round-trip | not gated | not met | 0 of 8 (both coupled pairs clear the checks no single part clears) | not met |

Reading:

- The amended floor still excludes the reject, as PLAN-v3's stopping rule
  requires.
- It does not admit pulldown-cmark or sqlparser. On both, every held-out
  check is reached by some single unit of repair; what held Astra on
  pulldown-cmark is coverage of a different kind (every attempt kept the
  1,388 guards green and declared itself done while missing cases of the
  rules it had fixed), which neither route measures.
- So on the five tasks measured, routes (a) and (b) give the same verdicts,
  and route (b) is a second witness rather than a wider door. Whether a task
  that misses both routes but holds the gate (pulldown-cmark, if the Opus 5.5
  half also holds) may be admitted is decision (g') in section 6.

Floors 1 to 7 stay as they are. They did not admit a weak task; they were
simply not sufficient, and they still rule out independent-fix breadth
(luxon, fmt, yyjson) and single-cause tasks (petgraph).

### Tooling

`scripts/frontier-v5/run_controls.py` already runs explicit variants from
`tests/controls.json` and grades each with the real verifier. Two small
additions, no model call involved:

- accept `naive:<name>` variants and report, per variant, the fail-to-pass
  checks cleared **and the pass-to-pass checks regressed** (today's table
  shows only held-out checks passing);
- a floor-8 line in the report: pass iff some `naive:` variant has
  `build_ok` 1, clears at least one fail-to-pass check and regresses at least
  one pass-to-pass check;
- `unit:<a+b>` variants, one per `coupled` group (base plus that group's
  parts together), so route (b) can be read off the single, unit and naive
  rows of the report.

### Calibration before sourcing (no quota)

- Run `naive:vacuous-anchor` on treesitter-query-anchor-matching: expected to
  pass floor 8 (DESIGN.md predicted it; the gate confirmed it). This is the
  positive calibration point and is recorded on the admitted task without
  re-gating it.
- Run a naive variant on one or two rejects (geos and commons-lang have
  per-cause parts already): expected to fail floor 8, because every gate
  attempt on them cleared every guard. This is the negative calibration
  point. If a reject passes floor 8, the floor is not the discriminator and
  this proposal is wrong; record that and stop.

### Sourcing under floor 8

The floor narrows what qualifies, on purpose. Look for upstream histories
where the obvious fix was tried and rejected: a first PR reverted or amended
("regresses X"), regression tests whose names negate the obvious rule
("does_not", "still_", "keeps_"), maintainers asking for a structural fix
over a local one, and tests added alongside the fix that pass before it (they
exist to pin behaviour the fix must not change). Plan on three candidates per
open slot (fifteen for F5-cpp, java, python, rust and javascript), each with
floors 1 to 8 measured before its first gate run, gated Astra-first so the
early exit keeps rejects cheap. The halt rule of PLAN.md section 7 (three
consecutive rejects in one family, then a post-mortem before continuing)
applies to the wave.

### Why not raise floors 1 to 6 instead

Because the rejects met them with margin and fell anyway, and because the
admitted task does not stand out on any of them. Raising cause counts or
interaction floors would reject tree-sitter-shaped tasks as readily as the
others while admitting nothing new.

## 3. Proposal B: F2 reference shape

### The problem

F2's hidden corpus "covers behavior reachable only by reading the reference
at depth". With the reference executable in the sandbox, nothing is reachable
only by reading: the agent fuzzes. comrak shows this needs no build system,
only a compiler for the reference's language, and `Dockerfile.base` gives
every image gcc, g++, make, python3, perl and node. A reference in C, C++,
Python, JavaScript or Perl is therefore runnable in every task image today.

### Option B1 (recommended): the reference is not runnable in the agent image

Add to the F2 definition:

> The agent image cannot build or run the reference. Verified at validation
> by a build probe: a script that attempts, inside the agent image, what an
> agent would (generate any configuration header by hand, invoke every
> compiler and interpreter present on the reference's sources, run any
> shipped binary or script entry point). The probe must fail, and its
> transcript is recorded in DESIGN.md. The agent image must still build the
> target offline, shown by the existing validation.

Consequences:

- The reference's language must have no toolchain in the task's agent image.
  With today's images that excludes C, C++, Python, JavaScript and Perl
  references everywhere, Rust references in the Rust image, Java references
  in the JVM image. Candidates: references in Go, OCaml, Haskell, C#, Zig,
  Nim, Scala or Kotlin (the last two only outside the JVM image), or a
  per-task image that removes a toolchain the target does not need (a pure
  Rust target whose pinned dependencies have no build scripts can lose gcc;
  the build probe and the offline target build both have to pass).
- The residual route is for the agent to port the reference itself into a
  language it can run. That costs about the task's own volume, which is the
  work F2 was meant to measure; it is acceptable.
- comrak as built does not meet B1 (C reference, gcc present). It would need
  the C toolchain removed from its agent image, which is only possible if
  comrak and its pinned dependencies build offline without a C compiler
  (to be probed, not assumed).

### Option B2: accept the oracle and raise the volume floors

Keep executable references and make the port long-horizon: a reconstruction
gap of at least 3,000 upstream lines (comrak's was 830), a hidden corpus of
at least 50x the visible sample, and at least three interacting subsystems
in the gap. This is DESIGN.md's own revisit rule for comrak ("the slot should
take a larger subsystem"). Risks: it measures porting speed against an oracle
rather than reading at depth, it drifts toward F3, and v4's env-orderflow
post-mortem found exploration cost near zero at 1.7k lines, so the volume
needed to hold a frontier agent with an oracle in hand is unknown and may be
large.

### Option B3: shrink F2 to a pilot slot or fold it into F3

A composition change (slots and weights), permitted in a v3 plan but not
recommended before B1 has been tested: it gives up on the one family built
around reading a large foreign implementation.

### The cheapest diagnostic (quota, owner's call)

comrak-v2: the same task with the C toolchain removed from the agent image,
if the target builds offline without it, gated Astra-first (three attempts,
early exit). If it holds, the oracle was the collapse and B1 is confirmed on
the one task that exists. If it falls in minutes again, volume is the
problem and B2's floors are the next lever. Six gate runs at most decide
between B1 and B2 before any new F2 candidate is sourced.

## 4. Composition (unchanged from v2)

26 tasks at equal weight 1/26; with three confirmation attempts the headline
is complete passes divided by 78.

| Family | Slots | Weight | Change in v3 |
| --- | --- | --- | --- |
| F1 Tool-provided oracles | 6, one per language | 6/26 | none (sourcing route still open: defects that ship a reproducer; fuzzing was a dead end) |
| F2 Differential parity | 6, one per language | 6/26 | reference-shape floor (Proposal B) |
| F3 Long-horizon volume | 6, one per language | 6/26 | none |
| F4 Opaque-component parity | 2, Python and Java | 2/26 | none |
| F5 Concentrated multi-bug correctness | 6, one per language | 6/26 | floor 8, either route (Proposal A) |

Slot state at the proposal date: F5-c admitted (tree-sitter); every other
slot open; six rejected candidates recycled or held per PLAN.md section 3
once their Fable half is run or waived.

## 5. Cost

- Proposal A: fifteen F5 candidates at three per open slot, each with
  controls (including the naive fix) before the gate; at the yield seen so
  far (one admit in six F5 candidates) that is about the number needed to
  fill five slots, with little margin. Gate cost 3 to 10 runs per candidate.
- Proposal B: six gate runs for comrak-v2, then F2 sourcing at the B1
  constraint, which is harder to source (non-mainstream reference languages)
  but cheaper to validate (the build probe is a script).
- Tooling: the run_controls.py additions and the build probe are a day of
  work with no model calls.

## 6. Owner decisions

(g) **F5 floor 8.** Decided by the owner on 2026-10-26: amend floor 8 so
either route of section 2 qualifies, and continue wave 1. Calibrated as
above; it takes effect with the v3 freeze.

(g') **A task that misses floor 8 and holds the gate.** Floor 8 is a
pre-gate screen, and the gate decides admission. pulldown-cmark misses both
routes and held Astra; sqlparser misses both and is not gated. Options:
keep floor 8 binding (neither is gated further, and pulldown-cmark's Astra
half stands as evidence only), or let a floor-8 miss be gated at the owner's
discretion and admitted on the gate alone, with the miss recorded. The
second costs little when the task is weak (Astra-first, early exit at 2) and
keeps a task that holds; the first keeps the floor meaningful as a filter.

(h) **F2 shape.** B1, B2 or B3. Recommended: B1, decided after the comrak-v2
diagnostic if the owner funds it, otherwise on the comrak evidence alone.

(i) **comrak-v2 diagnostic.** Fund six Astra gate runs, or not.

(j) **Fable half on the five F5 rejects.** Run it first (about ten attempts;
also settles their Routine v1 recycling) to meet the 2026-10-10 revisit
clause literally, or decide on the Astra evidence.

Still open from v1 and unchanged by this proposal: (d) publishing channel,
(f) gold location.

## Recommended next step

Without spending quota: add the naive-fix variant and the guard-regression
column to run_controls.py, run the calibration on tree-sitter and geos, write
the F2 build probe and try it on comrak's image with the C toolchain removed.
Those results decide (g) and (h) in a sitting. Then source the first F5 wave
under floor 8, five candidates, one per open language, pre-registered with
their naive fixes before any gate run.
