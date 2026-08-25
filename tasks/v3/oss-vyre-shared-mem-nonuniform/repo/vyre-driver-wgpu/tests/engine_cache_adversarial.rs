//! Adversarial tests for `vyre-libs::matching::engine`.
//!
//! Targets:
//!   - `MatchScan` trait object safety and dispatch
//!   - `MatchEngineCache` round-trip resilience
//!   - `cached_load_or_compile` corruption recovery, concurrency, and
//!     filesystem edge cases.
//!
//! Run:
//! `cargo test -p vyre-libs --features matching-regex --test engine_cache_adversarial`

#![cfg(feature = "matching-nfa")]
#![allow(deprecated)]
use std::sync::{Arc, Barrier};
use std::thread;

use vyre_foundation::match_result::Match;
use vyre_libs::scan::{cached_load_or_compile, engine_cache_path, GpuLiteralSet, MatchScan};

#[cfg(feature = "matching-nfa")]
use vyre_libs::scan::build_rule_pipeline;

// ---------------------------------------------------------------------------
// 1. Cache file corruption recovery (7 tests)
// ---------------------------------------------------------------------------

mod engine_cache_adversarial_part1 {

    include!("__split/engine_cache_adversarial_part1.rs");
}
mod engine_cache_adversarial_part2 {
    include!("__split/engine_cache_adversarial_part2.rs");
}
