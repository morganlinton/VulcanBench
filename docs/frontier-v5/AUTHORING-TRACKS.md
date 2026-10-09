# Frontier v5 authoring: two tracks, to stay on Fable 5.1

Why this exists: authoring v5 kept dropping from Fable 5.1 to Opus 4.8. The
drop is the cybersecurity-category safety fallback. It is triggered by
memory-safety work (sanitizers, fuzzing, reproducing overflows), and once a
session's history is full of that content the classifier flags even unrelated
replies in the same conversation. Proof: after a night of fuzzing work, a
plain search for a JavaScript date-library bug flagged and fell back. The
cause is session context, not the individual task.

## The method

Partition the work across sessions by category. Do not mix the two tracks in
one conversation.

### Track A: correctness families, on Fable 5.1 (fresh session each batch)

Everything that is not memory-safety: F2 differential parity, F3 long-horizon
volume, and ordinary multi-bug correctness tasks, across Python,
JavaScript/TypeScript, Rust, C, C++ and Java. These do not flag. Start a
clean session per batch so context stays unsaturated. The four committed
tasks prove the pipeline (Python and Rust) end to end; reuse their structure.

Every Track A task must ship the three verifier layers from PHASE1.md
("Verifier audit 2026-10-08"): structural (only upstream graded paths),
report-based grading (never the runner's exit status) and tamper detection
(`tests/tamper_scan.py`, copied unchanged, plus a must-fail sentinel for
in-process runners), and must be probed with at least an exit attack and
a test-framework shadowing attack before it counts as validated.

The probe is mechanized: add a tests/probes.json (edits that try to score
without fixing anything) and run python3 scripts/frontier-v5/probe_verifier.py
tasks/frontier-v5/<task>. It builds the images, checks base=0 and gold=1, and
fails if any probe scores 1. All nine current tasks pass it.

### Track B: the F1 sanitizer family, on Opus 4.8 (its designated model)

Reproducing heap overflows, data races and undefined behavior, and writing
fuzz harnesses, is genuinely the cybersecurity category. Opus 4.8 is the model
Anthropic routes this to. Author the F1 family in a session dedicated to it,
on Opus 4.8, on purpose. Do not try to phrase F1 work to stay on Fable; that
is gaming the classifier, not a workflow fix. The F1 sourcing rule from
PHASE1.md still holds: start from a defect that ships a reproducer.

## Track A progress

NetworkX and SymPy (Python), petgraph (Rust) and luxon (JavaScript) are done.
The luxon task, tasks/frontier-v5/luxon-duration-format-fixedzone, composes
#1781, #1784 and #1786 at their common base and proved the Node/jest pipeline
on 2026-10-07 (base=0, gold=1, x3, single-fix controls). It is an easy
pipeline-prover, labeled UNSLOTTED.

Java is done too: tasks/frontier-v5/commons-lang-fraction-lowest-terms
composes five Fraction fixes from Apache Commons Lang at their common base
and proved the JVM/Maven offline pipeline on 2026-10-07 (base=0, gold=1, x3,
single-fix controls, full 89k-execution suite as the guard wall). Every
Track A language now has at least one validated task: Python (2), Rust,
JavaScript and Java. The next Track A gaps are C and C++ correctness tasks
(not F1) and a second, harder JavaScript or Java arc; the pipeline shape is
settled, so the remaining work is sourcing difficulty.

C++ is done too: tasks/frontier-v5/fmt-format-spec-conformance composes nine
{fmt} fixes (#4878 to #4968, 2026-08-10 to 2026-10-07) at their common base
and proved the C++/CMake pipeline on v5-cfamily on 2026-10-07 (base=0,
gold=1, x3, nine single-fix controls), the first task validated on the
x86-64 cloud VM. It is wide (six headers) but its causes are independent,
so it is labeled UNSLOTTED as a breadth task, not an interaction task.
C is done too (2026-10-08): yyjson-incremental-and-mutation-fixes composes
four upstream yyjson fixes and proved the plain-C (CMake + ctest) pipeline
on v5-cfamily. And the first F2 (differential parity) task exists:
comrak-gfm-tables-autolinks-parity, a Rust port of GFM tables and autolinks
graded byte-for-byte against the C reference cmark-gfm; it is built to fill
the F2-rust slot, not UNSLOTTED. Every language now has a validated task.

C++ now also has an interacting arc (2026-10-09):
tasks/frontier-v5/geos-curved-overlay-arc-noding composes five GEOS fixes to
curved-geometry overlay (#1478, #1480, #1513, #1546, #1549) in two files;
no single fix clears more than 5 of its 16 held-out cases and leaving any
one out fails at least one. Validated on the arm64 Mac including the real
Harbor oracle path. UNSLOTTED pending the fifth-family decision.

Remaining Track A gaps, in priority order: gate-measure what exists (needs
the reference credentials, the real blocker); more F2 and F3 tasks toward
their six slots each; and harder interacting arcs (the commons-lang shape)
rather than more breadth tasks. Every new task must pass
scripts/frontier-v5/probe_verifier.py (see below).

## One-line startup for a fresh Fable session

"Continue VulcanBench Frontier v5 Track A authoring on branch
frontier-v5-authoring-2. Read docs/frontier-v5/AUTHORING-TRACKS.md and
NIGHT-STATUS.md. Source the next correctness arc (plain C, or a harder
interacting Java, JavaScript or C++ arc) and build it like
tasks/frontier-v5/fmt-format-spec-conformance. Do not do F1 sanitizer work in
this session."

## Running a session in the cloud (no laptop needed)

Start a Claude Code cloud session from the GitHub repo on the current
authoring branch and paste the startup line above; cloud sessions keep
running after the laptop is closed and can push and open PRs. Before the
first task work, the session must run

```bash
scripts/frontier-v5/setup-host.sh
```

which builds the four architecture-neutral sandbox tags every v5 task
starts from (`vulcanbench/sandbox:v5-base`, `:v5-cfamily`, `:v5-jvm`,
`:v5-rust`) for the host's CPU and installs Harbor 0.24.0. Cloud VMs are
x86-64; the laptop is arm64; the tags are the same, the content is native to
each. Put the script in the cloud environment's setup script so the images
are cached across sessions.

Cloud prerequisites learned on 2026-10-07 (PHASE1.md, "Cloud VM setup"):
the environment needs Custom network access with `deb.debian.org`,
`go.dev`, `dl.google.com` and `dlcdn.apache.org` added to the default list
(set on the Default environment now); dockerd is installed but not running
at session start and does not survive a VM restore, so start it first
(`dockerd > /tmp/dockerd.log 2>&1 &`); and the script itself adds the
egress CAs and routes builds through the session proxy, so no manual step
is needed for that. Build task images with the same proxy arguments
(`--network host --build-arg HTTPS_PROXY=$HTTPS_PROXY`). Constraints: the cloud VM has 4 vCPU, 16 GB RAM
and 30 GB disk, so build one task's images at a time and prune old ones;
only committed and pushed files exist there (the withdrawn libmbus scaffold
is untracked and will be absent); and gate runs still need the reference-
model credentials, which must be configured as environment secrets, a
separate step from authoring. Track B (F1) sessions in the cloud follow the
same rule: pick Opus 4.8 explicitly.
