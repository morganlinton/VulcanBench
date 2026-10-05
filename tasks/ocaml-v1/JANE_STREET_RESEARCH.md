# Jane Street relevance research

Researched 2026-09-30 from public first-party material. Recommendations below are
VulcanBench design proposals. They have not been reviewed by Jane Street engineers.

## Recommended question

Which coding agents can make useful changes to substantial OCaml projects while
preserving their type, behavioral, and efficiency contracts?

That question makes the suite useful for choosing agents, identifying work that
needs close review, and diagnosing why a seemingly successful patch fails in a
real project. It retains the requested balance between engineering and language
mastery. Financial examples can supply context when all rules are specified;
specialist trading knowledge should not be an unstated prerequisite.

## What the public evidence supports

Jane Street describes OCaml as its primary development platform across a broad
range of systems. Its public technology page features Base, Bonsai, Hardcaml,
Memtrace, and OxCaml, reflecting a wider engineering footprint than trading logic.
[Source: Technology](https://www.janestreet.com/technology/).

Its October 2024 developer-education account covers expect and property tests,
controlled time with `Async.Time_source`, marketdata, typed API design and GADTs,
performance, and incremental UI work. This is evidence of publicly described
teaching priorities at that date, not a complete current internal curriculum.
[Source: Developer education](https://blog.janestreet.com/developer-education-at-jane-street-index/).

In June 2026, Yaron Minsky described agent-generated code that achieves a goal
while missing codebase invariants and creating review burden. The post connects
their interest in formal methods to coding agents and emphasizes compiler/type
feedback as well as tests. This provides a particularly direct motivation for
evaluating invariant preservation.
[Source: Formal methods and the future of programming](https://blog.janestreet.com/formal-methods-at-jane-street-index/).

The OxCaml project identifies itself as Jane Street's production compiler. It
focuses on performance control, allocation, layouts, and concurrency safety, and
warns that extensions have no stability or backward-compatibility guarantee.
Public libraries have upstream-OCaml and OxCaml forms; some are OxCaml-only.
[Source: OxCaml](https://oxcaml.org/).

## Translate priorities into tasks

| Public technology | Proposed task | What to verify | Source |
|---|---|---|---|
| Base/Core ecosystem | Extend a comparator-aware collection API | Public interface, custom key ordering, immutability | [Base](https://github.com/janestreet/base) |
| Expect tests | Give realistic executable public examples | Independent hidden behavior checks, not snapshot promotion alone | [ppx_expect](https://github.com/janestreet/ppx_expect) |
| Property testing | Exercise many valid inputs and transitions | Seeded runs, useful failure examples, independent reference model | [base_quickcheck](https://github.com/janestreet/base_quickcheck) |
| Async | Repair a timed or buffered pipeline | Stated delivery/closure semantics, timeouts, resource lifecycle | [Async time-source interface](https://github.com/janestreet/async_kernel/blob/master/src/time_source_intf.ml) |
| Incremental | Repair a derived-data dependency graph | Reference equivalence, updates, recomputation work | [Incremental](https://github.com/janestreet/incremental) |
| Binary protocols | Extend a specified typed codec | Old fixtures, bounds, malformed input, round trips | [bin_prot](https://github.com/janestreet/bin_prot) |
| Performance engineering | Remove unnecessary allocation or traversal | Equivalent behavior, measured allocation/work growth | [Performance engineering](https://www.janestreet.com/performance-engineering/) |
| OxCaml | Refactor a local allocation or sharing boundary | Compiler-enforced modes plus runtime results | [Modes introduction](https://oxcaml.org/documentation/modes/intro/) |

The task column is a proposal, not a claim that a source repository contains a
particular defect. The first pilot should use the revised five briefs in the
charter. Broader candidates such as Bonsai and Hardcaml can follow if engineers
identify them as priorities and the environment cost is justified.

## Concrete task sketches

1. **Comparator-safe collection API.** Start with an abstract Base-backed
   collection and extend an operation used by multiple clients. Verify custom
   comparators and persistence. For a comparator-witness API, compile clients
   that use the intended witness and check rejection of incompatible witnesses.

2. **Typed versioned codec.** Extend an interface where phantom types or GADTs
   distinguish message kinds or versions. Supply the whole protocol and legacy
   fixtures. Require existing encodings to remain readable and new behavior to
   reject malformed inputs without weakening the typed boundary.

3. **Async pipeline lifecycle repair.** A consumer stops while producers or
   retries are pending. Specify whether pending work completes, fails, or is
   cancelled. Test bounded buffering, timeout boundaries, and closure using a
   controlled time source and explicit scheduler progress instead of sleeps.

4. **Incremental dependency repair.** Change the dependency selected by an input,
   then update the old and new sources. Require agreement with a simple reference
   model and a stated bound on redundant work. Allow equivalent graph designs.

5. **Allocation-conscious library change.** Preserve a batch operation's results
   while reducing a known source of per-element allocation or repeated traversal.
   Supply input families and an explicit efficiency contract. Establish bounds
   from measured reference runs and inefficient controls before admitting it.

## Two compiler tracks

The implemented upstream compiler task repairs shared field classification under
GADT equations. It combines runtime GC-root safety, preservation of immediate
field optimization, type-environment reasoning, and maintenance of existing
public expect tests. This is a design inference from the public emphasis on
typed invariants and performance control discussed above. It is not a claim that
Jane Street encountered this bug or uses this upstream compiler revision.
The task uses actual upstream OCaml source, not OxCaml; its provenance and
requirement-to-check mapping are in
[the compiler admission record](compiler-gadt-field-safety/ADMISSION.md).

Keep the first pilot on upstream OCaml with a compatible pinned release family
of Base/Core, Async, PPX, and test libraries. Exact versions remain a build
decision. For example, the opam metadata for Base v0.17.3 specifies OCaml at least
5.1.0 and Dune at least 3.11.0; this is a compatibility lead, not a selected lock.
[Source: Base v0.17.3 package metadata](https://opam.ocaml.org/packages/base/base.v0.17.3/).

Do not assume development heads compile on upstream OCaml. The Base map interface
inspected during research includes modal/template annotations, reinforcing the
need to examine each actual snapshot.
[Source: Base map interface](https://github.com/janestreet/base/blob/master/src/map_intf.ml).

Prepare a separate OxCaml extension track after the upstream pilot works. Start
with locality and safe sharing; unboxed representation changes are a later
candidate. Pin the compiler commit, libraries, build tools, and documentation.
Show compiler-track results independently so an aggregate does not confuse
standard OCaml capability with knowledge of additional language features.
[Source: Unboxed types](https://oxcaml.org/documentation/unboxed-types/intro/).

## Make the result actionable for engineers

- Show per-task outcomes and failure categories alongside aggregate pass@1.
  Distinguish behavior, regression, typed interface, asynchronous lifecycle,
  efficiency contract, and infrastructure incidents.
- Preserve patches, test output, model/tool configuration, and environment pins
  so engineers can reproduce and inspect failures.
- Report solve time and available token/cost information separately from the
  submitted program's performance. A fast coding run does not imply fast code.
- Use documented requirements and compiler checks for type safety. Require
  preservation of supplied interfaces and prohibit unsafe casts where those
  would defeat the task. Compilation alone is insufficient if the agent can
  simply replace the interface.
- Offer public tests and local documentation for pinned dependencies. If a
  documentation-assisted comparison is added later, hold tool access constant
  and report it separately from an unaided condition.
- Add human review notes for maintainability and unnecessary changes. Keep those
  notes separate from automated functional scores until a review rubric has been
  validated. Passing hidden tests is not a mergeability certification.

## Useful feedback to seek before expanding

The most valuable next external input would be a review of the pilot briefs by
an OCaml engineer familiar with Jane Street's ecosystem. Ask which resemble
useful agent work, which invariants are easy for generated patches to break, and
whether an upstream-compatible or OxCaml track is more actionable for their team.
This is a recommended future step; no outreach has been sent.

Keep the public name VulcanBench OCaml v1. Describe the relevance through the
selected ecosystem and evaluated contracts; no Jane Street endorsement or
internal-codebase coverage has been established.

## Upstream compiler candidate

The compiler-field candidate uses the actual upstream
[OCaml PR 15114](https://github.com/ocaml/ocaml/pull/15114), merged September 30,
2026. The repair prevents a shared record-field access from using one pattern
row's GADT equation to omit a native GC root. The upstream patch also classifies
known immediate tuple and constructor fields precisely. This combines type
environment reasoning, native GC behavior, performance, and compatibility with
existing compiler tests in a substantial repository.

Its relevance is a design inference from the publicly described interest in
typed APIs, GADTs, performance, compiler feedback, and invariant-preserving agent
patches cited above. It is an upstream OCaml compiler task and does not measure
OxCaml extensions or establish coverage of Jane Street's internal compiler work.
The public fix is available for retrieval, so the source explicitly makes no
decontamination claim. Repeated reference/control validation and fresh model
calibration are required before admitting it to the suite.
