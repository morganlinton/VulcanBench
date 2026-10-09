# Frontier v5 overnight status (2026-10-06 into 2026-10-07)

Read this first. It summarizes a night of autonomous authoring, what is
validated, what is blocked on you, and the one design question that matters.

## TL;DR

Seven tasks are built and locally validated (base scores 0, gold scores 1,
deterministic x3, through the real separate-verifier flow). The seventh,
added 2026-10-07 in a cloud session, is the first C++ task and the first
validated on the x86-64 cloud VM. Three are
genuinely hard correctness tasks (two NetworkX graph-algorithm, one SymPy
symbolic-math), one of them the direct successor to the only v4 task that ever
beat the stronger reference; the fourth is an easy Rust pipeline-prover. The
Harbor-native Python pipeline is proven end to end, and the Rust pipeline too
(native arm64 image, pinned Cargo.lock, offline build and test, via an easy
third task). Nothing is gate-measured,
because reference-model credentials are deferred by your call, so "challenges
the frontier" is designed-for and argued, not yet measured.

The honest catch: all four are v4-shaped (concentrated correctness fixes), not
members of a frozen v5 family (F1 sanitizer, F2 differential, F3 volume, F4
opaque). That is a real finding, not a slip: the difficulty that actually beat
frontier models in v4 lives in concentrated algorithmic interaction, which the
frozen family table has no home for. Decision needed (see bottom).

## Overnight 2026-10-07 into 2026-10-08 (second session): what changed

