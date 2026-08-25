//! Megakernel core contract tests  -  assert that the megakernel scheduler
//! and dispatch pipeline preserve key invariants across edge cases.
//!
//! Implementation lives in two `include!`-d chunks under `__split/`.

include!("__split/megakernel_core_contracts_chunk1.rs");
include!("__split/megakernel_core_contracts_chunk2.rs");
