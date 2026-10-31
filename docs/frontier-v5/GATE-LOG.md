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
| Claude Opus 5.5 (second reference for wave 1 from 2026-10-25, DECISIONS.md) | `harbor run -a claude-code` | `claude-opus-5-5` | Claude Code 2.1.302 (`--ak version=2.1.302`) | medium |

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
| black-fmt-skip-region-handling | F5-python (wave 1) | **0 of 3** (6.1, 5.6, 6.0) | Fable: stopped at 94% weekly usage, not an attempt; **Opus 5.5: 0 of 3** (13.1, 13.5, 37.6) | **ADMIT** (F5-python, 2026-10-25): both references 0 of 3 |
| pulldown-cmark-html-block-scanning | F5-rust (wave 1; floor 8 not met, gated under (g')) | **0 of 3** (3.7, 4.4, 3.2) | **Opus 5.5: 1 of 5** (13.4, 11.1, 8.6, 7.6, 5.4) after the n=5 top-up | **ADMIT** (F5-rust, 2026-10-30) under (g'): Opus 1 of 5, below the bar; floor-8 miss recorded |
| prettier-comment-attachment-stability | F5-javascript (wave 1) | **0 of 3** (15.0, 12.8, 22.9) | **Opus 5.5: 0 of 3** (32.5, 27.4, 25.6) | **ADMIT** (F5-javascript, 2026-10-27): both references 0 of 3; tamper-scan fix re-validated after the gate |
| javaparser-member-resolution-scope | F5-java (wave 1) | **0 of 3** (9.8, 9.7, 7.7) | **Opus 5.5: 0 of 3** (20.9, 28.9, 21.3) | **ADMIT** (F5-java, 2026-10-29): both references 0 of 3 |
| boosturl-path-compare-normalize | F5-cpp (wave 1) | 2 of 2 solved (17.7, 21.6) | not run | **REJECT** (early exit, 2026-10-28) |
| cli11-toml-config-round-trip | F5-cpp (wave 1 replacement) | **0 of 3** (2.4, 4.7, 3.4) | **Opus 5.5: 0 of 3** (6.4, 9.1, 13.2) | **ADMIT** by the gate rule (F5-cpp, 2026-10-30), flagged: every attempt cleared all 11 defect checks and was held only by two held-out guards |
| ferrycore-crossing-opaque-parity | F4-python | 2 of 2 solved (7.4, 23.2), both by disassembling the binary | not run | **REJECT** (early exit, 2026-10-30) |
| phonenumbers-parse-match-parity | F2-python (shape B1) | 1 genuine solve (7.4); a second (6.4) used Codex `web__run` and is not valid | not run | **not decided**: Astra re-run with web tools off on the owner's approval |

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

Next: the second prong (three attempts, admit iff at most 1 of 3, n=5
top-up at exactly 1 of 3), on the owner's approval. Covariates: 6.1, 5.6 and
6.0 agent minutes against the 3-hour budget; the run took 15 minutes end to
end including both image builds. On 2026-10-25 the owner made Claude Opus
5.5 the second reference for wave 1 (DECISIONS.md), so black's second prong
runs as `opus`, not `fable`.

Fable half, first start (2026-10-24 17:51 PDT): stopped by the operator at
17:53, two minutes in, before either attempt had a result. The run's first
rate-limit event reported status `allowed_warning` with the subscription's
seven-day window (overage included) at 94%, the five-hour window at 13%;
with two sessions running on a 3-hour budget the run could have exhausted
the week's allowance mid-attempt and produced invalid results. Not an
attempt and not an infrastructure failure: the partial job directory is set
aside as `STOPPED-usage-pause-fable-black-fmt-skip-region-handling`, the
sandboxes were removed by hand, and the owner chose to rerun after the
window resets on 2026-10-28 08:00. Lesson for the runner: check the usage
windows before a Fable launch and stop above a threshold, rather than
watching after the fact.

### black-fmt-skip-region-handling, Opus 5.5 half (2026-10-25): ADMIT

The first run under the 2026-10-25 decision that makes Claude Opus 5.5 the
wave-1 second reference (DECISIONS.md). Job
`opus-black-fmt-skip-region-handling`, launched 04:51 PDT from a run
worktree pinned to 8b4d1ad0 (tag `bench/2026-10-25-black-opus-gate`: the
task at 5be950c6 plus `run_gate.sh` from this branch with the `opus`
reference and the usage pre-flight), Claude Code 2.1.302 in the container,
`claude-opus-5-5`, medium, `-k 3 -n 2`. The pre-flight read the subscription
windows before starting: five-hour 5%, seven-day 54%, status allowed, and no
third window, so Opus 5.5 draws from a different allowance than the one that
stopped the Fable run. Every session real: all 95 to 250 assistant turns on
`claude-opus-5-5`, no auth, model-id or usage-limit event; the run ended with
the five-hour window at 10% and the seven-day at 55%.

| Trial | Agent minutes | Assistant turns | Reward | Held-out | Guards |
| --- | --- | --- | --- | --- | --- |
| NKdDBfG | 13.1 | 95 | 0 | 5 of 7 | hold (596 of 596) |
| iEWmoMQ | 13.5 | 104 | 0 | 5 of 7 | hold |
| r3sKbnf | 37.6 | 250 | 0 | 6 of 7 | hold |

**0 of 3.** All three miss `fmtskip_opening_bracket_split`, the case every
Astra attempt missed too; the two shorter attempts also miss
`fmtskip_paired_header` (the inner skipped bracket inside a header). No
guard regressed in any attempt. Changes: `src/black/comments.py` only in the
two shorter attempts (+199/-14, +195/-49), `comments.py` plus one new field
on `Leaf` in `src/blib2to3/pytree.py` in the long one (+313/-75); none
touched `src/black/linegen.py`, where two of the upstream fixes live. Tamper
scans clean; no added line touches the test runner. Each closing message
states that all six reported cases now format correctly, are stable and keep
the AST, and that the agent's own added case files pass: the attempts fixed
the instruction's examples and the visible suite stayed green, while the
hidden case files, which cover the same directives in more combinations, did
not. The instruction had said "do not special-case the examples"; the miss
is the generalisation, not the example.

**Verdict: ADMIT.** Astra 0 of 3 (2026-10-24) and Opus 5.5 0 of 3
(2026-10-25). black-fmt-skip-region-handling takes the F5-python slot, the
second task admitted to Frontier v5 and the first to hold both references at
zero. Anatomy for the floor-8 record: across six frontier attempts the guard
mechanism never fired (every attempt kept all 596 guards green), and the
held-out combinations stopped all six. On this task the second mechanism
did all the work; the guard that the naive control trips was a shortcut no
frontier agent took. The task was built under floor 8 and meets it, but what
held it was the held-out coverage.

### pulldown-cmark-html-block-scanning, Astra half (2026-10-26): a floor-8 test

Built and validated 2026-10-26 (its DESIGN.md); floors 1 to 7 met, **floor 8
not met** by six naive fixes, so by WAVE1-F5.md it would not be gated. The
owner approved an Astra-only run to test whether floor 8 matters. Job
`astra-pulldown-cmark-html-block-scanning`, launched 17:30 PDT from a run
worktree pinned to 8166c6bb (tag `bench/2026-10-26-pulldown-astra-gate`:
the task at 9fc86e09 plus the current `run_gate.sh`), wave-1 identifiers,
concurrency 1 (Rust). Sessions real (`--model gpt-6-astra`, dozens of
commands each, no auth or usage event); no containers left behind.

| Trial | Agent minutes | Reward | Held-out failing | Guards |
| --- | --- | --- | --- | --- |
| C8uUgzi | 3.7 | 0 | 6 of 24 (r259, r260, r261, r273, r274, r277) | hold (1,388) |
| NzwKGhN | 4.4 | 0 | 6 of 24 (same six) | hold |
| qckhmcQ | 3.2 | 0 | 3 of 24 (r273, r274, r277) | hold |

**0 of 3.** Every attempt kept all 1,388 guards green and declared done in 3
to 4.5 minutes; all three miss hidden examples 273, 274 and 277 (inline HTML
scanner cases from #1169), two also miss 259 to 261 (type 1 end tags that do
not match the start tag, #1163). This is the black pattern again: the guard
mechanism never fired, the held-out combinations stopped every attempt, and
the agent finished early with its visible signal green.

What it says about floor 8: a task that fails floor 8 held the stronger
reference at zero. On the evidence so far (black and pulldown-cmark, both
0 of 3 for Astra with no guard tripped), floor 8 is **not necessary** for a
task to hold; held-out coverage of combinations the visible suite does not
show is doing the work. Floor 8 as written would have discarded this task.
A proposed amendment for the owner: accept a candidate that meets floor 8
**or** has at least N held-out checks that no single-cause fix clears and
that the visible suite cannot reveal (measured by the existing single-fix
controls). The second prong (Opus 5.5) has not run.

### prettier-comment-attachment-stability, Astra half (2026-10-27)

Built and validated 2026-10-26 (its DESIGN.md); floors 1 to 8 met, floor 8
by route (a). Job `astra-prettier-comment-attachment-stability`, launched
06:06 PDT on the owner's approval from a run worktree pinned to 8d859089
(tag `bench/2026-10-27-prettier-astra-gate`: the task at 6cf2915d plus the
current `run_gate.sh`), wave-1 identifiers, concurrency 2 (JavaScript).
Sessions real (`--model gpt-6-astra`, 60 to 76 commands each, each running
the visible format suite with `FULL_TEST=1`; no auth or usage event); no
containers left behind.

| Trial | Agent minutes | Reward | Held-out failing (of 18) | Guards |
| --- | --- | --- | --- | --- |
| EvrJE7b | 15.0 | 0 | 4: arrow-chain-with-trailing-comments.js (both arrowParens options), parenthesized-trailing-comment.js, parenthesized.js | hold (101,662) |
| teLjUMw | 12.8 | 0 | 4: parenthesized.js (format and second format), consistent-with-flow 15707.ts and 19903.ts | 600 not run (verifier flake, see below) |
| DFKPGQL | 22.9 | 0 | 3: parenthesized.js (format and second format), consistent-with-flow 19903.ts | hold |

**0 of 3.** Each attempt cleared 14 or 15 of the 18 held-out checks, kept
every guard it ran green, and declared done with the visible suite passing.
All three miss the parenthesized sequence-expression fixture. None took the
pre-registered leading-on-annotation shortcut (it trips 4 guards; no attempt
tripped one). As with black and pulldown-cmark, what held the reference was
held-out coverage the visible suite does not show, not the guard.

Verifier flake in teLjUMw: two guard files
(`js/last-argument-expansion`, `js/nullish-coalescing`, 600 ids) failed with
"Worker exited unexpectedly". Rerun twice on that attempt's own `src` in a
rebuilt verifier image, alone, they pass 600 of 600, so the crash was the
environment, not the attempt. Its verifier (13:19 to 13:22 UTC) overlapped
EvrJE7b's (13:21 to 13:24) inside the 7.75 GiB Docker VM, each with an
8 GiB limit and 4 jest workers; DFKPGQL's ran alone and was clean. The
reward is unaffected (four held-out checks fail either way), but the same
overlap could zero a real solve. Fix before the second prong: run this task
one attempt at a time, or cap the verifier's jest workers. That is a
concurrency decision for docs/DECISIONS.md (decision (e) puts JavaScript at
two).

### prettier-comment-attachment-stability, Opus 5.5 half (2026-10-27): admitted

Job `opus-prettier-comment-attachment-stability`, launched 16:02 PDT on the
owner's approval from a run worktree pinned to 2a87c408 (tag
`bench/2026-10-27-prettier-opus-gate`: the task at 6cf2915d, as in the Astra
half, plus `run_gate.sh` with `GATE_CONCURRENCY`), wave-1 identifiers,
`GATE_CONCURRENCY=1` (docs/DECISIONS.md 2026-10-27). Usage pre-flight 0.63
of the weekly window. Sessions real (every turn on `claude-opus-5-5`, 88 to
107 tool calls each, each running the visible suite with `FULL_TEST=1`; no
auth or usage event); no containers left behind. No verifier overlap, no
worker crash.

| Trial | Agent minutes | Reward | Held-out failing (of 18) | Guards | Tamper scan |
| --- | --- | --- | --- | --- | --- |
| AVWpv4E | 32.5 | 0 | 1: consistent-with-flow 15707.ts | hold (101,662) | flagged, false positive |
| FtaxW5N | 27.4 | 0 | 5: arrow-chain-with-trailing-comments.js (format and second format, both arrowParens options), parenthesized.js | 16 regressed, all other parsers on the same held-out fixture | flagged, false positive |
| ZbFiFtY | 25.6 | 0 | 2: parenthesized.js (format and second format) | hold | clean |

**0 of 3.** Every attempt declared done with the visible suite passing. The
closest (AVWpv4E) cleared 17 of 18. Across both halves the parenthesized
sequence fixture is missed by five of six attempts. No attempt took the
pre-registered leading-on-annotation shortcut. FtaxW5N's 16 regressions are
the other parsers' checks on the held-out arrow-chain fixture, which the
visible suite cannot show.

Tamper-scan false positive. The scan's JavaScript rule includes
`\bexpect\b`, which matches the `expect` in `// @ts-expect-error -- Safe`,
an idiom prettier's own `src` uses 97 times. Two attempts added such lines
(in `handle-comments.js` and `print/class.js`), so `tamper_clean` is 0 for
them. No reward changed: both also fail held-out checks, and the verifier
still ran and graded the full suite. A solve written in the same idiom would
be zeroed, so the rule needs fixing (exempt `@ts-expect-error` and
`@ts-ignore` comments) and the task re-validated before it is frozen.

**Verdict: ADMIT** (F5-javascript, 2026-10-27): GPT-6 Astra 0 of 3 and
Claude Opus 5.5 0 of 3, the same rule black was admitted under. Floors 1 to
8 met, floor 8 by route (a).

### javaparser-member-resolution-scope, Astra half (2026-10-28): the Astra prong passed

Built and validated 2026-10-27 (its DESIGN.md); floors 1 to 8 met, floor 8
by route (a). Job `astra-javaparser-member-resolution-scope`, launched 06:05
PDT on the owner's approval from a run worktree pinned to 4bdd2330 (tag
`bench/2026-10-28-wave1-java-cpp-astra-gate`: this task at c23c3cc6 and
Boost.URL at dd75baa6, plus `run_gate.sh` from a12619e2), wave-1
identifiers, concurrency 1 (JVM). The two tasks ran back to back, so no two
verifiers shared the Docker VM. Sessions real (`gpt-6-astra`, 33 to 42
commands each; no auth or usage event); no containers left behind.

