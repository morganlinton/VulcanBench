# VulcanBench Frontier v5: composition v2, a fifth family

Planning baseline: October 10, 2026, America/Los_Angeles.
Status: frozen by `docs/frontier-v5/FREEZE-v2.json` together with the
machine-readable table in `plan-v2.json`. Once FREEZE-v2.json is written this
file is immutable.

This document replaces exactly one part of the frozen
[v1 plan](PLAN.md): its composition (section 2, the family table, slot
capacity, weights, family definitions and the language view). Everything
else in v1 (sections 1 and 3 to 8: the admission gate, the Harbor-native task
contract, the six languages, the determinism checklists, the sequencing and
the owner decisions with their DECISIONS.md resolutions) applies unchanged
to all five families. The v1 files are not edited and still match the hashes
in `FREEZE.json`, as v1's own rule requires ("a change to capacity, weights,
slot assignment or family definitions requires a separate v2 plan with its
own freeze"). No task is admitted or scored by this document, and no model
invocation is requested by it.

## 1. Why a fifth family

The v1 family table had no home for the task shape that actually beat the
frontier in v4 and that v5 authoring kept producing:

- The single v4 task that beat the stronger reference,
  `oss-networkx-digraph-node-cuts` (Opus 5 solved 0/3, codex 1/3), was a few
  interacting algorithmic defects in one surface, not volume, not opacity.
- Of the ten v5 tasks built and validated by 2026-10-09, the four judged
  hardest by design are of that shape (two NetworkX arcs, a commons-lang
  Fraction arc and a GEOS curved-overlay arc) and all sat UNSLOTTED, because
  each is far below the F3 volume floor (12 files, 1,000 changed lines) that
  v1 deliberately does not lower.
- The GEOS task measured the property directly: five fixes in two files, no
  single fix clears more than 5 of its 16 held-out cases, and leaving any one
  fix out of the gold leaves at least one failing.

The owner's open question (NIGHT-STATUS.md, "The design question") offered a
fifth family, a mid-band suite, or F3-only admission. The owner chose the
fifth family on 2026-10-10, at six slots (one per language) with every task
at equal weight.

## 2. Composition v2

26 tasks, every task at equal weight 1/26. With three confirmation attempts
per task the overall score equals complete passes divided by 78.

| Family | Slots | Weight | Oracle | What resists collapse |
| --- | --- | --- | --- | --- |
| F1 Tool-provided oracles | 6, one per language | 6/26 (23.1%) | as v1 | as v1 |
| F2 Differential parity against a source-available reference | 6, one per language | 6/26 (23.1%) | as v1 | as v1 |
| F3 Long-horizon volume | 6, one per language | 6/26 (23.1%) | as v1, floors unchanged | as v1 |
| F4 Opaque-component parity (v4 lineage) | 2, Python and Java | 2/26 (7.7%) | as v1 | as v1 |
| **F5 Concentrated multi-bug correctness** | **6, one per language** | **6/26 (23.1%)** | the pinned upstream project's own tests as held-out fail-to-pass checks, plus its full suite as the guard wall | Several root causes that interact inside one subsystem, found from symptoms alone; a local patch for one symptom does not clear the others |

Family and slot definitions for F1 to F4 are v1's, word for word. The v1
constraints still hold for all five families: two tasks in one language sit
in different families; two tasks in one family exercise different
obligations; no easy anchors (a task both references solve is recycled to
Routine v1 with its gate record).

### Language capacity

| Language | F1 | F2 | F3 | F5 | F4 | Slots |
| --- | --- | --- | --- | --- | --- | --- |
| Python | 1 | 1 | 1 | 1 | 1 | 5 |
| JavaScript / TypeScript | 1 | 1 | 1 | 1 | 0 | 4 |
| Rust | 1 | 1 | 1 | 1 | 0 | 4 |
| C | 1 | 1 | 1 | 1 | 0 | 4 |
| C++ | 1 | 1 | 1 | 1 | 0 | 4 |
| Java | 1 | 1 | 1 | 1 | 1 | 5 |

The per-language subscore is reported over F1, F2, F3 and F5 (four tasks
each, 4/26 of the suite each), so the six language views stay directly
comparable. F4 is still its own line, never folded into a language view.

