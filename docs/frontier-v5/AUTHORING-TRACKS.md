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

Java is the remaining empty language: mine a test-bearing correctness fix in
a pure-Java library (no JNI, no concurrency races, to stay in Track A). The
JVM image (vulcanbench/sandbox:jvm, Temurin 21, Maven 3.9.16, Gradle 9.8.0,
offline) is built and smoke-tested; the pipeline shape is the luxon one with
Maven in place of jest.

## One-line startup for a fresh Fable session

"Continue VulcanBench Frontier v5 Track A authoring on branch
frontier-v5-authoring. Read docs/frontier-v5/AUTHORING-TRACKS.md and
NIGHT-STATUS.md. Build the Java task next, same pipeline as
tasks/frontier-v5/luxon-duration-format-fixedzone. Do not do F1 sanitizer work in
this session."
