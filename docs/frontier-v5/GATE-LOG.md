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
| treesitter-query-anchor-matching | F5-c | **0 of 3** (9.9, 12.2, 7.6) | **2 of 5** (33.0, 27.3, 23.0, 79.6, 27.2) | **ADMIT** (F5-c): Fable 2 of 5 after the n=5 top-up, at the bar |
| comrak-gfm-tables-autolinks-parity | F2-rust | 2 of 2 solved (11.1, 8.2) | not run | **REJECT** (early exit, 2026-10-22) |
| black-fmt-skip-region-handling | F5-python (wave 1) | **0 of 3** (6.1, 5.6, 6.0) | not run | Astra prong passed (2026-10-24); pending Fable |

Astra runs 2026-10-18 on the M2 Pro Mac (16 GiB), one attempt at a time
except nx-digraph (two). Once a candidate had two solves, an early-exit
watcher stopped its Harbor job and the remaining attempt was not counted.
The Fable run is 2026-10-22 on the Mac Studio (M1 Max, 32 GiB, Docker VM
7.75 GiB), one attempt at a time, through `scripts/frontier-v5/run_gate.sh`.
Agent minutes are covariates only (decision (e)), so the host change does
not affect the gate. The comrak Astra run is 2026-10-22 on the Mac Studio
from a detached run worktree at cc8a2d4d.

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

Gate state after three attempts: Astra 0 of 3, Fable exactly 1 of 3. Under
decision (b) the Fable prong got the n=5 top-up (next section): two more
attempts, admit iff Fable finishes at or below 2 of 5.

Subscription usage during the run, from Claude Code's rate-limit events:
the five-hour window peaked at 34% and the seven-day window at 6%; overage
is disabled at the organisation level, so a run that exhausts the five-hour
window would stall rather than bill. Watch utilisation on longer runs.

### treesitter-query-anchor-matching, Fable 5.1 top-up (2026-10-22): ADMIT

Job `fable-treesitter-query-anchor-matching-topup`, launched 10:41 from
commit b87199be with `GATE_ATTEMPTS=2 GATE_JOB_SUFFIX=-topup`, otherwise the
same identifiers. Both sessions real (every assistant turn on
claude-fable-5-1; no auth, model-id or usage-limit event; the five-hour
window stayed allowed throughout):

| Trial | Agent minutes | Assistant turns | Output tokens | Reward | Tests |
| --- | --- | --- | --- | --- | --- |
| R2Eua83 | 79.6 | 221 | 517k | 0 | 317 of 318 |
| 6M56y22 | 27.2 | 111 | 269k | **1** | 318 of 318 |

R2Eua83 fails only the held-out
`test_query_matches_with_leading_anchor_before_zero_quantifier` (unit D) and
regresses no guard: the longest Fable attempt got closest without getting
there. That case is the one every failed attempt by either reference missed,
and the one DESIGN.md names as the predicted miss.