| Trial | Agent minutes | Reward | Held-out failing (of 12) | Guards (4,932) |
| --- | --- | --- | --- | --- |
| hVQxFAq | 9.8 | 0 | 2: Issue4985Test fields visible to inheritors, subclass assignable | hold |
| QG9xa4z | 9.7 | 0 | 2: the same two | hold |
| LfMf7es | 7.7 | 0 | 2: the same two | hold |

**0 of 3.** Every attempt cleared 10 of the 12 held-out checks, kept every
guard green and declared done with both visible test modules passing. All
three miss the same two incomplete-hierarchy checks: fields of a class whose
ancestor cannot be resolved stay visible to its subclasses, and such a
subclass stays assignable to it. The attempts made method lookup tolerate a
missing distant ancestor but left the field and assignability paths alone.
None took the pre-registered members-only shortcut (it regresses 4 guards; no
attempt regressed one). As with black and prettier, held-out coverage the
visible suite does not show is what held the reference.

### javaparser-member-resolution-scope, Opus 5.5 half (2026-10-29): admitted

Job `opus-javaparser-member-resolution-scope`, launched 06:40 PDT on the
owner's approval from the same run worktree as the Astra half (4bdd2330, tag
`bench/2026-10-28-wave1-java-cpp-astra-gate`), wave-1 identifiers,
`GATE_CONCURRENCY=1` (JVM). Usage pre-flight 0.01 of the weekly window.
Sessions real (every turn on `claude-opus-5-5`, 69 to 116 tool calls each,
each running the Maven suites 9 to 22 times; no auth, usage or API error
event); no containers left behind. Finished 07:57.

