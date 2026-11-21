# VulcanBench Frontier v5 release 1: GPT-6.2 Sol at max effort

Measured November 19, 2026 (America/Los_Angeles). This is the top-effort
measurement that decision R3 calls for (docs/DECISIONS.md, 2026-11-19): one
run of the strongest available model at its highest unblocked effort on
release 1's six public tasks. It tells whether release 1 separates the top
of the field. It is not a board entry.

## Result

GPT-6.2 Sol at max effort passed **5 of 18 attempts (27.8%, 95%
Clopper-Pearson 9.7% to 53.5%)**. Its mean progress score was **0.922**
(equal task weights, ±0.033 stderr across tasks). For comparison, GPT-6 Astra
at medium effort passed 0 of 18 public attempts in the release 1
confirmation ([report](frontier-v5-r1-astra-2026-11.md)).

| Task | Agent minutes (1, 2, 3) | Reward (1, 2, 3) | Held-out units passed (1, 2, 3) | Cost (USD) |
| --- | --- | --- | --- | --- |
| treesitter-query-anchor-matching | 27.9, 24.5, 27.0 | 1, 1, 1 | 13/13, 13/13, 13/13 | 5.47 |
| black-fmt-skip-region-handling | 24.9, 27.7, 27.1 | 0, 0, 0 | 6/7, 6/7, 6/7 | 5.31 |
| prettier-comment-attachment-stability | 37.9, 30.7, 38.9 | 0, 0, 0 | 22/26, 20/26, 20/26 | 8.56 |
| javaparser-member-resolution-scope | 27.6, 23.4, 23.9 | 0, 0, 0 | 18/19, 18/19, 18/19 | 7.12 |
| cli11-toml-config-round-trip | 17.4, 22.7, 20.6 | 0, 0, 0 | 16/17, 16/17, 16/17 | 3.90 |
| pulldown-cmark-html-block-scanning | 19.0, 20.0, 17.0 | 0, 1, 1 | 33/34, 34/34, 34/34 | 3.69 |

Attempts are in start-time order. Minutes measure agent execution only. Cost
is the API list-price equivalent Codex reports (the run used a ChatGPT
subscription): $34.05 for all 18 attempts.

## What failed

Every failed attempt kept every guard (tamper scan, build, sentinels, visible
tests) and lost one to six held-out units. Four of the six tasks failed on
the same held-out check in every attempt:

- **black:** the fixture `fmtskip_opening_bracket_split`, in all three
  attempts. Astra failed the same fixture in all three of its attempts.
- **CLI11:** `TomlMultiLineStringBinary`, a held-out guard that passes at
  base. All three attempts broke it while fixing multiline string handling.
  Astra broke it too.
- **javaparser:** `Issue4985Test`'s check that fields of a class with an
  unresolvable ancestor stay visible to inheritors, in all three attempts.
  Astra failed the same check.
- **prettier:** the parenthesized sequence-expression fixture and TypeScript
  case 19903 in all three attempts, plus one to three other comment cases.

Tree-sitter, which Astra failed three times, was solved three times.
pulldown-cmark, where Astra missed six regressions each time, was solved
twice; the third attempt missed regression 277 only.

## Reading the result

- **Release 1 is not a wall.** At max effort the strongest model solves two
  of the six tasks and nearly solves the other four, so pass rate already
  separates GPT-6.2 Sol at max from GPT-6 Astra at medium.
- **The progress score is compressed near the top.** Failed attempts score
  0.77 to 0.97, because they miss one sticky check rather than many. The
  score orders attempts but spans a narrow range at this level.
- **Four tasks hinge on one check each.** On black, CLI11 and javaparser,
  both models fail the same single held-out check in all six attempts. Those
  checks are guards or described defects recorded at admission (see each
  task's DESIGN.md and the CLI11 caveat in the Astra report), so they stand
  as designed. A board on these six tasks will show their pass rates as zero
  until a model clears those checks.

## Run conditions

- Tag `bench/2026-11-19-frontier-v5-r1-sol62-max` (`accec98d`), run worktree
  pinned to it, Harbor 0.24.0, Codex CLI 0.166.0, model `gpt-6.2-sol`,
  `model_reasoning_effort=max` (confirmed in every trial's agent log).
- `scripts/frontier-v5/run_gate.sh sol62` with `GATE_EFFORT=max`, three
  attempts per task, one at a time, early exit off. Web search, connected
  apps and plugins were off; no trial recorded a web or app call.
- Tasks are the frozen release 1.0.0 lock. Progress scores use rule C
  (docs/DECISIONS.md, 2026-11-19) as implemented by
  `scripts/frontier-v5/progress_score.py` on the open release 1.1 engine
  branch; every score agrees with its verifier's reward. javaparser 0.1.0
  records no per-check reports outside its container, so its scores come
  from the ids `check.err` lists as failing.
- Audit: 18 of 18 trials ran the stated model and effort, used real tools
  (36 to 112 tool calls each), recorded no web or app calls, and had clean
  tamper scans. The five passing attempts changed source files only.

## Disclosures

- **Safety refusal counted.** Tree-sitter attempt `4tryDJh` ended with a
  Codex safety-classifier stop after 72 tool calls. Its patch passed every
  check, and under the 2026-09-24 convention a provider classifier stop is a
  scored attempt graded on what was written. It counts as a pass.
- **First launch void.** The first launch at 13:38 PST failed on all 18
  trials because the local v5 sandbox images were deleted during the run.
  One tree-sitter attempt ran ($1.88) but was never graded, and no other
  attempt started. The images were rebuilt from the run worktree and
  the measurement relaunched at 14:46:59 PST. The void jobs are kept apart
  as infrastructure failures and do not count.
