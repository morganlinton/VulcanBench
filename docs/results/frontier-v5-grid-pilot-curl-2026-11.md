# VulcanBench Frontier v5 grid: the larger-repo pilot cell

Measured November 22, 2026 (America/Los_Angeles). The first pilot cell (C++,
smaller tier, on CLI11) sat at the ceiling for GPT-6.2 Sol and GPT-6.1 Terra
at every budget and worklist size tried
([first calibration](frontier-v5-grid-pilot-calibration-2026-11.md); round 2
is in [PR #332](https://github.com/morganlinton/VulcanBench/pull/332)). This
report asks the next question: does a large repo make the same kind of
worklist hard? A second pilot cell was built for the C, larger-tier slot on
curl (about 149,000 lines of first-party C) and each model ran it once.
These are calibration runs, not board entries.

**As scored, each model finished 22 of 27 items and was stopped at the 15
minute budget. A trace of the misses then showed that three of the unfinished
items fail on things their worklist entries do not say.** With those three
checks set aside each model would have 25 of 27, the same place as the CLI11
cell. The larger repo does not separate the two models, and most of its
apparent headroom is a defect in the environment, not difficulty.

## Result

| Run | Model | Budget (min) | Items done | Agent minutes | Stopped at budget | Existing tests broken | Tool calls | Input tokens | Output tokens | Cost (USD) |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 10 | GPT-6.2 Sol, max | 15 | 22 of 27 | 15.0 | yes | 1 | 103 | 5.49M | 38.8k | 1.31 |
| 11 | GPT-6.1 Terra, max | 15 | 22 of 27 | 15.0 | yes | 0 | 161 | 14.60M | 42.0k | 1.14 |

Run numbers continue from round 2. Each row is one run of 27 items: 22 of 27
is 0.815 (±0.075 binomial stderr). Minutes measure agent execution only.
Input tokens include cached input. Cost is the API list-price equivalent
Codex reports (the runs used a ChatGPT subscription): $2.45 for both, and
$11.89 for all eleven pilot runs.

- **Both models were stopped by the budget.** On CLI11 a 15 minute budget
  never bound; here it bound for both.
- **GPT-6.2 Sol had 23 items passing** and broke 1 of the 1,798 existing
  tests, so the guard charge took one finished item off its count.
- **GPT-6.1 Terra had 22 items passing** and broke no existing test.
- **Four of each model's five unfinished items are the same four**, failing
  on the same held-out tests: three of them are one test short, and one
  fails both of its tests. Both models had edited the code for all four.
  Terra's fifth is another item one test short; Sol's fifth is the item the
  guard charge cleared. The trace below finds that three of the shared four
  are not real misses.
- By kind, both models: 20 of 25 defects, the 1 diagnosis item and the 1
  feature.
- By the builder's difficulty rating (made before any run):

| Rated | Items | GPT-6.2 Sol | GPT-6.1 Terra |
| --- | --- | --- | --- |
| Over about 2 minutes for a strong agent | 13 | 10 | 9 |
| Borderline | 8 | 7 | 7 |
| Small | 6 | 5 (one cleared by the guard charge) | 6 |

## Trace of the misses

Two models failing the same held-out tests on the same four items can mean
the behaviours are hard, or that the entries under-describe them. To tell,
each model's collected tree was rebuilt in the verifier image, every failing
test was rerun on its own, and each failure was read against the test, the
worklist entry, the reference fix and the model's change. All failures
reproduced. No model was called.

| Unfinished item | Models | What the failing test wanted | Verdict |
| --- | --- | --- | --- |
| A | both | The exact wording of an error message. Both models returned the right error code with their own wording; the entry gives no wording. | Check defect |
| B | both | An empty file left at a path after a failed write. Both models detected the failure and left nothing there; the reference leaves an empty file as a side effect of how it opens the target. The entry does not ask for one. | Check defect |
| C | both | That a failed operation never calls the application's cleanup callback. Both models returned the required error; the entry does not mention callbacks, and the unfixed code calls them on other failures. | Entry under-described |
| D | both | The behaviours the entry lists. GPT-6.2 Sol's change aborts with a memory error; GPT-6.1 Terra's change rejects an input the original code accepted. | Real miss |
| E | Terra only | A behaviour the entry states. | Real miss |
| Guard | Sol only | An existing, visible test that Sol's change broke. | Real regression; the charge is correct |

- **Three of the four shared misses are the environment's fault.** Setting
  those three checks aside, GPT-6.2 Sol would have 26 items passing and 25
  after the guard charge, and GPT-6.1 Terra 25: 93% each. This is a
  reading of the logs, not a rescore; the scored result stays 22 of 27.
- **Two further weaknesses showed up on item D.** One of its held-out tests
  is the repo's main existing regression test for that area, so with the
  whole test held out the agent had nothing to tell it that it had broken
  old behaviour. Another calls an internal function directly, which fixes
  the internal design, not only the reported behaviour.
- **The admission checks cannot catch this.** They show that the reference
  passes the tests and that each test needs its fix. They do not show that
  the entry is enough to pass the tests. A check of each held-out test's
  assertions against its entry's text is owed for every environment.

## Reading the result

- **The larger repo is not off the ceiling for these two models.** The
  scored 81% is mostly check defects; the corrected reading is 93%, where
  the 27-item CLI11 worklist put them (93% and 96%).
- **It does not rank GPT-6.2 Sol against GPT-6.1 Terra.** Scored and
  corrected, the counts are equal. The two are also within about one point
  of each other on the Frontier v4 board at their best effort levels, so a
  tie may be the right answer for this pair, not a failure to measure. A
  model further away, weaker or stronger, is what would show whether the
  grid spreads a field.
- **The large repo did change the work.** Both sessions ran to the budget,
  which never happened on CLI11 at 15 minutes, and the real misses are of a
  kind CLI11 did not produce: a memory error, and a regression in existing
  behaviour.
- **Time or capability is not settled.** Both sessions were cut off, so a
  longer budget might lift both. One run per model at one budget cannot
  tell a time limit from a difficulty limit.
- **This cell must be repaired before it is used again.** Items A, B and C
  need their entries or their checks changed, and item D's existing
  regression test should stay visible to the agent.
- **The format held on a much larger tree.** Grading the whole 1,798-test
  suite took about 2 minutes per run, the budget stop and the guard charge
  both worked on paid runs, and the images came up built so no budget was
  spent on a cold build. Each run produced 27 graded items in about 18
  minutes of wall-clock time for a little over $1.

## After the repair (added later on November 22)

The environment's builder then repaired it as version 0.2.0 and checked
every held-out test's assertions against its entry. That work was pushed
before all of its admission checks had been rerun, so the numbers below were
provisional when written. The remaining checks were completed later the same
day (next section).

- **A leak in the version the two runs used.** The agent's tree in 0.1.0
  still held two data files containing the exact expected output of two
  held-out tests, both belonging to one item. GPT-6.2 Sol's session log
  refers to those files; GPT-6.1 Terra's does not. Both models finished
  that item, and it is counted in both scores above. Version 0.2.0 removes
  the files.
- **More entries were short than the trace found.** The assertion check
  found nine places where an entry did not state what its test requires,
  item C among them. Each now has a sentence in the entry.
- **The two defective checks were loosened**, one internal-design assertion
  on item D was cut, and 16 held-out tests that existed before their fixes
  now have a version in the agent's tree that passes on the unfixed code.
- **Regrade of the two saved trees with the repaired checks: 24 of 27
  each.** GPT-6.2 Sol has 25 passing less the guard charge. Item C still
  fails for both, because the trees were written before its entry stated
  the rule; that is where the trace's reading of 25 came from. A regrade is
  information, not a score.

## A third model on the repaired environment (added the evening of November 22)

Version 0.2.0 then finished admission on a second machine: the four verifier
probes score 0, and Harbor's oracle and no-op runs give 27 of 27 and 0 of 27
with the freeze hook. One more model then ran it once, with the same
launcher settings as the first two: GPT-6 Astra at max effort with a 15
minute budget.

| Run | Model | Budget (min) | Items done | Agent minutes | Stopped at budget | Existing tests broken | Tool calls | Input tokens | Output tokens | Cost (USD) |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 12 | GPT-6 Astra, max | 15 | 25 of 27 | 14.7 | no | 0 | 69 | 5.44M | 33.8k | 8.54 |

25 of 27 is 0.926 (±0.050 binomial stderr). Input tokens include 5.28M of
cached input. Cost is the API list-price equivalent Codex reports (the run
used a ChatGPT subscription). This brings the pilot to twelve runs and
$20.43.

- **GPT-6 Astra ended its own session** 17 seconds before the budget,
  reporting every item done. It broke none of the 1,798 existing tests.
- **Its two misses are not the first two models' misses.** It finished the
  two items both of them still fail in the regrade, and failed two items
  (call them E and F) that both of them finished. Each failed one of its two
  held-out tests.
- **Both misses were traced the same way as before,** by rebuilding the
  collected tree in the verifier image, rerunning the two tests and reading
  each against its entry, the reference fix and the model's change:

| Item | What the model did | Why the test fails | Verdict |
| --- | --- | --- | --- |
| E | Made the change the entry describes, and removed an older workaround that the change makes redundant. | The test compares generated text exactly and still expects the workaround's two lines. The model's output behaves the same without them. The entry's last sentence can be read as saying the workaround is no longer needed. | Environment defect |
| F | Fixed both problems the entry describes; both of the test's assertions about them hold. | A third assertion, on the content of a second output file, is not stated in the entry, and the unfixed code does not satisfy it either. | Environment defect, with a caveat: a careful engineer might have asked about that file |

- **Corrected reading: 27 of 27.** As before this is one reader's judgment
  and not a rescore; the scored result stays 25 of 27.
- **The repair's own assertion check missed both.** One test's third
  assertion was not in its table, and one entry sentence can be read two
  ways. Both were found only because a model followed the entry to the
  letter.
- **Three models now sit at the ceiling on this cell:** 25 scored for GPT-6
  Astra, and 24 each for GPT-6.2 Sol and GPT-6.1 Terra in the regrade. A
  worklist of small, fully described fixes does not separate them in a
  large repo any more than in a small one.
- **A Claude Fable 5.2 run was approved with this one and did not start.**
  The launcher's usage check on the Claude subscription was refused (usage
  credits required), so no session ran and nothing is scored for it.

## Run conditions

- Tag `bench/2026-11-21-frontier-v5-grid-pilot-calibration` (`d9305f64b`),
  run worktree pinned to it, Harbor 0.24.0. GPT-6.2 Sol on Codex CLI 0.166.0
  (`gpt-6.2-sol`), GPT-6.1 Terra on Codex CLI 0.159.0 (`gpt-6.1-terra`),
  both with `model_reasoning_effort=max`.
- `scripts/frontier-v5/run_gate.sh --grid <ref>` with `GATE_EFFORT=max` and
  `GATE_BUDGET_MIN=15`. One environment per run, one run at a time. Web
  search, connected apps and plugins were off; no trial recorded a web or
  app call.
- The environment is private (decision R8), version 0.1.0, pinned at one
  commit for both runs (task checksum `7658ade2`). It has 27 items (25
  defects, 1 diagnosis, 1 feature) with 50 held-out tests and 1,798 guard
  tests, which are curl's whole test suite as it builds in the image.
  Thirteen items combine 2 to 5 upstream fixes. Every item is an upstream
  fix taken back out of the code at the pinned commit; six use hand-made
  inverses of changes that no longer revert cleanly, each described in the
  environment's private design record.
- Admission, as reported by the environment's builder: the base scores 0 of
  27, the full reference 27 of 27, each item's reference alone scores
  exactly its item, the verifier probes score 0, five repeats of the base
  and of the reference are byte-identical, and Harbor's oracle and no-op
  runs are accepted.
- Audit: both trials ran the stated model at max effort, used real tools
  (103 and 161 tool calls, all shell commands or file edits) and had clean
  tamper scans. Both ended with Harbor's budget stop (`AgentTimeoutError`
  at 900 seconds), which is the designed stop and is graded.

