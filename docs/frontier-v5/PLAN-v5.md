# VulcanBench Frontier v5: composition v5 (release 1.1 board, release 2 shapes)

Planning baseline: November 19, 2026, America/Los_Angeles.
Status: **adopted by the owner in chat on 2026-11-19 and frozen** by
`docs/frontier-v5/FREEZE-v5.json` with the machine-readable `plan-v5.json`.
The decisions are recorded in `docs/DECISIONS.md` (2026-11-19, R1 to R9 and
the fast path). `PLAN.md` to `PLAN-v4.md` and their freezes are not edited.
Composition changes after this need a new plan with its own freeze.

Scope. This document sets the composition of release 1.1 (the first board)
and release 2, and the admission band. The correctness gate (PLAN.md section
3, Part 1), the Harbor-native task contract, the determinism checklists and
every admission and rejection in GATE-LOG.md stand. The rollout is in
[ROLLOUT.md](ROLLOUT.md).

## 1. Release 1.1: the first board

The six public F5 tasks of release 1, at equal weight 1/6:
treesitter-query-anchor-matching (C), black-fmt-skip-region-handling
(Python), prettier-comment-attachment-stability (JavaScript),
javaparser-member-resolution-scope (Java), cli11-toml-config-round-trip
(C++), pulldown-cmark-html-block-scanning (Rust).

- ferrycore-crossing-opaque-parity is not in release 1.1 (R2: F4 dropped).
  Release 1.0.0 and its confirmation stay as published.
- A task whose verifier records no per-check results gets a verifier change
  and a task version bump, so the progress score can be computed.
- Headline (R3): pass rate, complete passes over attempts. Secondary:
  progress score, the fraction of held-out checks passed, 0 unless the
  tamper scan is clean and every guard check passes. The exact rule is
  frozen in the release 1.1 lock PR before any model is scored on it.
- The board is published only if the release 1 measurement shows the
  progress score separates models (fast path, ROLLOUT.md section 4a). It
  is disclosed as one task shape.

## 2. Release 2: four new shapes

Twelve tasks: the six F5 tasks plus six new ones, one per language, so every
language sits in two shapes and no shape exceeds half the suite.

| Slot | Shape | Language |
| --- | --- | --- |
| F3-ts | Long-horizon feature work | TypeScript or JavaScript |
| F3-java | Long-horizon feature work | Java |
| F1-python | Property oracle (stored counterexamples) | Python |
| F1-rust | Property oracle (stored counterexamples) | Rust |
| F6-cpp | Performance under a deterministic cost oracle | C++ |
| F7-c | Diagnosis from field evidence | C |

Shape definitions: ROLLOUT.md section 3. Release 2 has no F2 or F4 slot.
A slot with no admitted task by the release 2 freeze ships empty, and the
headline is computed over the admitted tasks, disclosed; it is never filled
with an easier task to reach the count.

## 3. Admission band (R4, R5)

Correctness gate unchanged. Frontier part:

- **First prong:** GPT-6.2 Sol (Codex 0.166.0) at max effort, five attempts
  (three, then a two-attempt top-up when the first three are not decisive).
  Admit range: 1 to 3 solves of 5. Zero of five is not admitted to release 2
  (too hard for a board); four or five is a reject.
- **Second prong**, only when the first lands in the band: Claude Fable 5.2
  at medium (funded 2026-11-19), at most 1 of 3. DeepSeek V4.1 Flash at
  medium is the fallback reference if Claude runs are unavailable.
- Web search, connected apps and plugins off for every reference, as today.

Release 1.1's six tasks were admitted under the earlier rule (each reference
at most 1 of 3 at medium) and are not re-gated; the measurement records
where they sit against the band.

## 4. Gold and publishing

- Release 2 reference solutions and held-out tests live in
  `morganlinton/vulcanbench-frontier-private`; the public lock holds their
  hashes (R8). Release 1.1's golds stay public as published.
- Runs go through Harbor via `scripts/frontier-v5/run_gate.sh` (R6).
- The board shows pass rate, progress score and speed (serial runs only),
  `n=` and ±1 stderr on every column, a per-task table, and public traces
  for public tasks only; no Code quality panel at launch (R7).
- Launch lineup (R9): GPT-6.2 Sol, Claude Fable 5.2, GPT-6.1 Terra and
  DeepSeek V4.1 Flash, each in its native harness at its highest unblocked
  effort, three attempts per task. The Frontier v4 board is archived the
  day the Frontier v5 board launches.
