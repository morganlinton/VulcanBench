//! Regression: the batched megakernel must not lose recall when files in one
//! subgroup have different lengths (divergent per-file scan loops).
//!
//! Root cause (fixed): the default batch hit writer used to be the hierarchical
//! subgroup aggregator, which REQUIRES uniform control flow (see the
//! `hierarchical_atomics` module contract). The batch kernel scans each file
//! `file_start..file_end`, so subgroup lanes exit the loop at different
//! iterations; once a subgroup's elected leader lane finished a SHORTER file its
//! reserved ring slot was never broadcast, and matches found afterward by
//! still-running lanes were silently dropped. The scalar default is correct
//! under any divergence. This is the vyre-level twin of keyhog's
//! `megakernel_cpu_parity` gate (which caught 6 of 46 detector firings dropped,
//! every miss a match found past its subgroup leader's shorter file).
//!
//! Run: cargo test -p vyre-driver-wgpu --features megakernel-batch \
//!        --test megakernel_divergent_recall -- --ignored --nocapture

#![cfg(feature = "megakernel-batch")]

use std::collections::BTreeSet;
use std::time::Duration;

use vyre_driver_wgpu::megakernel::{
    BatchDispatchConfig, BatchDispatcher, BatchFile, BatchHitWriter, FileBatch, HitRecord,
};
use vyre_driver_wgpu::WgpuBackend;
use vyre_runtime::megakernel::BatchRuleProgram;

/// Sentinel byte that appears ONLY in long files, at an offset past every short
/// file's length — so its match is reachable only after the short lanes in a
/// subgroup have already exited their loop.
const MARKER: u8 = b'@';

/// A 2-state unanchored DFA that accepts wherever `MARKER` occurs:
///   state 0 --MARKER--> 1 (accept); any other byte --> 0
///   state 1 --MARKER--> 1;          any other byte --> 0
/// Dense `state * 256 + byte` table, `accept[state] != 0` == hit.
fn marker_finder_rule(rule_idx: u32) -> BatchRuleProgram {
    let mut transitions = vec![0u32; 2 * 256];
    transitions[MARKER as usize] = 1; // state 0, byte MARKER -> 1
    transitions[256 + MARKER as usize] = 1; // state 1, byte MARKER -> 1
    let accept = vec![0u32, 1u32]; // only state 1 accepts
    BatchRuleProgram::new(rule_idx, transitions, accept, 2).expect("2-state marker DFA is valid")
}

#[test]
#[ignore = "live GPU; run with --ignored --nocapture"]
fn default_dispatcher_keeps_recall_under_file_length_divergence() {
    let backend = WgpuBackend::new()
        .expect("Fix: live GPU required (missing GPU is a configuration bug, not a fallback)");

    // Interleave many SHORT files (no marker, ~24 scan iterations) with LONG
    // files whose ONLY marker sits at offset 300 (reached only after ~300
    // iterations). Within any subgroup mixing the two, the short lanes exit long
    // before the long lanes find their marker — the exact divergence that the old
    // hierarchical hit writer mishandled.
    const FILES: usize = 1024;
    const SHORT_LEN: usize = 24;
    const LONG_LEN: usize = 320;
    const MARK_OFFSET: usize = 300;

    let mut files = Vec::with_capacity(FILES);
    let mut expected_long = 0usize;
    for i in 0..FILES {
        if i % 2 == 0 {
            files.push(BatchFile::new(i as u64, 0, vec![b'x'; SHORT_LEN]));
        } else {
            let mut buf = vec![b'x'; LONG_LEN];
            buf[MARK_OFFSET] = MARKER;
            files.push(BatchFile::new(i as u64, 0, buf));
            expected_long += 1;
        }
    }

    let rules = vec![marker_finder_rule(0)];
    let hit_capacity: u32 = 1 << 16;
    let batch = FileBatch::upload(
        backend.device_queue(),
        &files,
        rules.len() as u32,
        hit_capacity,
    )
    .expect("FileBatch upload");
    let config = BatchDispatchConfig {
        workgroup_size_x: 64,
        worker_groups: 256,
        hit_capacity,
        timeout: Duration::from_secs(30),
        ..Default::default()
    };
    let mut dispatcher = BatchDispatcher::new(backend.clone(), config).expect("default dispatcher");
    let mut hits: Vec<HitRecord> = Vec::new();
    let report = dispatcher
        .dispatch_into(&batch, &rules, &mut hits)
        .expect("dispatch");

    let fired: BTreeSet<u32> = hits.iter().map(|h| h.file_idx).collect();
    let missing: Vec<usize> = (1..FILES)
        .step_by(2)
        .filter(|i| !fired.contains(&(*i as u32)))
        .collect();
    let spurious: Vec<u32> = fired.iter().copied().filter(|f| f % 2 == 0).collect();

    eprintln!(
        "divergent recall: {FILES} files ({expected_long} long), {} raw hits, items_processed={}",
        hits.len(),
        report.items_processed
    );

    assert!(
        missing.is_empty(),
        "RECALL LOSS: {} of {expected_long} long files dropped their marker hit: {:?}",
        missing.len(),
        &missing[..missing.len().min(12)]
    );
    assert!(
        spurious.is_empty(),
        "false positives in short (markerless) files: {:?}",
        &spurious[..spurious.len().min(12)]
    );
    eprintln!("OK: all {expected_long} long files fired, 0 short-file false positives");
}

/// The hierarchical-subgroup writer is unsound for the divergent batch kernel,
/// so an EXPLICIT request must fail LOUDLY rather than silently lose recall
/// (Law: fail closed; no silent fallback). `Auto` downgrades to scalar instead;
/// that path is covered by the recall test above using the default constructor.
#[test]
#[ignore = "live GPU; run with --ignored --nocapture"]
fn explicit_hierarchical_writer_is_rejected_for_batch_kernel() {
    let backend = WgpuBackend::new().expect("Fix: live GPU required");
    let config = BatchDispatchConfig {
        workgroup_size_x: 64,
        worker_groups: 64,
        hit_capacity: 4096,
        timeout: Duration::from_secs(10),
        ..Default::default()
    };
    let err = BatchDispatcher::new_with_hit_writer(
        backend.clone(),
        config,
        BatchHitWriter::HierarchicalSubgroup,
    )
    .expect_err("explicit hierarchical writer must be rejected for the divergent batch kernel");
    let msg = err.to_string();
    assert!(
        msg.contains("unsound") && msg.to_ascii_lowercase().contains("uniform control flow"),
        "rejection must explain the divergence; got: {msg}"
    );
}
