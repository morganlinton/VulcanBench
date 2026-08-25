//! P0 inventory #16–#23 (Phase 1 wave 1.2)  -  pipeline cache observability and
//! bounded growth via the public `WgpuBackend` API.
//!
//! In-memory eviction semantics with a microscopic cap are exercised in
//! `vyre-driver-wgpu` crate tests (`pipeline_cache_eviction_respects_entry_cap`).
//! Disk poisoning contracts live next to `pipeline_disk_cache` (wave 1.2
//! `disk_cache_adversarial_*` tests). Byte-budget enforcement for GPU buffer
//! tiers remains covered by the tiered-cache / buffer-pool unit tests (P0 #17
//! implementation still pending for pipeline artifacts specifically).
#![allow(missing_docs)]

mod common;
use common::acquire_live_backend as live_backend;

use vyre::ir::{BufferDecl, DataType, Expr, Node, Program};
use vyre::{DispatchConfig, VyreBackend};
use vyre_driver::pipeline::DEFAULT_PIPELINE_CACHE_ENTRIES;

fn tiny_unique_program(salt: u32) -> Program {
    Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32)
            .with_count(8)
            .with_output_byte_range(0..32)],
        [1, 1, 1],
        vec![
            Node::store("out", Expr::u32(0), Expr::u32(salt)),
            Node::return_(),
        ],
    )
}

#[test]
fn pipeline_cache_stats_report_capacity_and_bounded_entries() {
    let backend = live_backend();
    let program = tiny_unique_program(1);

    let _ = backend
        .dispatch(&program, &[], &DispatchConfig::default())
        .expect("Fix: warm dispatch must succeed");
    let _ = backend
        .dispatch(&program, &[], &DispatchConfig::default())
        .expect("Fix: hot dispatch must succeed");

    let stats = backend.stats();
    assert_eq!(
        stats.pipeline_cache_capacity, DEFAULT_PIPELINE_CACHE_ENTRIES,
        "Fix: stats must surface the shared driver pipeline-cache soft cap"
    );
    assert!(
        stats.pipeline_cache_entries <= stats.pipeline_cache_capacity,
        "Fix: live pipeline cache must never exceed its declared capacity (entries={}, cap={})",
        stats.pipeline_cache_entries,
        stats.pipeline_cache_capacity
    );
    assert!(
        stats.pipeline_cache_entries >= 1,
        "Fix: after compiling a program the cache should retain at least one artifact key"
    );
    assert!(
        stats.pipeline_cache_hits >= 1,
        "Fix: second dispatch of the same program should record at least one pipeline-cache hit"
    );
    assert!(
        stats.pipeline_cache_misses >= 1,
        "Fix: first dispatch should record the cold pipeline-cache miss before compilation"
    );
    assert!(
        stats.pipeline_cache_hit_rate > 0.0 && stats.pipeline_cache_hit_rate <= 1.0,
        "Fix: cache hit-rate telemetry must be a bounded ratio after mixed hit/miss traffic (got {})",
        stats.pipeline_cache_hit_rate
    );
}

#[test]
fn pipeline_cache_many_unique_programs_stay_within_observed_cap() {
    let backend = live_backend();
    let config = DispatchConfig::default();

    for salt in 0..48u32 {
        let program = tiny_unique_program(10_000 + salt);
        let _ = backend
            .dispatch(&program, &[], &config)
            .expect("Fix: each distinct tiny program must dispatch successfully");
    }

    let stats = backend.stats();
    assert!(
        stats.pipeline_cache_entries <= stats.pipeline_cache_capacity,
        "Fix: flood of unique programs must not grow the pipeline cache beyond cap (entries={}, cap={})",
        stats.pipeline_cache_entries,
        stats.pipeline_cache_capacity
    );
    assert_eq!(
        stats.pipeline_cache_capacity,
        DEFAULT_PIPELINE_CACHE_ENTRIES
    );

    let verify = tiny_unique_program(10_000 + 47);
    let _ = backend
        .dispatch(&verify, &[], &config)
        .expect("Fix: cache eviction must not break subsequent dispatches");
}
