# OCaml v1 candidate record

See [CHARTER.md](CHARTER.md) for the proposed coverage and admission rules.
The five pilot entries have original starting projects, hidden tests, reference
patches, and incomplete-solution controls. Single-configuration development
calibration of seven original candidates is complete at 81.0% task pass@1. An
eighth upstream compiler task passed revised public-workflow validation; it has
zero fresh revised-task outcomes, and expanded calibration is incomplete.
Results and decisions are in [CALIBRATION.md](CALIBRATION.md).

| Candidate | Primary view | Source | Status |
|---|---|---|---|
| `interval-map-transaction` | Language mastery | Original using Base | 3/3 passes after work/allocation revision; revise or replace for hard pool |
| `typed-frame-stream` | Language mastery | Original GADT codec | 3/3 passes; revise or replace for hard pool |
| `async-bounded-dispatch` | Engineering | Original using Async_kernel | 3/3 passes; keep as possible easy anchor |
| `incremental-atomic-graph` | Both | Original using Incremental | 0/3 passes; retain as provisional hard candidate |
| `window-watermark-allocation` | Engineering | Original allocation task | 3/3 passes; revise or replace for hard pool |
| `typed-query-optimizer` | Language mastery | Original typed DSL | 3/3 passes; useful coverage, hard-pool status unproven |
| `async-generation-cache` | Engineering | Original using Async_kernel | 2/3 passes; overdue-generation subscription failed once |
| `compiler-gadt-field-safety` | Both | Actual upstream OCaml compiler | Revised public workflow validated; admitted with zero fresh revised-task outcomes |

## Source leads

The [Jane Street research note](JANE_STREET_RESEARCH.md) explains the rationale
and links to primary sources. These are libraries to investigate, not verified
bug reports or selected PRs:

- `janestreet/base`: comparator-aware collections, abstract interfaces, utilities.
- `janestreet/async_kernel`: deferred results, pipes, controlled time sources.
- `janestreet/incremental`: dependency updates and stabilization.
- `janestreet/bin_prot`: binary codec behavior and boundary handling.
- `janestreet/base_quickcheck` and `janestreet/ppx_expect`: public and hidden test
  infrastructure, not automatic sources of task difficulty.

For each source, inspect a compatible release or revision, dependency closure,
license, and behavioral contract before choosing a task. Preserve enough project
structure for meaningful navigation. Do not assume recent public commits expose
the associated internal issue or its complete specification.

## Sources checked during overnight expansion

[bin_prot issue 33](https://github.com/janestreet/bin_prot/issues/33) proposes
passing a typed context through nested readers for schema migration while
preserving bytes. Its [draft PR 34](https://github.com/janestreet/bin_prot/pull/34)
was closed without a merge, and the author explicitly had not run its tests.
This is a useful original migration-task lead, not an accepted upstream repair.

[Core_kernel PR 57](https://github.com/janestreet/core_kernel/pull/57) proposes a
GADT and inline-record union-find representation to remove indirections and
allocation. It is an unmerged 2016 proposal. Its age, compiler assumptions, and
unknown training exposure make it unsuitable as a new decontaminated bug task.
Keep it as a representation/performance design lead only.

The typed-query optimizer is original, rather than a reproduction of either
proposal. Its scope/effect contracts are fully supplied in the issue and typed
interfaces. It tests safe normalization of a DSL rather than trading knowledge.

[OCaml PR 15114](https://github.com/ocaml/ocaml/pull/15114) is a verified merged
repair, dated September 30, 2026, to shared field classification under GADT
equations. The pinned pre-fix Git snapshot has 4984 tracked files and an LGPL 2.1
license with a linking exception. The release archive omits the upstream test
corpus; the candidate uses the complete pinned Git checkout. The offline baseline
compiler build passed. Admission
requires independent native-GC and classification checks, regression guards,
compiling controls, and repeated validation. The public patch remains a possible
retrieval source; its date does not establish any model's training cutoff.

## Optional OxCaml extension candidates

These are outside the initial five-task upstream OCaml pilot and require a
separate compiler image and score:

- Locality-preserving hot-path refactor: reduce allocation while retaining a
  supplied non-escaping interface and identical behavior.
- Safe-sharing repair: satisfy a supplied portability/contention contract without
  unsafe casts, with valid-client and invalid-client compiler checks.

For each implemented candidate, record the task id, source and license, base
revision, compiler/dependency pins, requirement coverage, base/reference validation
logs, repeated calibration results, infrastructure incidents, and admission
decision. A recent PR date alone is not proof of decontamination; record the
available evidence and limitations without claiming unknown training cutoffs.
