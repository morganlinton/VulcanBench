# VulcanBench Frontier v5: composition v4 proposal (release 1 is the F5 family)

Planning baseline: October 31, 2026, America/Los_Angeles.
Status: **proposal for the owner's sign-off, not frozen.** The owner chose
the direction in chat on 2026-10-31 ("ship a smaller v5"); this document is
the composition that direction implies. If adopted, it is frozen by
`docs/frontier-v5/FREEZE-v4.json` with a machine-readable `plan-v4.json`, and
a dated `docs/DECISIONS.md` entry records the decision. `PLAN.md`,
`PLAN-v2.md`, `PLAN-v3.md` and their freezes are not edited ("capacity,
weights or slot changes require a new plan with its own freeze"). Until the
owner signs off, nothing here governs a task, a gate run or a slot.

Scope. This document changes the composition only. The admission gate
(PLAN.md section 3), the F5 floors (PLAN-v2.md section 2 and PLAN-v3.md
Proposal A), the Harbor-native task contract and the determinism checklists
are unchanged. Every admission and rejection in GATE-LOG.md stands.

## 1. Why

Between 2026-10-18 and 2026-10-31 the gate admitted six tasks, all in family
F5, and rejected every candidate from the other families on the GPT-6 Astra
half alone:

| Family | Candidates gated | Admitted | How the rejects fell |
| --- | --- | --- | --- |
| F5 Concentrated multi-bug | 13 | **6 of 6 slots** | the six wave-0 rejects in 4 to 12 minutes; Boost.URL with a self-written RFC oracle |
| F2 Differential parity | 2 | 0 | comrak: the shipped C reference compiled into an oracle; phonenumbers (shape B1): a public port, ported from the Java reference in 5 to 7 minutes |
| F3 Long-horizon volume | 1 (pilot) | 0 | apollo: 1,042 changed lines, one contract test per named symptom, about 22 minutes each |
| F4 Opaque-component parity | 1 so far (two more in the gate) | 0 so far | ferrycore 0.1.0: the binary disassembled with a hand-written x86-64 decoder |
| F1 Tool-provided oracles | 0 | 0 | not sourced (Track B, Opus 4.8; fuzzing was a dead end) |

The gate also found two routes to the internet that the network allowlist
cannot see (Codex's `web__run`, then the account's connected apps). Both are
closed for every gate run from 2026-10-31 (DECISIONS.md 2026-10-30 and
2026-10-31). Web access can only add solves, so no admission is affected.

The F5 shape (several interacting defects concentrated in one subsystem,
held-out checks, a guard wall that punishes the blunt fix) is the only shape
measured to hold both references. The 26-slot plan needs 20 more admits in
four families whose shapes have not held once. At the yield seen (6 admits
from 17 candidates, all in one family) that is weeks of sourcing and on the
order of 200 gate runs, well past the owner's target of finishing this week.

## 2. Proposal: release 1 is the F5 family, release 2 carries F1 to F4

**Frontier v5 release 1** (this proposal):

| Slot | Task | Admitted | Record notes |
| --- | --- | --- | --- |
| F5-c | treesitter-query-anchor-matching | 2026-10-22 | Astra 0 of 3, Fable 5.1 2 of 5 (at the bar); its Opus 5.5 prong is decision (k) |
| F5-python | black-fmt-skip-region-handling | 2026-10-25 | Astra 0 of 3, Opus 5.5 0 of 3 |
| F5-javascript | prettier-comment-attachment-stability | 2026-10-27 | Astra 0 of 3, Opus 5.5 0 of 3; tamper-scan fix re-validated |
| F5-java | javaparser-member-resolution-scope | 2026-10-29 | Astra 0 of 3, Opus 5.5 0 of 3 |
| F5-cpp | cli11-toml-config-round-trip | 2026-10-30 | Astra 0 of 3, Opus 5.5 0 of 3; **held only by two held-out guards**, flag kept by the owner |
| F5-rust | pulldown-cmark-html-block-scanning | 2026-10-30 | Astra 0 of 3, Opus 5.5 1 of 5; **floor 8 not met**, admitted under (g') |

Six tasks at equal weight 1/6; with three confirmation attempts per task the
headline is complete passes divided by 18. One task per language, so the
language view is per task and is reported as such, with no per-language
subscore.

**Optional F4 extension (decision (l)).** If bothycore-booking-opaque-parity
(F4-java) or ferrycore-crossing-opaque-parity 0.2.0 (F4-python) is admitted
by the end of 2026-11-04, it joins release 1 as a second family, reported on
its own line (as F4 always was), and the headline becomes passes divided by
three times the task count. ferrycore 0.2.0 was redesigned after its 0.1.0
gate result; its verdict records that.

**Release 2** (a later plan): F1, F2, F3 and the remaining F4 slot, sourced to
the lessons below, gated with web tools and connected apps off, and frozen
with their own plan. Release 1 results are not renormalized when release 2
lands; release 2 is a separate suite version with its own headline.

This is a composition change made in a plan with its own freeze, not a
renormalization over surviving slots, so PLAN.md's freeze rule (never
renormalize; withhold the headline on a missing slot) is respected: release 1
has no missing slot.

## 3. Lessons carried to release 2

- **Provider-side tools.** Gate references run with web search, connected
  apps, plugins and browser tools off (`scripts/frontier-v5/codex-gate.toml`,
  `--ak disable_web_search=true`), on a dedicated Codex login with no
  connected apps once the owner creates it. Any trial that reaches such a
  tool is an infrastructure failure, not a result.
- **F2.** B1 (the agent cannot run the reference) is necessary but not
  enough: the completed target must not be public anywhere, and the
  reconstruction gap must be well past 3,000 lines.
- **F3.** The volume floor did not hold when the instruction names each
  symptom. Release 2's F3 needs work beyond one sitting (several thousand
  lines or several subsystems that must change together) and symptoms that
  do not map one to one onto tests.