### F5 definition and admission floors

**F5 Concentrated multi-bug correctness.** Sourced from a complete pinned
upstream repository: several upstream bug fixes, each with its own
regression test, composed at a workspace where all of them are absent. The
agent gets symptoms only and must find and fix every root cause without
breaking the project's full test suite. Floors, all pre-registered and
checked before a gate run:

1. **Several causes.** At least three distinct root causes fixed upstream,
   in one or several merged changes, with upstream regression tests that
   exercise them, merged after the reference models' training cutoffs as far
   as can be established (recorded per task, a risk where unknown). When one
   upstream change fixes several causes, the gold is split into per-cause
   parts for the controls below.
2. **Concentration.** All causes live in one subsystem: one class or module,
   one algorithm family, or at most three tightly coupled source files.
3. **Interaction.** At least one of: two fixes coupled at the source level
   (one does not apply, or does not work, without the other), or a held-out
   check that passes only with two or more fixes together. Shown by
   controls, not argued.
4. **Necessity.** Leave-one-out controls: removing any single fix (or
   per-cause part) from the gold leaves at least one fail-to-pass check
   failing. Fixes that are coupled at the source level are left out together.
5. **No dominant fix.** Single-fix controls: no fix alone clears more than
   half of the fail-to-pass checks.
6. **Enough signal.** At least five fail-to-pass checks.
7. **Contract.** Symptoms-only instruction (no file, function or commit
   references); the full upstream suite as the guard wall; the three
   verifier layers and cheating probes from PHASE1.md ("Verifier audit
   2026-10-08") as for every family.

A workspace may be the earliest fix's parent, or the last fix's commit with
only the touched files restored to their pre-fix state when that is the only
way to keep every upstream test usable unchanged (the GEOS design); the
choice and its trade-off are recorded in the task's DESIGN.md. Visible tests
never contain a held-out case, and a held-out case is deleted from them,
never reverted to an old expectation.

## 3. Slot candidates at freeze (not admissions)

Slotting names the slot a task is built for. Admission still requires
v1's full gate (section 3 of v1); a candidate that misses an F5 floor stays
unslotted.

| Slot | Candidate(s) | F5 floor evidence at freeze |
| --- | --- | --- |
| F5-cpp | geos-curved-overlay-arc-noding | All floors met and measured: 5 causes, 2 coupled files, #1546 does not apply without #1480, no single fix clears more than 5 of 16, leave-one-out each necessary. |
| F5-java | commons-lang-fraction-lowest-terms | 5 causes in `Fraction`; #1787 does not apply alone (coupled at the source level); single-fix controls each leave 4 or 5 of 6 failing. One of five leave-one-out controls run (gold minus #1787 fails exactly one check); the other four still to run. |
| F5-python | nx-digraph-node-connectivity; nx-group-betweenness-epic | nx-digraph: five causes in node connectivity fixed by one upstream PR (#8837), so its gold must be split per cause for controls. nx-group: six upstream PRs against `group_betweenness_centrality`, two of them (#8880, #8882) expected to interact. Single-fix and leave-one-out controls still to author for both. One slot: the gate decides; the other is recycled or held as a replacement. |
| F5-javascript | none | Open. luxon is three independent fixes and does not qualify. |
| F5-rust | none | Open. petgraph is a single easy cause and does not qualify. |
| F5-c | none | Open. yyjson's causes sit in three separate subsystems (incremental reader, mutable iterator, file writer), so it fails the concentration floor. |

Tasks that remain UNSLOTTED after v2: sympy-monotonic-sign-signed-groups (a
single upstream change, two fail-to-pass checks), luxon, fmt and yyjson
(independent or spread causes, the breadth shape), and petgraph. They keep
their validation records and are Routine v1 candidates.

## 4. Cost

Six more admits than v1. At v1's yield assumption (about half of candidates
admit) that is roughly twelve more candidates and 36 to 60 more gate runs.
Three of the six F5 slots already have built candidates, which is the
cheapest start of any family.

## Recommended next step

Run the missing controls on the slotted candidates (leave-one-out for
commons-lang, single-fix and leave-one-out for the two NetworkX arcs), so
every F5 candidate has its floors measured before the first gate run.
