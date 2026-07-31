//! Head-to-head: combined-AC vs per-rule segmented megakernel on ONE catalog.
//!
//! Both paths scan the SAME 8 MiB buffer for the SAME 64-byte catalog and must
//! conserve the SAME match set (differential vs the `classic_ac_scan` oracle).
//! The point is the WORK they do to get there:
//!
//!   * per-rule runs one DFA per (segment, rule) ⇒ `queue_len =
//!     segment_count * 64`: every byte is re-read 64 times.
//!   * combined runs ONE automaton per segment ⇒ `queue_len = segment_count`:
//!     one transition read per byte regardless of catalog size.
//!
//! So on a many-pattern catalog the combined path must be materially faster.
//! This test pins that: both conserve exactly, and combined's best conserving
//! throughput strictly beats per-rule's (the `rule_count` multiplier is real
//! and the combined path removes it).
//!
//! Run: cargo test -p vyre-driver-wgpu --features megakernel-batch,wgpu \
//!        --test megakernel_combined_vs_perrule -- --ignored --nocapture
#![cfg(feature = "megakernel-batch")]

use std::collections::BTreeSet;
use std::time::Duration;

use vyre_driver_wgpu::megakernel::{
    BatchDispatchConfig, BatchDispatcher, BatchFile, CombinedBatch, CombinedDispatcher, FileBatch,
    HitRecord,
};
use vyre_driver_wgpu::WgpuBackend;
use vyre_libs::scan::classic_ac::{classic_ac_compile, classic_ac_scan};
use vyre_runtime::megakernel::BatchRuleProgram;

const FILE_LEN: usize = 8 * 1024 * 1024; // 8 MiB
const N_PATTERNS: u32 = 64;
const PLANTS_PER_BYTE: usize = 32;
const FILLER: u8 = 0x00;
/// Catalog bytes 0x80..0xC0 — disjoint from the 0x00 filler, so the ONLY
/// matches are the deliberately planted ones.
fn catalog_bytes() -> Vec<u8> {
    (0..N_PATTERNS).map(|i| 0x80u8 + i as u8).collect()
}

/// 2-state unanchored DFA accepting at every `byte` occurrence.
fn byte_finder_rule(rule_idx: u32, byte: u8) -> BatchRuleProgram {
    let mut t = vec![0u32; 2 * 256];
    t[byte as usize] = 1;
    t[256 + byte as usize] = 1;
    BatchRuleProgram::new(rule_idx, t, vec![0u32, 1u32], 2).expect("valid 2-state DFA")
}

/// One distinct planted offset per (byte, j): a uniform spread across the file,
/// so the oracle has exactly `N_PATTERNS * PLANTS_PER_BYTE` matches and both
/// engines must find all of them.
fn build_haystack(bytes: &[u8]) -> Vec<u8> {
    let mut buf = vec![FILLER; FILE_LEN];
    let total = N_PATTERNS as usize * PLANTS_PER_BYTE;
    let stride = FILE_LEN / total;
    for (i, &byte) in bytes.iter().enumerate() {
        for j in 0..PLANTS_PER_BYTE {
            let slot = i * PLANTS_PER_BYTE + j;
            let offset = slot * stride;
            if offset < FILE_LEN {
                buf[offset] = byte;
            }
        }
    }
    buf
}

fn best_conserving_gbps(
    label: &str,
    oracle: &BTreeSet<(u32, u32)>,
    mut run: impl FnMut(u32) -> Option<(BTreeSet<(u32, u32)>, u32, f64)>,
    seg_lens: &[u32],
) -> f64 {
    eprintln!("  {label}:");
    eprintln!(
        "    {:>9}  {:>8}  {:>8}  {:>8}  {:>9}",
        "seg_len", "found", "dropped", "GB/s", "conserves?"
    );
    let mut best = 0.0f64;
    for &seg_len in seg_lens {
        let Some((found, dropped, gbps)) = run(seg_len) else {
            eprintln!(
                "    {:>9}  {:>8}  {:>8}  {:>8}  {:>9}",
                seg_len, "-", "-", "-", "under-claim"
            );
            continue;
        };
        let conserves = found == *oracle && dropped == 0;
        eprintln!(
            "    {:>9}  {:>8}  {:>8}  {:>8.3}  {:>9}",
            seg_len,
            found.len(),
            dropped,
            gbps,
            if conserves { "yes" } else { "DIVERGES" }
        );
        assert!(
            found.len() == oracle.len() || found.is_empty(),
            "{label} seg_len={seg_len}: found {} matches, oracle has {} (not conserving, dropped={dropped})",
            found.len(),
            oracle.len()
        );
        if conserves {
            best = best.max(gbps);
        }
    }
    best
}