- **F4.** A shipped binary can be read in minutes, and long fuzzing reaches
  most rare behaviours. The query-only service with a per-trial budget is
  the design under test now.
- **F1.** Unsourced; it stays Track B (a dedicated Opus 4.8 session), starting
  from defects that ship a reproducer.

## 4. Freeze and confirmation (release 1)

1. Merge the admitted tasks and the gate record (their branches are pushed:
   `frontier-v5-javaparser`, `frontier-v5-cli11`, `frontier-v5-pulldown-cmark`,
   `frontier-v5-prettier`, `frontier-v5-black-fmt-skip`,
   `frontier-v5-fable-gate`, `frontier-v5-gate-black-record`) and the gate
   integrity fix (PR #248), each in its own PR, engine and results apart.
2. Write `tasks/frontier-v5/suite.lock.json` over the six (or seven or eight)
   tasks: task hashes, image digests, checklist versions, reference model
   IDs and CLI versions.
3. Confirmation runs (decision (m)): three fresh attempts per task per
   reference at unchanged settings, from a run worktree pinned to a bench tag
   cut from main after the merge, separate from every gate run. At the gate
   rule, release 1 is expected to score low for both references; that is the
   point of the suite.
4. Publish the per-task results with uncertainty, the record notes above,
   and the admission caveats (CLI11's guard-only hold, pulldown-cmark's
   floor-8 miss).

## 5. Cost

- Release 1 freeze: no model calls; a day of merge and lock work.
- Confirmation (decision (m)): 6 tasks x 2 references x 3 attempts = 36
  runs (48 with both F4 tasks), at the medians seen so far about 15 to 20
  hours of serial machine time, inside the week at one attempt at a time.
- Decision (k): 3 Opus 5.5 runs on tree-sitter.

## 6. Owner decisions

(j) **Adopt this composition** for Frontier v5 release 1, or keep the
    26-slot plan.

(k) **tree-sitter's Opus 5.5 prong.** Run it for comparability with the other
    five (it was admitted on Fable 5.1, before Opus 5.5 became the second
    reference), or keep the Fable record as its admission.

(l) **F4 extension.** Admit bothycore and ferrycore 0.2.0 into release 1 if
    they clear the gate by 2026-11-04, or hold F4 for release 2 regardless.

(m) **Confirmation runs.** Fund the 36 (or 48) confirmation runs this week,
    or publish release 1 on the gate record alone with confirmation to follow.

(n) **Gate login.** Create the dedicated Codex login with no connected apps
    before the confirmation runs (`CODEX_HOME=~/.config/vulcanbench/codex-gate
    codex login`; `run_gate.sh` picks it up).
