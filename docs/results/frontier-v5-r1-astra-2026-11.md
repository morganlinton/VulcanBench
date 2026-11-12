# VulcanBench Frontier v5 release 1 confirmation

**GPT-6 Astra recorded 0 complete passes in 21 attempts (0%). The two-sided
95% exact binomial confidence interval is 0% to 16.1%.** Each of the seven
frozen tasks received three fresh attempts at medium effort. A complete
pass requires verifier reward 1.

**Claude Opus 5.5 was not run due to cost.** The frozen version 1.0.0 lock
names both GPT-6 Astra and Claude Opus 5.5. The owner amended decision (m)
to close release 1 with Astra alone. On November 3, an organization access
error stopped all 21 Opus trials before a model call. Those records are void
and contribute nothing to the score or denominator. [Decision record](../DECISIONS.md#2026-11-10-vulcanbench-frontier-v5-release-1-closes-with-gpt-6-astra-as-its-sole-confirmation-reference)

## Results by task

Attempts 1, 2 and 3 are in start-time order within each task. Minutes measure
agent execution only, rounded to one decimal place. The last column gives the
verifier findings for the six public tasks.

| Task | Agent minutes (1, 2, 3) | Reward (1, 2, 3) | What failed |
| --- | --- | --- | --- |
| treesitter-query-anchor-matching | 13.2, 7.7, 13.6 | 0, 0, 0 | All three missed a leading anchor before a zero quantifier and regressed the case with anchors on both sides of that quantifier. |
| black-fmt-skip-region-handling | 6.0, 4.7, 5.5 | 0, 0, 0 | All three failed the same formatting fixture, a split opening bracket under `fmt: skip`. Existing regression checks passed. |
| prettier-comment-attachment-stability | 15.5, 11.5, 13.0 | 0, 0, 0 | Attempt 1 missed six checks across arrow-chain comments, parenthesized sequences and TypeScript comments. Attempt 2 missed only TypeScript case 19903. Attempt 3 missed only the parenthesized sequence. |
| javaparser-member-resolution-scope | 5.1, 6.1, 5.8 | 0, 0, 0 | All three failed two checks involving an unresolvable ancestor. One checks inherited field visibility and the other checks subclass assignability. |
| cli11-toml-config-round-trip | 2.4, 2.3, 1.9 | 0, 0, 0 | All three passed every defect check and failed the same two held-out multiline TOML guards, one for binary content and one for text after quotes. |
| pulldown-cmark-html-block-scanning | 2.5, 2.4, 3.6 | 0, 0, 0 | All three missed the same six regressions covering mismatched HTML end tags (259, 260, 261) and inline HTML scanning (273, 274, 277). Existing regression checks passed. |
| ferrycore-crossing-opaque-parity | 38.7, 38.1, 17.2 | 0, 0, 0 | |

## Run conditions and uncertainty

The confirmation ran at `bench/2026-11-02-frontier-v5-r1-confirmation`
(`27ed21e1`) on Harbor 0.24.0 with Codex CLI 0.159.0 and model
`gpt-6-astra`. Attempts ran one at a time with early exit disabled. The
session audit found real tool use, no connected-app or web calls, full
verifier output and clean tamper scans on all 21 attempts. The source jobs
are `~/vulcanbench-confirm/jobs/astra-*-confirm`. Times come from each
trial's `result.json` under `agent_execution`. Rewards and public failure
descriptions come from the trial verifiers. [Frozen suite lock](../../tasks/frontier-v5/suite.lock.json)

With zero passes, the Clopper-Pearson upper limit is
`1 - 0.025^(1/21) = 0.16110`. This interval treats the 21 attempts as
independent trials with a common pass probability. The seven tasks were
selected through admission gates, and each was tried three times. The
interval does not quantify uncertainty across a broader population of tasks.

## Admission caveats

- **CLI11.** Both references passed all 11 defect checks at admission. Only
  two held-out regression guards kept them from a complete pass. The same
  two guards failed in all three Astra confirmation attempts.
- **pulldown-cmark.** The task missed floor 8, the pre-gate requirement for
  a guarded fix or combination coverage. Decision (g') allowed admission on
  gate performance with that miss recorded. Opus passed one of five
  admission attempts, below the two-of-five threshold. [Admission record](../frontier-v5/GATE-LOG.md)
- **Prettier.** After admission, the tamper scan was fixed and revalidated.
  It had flagged legitimate TypeScript directive comments in two gate
  attempts. Rescanning cleared both flags. Both attempts also failed
  held-out checks, so their gate rewards stayed zero. [Verifier revalidation](../../tasks/frontier-v5/prettier-comment-attachment-stability/DESIGN.md)
- **Tree-sitter.** The Astra admission gate ran on October 18 using an
  earlier build of the v5 sandbox images. The lock records the October 22
  image builds used for confirmation. [Image provenance in the frozen lock](../../tasks/frontier-v5/suite.lock.json)

For any later Opus comparison, the prepared setup calls for a second Claude
account and the later tag `bench/2026-11-10-frontier-v5-r1-confirmation-opus`
(`87d23f69`). Only `run_gate.sh` differs on the gate execution path between
the two tags. No scored Opus trial ran under that setup.