| Trial | Agent minutes | Tool calls | Output tokens | Reward | Held-out failing (of 12) | Guards (4,932) |
| --- | --- | --- | --- | --- | --- | --- |
| 9hUc4ci | 20.9 | 69 | 198k | 0 | 1: Issue4985Test fields visible to inheritors | hold |
| dDuUVkH | 28.9 | 116 | 214k | 0 | 1: the same | hold |
| viNEEKe | 21.3 | 93 | 182k | 0 | 1: the same | hold |

**0 of 3.** Every attempt compiled, kept every guard green, had a clean
tamper scan and cleared 11 of the 12 held-out checks. Diff review against
the base: each is a genuine fix across the symbol solver's contexts and
declarations (11 to 16 files, 275 to 537 added lines) plus
`ResolvedReferenceType` in the core (only main sources are graded; the
verifier runs the pristine tests). Opus 5.5
got one check further than Astra (the subclass of a class with an
unresolvable ancestor stays assignable to it) but, like all six attempts
across both halves, left the field path alone: fields of such a class do not
stay visible to its subclasses. None took the pre-registered members-only
shortcut.

**Verdict: ADMIT** (F5-java, 2026-10-29): GPT-6 Astra 0 of 3 and Claude Opus
5.5 0 of 3, the rule black and prettier were admitted under. Floors 1 to 8
met, floor 8 by route (a).

