# vyrec parity manifest v1

The parity manifest is the executable evidence format for the release target. It must be produced by tools, consumed by comparators, archived by CI, and readable by contributors.

## Purpose

The manifest records clang-oracle facts and vyrec facts for a frozen source set. Passing tests are not enough for release: the manifest must prove that every required fact category for the target has either matched clang, produced an explained mismatch, or failed the release gate.

## Required identity fields

- `schema`: manifest schema version.
- `target_id`: frozen target identifier, such as `linux-lib-math-v6.8`.
- `upstream`: source repository URL.
- `commit`: exact source commit.
- `target_triple`: compilation target.
- `language`: C language mode.
- `clang_version`: clang oracle version.
- `vyrec_version`: vyrec build version or commit.
- `gpu`: GPU name and driver used for vyrec execution.
- `mode`: staged, resident-graph, or megakernel.

## Required source fields

- `sources`: every translation unit in scope.
- `metadata_files`: build metadata that shaped the target.
- `direct_headers`: direct includes discovered from source files.
- `transitive_headers`: full include closure discovered by the harness.
- `compile_commands`: exact command lines or normalized compiler arguments.

## Required fact categories

- `preprocessor`: include graph, macro definitions, macro expansions, conditional state, provenance, diagnostics.
- `lexer`: token kind, spelling, span, line table, literal facts, macro expansion frame.
- `parser`: declarations, declarators, statements, expressions, initializers, attributes, GNU extensions.
- `semantic_analysis`: scopes, symbols, references, redeclarations, types, conversions, lvalue/rvalue rules, constants, diagnostics.
- `abi_layout`: size, alignment, offsets, bitfields, enum representation, function ABI facts.
- `object_evidence`: section inventory, row counts, schema IDs, checksums, decoder validation status.
- `performance`: clang timing, vyrec timing, launches, transfers, allocation, occupancy evidence, resident reuse, megakernel queue metrics.

## Comparator contract

The comparator must classify every difference as one of:

- `match`: clang and vyrec facts agree.
- `explained_target_difference`: accepted only with a release-approved reason.
- `vyrec_missing`: vyrec failed to produce a required fact.
- `vyrec_extra`: vyrec produced an unsupported or unjustified extra fact.
- `span_mismatch`: fact agrees semantically but location/provenance differs.
- `semantic_mismatch`: fact disagrees in meaning.
- `diagnostic_mismatch`: severity, category, recovery, or span differs.
- `performance_failure`: vyrec failed the release performance contract.
- `gpu_residency_failure`: host fallback, excessive readback, or unexpected synchronization occurred.

## Release rule

The official launch cannot pass with `vyrec_missing`, `semantic_mismatch`, `diagnostic_mismatch`, `performance_failure`, or `gpu_residency_failure` in the frozen Linux subsystem target. Any `explained_target_difference` must be explicitly approved and archived with the release artifacts.
