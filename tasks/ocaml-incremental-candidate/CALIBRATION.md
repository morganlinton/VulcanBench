# Incremental candidate calibration

GPT-6.1 Sol medium passed 3/3 complete attempts on the frozen full-repository
Incremental candidate, identity 6b37b41277dd. Each attempt passed all seven
behavior groups and both regression guards. This candidate did not establish
the requested difficulty level.

| Attempt | Complete pass | Behavioral groups | Guards | Recorded run duration |
| --- | --- | --- | --- | --- |
| 1 | Yes | 7/7 | 2/2 | 347.317 seconds |
| 2 | Yes | 7/7 | 2/2 | 296.405 seconds |
| 3 | Yes | 7/7 | 2/2 | 298.655 seconds |

All three were fresh, serial attempts with pinned Codex 0.159.0, subscription
billing, native local execution, no agent container, no judges, medium effort,
and unchanged 10800-second / 540-configured-step allowances. No invocation
error, automatic retry, exclusion or unscored outcome occurred. Configured steps
are not an attestation of CLI turn enforcement. Model identity is requested-only.
Receipts and JSON/HTML cards are in
../../runs-ocaml-incremental-20261002T135901Z. The dispatcher and its scoped
caffeinate process have exited successfully.

The offline gate passed three clean base/reference pairs, all guarded interfaces
and fixtures, all four compiling faulty controls, and standard validation.
Faults produced distinct failures: counting heap pops violates five groups;
requiring an extra slice violates three; restarting a paused cycle violates
publication; accepting a recursive step violates reentrancy. Earlier fixture
and control compiler failures remain preserved and are not admission evidence.
Full harness CI passed 1094 tests with five unrelated analyzer skips, four
deselections, one dependency warning and 86.38% coverage. The supported portable
preflight compiles the library and upstream test library and runs two public
clients. It does not execute all upstream inline/debug tests.

All three patches use state-local driver guards, queue direct parents only
during bounded calls, preserve ordinary optimizations, and add their own public
tests. Source review found no new unsafe cast, hidden registry, counter forgery,
guarded fixture edit or generated build artifact. The recorded integrity audit
found no benchmark or answer-key read, web fetch, or contamination flag. Native
telemetry observed shell access and, on the third attempt, a temporary-directory
path. This is observational evidence, not native filesystem isolation.
RECEIPT_AUDIT.json preserves every patch inventory and digest. There were no
failing model outcomes to replay; FAILURE_REPLAY.json records that limitation.

Preserved earlier results are 19/21 across seven original native library tasks
and 7/9 across the three larger candidates. Adding this task gives contextual
29/33 complete passes across eleven library tasks (87.9%). The four larger
candidates together are 10/12 (83.3%). These are contextual arithmetic across
development cohorts, not a frozen expanded-suite card or independent
confirmation. EXPANSION_CONTEXT.json preserves the counts. No old task or
outcome was selectively omitted or changed.

The full twelve-task headline remains withheld because
compiler-gadt-field-safety has no current fresh native outcomes and its earlier
provider refusal remains unresolved. No compiler retry or expired overnight
automation was started. Public-source training exposure remains disclosed;
this is an original extension to an OSS checkout, not a decontamination claim.
Functional cards do not claim OCaml quality or security scores. HTML rendering
remains unverified under the existing app preview limitation.

Suggested next step: design atomic dependency rewiring in the full Incremental
repository. The original small atomic-graph task scored 1/3, so that provides
evidence for a stronger engineering direction. Specify its semantics openly,
validate references and controls, then measure fresh attempts at the same
settings before claiming an under-80% expanded-suite result.
