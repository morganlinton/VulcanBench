# VulcanBench OCaml v1

Status: eight validated development tasks, October 1, 2026. Seven original
tasks have historical calibration at 81.0% (17/21), so the target remains unmet.
The compiler task passed complete validation with public expect maintenance as
a scored guard. It has zero fresh outcomes on that revised definition. An old
core-only pass and an unscored provider refusal are preserved separately. The
expanded aggregate is withheld; no independent confirmation is claimed.
Suite id: `ocaml-v1`. Full suite size will follow pilot calibration.

## Purpose

Measure a balanced mix of practical OCaml engineering and language mastery.
Successful agents must navigate a realistic project, implement the requested
behavior, preserve existing contracts, and use OCaml's type and module systems
correctly. Language mastery is exercised through executable changes rather than
trivia questions.

The intended balance is approximately half repository engineering tasks and half
focused language tasks, with overlap encouraged. Report both views alongside the
aggregate so a strength in one does not conceal a weakness in the other.

## Jane Street relevance

The suite should help engineers evaluate coding agents on work resembling Jane
Street's publicly documented OCaml engineering: typed libraries, deterministic
testing, asynchronous systems, incremental computation, and performance-aware
changes. The research and source links are in
[JANE_STREET_RESEARCH.md](JANE_STREET_RESEARCH.md). This is an independent design
based on public evidence; internal representativeness has not been validated.

Use compatible, pinned Base/Core libraries and Jane Street testing tools where
they serve the task. The original pilot provides public smoke tests, specified
examples, and independent hidden assertions, including seeded reference-model
checks. Expect-test integration and richer public examples remain an expansion
goal. A task should exercise a stated invariant, such as comparator
consistency, protocol compatibility, shutdown behavior, or bounded recomputation.

Keep the initial pilot compatible with upstream OCaml. Plan a separately scored
OxCaml extension track for locality, allocation, and safe sharing, using its own
pinned compiler and library set. Do not mix these tasks into the upstream score.

## Coverage

| Area | Engineering behavior | OCaml-specific reasoning |
|---|---|---|
| Libraries and interfaces | Extend an API without breaking callers | Abstract types, `.mli` contracts, functors |
| Typed representations | Maintain an evaluator or transformation | Variants, GADTs, existential types, exhaustive matching |
| Parsing and serialization | Handle malformed inputs and round trips | Recursive variants, parser state, structured errors |
| Stateful components | Fix a cache, index, or persistent collection | Mutation, sharing, comparison, resource cleanup |
| Tooling and concurrency | Repair integration or lifecycle behavior | Dune modules, PPX interfaces, OCaml 5 effects where relevant |

Do not force every advanced feature into v1. Concurrency and compiler-tooling
tasks enter only when their dependencies and outcomes can be made reproducible.
Avoid a suite dominated by one library or by compiler implementation internals.

## Five-task pilot

These are implemented original fixtures, not claims of measured difficulty. The
five starting projects are small implementation exercises with multiple modules;
larger OSS repositories remain an expansion requirement.

| Candidate | Primary view | Brief | Key verification |
|---|---|---|---|
| `interval-map-transaction` | Language mastery | Implement comparator-safe interval edits and atomic batches | Custom ordering; restoration; normalization; persistence |
| `typed-frame-stream` | Language mastery | Implement a GADT-based codec and fragmented stream decoder | Canonical fixtures; malformed inputs; recovery; typed clients |
| `async-bounded-dispatch` | Engineering | Implement bounded dispatch, graceful close, and abort | Controlled deferred completions; FIFO; lifecycle; errors |
| `incremental-atomic-graph` | Both | Implement atomic formula edits and dynamic dependencies | Stable observers; inactive branches; cutoffs; work counters |
| `window-watermark-allocation` | Engineering | Repair event-time window semantics and allocation | Reference model; boundary cases; native allocation/work limits |

Original tasks should resemble maintained libraries, with multiple modules,
public tests, interfaces, and a clear issue. Do not reduce them to isolated
function-completion puzzles. Real OSS tasks require an identified upstream issue
or PR, a pinned base revision, license preservation, and verified provenance.
Replace a brief if sourcing or validation shows it is unsuitable.

At least one engineering candidate should be grounded in a verified OSS change
if a suitable source is available. An original fixture using a public library is
an original task, not an upstream bug claim. Briefs and source leads are tracked
in [CANDIDATES.md](CANDIDATES.md).

## Execution environment

Use `vulcanbench/sandbox:ocaml-v1`, with OCaml 5.2.1, Dune 3.17.2, and pinned
Jane Street v0.17 packages. The dependency list is in `TOOLCHAIN.lock`.
Install dependencies during image construction and run tasks without network
access. Keep dependencies readable and build/cache locations writable for the
non-root agent and verifier.