#[test]
#[ignore = "live GPU; run with --ignored --nocapture"]
fn combined_beats_per_rule_on_a_many_pattern_catalog() {
    let backend = WgpuBackend::new()
        .expect("Fix: live GPU required (missing GPU is a configuration bug, not a fallback)");

    let bytes = catalog_bytes();
    let buf = build_haystack(&bytes);
    let hit_capacity: u32 = 1 << 20;

    // Oracle: the combined automaton's linear scan over the whole buffer.
    let patterns: Vec<&[u8]> = bytes.iter().map(std::slice::from_ref).collect();
    let ac = classic_ac_compile(&patterns);
    let oracle: BTreeSet<(u32, u32)> = classic_ac_scan(&ac, &buf).into_iter().collect();
    assert_eq!(
        oracle.len(),
        N_PATTERNS as usize * PLANTS_PER_BYTE,
        "fixture must plant exactly one match per (byte, slot)"
    );

    let seg_lens = [1024u32, 512, 256];

    eprintln!(
        "8 MiB / {N_PATTERNS} single-byte patterns — combined-AC vs per-rule ({} oracle matches):",
        oracle.len()
    );

    // ── per-rule path ──────────────────────────────────────────────────────
    let rules: Vec<BatchRuleProgram> = bytes
        .iter()
        .enumerate()
        .map(|(i, &b)| byte_finder_rule(i as u32, b))
        .collect();
    let per_rule_best = best_conserving_gbps(
        "per-rule (queue = segment_count * 64)",
        &oracle,
        |seg_len| {
            let config = BatchDispatchConfig {
                workgroup_size_x: 64,
                worker_groups: 2048,
                hit_capacity,
                timeout: Duration::from_secs(180),
                ..Default::default()
            };
            let mut dispatcher = BatchDispatcher::new(backend.clone(), config).expect("dispatcher");
            let files = vec![BatchFile::new(0xC0FFEE, 0, buf.clone())];
            let mut batch =
                FileBatch::upload(backend.device_queue(), &files, N_PATTERNS, hit_capacity)
                    .expect("upload");
            batch.set_segmentation(seg_len, 8).expect("set_segmentation");
            let mut hits: Vec<HitRecord> = Vec::new();
            let _ = dispatcher.dispatch_into(&batch, &rules, &mut hits);
            let mut best = Duration::from_secs(3600);
            let mut found = BTreeSet::new();
            let mut dropped = 0u32;
            for _ in 0..3 {
                hits.clear();
                match dispatcher.dispatch_into(&batch, &rules, &mut hits) {
                    Ok(s) => {
                        best = best.min(s.wall_time);
                        found = hits.iter().map(|h| (h.rule_idx, h.match_offset)).collect();
                        dropped = s.dropped_hits;
                    }
                    Err(e) if e.is_drain_incomplete() => return None,
                    Err(e) => panic!("per-rule dispatch error at seg_len={seg_len}: {e}"),
                }
            }
            Some((found, dropped, FILE_LEN as f64 / best.as_secs_f64() / 1e9))
        },
        &seg_lens,
    );

    // ── combined path ──────────────────────────────────────────────────────
    let transitions = &ac.dfa.transitions;
    let output_offsets = &ac.dfa.output_offsets;
    let output_records = &ac.dfa.output_records;
    let state_count = ac.dfa.state_count;
    let max_pattern_len = 1u32;
    let combined_best = best_conserving_gbps(
        "combined  (queue = segment_count)",
        &oracle,
        |seg_len| {
            let config = BatchDispatchConfig {
                workgroup_size_x: 64,
                worker_groups: 2048,
                hit_capacity,
                timeout: Duration::from_secs(180),
                ..Default::default()
            };
            let files = vec![BatchFile::new(0xC0FFEE, 0, buf.clone())];
            let batch = CombinedBatch::upload(
                backend.device_queue(),
                &files,
                transitions,
                output_offsets,
                output_records,
                state_count,
                max_pattern_len,
                seg_len,
                hit_capacity,
            )
            .expect("combined upload");
            let mut dispatcher = CombinedDispatcher::new(backend.clone(), config);
            let mut hits: Vec<HitRecord> = Vec::new();
            let _ = dispatcher.dispatch_into(&batch, &mut hits);
            let mut best = Duration::from_secs(3600);
            let mut found = BTreeSet::new();
            let mut dropped = 0u32;
            for _ in 0..3 {
                hits.clear();
                match dispatcher.dispatch_into(&batch, &mut hits) {
                    Ok(s) => {
                        best = best.min(s.wall_time);
                        found = hits.iter().map(|h| (h.rule_idx, h.match_offset)).collect();
                        dropped = s.dropped_hits;
                    }
                    Err(e) if e.is_drain_incomplete() => return None,
                    Err(e) => panic!("combined dispatch error at seg_len={seg_len}: {e}"),
                }
            }
            Some((found, dropped, FILE_LEN as f64 / best.as_secs_f64() / 1e9))
        },
        &seg_lens,
    );

    eprintln!(
        "per-rule best {per_rule_best:.3} GB/s; combined best {combined_best:.3} GB/s ⇒ combined is {:.2}× the per-rule path",
        if per_rule_best > 0.0 {
            combined_best / per_rule_best
        } else {
            f64::INFINITY
        }
    );

    assert!(
        per_rule_best > 0.0,
        "per-rule path produced no conserving geometry to compare against"
    );
    assert!(
        combined_best > 0.0,
        "combined path produced no conserving geometry"
    );
    assert!(
        combined_best > per_rule_best,
        "combined ({combined_best:.3} GB/s) must beat per-rule ({per_rule_best:.3} GB/s) on a \
         {N_PATTERNS}-pattern catalog — the rule_count multiplier should make per-rule slower"
    );
}
