# Frontier v5 admission gate log

The candidate log the gate protocol asks for (docs/DECISIONS.md, 2026-10-06,
decision (b)): each reference model runs a candidate three times at medium
effort; admit iff each solves at most 1 of 3 (exactly 1/3 gets the n=5
top-up and must finish at or below 2/5); two solves by either reference end
the gate early as a reject. Effort, tokens, cost and wall clock are
covariates only. Concurrency per decision (e): Python and JavaScript tasks
may pair (`-n 2`), compiled and JVM tasks run one at a time on this 16 GiB
host.

## Wave 1 identifiers (fixed with the first gate run, 2026-10-18)

| Reference | Harbor invocation | Model string | Agent CLI | Effort |
| --- | --- | --- | --- | --- |
| GPT-6 Astra | `harbor run -a codex` | `gpt-6-astra` | Codex CLI 0.159.0 (`--ak version=0.159.0`) | medium |
| Claude Fable 5.1 | `harbor run -a claude-code` | `claude-fable-5-1` | Claude Code 2.1.302 (`--ak version=2.1.302`) | medium |

Harbor 0.24.0, `-k 3` attempts, no retries. Astra authenticates with the
owner's ChatGPT login (`~/.codex/auth.json`, injected by Harbor when
`CODEX_FORCE_AUTH_JSON=1` is set in Harbor's own environment). Fable uses the
owner's Claude subscription token from a private env file the owner created
(`--env-file`, with `CLAUDE_FORCE_OAUTH=1`); the token is never read by the
launcher.

## Results

| Candidate | Slot | GPT-6 Astra (agent minutes) | Claude Fable 5.1 | Verdict |
| --- | --- | --- | --- | --- |
| nx-digraph-node-connectivity | F5-python | 2 of 2 solved (about 6, 6) | not run | **REJECT** (early exit) |
| geos-curved-overlay-arc-noding | F5-cpp | 2 of 2 solved (4.5, 4.2) | not run | **REJECT** (early exit) |
| ioverlay-hole-binding-extraction | F5-rust | 2 of 2 solved (6.2, 7.3) | not run | **REJECT** (early exit) |
| tseslint-type-params-mapped-types | F5-javascript | 2 of 2 solved (12.2, 10.8) | not run | **REJECT** (early exit) |
| commons-lang-fraction-lowest-terms | F5-java | 2 of 2 solved (4.3, 4.4) | not run | **REJECT** (early exit) |
| treesitter-query-anchor-matching | F5-c | **0 of 3** (9.9, 12.2, 7.6) | **1 of 3** (33.0, 27.3, 23.0) | Astra prong passed; Fable at exactly 1 of 3, n=5 top-up pending |

Astra runs 2026-10-18 on the M2 Pro Mac (16 GiB), one attempt at a time
except nx-digraph (two). Once a candidate had two solves, an early-exit
watcher stopped its Harbor job and the remaining attempt was not counted.
The Fable run is 2026-10-22 on the Mac Studio (M1 Max, 32 GiB, Docker VM
7.75 GiB), one attempt at a time, through `scripts/frontier-v5/run_gate.sh`.
Agent minutes are covariates only (decision (e)), so the host change does
not affect the gate.

### Diff review of every solve

PHASE1.md requires a review of each solved run before it counts. All ten
solves were diffed against their task's base (extracted from the task's own
environment image) and their added lines scanned for test-framework,
assertion, exit, environment and process-spawn tokens. Every change is in
the expected source files (geos: CircularArcIntersector, NodableArcString
and one or two neighbours; ioverlay: the bind and extract modules, one run
adding its own unit-test module, which grading ignores; tseslint and
commons-lang: the one rule file or class); the only token hit is a Rust
`Option::expect`. All tamper scans were clean and every sentinel failed as
required. The solves count.

### treesitter-query-anchor-matching: the one that held (2026-10-18)