Three things happened after the first night, all on branch
frontier-v5-authoring-2 (PRs #177 merged, #178 open):

1. **A verifier audit (below) found five of seven tasks could be scored
   without fixing anything.** All are fixed and re-validated; the cheating
   attempts are now a committed, reusable check,
   `scripts/frontier-v5/probe_verifier.py`, with a `tests/probes.json` per
   task. Every task now passes base=0, gold=1 and every probe=0 on it.
2. **First F2 (differential parity) task:**
   `comrak-gfm-tables-autolinks-parity` (Rust). Port GFM tables and extended
   autolinks into comrak so its HTML matches the C reference cmark-gfm byte
   for byte on 5,410 cases (150 visible, 5,260 hidden). A real F2 slot
   candidate, not UNSLOTTED. Validated base 0 / gold 1 x3.
3. **First plain-C task:** `yyjson-incremental-and-mutation-fixes`, four
   upstream yyjson fixes (incremental reader, mutable iterator, file
   writer). Validated base 0 / gold 1 x3 + single-fix controls. UNSLOTTED.

Count now: nine built-and-validated tasks. Languages with at least one task:
Python (2), Rust (2: petgraph UNSLOTTED, comrak F2), JavaScript, Java, C++,
C. Slot-wise still almost all UNSLOTTED; comrak is the first task built to
fill a frozen slot (F2-rust). Nothing is gate-measured; reference-model
credentials are still the blocker.

## Local session 2026-10-08 to 10-09 (back on the arm64 Mac)

Authoring moved back from cloud sessions to the Mac. Three outcomes:

1. **The cloud-authored tasks are confirmed on arm64.** yyjson and comrak
   passed as built; fmt needed one metadata fix (an x86-only test,
   `write_float128`, was required to pass) and now passes too.
2. **Tenth task: `geos-curved-overlay-arc-noding`** (C++), five interacting
   GEOS fixes to curved-geometry overlay, the "harder interacting arc" the
   C++ gap called for. No single fix clears more than 5 of its 16 held-out
   cases and leaving any one out fails at least one. Validated base 0 x3,
   gold 1 x3, three cheating probes 0, and through the real Harbor path
   (oracle 1, nop 0). UNSLOTTED.
3. **Verifier robustness lessons** (PHASE1.md, "Back on the arm64 Mac"):
   generous per-case bounds, pools sized to the CPU quota, conditional tests
   out of pass_to_pass, never discard a failing self-check's output.

Count now: ten built-and-validated tasks. Still nothing gate-measured.

## Verifier audit (2026-10-08): read before trusting any reward

Every Track A verifier was attacked on purpose after a review finding. Five
of the seven could be beaten by a few lines of graded code that fix
nothing; the worst, nx-digraph-node-connectivity (the headline task),
scored reward 1 from a three-line `os._exit(0)`. All seven now grade from
the runner's own report rather than its exit status, take only the
upstream graded paths, run a shared tamper scan and (for pytest, jest and
googletest) require a sentinel test that asserts something false to fail.
Every task re-probed at base 0, gold 1, every attack 0. Details, the attack
table and the rule for new tasks are in PHASE1.md, "Verifier audit
2026-10-08". Any reward recorded before this audit should not be relied on;
none had been gate-measured.

## What is on the PR (#174, branch frontier-v5-authoring)

| Task | What it is | Validated | Family |
| --- | --- | --- | --- |
| nx-group-betweenness-epic | 6 interacting bugs in group_betweenness_centrality (directed, disconnected, zero path counts, endpoints normalization, order independence) | base=0 gold=1 x3 | UNSLOTTED (below F3 floor) |
| nx-digraph-node-connectivity | 5 interacting bugs in node_connectivity / minimum_node_cut / minimum_st_node_cut for digraphs, self-loops, parallel edges | base=0 gold=1 x3 | UNSLOTTED; v4 near-miss successor |
| petgraph-maxflow-sparse-index | Rust: max-flow scratch vectors sized by count not index bound; panics on graphs with removed nodes/edges (Dinic's + Ford-Fulkerson) | base=0 gold=1 x3 offline | UNSLOTTED; easy Rust pipeline-prover |
| sympy-monotonic-sign-signed-groups | SymPy: sign inference fails to combine bounds of several same-signed terms, must prove more signs without ever claiming an unguaranteed one | base=0 gold=1 x3 | UNSLOTTED; symbolic-math domain |
| luxon-duration-format-fixedzone | JavaScript: three independent Luxon defects at a common base (Duration#toISO exponential notation, Duration#toFormat losing the sign when the largest unit is zero, FixedOffsetZone valid for non-numeric offsets) | base=0 gold=1 x3 + single-fix controls | UNSLOTTED; easy JS pipeline-prover (added 2026-10-07, Track A session) |
| commons-lang-fraction-lowest-terms | Java: five interacting Fraction defects at a common base (unreduced-operand overflow in multiplyBy and add/subtract, unreduced zero-operand results, Integer.MIN_VALUE reduction, string factory leaking ArithmeticException); full 89k-execution suite as the guard wall | base=0 gold=1 x3 + single-fix controls | UNSLOTTED; Java pipeline-prover, moderate difficulty (added 2026-10-07, Track A session) |
| fmt-format-spec-conformance | C++: nine independent {fmt} defects at a common base where output departs from printf or std::format (hexfloat zero, inf/nan alignment, debug width, named-arg store count, calendar width and fill, debug-string ranges, printf zero precision and negative precision, high-precision floats, compiled variant output); six headers, every one of 584 cases run in its own process as the guard wall | base=0 gold=1 x3 + nine single-fix controls, on v5-cfamily (x86-64 cloud VM) | UNSLOTTED; C++ pipeline-prover, breadth not interaction (added 2026-10-07, Track A cloud session, PR #177) |

Each ships: a symptoms-only instruction with no file or function-location
hints, a pinned single-commit base workspace (fix history stripped), a
separate verifier that imports the agent's sources in-tree and protects the
guard wall from a pristine image, a gold patch and oracle, a pre-registered
DESIGN.md with the difficulty hypothesis, and a family-fit caveat.

## Why nx-digraph-node-connectivity is the headline

Its v4 ancestor, oss-networkx-digraph-node-cuts, is the single task that beat
the stronger reference in the whole v4 program (Opus 5 solved 0/3, codex 1/3).
Upstream later fixed exactly that surface in one PR with five interacting bugs,
where the correct fix needs the Even-Tarjan both-orderings insight, not a local
patch. If any v5 task challenges the frontier, this is the most likely one. It
needs a gate run to confirm.

## Proven and recorded this night (docs/frontier-v5/PHASE1.md)

- The Harbor-native Python task pipeline works end to end on Docker.
- Two Python traps are designed around: an editable install shadows worktree
  edits (so the verifier imports strictly in-tree), and read-write mounts let a
  swap contaminate the base (so validation mounts read-only and mutates a copy).
- F1-by-fuzzing is a dead end (three attempts, OpenAPV and libmbus): the
  shallow defects are already guarded and fuzzer-found crashes are co-resident
  bugs the gold does not fix. F1 must be sourced from defects that ship a
  reproducer (OSS-Fuzz regression, CVE PoC, committed fuzz-corpus entry).

## Blocked on you

1. Reference-model credentials for Harbor (deferred). Until these exist, no
   task can run the admission gate, so difficulty is unconfirmed. Both tasks
   are built to go straight to the gate when credentials land.
2. The design question below.

## The design question

The frozen v5 family table (F1/F2/F3/F4) has no home for concentrated
multi-bug algorithmic correctness, yet that is where v4's real
frontier-beating difficulty lived and where both of tonight's tasks sit. Three
options, your call, none taken unilaterally because the plan is frozen:

- Add a fifth v5 family for concentrated multi-bug correctness, and these two
  seed it.
- Keep them for a revived mid-band suite and keep v5 to the four frozen
  families.
- Admit into F3 only arcs large enough to meet the volume floor, and treat
  these as practice that proved the pipeline.

My recommendation: the first. The evidence from both v4 and tonight is that
file-count breadth (the F3 floor) is not where difficulty comes from;
interaction depth is. A family defined by "N interacting defects, no location
hints, wide guard wall" captures the real lever.

## Suggested next step

Decide the family question, then set up Harbor credentials so these two tasks
can be gate-measured. Everything else (more candidates, other languages) is
downstream of knowing whether this task shape actually beats the current
frontier.
