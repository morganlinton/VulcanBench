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

## Ready now for a Track A (Fable) session: JavaScript

NetworkX and SymPy (Python) and petgraph (Rust) are done. JavaScript/TypeScript
and Java have no tasks yet. Clean JavaScript correctness candidates found in
moment/luxon (MIT), all recent and test-bearing, none security-related:

- #1784 avoid exponential notation in Duration#toISO
- #1781 FixedOffsetZone reporting isValid=true for unsupported inputs
- #1786 lost negative sign in Duration#toFormat when the largest unit is zero

A single composed luxon Duration task (compose the three) would be a Track A
JS task built exactly like the NetworkX tasks: clone at the common base, strip
history, Node sandbox image, separate verifier importing the agent's build,
gold tests overlaid, base=0 gold=1 validated. Java is the other gap; mine a
test-bearing correctness fix in a pure-Java library (no JNI, no concurrency
races, to stay in Track A).

## One-line startup for a fresh Fable session

"Continue VulcanBench Frontier v5 Track A authoring on branch
frontier-v5-authoring. Read docs/frontier-v5/AUTHORING-TRACKS.md and
NIGHT-STATUS.md. Build the luxon Duration JavaScript task next, same pipeline
as tasks/frontier-v5/nx-group-betweenness-epic. Do not do F1 sanitizer work in
this session."