All three Astra attempts finished without a solve, each in about ten
minutes, and failed the same way: 10 of the 11 held-out tests pass, but
`test_query_matches_with_leading_anchor_before_zero_quantifier` (1ffd612b,
unit D) fails, and the upstream guard test
`test_query_matches_with_anchors_on_both_sides_of_zero_quantifier`
(dfcf7392), which passes at base, breaks. That is the predicted miss in its
DESIGN.md: the naive "an anchor after a zero-matched quantifier is vacuous"
rule regresses the guard. The verifier ran normally (316 of 318 passing in
each run), so these are genuine failures.

### nx-digraph-node-connectivity (2026-10-18)

Astra job `astra-nx-digraph-node-connectivity`: trials FDiVG2v and FSrMHu5
both finished with reward 1 about six minutes after starting. Two solves end
the gate: REJECT. The third trial was stopped once the gate had decided and
is not counted.

Evidence and review. The launcher had passed `CODEX_FORCE_AUTH_JSON=1` with
`--ae`, and Harbor scrubs every `--ae` value from the files it writes, so
every "1" in that job's stored files reads `[REDACTED]`. Grading ran inside
the verifier container before the scrubbing, so the rewards are valid; to
make the solves reviewable, both stored artifacts were restored
(`[REDACTED]` back to `1`) and re-graded with the task's real verifier image:
reward 1 both, with the must-fail sentinel failing, a clean tamper scan, and
every fail_to_pass and pass_to_pass test passing. Diff review: both changed
only `connectivity.py` and `cuts.py` (about 50 lines each) with the same
ideas as the upstream fix (the smaller of distinct predecessors and
successors, deduplicated neighbours, the s-t edge in one direction only,
both orderings of a pair), plus their own added tests, which the verifier
ignores; no test-framework or exit tampering.

What it means: the strongest F5-python candidate, the successor to v4's only
task that beat the stronger reference, is solved quickly by Astra and leaves
Frontier v5. PLAN.md section 3 recycles a task to Routine v1 when both
references solve it; that is decided when the Fable half runs.

### treesitter-query-anchor-matching, Fable 5.1 half (2026-10-22)

Job `fable-treesitter-query-anchor-matching` (Harbor 0.24.0, `-k 3 -n 1`,
Claude Code 2.1.302 pinned in the container, `claude-fable-5-1`, medium,
subscription token with `CLAUDE_FORCE_OAUTH=1`). Three real sessions: every
assistant turn reports the model as claude-fable-5-1, and the stream carries
no auth, model-id or usage-limit event. Trials in run order:

| Trial | Agent minutes | Assistant turns | Output tokens | Reward | Tests |
| --- | --- | --- | --- | --- | --- |
| MSBrnJS | 33.0 | 138 | 321k | 0 | 316 of 318 |
| yaT8UJB | 27.3 | 151 | 449k | **1** | 318 of 318 |
| L3VGnaw | 23.0 | 94 | 233k | 0 | 315 of 318 |