- Run 12 (GPT-6 Astra): tag
  `bench/2026-11-22-frontier-v5-grid-pilot-curl-wider` (`0e9b30fb`), a
  different machine from runs 10 and 11 (Apple M1 Max, Docker with 10 CPUs
  and 7.75 GiB), Harbor 0.24.0, Codex CLI 0.159.0 (`gpt-6-astra`),
  `model_reasoning_effort=max`, the same launcher settings, environment
  version 0.2.0 (task checksum `92e5f419`). Audit: the stated model and
  effort, 69 tool calls, all shell commands, no web or app call, a clean
  tamper scan, no Harbor exception.

## Disclosures

- **One run per model at one budget.** A tie between two single runs says
  the models are close on this cell, not that they are equal.
- **Two held-out tests cannot run in the image** (they need a TLS tunnel
  tool it does not have). They are skipped for every model and are not
  among the 50 that are counted.
- **The environment was built and validated a few hours before the runs,**
  by one session, and its admission checks were not repeated independently.
  The defects the trace found were in it when the runs were made.
- **The trace is one reader's judgment** of six failures, made after the
  runs. The corrected count is not a score.
- **Expected output of two held-out tests was visible to the agents** in
  the version both runs used, and one model's log refers to it (see "After
  the repair"). One item in each score is affected.
- **Run 12 used a different environment version and machine** from runs 10
  and 11. Its 25 is comparable with their regrade (24 each), not with their
  scored 22, and the regrade is not a score. The importer's price table has
  no entry for GPT-6 Astra, so run 12's cost is the harness's own figure.
- **Background load.** When run 10 started, a macOS background process
  unrelated to the benchmark was using about one of the machine's 12 cores.
  Each environment is allotted 4.
