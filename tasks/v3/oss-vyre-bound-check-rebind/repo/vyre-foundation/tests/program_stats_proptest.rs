//! ProgramStats cache invariants  -  50 random programs verify every field.
//! Implementation lives in two `include!`-d chunks under `__split/`.
#![allow(dead_code)]
include!("__split/program_stats_proptest_chunk1.rs");
include!("__split/program_stats_proptest_chunk2.rs");