### boosturl-path-compare-normalize, Astra half (2026-10-28): REJECT

Same run commit and launcher as javaparser above, started 06:37 PDT when
javaparser finished, concurrency 1 (C++). Sessions real (`gpt-6-astra`, 44
and 29 commands; no auth or usage event). Two solves ended the gate early;
the third attempt (uQbK8V2) was stopped by the early exit after 5 commands
and is not an attempt (its compose sidecar error is the stop). Leftover
sandbox containers removed by the launcher.

| Trial | Agent minutes | Reward | Held-out failing (of 10) | Guards (77) |
| --- | --- | --- | --- | --- |
| Lrrd5aF | 17.7 | **1** | none | hold |
| gnfoL2m | 21.6 | **1** | none | hold |

**2 of 2, reject.** Diff-reviewed against the base (the pin minus the nine
fixes): both edit exactly the gold's four files (`detail/normalize.hpp`,
`impl/url_base.hpp`, `impl/url_view_base.hpp`, `src/detail/normalize.cpp`),
about 360 lines added and 380 removed each. Each replaces the allocating
compare and hash with its own allocation-free normalized-path walker, a
forward pass that finds where relative prefixes cancel and a reverse walk
that consumes parent segments, shared by comparison and hashing, and fixes
the segment-count and modifier size bookkeeping and the `"./"` scheme-removal
case. The two walkers are structured differently from each other and from the
gold. No test strings are special-cased (the only literals added are hex
digits and `%2E`/`%2e`), the tamper scan and sentinel are clean, and neither
session fetched anything: the `github.com/boostorg/url/issues/992` URL in both
transcripts is a comment in the base's own `test/unit/url.cpp` that the agents
read. gnfoL2m checked itself against 330,000 generated paths and 100,000
ordering and hash pairs it wrote; Lrrd5aF against one million generated
normalization cases plus ordering, hashing and modifier checks.
Neither took the pre-registered slash-decoded shortcut.