Pin a compatible upstream release family of the Jane Street libraries, including
PPX and testing dependencies. Inspect actual source requirements before selecting
a snapshot; a repository's current development branch may use OxCaml extensions.
The compiler-source task uses `vulcanbench/sandbox:ocaml-compiler-v1`, pinned to
upstream OCaml 5.6.0+dev0 at its documented base revision. The seven original
fixtures retain the 5.2.1 image. The optional OxCaml track needs a separate image
and compiler revision.

Dune supports executable test stanzas, and OCaml's modules and interfaces are
part of the language contract. See the official documentation for
[Dune tests](https://ocaml.org/docs/running-executables-and-tests-with-dune) and
[modules](https://ocaml.org/docs/modules).

Cold compiler/dependency setup must be checked before model measurement. Any
warm-up may compile the starting project and public tests only; hidden tests and
the reference patch must remain unavailable to the agent.

## Grading and validity

Use the existing declarative `grader: "tests"` format. Hidden OCaml tests are
installed only after the agent finishes. Grade observable behavior and documented
type/interface contracts, allowing alternate correct implementations.

- Each task has at least three distinct fail-to-pass checks and independent
  pass-to-pass regression guards that pass in the starting project.
- The reference patch passes every required check. Every fail-to-pass check fails
  at base for the intended task defect, with logs inspected to exclude missing
  dependencies or a broken test harness.
- Validate base and reference states in Docker over at least three repetitions.
- Separate public and hidden Dune test targets. Hidden target failures must not
  prevent unrelated regression targets from compiling or running.
- Interface tasks compile representative external clients. If a negative compile
  check is required, verify its expected diagnostic; an arbitrary compiler error
  must not count as successful rejection.
- A compile error introduced by the submitted solution is a capability failure.
  A missing compiler, unavailable dependency, or verifier malfunction is an
  infrastructure failure.
- Use seeded property checks and fixed fixtures where helpful. Avoid dependence
  on scheduler timing, network services, or implementation-specific diagnostics
  unless explicitly part of the pinned contract.
- Preserve externally checked type/API invariants. Tasks requiring type safety
  explicitly disallow unsafe casts or weakening the supplied interface; inspect
  the patch as well as compiler results when assessing that requirement.
- Public expect snapshots may be edited when behavior legitimately changes;
  hidden expected results remain controlled by the verifier. Passing by promoting
  snapshots alone must not satisfy the task.
- For efficiency tasks, prefer specified work counters and measured allocation
  growth over wall-clock gates. Calibrate bounds against reference and deliberately
  inefficient implementations in the pinned environment. Timing is diagnostic
  until its stability is demonstrated.

Primary metric: task pass@1, requiring all required tests and regression guards
to pass. Preserve the existing per-check functional score for diagnostics.
Publish engineering and language views with their denominators. Treat the small
pilot's scores as calibration evidence rather than a stable leaderboard.

For engineer-facing results, include a per-task failure breakdown: behavior,
regression, interface/type contract, asynchronous lifecycle, or specified
efficiency contract. Report solve time, tokens, and cost when available. Keep
human review of maintainability separate from automated pass@1; test success is
not a claim that a patch is ready to merge.

## Calibration

Measure at least three repetitions per task and model configuration. Record the
reference configuration explicitly, including any provisional default chosen
during authorized build-and-test work.
Record model, harness, effort, compiler/image identity, and task revision.

Favor tasks that separate configurations across repeated runs. Keep a small
number of easy anchors and fair hard tasks; replace saturated, ambiguous, flaky,
or environment-dominated candidates. Set full-suite size and difficulty targets
from pilot evidence rather than copying another language's task count.

Use repository operating policy for blocked efforts. Record any new timeout or
concurrency decision and its evidence in `docs/DECISIONS.md` before launching
measurements. This draft sets no new run budget.

## Integration work identified

The existing suite loader can load an `ocaml-v1` directory, and the verifier
already executes arbitrary named commands. New suite work should reuse those
paths.

Implemented integration:

1. Build the pinned OCaml sandbox image. Tasks select it through
   existing `metadata.image`; automatic language selection is optional.
2. The agent's default `run_tests` command recognizes a Dune project.
   Hidden verification commands must retain their real exit codes.
3. Missing OCaml, Dune, and opam commands are recognized in verifier infrastructure
   detection, with focused regression tests.
4. Five fixtures include public tests, hidden tests, reference patches, and
   deliberately incomplete solution controls.
5. `suite.json` declares the explicit pilot list and its measurement status.
6. `scripts/validate_ocaml_pilot.py` audits every check at base and reference over
   three repetitions and rejects incomplete controls. Calibration and admission
   follow [MEASUREMENT_PLAN.md](MEASUREMENT_PLAN.md).

## Pilot completion criteria

Five implemented candidates spanning both views, all passing base/reference and
determinism validation in the pinned sandbox; a reproducible calibration record;
and a documented decision on which tasks to admit, revise, or replace before
expanding the suite.
