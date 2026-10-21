# vyre-debug

`vyre-debug` is the static diagnostic and inspection toolkit for the Vyre 0.4.2 megakernel ecosystem.

## Overview

This crate provides tools to validate, diff, and inspect Vyre's intermediate representation (IR) at the descriptor level, ensuring that passes like `vyre-lower` and `vyre-emit-naga` behave correctly. It is heavily utilized by `vyre-bench` and internal fuzzing infrastructure to catch regressions in descriptor equivalence and detect invalid control-flow rewrites.

## Features

- **Dangling Reference Detection:** Inspects nested `KernelBody` structures to catch references to SSA IDs that are out of scope, accounting for inherited results and completed child body variables.
- **Descriptor Diffing:** Compares two kernel descriptors (e.g., before and after an optimization pass) and generates a structured summary of changes (dropped bindings, added bindings, operation counts).
- **WGSL Emission Inspection:** Wraps `vyre-emit-naga` to provide validated, line-numbered WGSL dumps for any `Program` or `KernelDescriptor`.
- **Carrier Tracing:** Identifies loops without valid loop carriers and summarizes reads/writes for debugging `loop_carry` and `bank_conflict` rewrites.
- **Scan Explain Reports:** Summarizes scan decomposition metadata from descriptor intents and scan database headers, including extracted factors, selected engines, rejected accelerator candidates, verifier fragments, exactness class, verifier cost estimate, literal selectivity basis, streaming state, table bytes, baseline id, and release evidence links.

## Command Line Interface

The crate provides the `vyre-dbg` executable for interactive debugging. It operates on a registry of hardcoded test fixtures.

```bash
# Dump the generated WGSL for the 'loop_carry_smoke' fixture
cargo run --bin vyre-dbg -- dump-wgsl --prog loop_carry_smoke --lines

# Check for dangling SSA references
cargo run --bin vyre-dbg -- find-dangling --prog c11_extract_calls --num-tokens 8

# Compare descriptors before and after lowering passes
cargo run --bin vyre-dbg -- diff-lower --prog c11_extract_calls --num-tokens 4
```

## Architecture

As part of the Santh monorepo architecture, `vyre-debug` adheres to strict tier separation. It provides both a reusable library for agent-driven fuzzing and a standalone tool (`vyre-dbg`) for operational config. 

All diagnostics are structured and support JSON emission (`--json`) for automated regressions.

## License

Part of the Santh Security ecosystem.