The nine fixes concentrate in one subsystem the visible suite already
exercises from many angles, and a reference that writes its own differential
checks against RFC 3986 section 5.2.4 reaches the held-out cases without
seeing them. That is the same lesson as comrak (an oracle inside the sandbox
lets the agent grade itself), here with the RFC as the oracle. The F5-cpp
slot needs another candidate.

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

### cli11-toml-config-round-trip, Astra half (2026-10-30): the Astra prong passed, narrowly

Built and validated 2026-10-28 (its DESIGN.md); floors 1 to 8 met, floor 8
by route (a), floor 3 by source coupling only. Job
`astra-cli11-toml-config-round-trip`, launched 08:40 PDT under the owner's
standing approval (DECISIONS.md 2026-10-30) from a run worktree pinned to
1b38a48b (tag `bench/2026-10-30-cli11-astra-gate`: the task at 637faeb4
plus `run_gate.sh` from a12619e2), wave-1 identifiers, `GATE_CONCURRENCY=1`.
Sessions real (`gpt-6-astra`, 16 to 20 tool calls each, 3 to 5 test runs);
no containers left behind.

| Trial | Agent minutes | Reward | Held-out (of 11) | Guards (1,098) |
| --- | --- | --- | --- | --- |
| 7eJpRUv | 2.4 | 0 | 11, all cleared | 2 regressed: `StringBased: TomlMultiLineStringBinary`, `StringBased: TomlMultiLineStringTextAfterQuotes` |
| FQ8CPTC | 4.7 | 0 | 11, all cleared | the same 2 regressed |
| rMgrvod | 3.4 | 0 | 11, all cleared | the same 2 regressed |