Diff review of the solve (6M56y22), against the same extracted base: three
files differ, `query.c`, `tree_cursor.c` and `tree_cursor.h` (one new
internal helper's declaration; the gold touches only the two `.c` files);
366 lines added and 82 removed. A third distinct design for the same
semantics: one state-transition function that carries a pending named-anchor
flag across skipped optional and repeated steps and distinguishes
sequential, dead-end, pass-through, alternative and repeat hops; trailing
anchors marked on every child step of the level and enforced when a state
leaves the level, through a new tree-cursor helper that asks whether a
visible ancestor has later named siblings; anchor-aware fallibility in query
analysis; a strictness-ordered guard on the longest-match state drop; the
trailing group anchor as a syntax error; the identical `length == 7`
MISSING-prefix fix; and the tree-cursor sibling scan factored into a helper
that keeps scanning invisible ancestors until a named sibling is found. The
added lines' token scan hits one `printf` inside the `DEBUG_ANALYZE_QUERY`
step dump and nothing else; no tests added or edited. The solve counts.

**Verdict: ADMIT.** Astra 0 of 3; Fable 2 of 5, at the bar set by decision
(b). treesitter-query-anchor-matching takes the F5-c slot, the first task
admitted to Frontier v5. Covariates: Fable's five attempts took 33.0, 27.3,
23.0, 79.6 and 27.2 agent minutes against the 3-hour budget, Astra's three
9.9, 12.2 and 7.6. The gold and the two solves are three different designs
for the same anchor semantics.

### comrak-gfm-tables-autolinks-parity, Astra half (2026-10-22): REJECT

The first F2 (differential parity) candidate at the gate, and the first
gate run from a run worktree: job `astra-comrak-gfm-tables-autolinks-parity`
launched 20:02 PDT on the Mac Studio by
`~/vulcanbench-gate/worktree-cc8a2d4d/scripts/frontier-v5/run_gate.sh`, a
detached worktree at cc8a2d4d, because the main checkout had moved to
another branch minutes earlier. Identifiers as in wave 1 (Codex CLI 0.159.0,
`gpt-6-astra`, medium, `-k 3 -n 1`); Harbor's invocation records
`--model gpt-6-astra` and `model_reasoning_effort=medium`.

| Trial | Agent minutes | Commands | Reward | Cases matched |
| --- | --- | --- | --- | --- |
| uXoeb6L | 11.1 | 104 | **1** | 5,410 of 5,410 |
| 3gbdiJb | 8.2 | 110 | **1** | 5,410 of 5,410 |
| GMSgjqp | stopped by the early exit | 34 | none | not counted |

Two solves end the gate: **REJECT**. The early exit stopped the Harbor job
26 minutes after launch; the third trial's sandbox containers outlived the
kill and were removed by hand (a leak in the runner, noted below).

Diff review of both solves, against `/app/src` extracted from the task
image rebuilt from the task's own Dockerfile. Each changes exactly the two
stubbed files plus one line of `src/parser/inlines.rs` (the autolink
bracket-context test, `within_brackets` replaced by `!self.brackets.is_empty()`,
identical in both) and adds a unit-test module that grading never runs.
Added lines 830 and 913; tamper scans clean; the added-line token scan
(process spawn, file and environment reads, FFI, embeds, exit) hits nothing.
The ports are not recalled upstream code: against the gold's `table.rs` and
`autolink.rs` (387 and 443 lines) the solves run 176 and 498, and 194 and
492 lines, with difflib ratios of 0.04 to 0.07 and at most 17 non-trivial
identical lines per file. Both count.

How they did it, from the sessions: each agent read the two C extensions
and the scanner definitions, wrote the two Rust modules, then ran `cmake`
and `gcc` (present in the agent image) on the shipped reference source to
build `cmark-gfm` itself, and fuzzed thousands of random inputs through both
binaries until the outputs agreed, fixing each mismatch. The reference was
meant to be read at depth; with a C toolchain in the sandbox it became a
local oracle, and differential parity collapsed into "diff until identical"
in about ten minutes. DESIGN.md's own revisit rule applies ("a reference that
solves 3/3 means the reconstruction volume is too small for F2 and the slot
should take a larger subsystem"); the oracle route is a second lever for the
owner: ship the reference in a form the sandbox cannot build or run, or
accept that F2 measures porting speed against an oracle.

### black-fmt-skip-region-handling, Astra half (2026-10-24): the Astra prong passed

The first wave-1 candidate (WAVE1-F5.md), built under floor 8 and validated
the same day (its DESIGN.md). Job `astra-black-fmt-skip-region-handling`,
launched 15:54 PDT on the Mac Studio from a run worktree pinned to d75dd420
(the task's commit), `run_gate.sh` at that commit (main's version, without
the top-up knobs and the container cleanup of this branch), identifiers as in
wave 1 (Codex CLI 0.159.0, `gpt-6-astra`, medium, `-k 3`), concurrency 2 per
decision (e) for a Python task. Every session real: Harbor's invocation
records `--model gpt-6-astra` and `model_reasoning_effort=medium`, 76 to 86
command executions per trial, no auth, model-id or usage-limit event. No
containers were left behind (no early exit fired).

| Trial | Agent minutes | Reward | Held-out | Guards |
| --- | --- | --- | --- | --- |
| LBgCaAJ | 6.1 | 0 | 6 of 7 | hold (596 of 596) |
| dRzhS4G | 6.0 | 0 | 6 of 7 | hold |
| Qf6uoFa | 5.6 | 0 | 6 of 7 | hold |

**0 of 3: the Astra prong passed.** All three attempts fail the same way:
`fmtskip_opening_bracket_split` (the #5538 case, code after a hidden
opening-bracket region on the same line when the hidden code ends in code)
fails, and nothing else does. Each attempt changed `src/black/comments.py`
(the walk and the serializer) and `src/blib2to3/pytree.py` (the `Leaf`
class), about 100 to 130 lines added and 70 to 90 removed, and none touched
`src/black/linegen.py`, where upstream's #5538 and #5586 live; the
conditional case nonetheless passes under their changes. The tamper scans
are clean and no added line touches the test runner. Each closing message
reports the visible suite green ("598 passed, 3 skipped") plus 51 to 121
self-generated combinations checked for AST equivalence and stability, and
declares the work done at about six minutes.

What held, and what did not. The floor-8 guard mechanism did not fire: no
attempt took the `climb-any-parent` shortcut that the naive control showed
would break `fmtskip_type_params`; all three widened the walk in a way the
596 guards accept. What stopped them is the second mechanism recorded for
tree-sitter and in this task's DESIGN.md: a held-out combination that the
visible suite cannot reveal and that their own randomized checks did not
reach. Astra's six-minute finish is the premature-completion pattern of the
first wave (every visible signal green, then done), and here that signal was
wrong by exactly one case.

Next: the Fable 5.1 half (three attempts, admit iff at most 1 of 3, n=5
top-up at exactly 1 of 3), on the owner's approval. Covariates: 6.1, 5.6 and
6.0 agent minutes against the 3-hour budget; the run took 15 minutes end to
end including both image builds.

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
- The early exit kills `harbor run`, but the running trial's sandbox
  containers (`<task>__<trial>__env-main-1` and the egress sidecar) outlive
  it and keep the agent, and its quota use, alive until the task timeout.
  Remove them after an early exit (seen 2026-10-22, comrak).
- Run the gate from a run worktree pinned to a commit, never from a
  checkout that may be edited during the run; `run_gate.sh` resolves the
  repository from its own path, so invoking the worktree's copy is enough.

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