The two failures have the Astra signature. Both miss the held-out
`test_query_matches_with_leading_anchor_before_zero_quantifier` (unit D) and
regress the upstream guard
`test_query_matches_with_anchor_after_nested_zero_quantifier`, which passes
at base; L3VGnaw also regresses the second guard,
`test_query_matches_with_anchors_on_both_sides_of_zero_quantifier`, exactly
as all three Astra runs did. The verifier ran normally in every trial (built,
tamper scan clean, graded from libtest's own logfile).

Diff review of the solve (yaT8UJB), against lib/src extracted from the task
image rebuilt from the task's own Dockerfile: only `query.c` and
`tree_cursor.c` differ, the two files the gold patch touches; 345 lines
added and 29 removed (gold: 79 and 11). The change is a fuller
reimplementation of the same semantics rather than a copy of upstream: a
step flag for the zero-match skip of `?` and `*` (as in gold) plus a leading-
anchor flag, per-state `force_immediate` and `suppress_immediate` to carry an
anchor across a zero-matched quantifier or cancel a one-sided one, a
synthetic last-child check step for trailing anchors with a per-depth bitset
of later-named-sibling facts, syntax errors for anchors at either edge of a
group, the identical `length == 7` MISSING-prefix fix, the same
tree-cursor change (keep scanning enclosing invisible ancestors until a later
named sibling is found), and a constraint-aware guard on the longest-match
state drop. The added lines' token scan hits only one `#define` for a bit
count and four `printf` lines inside the existing `DEBUG_ANALYZE_QUERY` step
dump; no test-framework, assertion, exit, environment or process-spawn
tokens. No tests were added or edited (the artifact is lib/src only). The
solve counts.

Gate state: Astra 0 of 3, Fable exactly 1 of 3. Under decision (b) the Fable
prong gets the n=5 top-up: two more attempts, admit iff Fable finishes at or
below 2 of 5 (at most one more solve). Pending the owner's go-ahead;
`GATE_ATTEMPTS=2 GATE_JOB_SUFFIX=-topup scripts/frontier-v5/run_gate.sh fable
treesitter-query-anchor-matching` runs it with the same identifiers.

Subscription usage during the run, from Claude Code's rate-limit events:
the five-hour window peaked at 34% and the seven-day window at 6%; overage
is disabled at the organisation level, so a run that exhausts the five-hour
window would stall rather than bill. Watch utilisation on longer runs.

## Blocked (resolved 2026-10-22): the Fable 5.1 half

On 2026-10-18 every Fable attempt was refused before starting: "Fable 5.1
requires usage credits." The owner's Claude plan does not include Fable 5.1
without usage credits. The owner chose to run the Astra half first; the
Fable half needs usage credits on the plan or an Anthropic API key (owner's
decision).

Resolved 2026-10-22: the owner's Claude subscription runs Fable 5.1 with a
`claude setup-token` token and no usage credits; the 2026-10-18 refusal did
not recur, and the infrastructure guard in `run_gate.sh` would have stopped
the run on that text before any attempt counted.

## Infrastructure failures (never counted)

Set aside under `logs/frontier-v5-gate/jobs/INFRA-*` on the host:

- 2026-10-17, Fable, nx-digraph: 401 "OAuth access token is invalid"; the
  first saved token had been cut off at a terminal line wrap (85 of 108
  characters).
- 2026-10-18, Fable, nx-digraph: the model string `anthropic/claude-fable-5-1`
  was passed through as is; Claude Code accepts only the bare
  `claude-fable-5-1`.
- 2026-10-18, Fable, nx-digraph: "Fable 5.1 requires usage credits".

## Lessons for the gate runner

- In `harbor run`, `-k` is the number of attempts and `-n` the concurrency.
- Never pass a flag value with `--ae`: Harbor treats `--ae` values as
  secrets and scrubs them from every file it writes. Set such flags in
  Harbor's own environment.
- Check a new reference's first attempt for real work (model name in the
  session, tool calls) within minutes, not just for a login: two of the
  three infrastructure failures above produced a single synthetic message
  and a reward of 0 that looked like a normal failed attempt.

## What the first wave says (2026-10-18)

Five of six F5 candidates fell to GPT-6 Astra in 4 to 12 minutes of agent
time, against a 3-hour budget. The F5 floors (several causes, interaction
shown by controls, every fix necessary, no dominant fix) are met by all six,
so they do not by themselves make a task hard for Astra: an agent that
reads the code and the symptoms finds every cause in minutes, whether or
not the fixes interact.

The one that held differs in a specific way: upstream added guard tests
that pass at base and fail under the obvious fix, so a symptom-level change
regresses existing behaviour. That is a property to select for. The plan's
own revisit condition (DECISIONS.md 2026-10-10: "if F5 candidates are solved
3/3 by both references ... F5's floors (not the family) need raising in a v3
plan") is not yet triggered, since Fable has not run, but the Astra half
alone points the same way. A v3 floor along the lines of "upstream guard
tests that a symptom-level fix breaks, shown by a naive-fix control" is the
candidate change, for the owner to decide.