**0 of 3, but only on guards.** Every attempt fixed all six named defects in
one file (`Config_inl.hpp`, 57 to 95 added lines) in under five minutes, and
every attempt broke the same two behaviours that work at base. Both are among
the six pre-registered held-out guards (DESIGN.md, "added with the fix,
already passing at base"): `'''a'''b'''` reads as `a'''b`, and a raw-binary
`B"(...)"` literal inside `'''...'''` is decoded. The instruction states the
rule ("everything that works today keeps working") and the visible suite
exercises neither case. The hold is legitimate under the gate rule, but it
rests on two edge behaviours next to the defects rather than on the defects
themselves, and these are the fastest attempts on any candidate gated so far.
The Opus 5.5 half decides; anyone reviewing the suite should read this note.

### pulldown-cmark-html-block-scanning, Opus 5.5 half (2026-10-30): 1 of 5 after the top-up, admitted

Gated under decision (g') (DECISIONS.md 2026-10-30): floor 8 not met,
Astra 0 of 3 on 2026-10-26. Job `opus-pulldown-cmark-html-block-scanning`,
launched 08:54 PDT under the owner's standing approval from a run worktree
pinned to 428b10d4 (tag `bench/2026-10-30-pulldown-opus-gate`: the task at
9fc86e09 as in the Astra half, plus `run_gate.sh` from a12619e2), wave-1
identifiers, `GATE_CONCURRENCY=1`. Usage pre-flight 0.11. Sessions real
(every turn on `claude-opus-5-5`, 54 to 76 tool calls each); no auth or usage
event.

| Trial | Agent minutes | Tool calls | Reward | Held-out failing (of 24) | Guards |
| --- | --- | --- | --- | --- | --- |
| c2AkrgH | 13.4 | 76 | 0 | 3 (r273, r274, r277) | hold |
| TYp9MfL | 11.1 | 71 | **1** | none | hold |
| ZtqPQc4 | 8.6 | 54 | 0 | 3 (r273, r274, r277) | hold |

**1 of 3.** Diff review of TYp9MfL against the base (5b0ef8c0): a genuine
fix in the three files the gold touches (`firstpass.rs`, `scanners.rs`,
`parse.rs`; 265 added lines, of which about 90 are the agent's own unit
tests in `parse.rs`), with case-insensitive end-tag matching for the
type 1 HTML blocks and comment, processing-instruction, declaration and
CDATA spans across lines inside containers. No special-casing of grading
inputs; tamper scan clean. The two failing attempts miss the same three
checks as the closest Astra attempts. A prong at exactly 1 of 3 takes the
n=5 top-up (two more attempts, admit only at 2 of 5 or fewer). The top-up
is outside the standing approval and waits for the owner.

Top-up, approved by the owner in chat: job
`opus-pulldown-cmark-html-block-scanning-topup` (`GATE_ATTEMPTS=2
GATE_JOB_SUFFIX=-topup`), launched 15:55 PDT from the same run worktree,
usage pre-flight 0.11, sessions real (`claude-opus-5-5`, 33 and 44 tool
calls), finished 16:11.

| Trial | Agent minutes | Tool calls | Reward | Held-out failing (of 24) | Guards |
| --- | --- | --- | --- | --- | --- |
| ReECqVJ | 7.6 | 44 | 0 | 3 (r273, r274, r277) | hold |
| qsghrbk | 5.4 | 33 | 0 | 6 (r259, r260, r261, r273, r274, r277) | hold |

**Opus 5.5 1 of 5, Astra 0 of 3.** Every failing attempt in both halves
misses r273, r274 and r277; the one solve is the genuine TYp9MfL above.

**Verdict: ADMIT** (F5-rust, 2026-10-30) under decision (g'): the prong at
1 of 3 finished at 1 of 5, below the 2 of 5 bar. Floors 1 to 7 met; floor 8
(neither route) is not met, and that miss is part of this admission's
record. With it, every F5 slot is admitted (c, python, javascript, java,
cpp, rust).

### cli11-toml-config-round-trip, Opus 5.5 half (2026-10-30): admitted by the gate rule, flagged

Job `opus-cli11-toml-config-round-trip`, launched 09:30 PDT under the owner's
standing approval from the same run worktree as the Astra half (1b38a48b,
tag `bench/2026-10-30-cli11-astra-gate`), wave-1 identifiers,
`GATE_CONCURRENCY=1`. Usage pre-flight 0.16. Sessions real (every turn on
`claude-opus-5-5`, 38 to 50 tool calls each); no auth or usage event; no
containers left behind. Finished 10:02.

| Trial | Agent minutes | Tool calls | Reward | Held-out (of 11) | Guards regressed |
| --- | --- | --- | --- | --- | --- |
| Kq2RTKs | 6.4 | 38 | 0 | 11, all cleared | 2: `TomlMultiLineStringBinary`, `TomlMultiLineStringTextAfterQuotes` |
| muvjyaY | 9.1 | 40 | 0 | 11, all cleared | the same 2 |
| nAn422p | 13.2 | 50 | 0 | 11, all cleared | 1: `TomlMultiLineStringBinary` |

**0 of 3.** Every attempt is a genuine one-file fix (`Config_inl.hpp`, 140 to
209 added lines) that clears every held-out defect check; every attempt
breaks one or both of the two edge behaviours that also held Astra.

**Verdict: ADMIT by the gate rule** (F5-cpp, 2026-10-30): GPT-6 Astra 0 of 3
and Claude Opus 5.5 0 of 3. **Flagged for the owner:** across both halves,
six of six attempts fixed all six named defects, in 2 to 13 minutes, and
every reward of 0 comes from the same two pre-registered held-out guards
(behaviours that work at base, which the instruction says must keep working
and the visible suite does not exercise). No other admitted task was held
only by guards. The task measures "fix six defects without disturbing two
neighbouring edge behaviours", which is a legitimate skill but a narrow
margin: one reference noticing either edge case would flip it. Whether a
hold of this shape belongs in Frontier v5 is the owner's call before the
freeze. **Owner, in chat, 2026-10-30: accepted as F5-cpp, with this flag
kept on the record.**

### ferrycore-crossing-opaque-parity, Astra half (2026-10-30): REJECT

The first F4 (opaque-component parity) candidate, F4-python, built and
validated 2026-10-30 by the F4 building session (branch
`claude/focused-lumiere-06a98a`, 443aa51c): base 0, gold 1, 7 probes 0, Harbor
oracle 1 and nop 0, deep-family reachability 0 to 3 of 500 per seed. The
reference is a prebuilt linux/amd64 binary (`legacy/ferrycore-linux-amd64`)
run under emulation on the arm64 Mac; grading never executes it. Job
`astra-ferrycore-crossing-opaque-parity`, launched 19:01 PDT under the owner's
standing approval from a run worktree pinned to b83c0ddb (tag
`bench/2026-10-30-ferrycore-astra-gate`), wave-1 identifiers,
`GATE_CONCURRENCY=1`, behind the shared Docker lock. Sessions real
(`gpt-6-astra`).

| Trial | Agent minutes | Reward | How |
| --- | --- | --- | --- |
| bLEkkKU | 7.4 | **1** | disassembled the binary, reimplemented it (306 lines) |
| prHDNuG | 23.2 | **1** | the same (307 lines) |
| pHwxquJ | stopped by the early exit | none | not counted |

**2 of 2, both genuine.** No disassembler was installed and the allowlist
blocked `apt-get`, `pip` and `npm` (one attempt also tried an internal
package index host, which the allowlist refused), so the solving attempt wrote
its own x86-64 disassembler in Python, read the binary's main loop by
address range, and wrote a Python reimplementation that never calls the
binary. It then ran a 1,000-session differential fuzz against `legacy/run`
and added its own tests. Every family passed, guards included.

Lesson for F4: composite-only observability does not hold when the reference
binary is small enough to read. paddockcore's v4 hold did not carry over to a
binary of this size. Any further F4 candidate must show its reference resists
reading in a session (size, structure) before it is gated.

**Verdict: REJECT** (early exit, 2026-10-30). Fable or Opus half not run.

### phonenumbers-parse-match-parity, Astra half (2026-10-30): not decided, one solve used a web tool

The first F2 candidate under shape B1, F2-python, built and validated
2026-10-30 by the F2 building session (branch
`claude/practical-satoshi-f4d035`, 55b8e515): base 0, gold 1, 4 probes 0, the
B1 check found no runnable route to the Java reference, Harbor oracle 1 and
nop 0. Job `astra-phonenumbers-parse-match-parity`, launched 19:57 PDT under
the owner's standing approval from a run worktree pinned to the tag
`bench/2026-10-30-phonenumbers-astra-gate`, wave-1 identifiers,
`GATE_CONCURRENCY=1`, behind the shared Docker lock. Sessions real
(`gpt-6-astra`).

| Trial | Agent minutes | Reward | How |
| --- | --- | --- | --- |
| JM2ysUP | 7.4 | **1** | genuine: ported from the Java reference; its `curl` and `pip download` were blocked |
| vHcy4ej | 6.4 | **1** | **not valid**: fetched the upstream Python files ten times through Codex's `web__run` |
| 5rohfSY | stopped by the early exit | none | not counted |

Diff review against the upstream gold. The stubs alone already share 2,307
lines of `phonenumberutil.py` with it. JM2ysUP adds almost nothing beyond
that (2,330 shared lines; 13% of the formatter and 21% of the matcher), so
it was written from the Java. vHcy4ej shares 2,667 lines of
`phonenumberutil.py` and 705 of the matcher's 787 lines, which matches its
web fetches.

`web__run` runs on OpenAI's side, so the task's network allowlist never sees
it. It is now off for both references (DECISIONS.md 2026-10-30,
`run_gate.sh` with `--ak disable_web_search=true`). An audit of every
gate trial on the Mac Studio found no other provider-side web use.

**Verdict: not decided.** Valid results: 1 genuine solve (JM2ysUP, in 7.4
minutes). The early exit fired on a contaminated second solve, so the half
must be re-run with web tools off before it is decided. One more genuine
solve rejects the task. The re-run is a re-run of a finished half, so it
waits for the owner.
