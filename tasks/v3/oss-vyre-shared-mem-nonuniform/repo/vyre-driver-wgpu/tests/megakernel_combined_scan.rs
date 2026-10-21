//! Combined-AC segmented megakernel: differential conservation against the
//! `classic_ac_scan` CPU oracle, plus 8 MiB throughput.
//!
//! The combined path compiles a whole catalog into ONE Aho-Corasick automaton
//! and runs it ONCE per segment (`queue_len = segment_count`), emitting the SET
//! of pattern ids accepting at each state. This test pins the two contracts:
//!
//!   1. CONSERVATION (differential vs oracle, Law 10 — no silent drop). For
//!      EVERY segmentation geometry the GPU's `(pattern_id, end_offset)` set
//!      must EXACTLY equal `classic_ac_scan` over the whole buffer — no miss,
//!      no duplicate, no fabricated hit, `dropped_hits == 0`. `classic_ac_scan`
//!      is the same linear oracle the `segmentation.rs` `combined_segmented_scan`
//!      proptest is proven equal to, so this closes the loop on the REAL GPU.
//!   2. THROUGHPUT. The fastest conserving geometry must beat Hyperscan's
//!      ~1.5 GB/s warm device floor on a single 8 MiB input — and it does so
//!      with the per-rule `rule_count` queue multiplier GONE (one transition
//!      read per byte regardless of catalog size).
//!
//! Run: cargo test -p vyre-driver-wgpu --features megakernel-batch,wgpu \
//!        --test megakernel_combined_scan -- --ignored --nocapture
#![cfg(feature = "megakernel-batch")]

use std::collections::BTreeSet;
use std::time::Duration;

use vyre_driver_wgpu::megakernel::{
    BatchDispatchConfig, BatchFile, CombinedBatch, CombinedDispatcher, HitRecord, TransitionWidth,
    DEFAULT_SEG_LEN_CANDIDATES,
};
use vyre_driver_wgpu::WgpuBackend;
use vyre_libs::scan::classic_ac::{classic_ac_compile, classic_ac_scan};

const FILE_LEN: usize = 8 * 1024 * 1024; // 8 MiB
const HS_FLOOR_GBPS: f64 = 1.5;

/// A varied-length literal catalog: short and long patterns so the warm-up
/// `overlap = max_pattern_len` is genuinely exercised at window boundaries.
fn catalog() -> Vec<&'static [u8]> {
    vec![
        b"alpha", b"bravo", b"charlie", b"delta", b"echo", b"foxtrot", b"golf",
        b"hotel", b"india", b"juliet", b"kilo", b"lima", b"mike", b"november",
        b"oscar", b"papa", b"quebec", b"romeo", b"sierra", b"tango", b"uniform",
        b"victor", b"whiskey", b"xray", b"yankee", b"zulu", b"AB", b"ABCDEFGH",
        b"needle", b"haystack", b"0123456789", b"the-quick-brown-fox",
    ]
}

/// Plant catalog patterns at a fixed spread of offsets — including offsets that
/// straddle the 512 / 1024 / 4096 segment boundaries the geometry sweep uses —
/// so warm-up + emit-guard ownership is tested at the seams. Offsets are
/// geometry-invariant so the oracle ground truth is the same for every seg_len.
fn build_haystack(patterns: &[&[u8]]) -> Vec<u8> {
    let mut buf = vec![b'.'; FILE_LEN];
    let boundary_seeds = [500usize, 1020, 4090, 65530];
    for (pattern_index, pattern) in patterns.iter().enumerate() {
        // A uniform spread across the file...
        let mut offset = 4096 + pattern_index * 211;
        while offset + pattern.len() <= FILE_LEN {
            buf[offset..offset + pattern.len()].copy_from_slice(pattern);
            offset += 131_072; // 128 KiB stride
        }
        // ...plus deliberate boundary-straddling placements.
        for &seed in &boundary_seeds {
            let at = seed + pattern_index;
            if at + pattern.len() <= FILE_LEN {
                buf[at..at + pattern.len()].copy_from_slice(pattern);
            }
        }
    }
    buf
}

struct GeomResult {
    seg_len: u32,
    found: BTreeSet<(u32, u32)>,
    dropped: u32,
    gbps: f64,
    under_claimed: bool,
}

#[allow(clippy::too_many_arguments)]
fn run_geometry(
    backend: &WgpuBackend,
    buf: &[u8],
    transitions: &[u32],
    output_offsets: &[u32],
    output_records: &[u32],
    state_count: u32,
    max_pattern_len: u32,
    seg_len: u32,
    transition_width: TransitionWidth,
) -> GeomResult {
    let hit_capacity: u32 = 1 << 20;
    let config = BatchDispatchConfig {
        workgroup_size_x: 64,
        worker_groups: 2048,
        hit_capacity,
        timeout: Duration::from_secs(120),
        ..Default::default()
    };
    let files = vec![BatchFile::new(0xC0FFEE, 0, buf.to_vec())];
    let mut batch = CombinedBatch::upload_with_transition_width(
        backend.device_queue(),
        &files,
        transitions,
        output_offsets,
        output_records,
        state_count,
        max_pattern_len,
        seg_len,
        hit_capacity,
        transition_width,
    )
    .expect("Fix: CombinedBatch upload must succeed for a valid automaton");
    let mut dispatcher = CombinedDispatcher::new(backend.clone(), config);

    // Warm once (compile out of the timing). A loud under-claim is acceptable.
    let mut hits: Vec<HitRecord> = Vec::new();
    let _ = dispatcher.dispatch_into(&batch, &mut hits);

    let mut best = Duration::from_secs(3600);
    let mut found: BTreeSet<(u32, u32)> = BTreeSet::new();
    let mut dropped = 0u32;
    let mut under_claimed = false;
    let _ = &mut batch;
    for _ in 0..3 {
        hits.clear();
        match dispatcher.dispatch_into(&batch, &mut hits) {
            Ok(summary) => {
                best = best.min(summary.wall_time);
                found = hits
                    .iter()
                    .map(|h| (h.rule_idx, h.match_offset))
                    .collect::<BTreeSet<_>>();
                dropped = summary.dropped_hits;
            }
            Err(e) if e.is_drain_incomplete() => {
                under_claimed = true;
                break;
            }
            Err(e) => panic!("unexpected combined dispatch error at seg_len={seg_len}: {e}"),
        }
    }
    GeomResult {
        seg_len,
        found,
        dropped,
        gbps: if best == Duration::from_secs(3600) {
            0.0
        } else {
            FILE_LEN as f64 / best.as_secs_f64() / 1e9
        },
        under_claimed,
    }
}

#[test]
#[ignore = "live GPU; run with --ignored --nocapture"]
fn combined_scan_conserves_every_match_and_beats_hyperscan() {
    let backend = WgpuBackend::new()
        .expect("Fix: live GPU required (missing GPU is a configuration bug, not a fallback)");

    let patterns = catalog();
    let max_pattern_len = patterns
        .iter()
        .map(|p| p.len())
        .max()
        .expect("non-empty catalog") as u32;
    let buf = build_haystack(&patterns);

    // Ground truth: the linear classic-AC oracle over the WHOLE buffer. The GPU
    // must reproduce this set exactly for every geometry.
    let ac = classic_ac_compile(&patterns);
    let oracle: BTreeSet<(u32, u32)> = classic_ac_scan(&ac, &buf).into_iter().collect();
    assert!(
        !oracle.is_empty(),
        "fixture must plant matches; oracle found none"
    );

    let transitions = &ac.dfa.transitions;
    let output_offsets = &ac.dfa.output_offsets;
    let output_records = &ac.dfa.output_records;
    let state_count = ac.dfa.state_count;

    // The combined kernel byte-class compresses this dense table. Confirm the
    // compression is REAL (fewer than 256 classes) for this catalog, so the
    // conservation result below actually exercises the compressed transition
    // path, not a 256-class identity passthrough.
    let mut class_map = Vec::new();
    let num_classes = vyre_runtime::megakernel::rule_catalog::build_byte_class_map_for_table(
        transitions,
        state_count as usize,
        &mut class_map,
    );
    assert!(
        num_classes < 256,
        "expected byte-class compression to collapse the alphabet (<256 classes) for this \
         catalog; got {num_classes} — the compressed-transition path would be untested"
    );
    eprintln!(
        "combined automaton: {state_count} states, {num_classes} byte-classes (dense 256 → compressed {num_classes})",
    );

    // seg_len = u32::MAX is one segment per file (whole-file, no segmentation);
    // the rest tile the 8 MiB file into many windows to saturate the device.
    // The sub-512 tail (256/128/64) probes the saturation/overlap-waste turnover:
    // at seg_len=512 the 8 MiB file is only 16_384 segments — fewer than the
    // device's resident-thread count — so smaller windows raise parallelism until
    // the fixed `overlap = max_pattern_len` (19 B here) per window dominates. The
    // boundary seeds (500/1020/4090/65530) straddle 512/1024/4096, all multiples
    // of 64/128/256, so every sub-512 geometry's seams are still covered.
    let geometries = [
        u32::MAX, 65_536u32, 16_384, 4_096, 1_024, 512, 256, 128, 64,
    ];

    eprintln!(
        "8 MiB / {} patterns — combined-AC conservation + throughput (oracle has {} matches, Hyperscan {HS_FLOOR_GBPS} GB/s):",
        patterns.len(),
        oracle.len()
    );
    eprintln!(
        "  {:>9}  {:>8}  {:>8}  {:>8}  {:>6}  {:>9}",
        "seg_len", "found", "dropped", "GB/s", "vs HS", "conserves?"
    );

    let mut results = Vec::new();
    for seg_len in geometries {
        let r = run_geometry(
            &backend,
            &buf,
            transitions,
            output_offsets,
            output_records,
            state_count,
            max_pattern_len,
            seg_len,
            TransitionWidth::Bits32,
        );
        let conserves = r.found == oracle && r.dropped == 0;
        let status = if conserves {
            "conserves"
        } else if r.under_claimed {
            "LOUD-underclaim"
        } else {
            "DIVERGES"
        };
        eprintln!(
            "  {:>9}  {:>8}  {:>8}  {:>8.3}  {:>5.2}x  {:>15}",
            if r.seg_len == u32::MAX {
                "whole".to_string()
            } else {
                r.seg_len.to_string()
            },
            r.found.len(),
            r.dropped,
            r.gbps,
            r.gbps / HS_FLOOR_GBPS,
            status,
        );
        results.push(r);
    }

    // Contract 1 — exact conservation vs the oracle for every NON-under-claimed
    // geometry. A geometry that returned an Ok dispatch but a different hit set
    // is a silent recall bug (miss, dup, or fabrication).
    for r in &results {
        if r.under_claimed {
            continue;
        }
        assert_eq!(
            r.found.len(),
            oracle.len(),
            "seg_len={} found {} matches, oracle has {} — combined scan is not conserving (dropped={})",
            r.seg_len,
            r.found.len(),
            oracle.len(),
            r.dropped
        );
        assert_eq!(
            r.found, oracle,
            "seg_len={} hit set diverged from the classic_ac_scan oracle (same cardinality, different pairs)",
            r.seg_len
        );
        assert_eq!(
            r.dropped, 0,
            "seg_len={} dropped {} hits — the ring overflowed and the result is incomplete",
            r.seg_len, r.dropped
        );
    }

    // Contract 2 — throughput. Fastest conserving geometry beats Hyperscan.
    let best_ok_gbps = results
        .iter()
        .filter(|r| r.found == oracle && r.dropped == 0)
        .map(|r| r.gbps)
        .fold(0.0f64, f64::max);
    eprintln!(
        "best conserving throughput: {best_ok_gbps:.3} GB/s ⇒ combined-AC {} Hyperscan",
        if best_ok_gbps > HS_FLOOR_GBPS {
            "BEATS"
        } else {
            "does NOT beat"
        }
    );
    assert!(
        best_ok_gbps > HS_FLOOR_GBPS,
        "no conserving combined-AC geometry beat Hyperscan: best {best_ok_gbps:.3} GB/s <= {HS_FLOOR_GBPS} GB/s"
    );
}

/// ~2048 distinct secret-shaped literals approximating keyhog's many-literal
/// catalog (the real workload is ~6000): varied real-detector prefixes + base62
/// bodies of varied length, so the combined Aho-Corasick has thousands of states
/// and a realistically compressed byte alphabet. Deterministic (no RNG) so the
/// oracle ground truth is stable across runs.
fn large_catalog() -> Vec<Vec<u8>> {
    let prefixes: &[&[u8]] = &[
        b"AKIA", b"ghp_", b"gho_", b"xoxb-", b"xoxp-", b"AIza", b"sk-", b"pk_",
        b"rk_", b"glpat-", b"ya29.", b"ASIA", b"SG.", b"shpat_", b"AccountKey=",
        b"eyJ", b"-----BEGIN", b"npm_", b"dop_v1_", b"sk_live_",
    ];
    let body: &[u8] = b"0123456789abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ";
    let mut out: Vec<Vec<u8>> = Vec::with_capacity(2048);
    let mut seen = std::collections::HashSet::new();
    let mut i = 0usize;
    while out.len() < 2048 {
        let prefix = prefixes[i % prefixes.len()];
        let len = 6 + (i % 27); // bodies 6..=32 bytes
        let mut p = prefix.to_vec();
        for k in 0..len {
            // Deterministic, well-spread index into the base62 alphabet.
            let idx = i
                .wrapping_mul(2_654_435_761)
                .wrapping_add(k.wrapping_mul(40_503))
                % body.len();
            p.push(body[idx]);
        }
        if seen.insert(p.clone()) {
            out.push(p);
        }
        i += 1;
    }
    out
}

#[test]
#[ignore = "live GPU; run with --ignored --nocapture"]
fn combined_scan_beats_hyperscan_at_keyhog_catalog_scale() {
    // The KEYSTONE the smaller test does not cover: keyhog's catalog is thousands
    // of literals ("few files × MANY literals"), not the 32 above. The combined-AC
    // geometry collapse is O(input) and INDEPENDENT of rule count, so the 8 MiB
    // win must HOLD as the automaton grows to thousands of states. This proves it
    // on a realistic-scale catalog: still 0 dropped, still beats Hyperscan.
    let backend = WgpuBackend::new()
        .expect("Fix: live GPU required (missing GPU is a configuration bug, not a fallback)");

    let owned = large_catalog();
    let patterns: Vec<&[u8]> = owned.iter().map(|p| p.as_slice()).collect();
    let max_pattern_len = patterns
        .iter()
        .map(|p| p.len())
        .max()
        .expect("non-empty catalog") as u32;
    let buf = build_haystack(&patterns);

    let ac = classic_ac_compile(&patterns);
    let oracle: BTreeSet<(u32, u32)> = classic_ac_scan(&ac, &buf).into_iter().collect();
    assert!(
        oracle.len() > 10_000,
        "a {}-literal catalog planted across 8 MiB must yield a large oracle; got {}",
        patterns.len(),
        oracle.len()
    );

    let transitions = &ac.dfa.transitions;
    let output_offsets = &ac.dfa.output_offsets;
    let output_records = &ac.dfa.output_records;
    let state_count = ac.dfa.state_count;

    let mut class_map = Vec::new();
    let num_classes = vyre_runtime::megakernel::rule_catalog::build_byte_class_map_for_table(
        transitions,
        state_count as usize,
        &mut class_map,
    );
    eprintln!(
        "keyhog-scale combined automaton: {} patterns, {state_count} states, {num_classes} byte-classes",
        patterns.len()
    );
    assert!(
        state_count > 5_000,
        "a {}-literal catalog must build a large automaton (>5000 states) to exercise scale; got {state_count}",
        patterns.len()
    );

    // Segmenting geometries only (no whole-file: at this scale the unsegmented
    // pass is the slow path the segmentation exists to beat). Swept down to very
    // fine windows because a large automaton is memory-bound — more, smaller
    // segments trade warm-up overhead for parallelism, and the sweep finds where
    // that balance peaks for a thousands-of-states catalog.
    let geometries = [16_384u32, 4_096, 1_024, 512, 256, 128];
    eprintln!(
        "8 MiB / {} patterns — combined-AC conservation + throughput (oracle {} matches, Hyperscan {HS_FLOOR_GBPS} GB/s):",
        patterns.len(),
        oracle.len()
    );
    eprintln!(
        "  {:>9}  {:>8}  {:>8}  {:>8}  {:>6}  {:>9}",
        "seg_len", "found", "dropped", "GB/s", "vs HS", "conserves?"
    );

    let mut results = Vec::new();
    for seg_len in geometries {
        let r = run_geometry(
            &backend,
            &buf,
            transitions,
            output_offsets,
            output_records,
            state_count,
            max_pattern_len,
            seg_len,
            TransitionWidth::Bits32,
        );
        let conserves = r.found == oracle && r.dropped == 0;
        let status = if conserves {
            "conserves"
        } else if r.under_claimed {
            "LOUD-underclaim"
        } else {
            "DIVERGES"
        };
        eprintln!(
            "  {:>9}  {:>8}  {:>8}  {:>8.3}  {:>5.2}x  {:>15}",
            r.seg_len, r.found.len(), r.dropped, r.gbps, r.gbps / HS_FLOOR_GBPS, status,
        );
        results.push(r);
    }

    // Conservation: every non-under-claimed geometry must reproduce the oracle
    // EXACTLY at scale — no miss, dup, or fabrication when the automaton is large.
    for r in &results {
        if r.under_claimed {
            continue;
        }
        assert_eq!(
            r.found, oracle,
            "seg_len={} diverged from the oracle at keyhog scale (found {}, oracle {}, dropped {})",
            r.seg_len,
            r.found.len(),
            oracle.len(),
            r.dropped
        );
        assert_eq!(r.dropped, 0, "seg_len={} dropped {} hits", r.seg_len, r.dropped);
    }

    let best_ok_gbps = results
        .iter()
        .filter(|r| r.found == oracle && r.dropped == 0)
        .map(|r| r.gbps)
        .fold(0.0f64, f64::max);
    eprintln!(
        "best conserving throughput at {}-literal scale: {best_ok_gbps:.3} GB/s ⇒ combined-AC {} Hyperscan",
        patterns.len(),
        if best_ok_gbps > HS_FLOOR_GBPS { "BEATS" } else { "does NOT beat" }
    );
    assert!(
        best_ok_gbps > HS_FLOOR_GBPS,
        "the combined-AC win did NOT hold at {}-literal keyhog scale: best {best_ok_gbps:.3} GB/s <= {HS_FLOOR_GBPS} GB/s",
        patterns.len()
    );
}

/// The DECISIVE u16 A/B (the GPU half of the profile-first lane the CPU
/// measurement opened). The CPU ceiling test confirmed u16 halves the
/// transition table at keyhog scale; this proves on the real RTX 5090 that the
/// u16 packing is (1) LOSSLESS — its hit set equals BOTH the u32 path's and the
/// `classic_ac_scan` oracle, with 0 dropped, at every fine geometry — and (2)
/// measures whether halving bytes/transaction actually beats the added unpack
/// ALU. Soundness is asserted; the speedup is reported (its sign is the answer
/// the doc's open question wanted, and it varies run-to-run within thermal
/// noise, so it is logged, not asserted).
#[test]
#[ignore = "live GPU; run with --ignored --nocapture"]
fn u16_transitions_are_lossless_and_measured_vs_u32_at_keyhog_scale() {
    let backend = WgpuBackend::new()
        .expect("Fix: live GPU required (missing GPU is a configuration bug, not a fallback)");

    let owned = large_catalog();
    let patterns: Vec<&[u8]> = owned.iter().map(|p| p.as_slice()).collect();
    let max_pattern_len = patterns.iter().map(|p| p.len()).max().expect("non-empty") as u32;
    let buf = build_haystack(&patterns);

    let ac = classic_ac_compile(&patterns);
    let oracle: BTreeSet<(u32, u32)> = classic_ac_scan(&ac, &buf).into_iter().collect();
    let transitions = &ac.dfa.transitions;
    let output_offsets = &ac.dfa.output_offsets;
    let output_records = &ac.dfa.output_records;
    let state_count = ac.dfa.state_count;
    assert!(
        state_count <= u16::MAX as u32 + 1,
        "u16 packing requires state_count <= 65536; got {state_count} — this catalog must stay on u32",
    );

    // Fine windows only: the keyhog-scale win lives at seg_len <= 512 (coarse
    // loses), and that is exactly where the transition table is hammered hardest,
    // so it is where u16 has the most to give.
    let geometries = [512u32, 256, 128];
    eprintln!(
        "8 MiB / {} patterns / {state_count} states — u16 vs u32 transition A/B (oracle {} matches):",
        patterns.len(),
        oracle.len()
    );
    eprintln!(
        "  {:>9}  {:>10}  {:>10}  {:>8}  {:>8}  {:>8}",
        "seg_len", "u32 GB/s", "u16 GB/s", "u16/u32", "u32 ok?", "u16 ok?"
    );

    for seg_len in geometries {
        let u32_r = run_geometry(
            &backend,
            &buf,
            transitions,
            output_offsets,
            output_records,
            state_count,
            max_pattern_len,
            seg_len,
            TransitionWidth::Bits32,
        );
        let u16_r = run_geometry(
            &backend,
            &buf,
            transitions,
            output_offsets,
            output_records,
            state_count,
            max_pattern_len,
            seg_len,
            TransitionWidth::Bits16,
        );
        let u32_ok = u32_r.found == oracle && u32_r.dropped == 0;
        let u16_ok = u16_r.found == oracle && u16_r.dropped == 0;
        eprintln!(
            "  {:>9}  {:>10.3}  {:>10.3}  {:>7.3}x  {:>8}  {:>8}",
            seg_len,
            u32_r.gbps,
            u16_r.gbps,
            if u32_r.gbps > 0.0 {
                u16_r.gbps / u32_r.gbps
            } else {
                0.0
            },
            u32_ok,
            u16_ok,
        );

        // SOUNDNESS (asserted): the u16-packed table must reproduce the oracle
        // EXACTLY — identical to the u32 path and to classic_ac_scan, 0 dropped.
        // A single divergence means the pack/unpack corrupted a transition.
        assert!(
            !u32_r.under_claimed && !u16_r.under_claimed,
            "seg_len={seg_len}: a width under-claimed (drain incomplete) — raise timeout/worker_groups",
        );
        assert_eq!(
            u32_r.found, oracle,
            "seg_len={seg_len}: the u32 baseline diverged from the oracle",
        );
        assert_eq!(
            u16_r.found, oracle,
            "seg_len={seg_len}: the u16-packed transitions diverged from the oracle — \
             the pack/unpack is NOT lossless on the GPU",
        );
        assert_eq!(u16_r.dropped, 0, "seg_len={seg_len}: u16 dropped hits");
    }
}

/// PROFILE-FIRST measurement for the megakernel's open optimization lane
/// (`docs/GPU_OOM_SEGMENTATION.md`: the throughput degradation at catalog scale
/// is an L1 working-set / memory-transaction effect, NOT L2 capacity). Two
/// candidate levers narrow each transition read: (1) **row deduplication** —
/// merge identical compressed transition rows behind a `state → row`
/// indirection, shrinking the count of DISTINCT physical rows a warp's 32 lanes
/// touch; (2) **u16 targets** — halve every transition word when
/// `state_count <= 65535`. The doc says both are profile-gated for the
/// *throughput* claim, but their *ceilings* are exact CPU facts that decide
/// whether either is worth a kernel change at all. This test pins those facts on
/// the real keyhog-scale combined automaton (CPU-only — no GPU), so the lever
/// choice is grounded in measured structure, not a guess.
///
/// It also PROVES the row→indirection transform is LOSSLESS: every state's
/// deduplicated row must be byte-identical to its compressed row, so adopting it
/// cannot change a single firing (Law 6/9). The distinct-row count is asserted
/// EXACTLY (the catalog is deterministic) — a real regression value, not a shape
/// check.
#[test]
fn row_dedup_and_u16_ceiling_on_keyhog_scale_combined_automaton() {
    use std::collections::HashMap;
    use vyre_runtime::megakernel::rule_catalog::{
        build_byte_class_map_for_table, compress_dense_transitions_into,
    };

    let owned = large_catalog();
    let patterns: Vec<&[u8]> = owned.iter().map(|p| p.as_slice()).collect();
    let ac = classic_ac_compile(&patterns);
    let state_count = ac.dfa.state_count as usize;

    // Compress the dense state*256 table to state*num_classes (the form the
    // kernel actually indexes), via the SHARED primitive — no fork.
    let mut class_map = Vec::new();
    let num_classes =
        build_byte_class_map_for_table(&ac.dfa.transitions, state_count, &mut class_map) as usize;
    let mut compressed = Vec::with_capacity(state_count * num_classes);
    compress_dense_transitions_into(
        &ac.dfa.transitions,
        state_count,
        &class_map,
        num_classes as u32,
        &mut compressed,
    );
    assert_eq!(compressed.len(), state_count * num_classes);

    // (1) Row-dedup ceiling: assign each DISTINCT compressed row a physical id,
    // build the state→row indirection, and count distinct rows.
    let mut row_to_id: HashMap<&[u32], u32> = HashMap::with_capacity(state_count);
    let mut row_of: Vec<u32> = Vec::with_capacity(state_count);
    let mut distinct_rows: Vec<&[u32]> = Vec::new();
    for s in 0..state_count {
        let row = &compressed[s * num_classes..(s + 1) * num_classes];
        let id = *row_to_id.entry(row).or_insert_with(|| {
            let id = distinct_rows.len() as u32;
            distinct_rows.push(row);
            id
        });
        row_of.push(id);
    }
    let distinct = distinct_rows.len();

    // LOSSLESS: reconstruct each state's row from the deduped table + indirection
    // and assert byte-identity with the compressed row. A deduped table that
    // changed any transition would silently drop/forge matches — refuse it.
    for s in 0..state_count {
        let original = &compressed[s * num_classes..(s + 1) * num_classes];
        let deduped = distinct_rows[row_of[s] as usize];
        assert_eq!(
            original, deduped,
            "state {s}: deduped row diverged from compressed row — the indirection is NOT lossless",
        );
    }

    // (2) u16 viability: every transition target is a state index < state_count;
    // u16 packing is sound iff state_count fits u16.
    let max_target = ac.dfa.transitions.iter().copied().max().unwrap_or(0);
    let u16_viable = state_count <= u16::MAX as usize + 1 && max_target <= u16::MAX as u32;

    // Byte accounting (u32 words today).
    let dense_bytes = state_count * 256 * 4;
    let compressed_bytes = state_count * num_classes * 4;
    let deduped_bytes = distinct * num_classes * 4 + state_count * 4; // table + indirection
    let u16_compressed_bytes = state_count * num_classes * 2;

    eprintln!("keyhog-scale combined automaton (CPU profile-first):");
    eprintln!("  patterns            = {}", patterns.len());
    eprintln!("  states              = {state_count}");
    eprintln!("  byte-classes        = {num_classes}");
    eprintln!(
        "  dense table         = {} KiB ({state_count} x 256 x 4B)",
        dense_bytes / 1024
    );
    eprintln!(
        "  byte-class table    = {} KiB ({state_count} x {num_classes} x 4B)  [SHIPS today]",
        compressed_bytes / 1024
    );
    eprintln!(
        "  distinct rows       = {distinct}  ({:.1}% of states)  ratio {:.3}x",
        100.0 * distinct as f64 / state_count as f64,
        state_count as f64 / distinct as f64
    );
    eprintln!(
        "  + row-dedup table   = {} KiB (table {} KiB + indirection {} KiB)  => {:.3}x vs byte-class",
        deduped_bytes / 1024,
        distinct * num_classes * 4 / 1024,
        state_count * 4 / 1024,
        compressed_bytes as f64 / deduped_bytes as f64,
    );
    eprintln!(
        "  + u16 table         = {} KiB  => {:.3}x vs byte-class  (viable: {u16_viable}, max_target {max_target})",
        u16_compressed_bytes / 1024,
        compressed_bytes as f64 / u16_compressed_bytes as f64,
    );

    // --- Pinned regression facts (deterministic catalog ⇒ exact values).
    // Measured once on CPU; a build-side change (alphabet, DFA construction) that
    // shifts these is caught. Update deliberately with a note, never to paper over
    // an unexplained drift. ---
    assert_eq!(
        patterns.len(),
        2048,
        "large_catalog() must yield 2048 literals"
    );
    assert_eq!(
        state_count, 13_199,
        "combined automaton state count drifted"
    );
    assert_eq!(num_classes, 67, "byte-class count drifted");
    assert_eq!(
        distinct, 12_579,
        "row-dedup ceiling (distinct compressed rows) drifted"
    );

    // DECISION ENCODED — row-dedup is REFUTED at this scale, u16 is the lever:
    //
    // 95.3% of compressed rows are already distinct (ratio 1.049x), so a
    // `state -> row` indirection shrinks the byte-class table by only ~3% while
    // ADDING a per-byte `row_of[state]` load to the hot loop and barely reducing
    // the distinct-rows-per-warp working set that is the named L1 limiter. That is
    // a net pessimization, not a win — do NOT build it. If a future build ever
    // makes rows highly redundant (dedup ratio >= 1.5x), this assert fires so the
    // decision is revisited with fresh evidence rather than silently stale.
    let dedup_ratio = state_count as f64 / distinct as f64;
    assert!(
        dedup_ratio < 1.5,
        "row-dedup was refuted at {dedup_ratio:.3}x; a >=1.5x ratio means rows are now \
         redundant enough to reconsider the state->row indirection lever",
    );

    // u16 is the structurally stronger lever: viable at this scale (13k states <<
    // 65535) and HALVES the byte-class table with NO extra indirection load. The
    // only open question is the WGSL u16-unpack ALU cost (GPU-profile-gated).
    assert!(
        u16_viable,
        "u16 targets must be viable at keyhog scale ({state_count} states, max_target {max_target})",
    );
    assert_eq!(
        compressed_bytes,
        2 * u16_compressed_bytes,
        "u16 packing must halve the byte-class transition table exactly",
    );
}

/// Settle the doc's LAST open GPU-scale lever — "reduce the SCATTER itself".
///
/// The keyhog-scale throughput degradation is an L1 working-set effect: a GPU
/// warp's 32 lanes (32 consecutive segments) sit at 32 DIFFERENT DFA states each
/// step, so their `transitions[state*nc+class]` reads scatter across the table.
/// With row-dedup + u16 measure-exhausted, the only lever left is reducing that
/// scatter via a state relabeling. But a FIXED relabeling can only change WHICH
/// addresses 32 rows occupy — it can NEVER reduce the COUNT of distinct states a
/// warp touches, which is data-determined. This measures that count.
///
/// Metric (modeling-free): the EXPECTED number of distinct states among 32
/// samples of the automaton's state-visit distribution = Σ_s [1 - (1-p_s)^32].
/// A warp's lane `i` reads byte `base + i*seg_len + k`, so its 32 lanes sample
/// the file strided by `seg_len` — ≈independent draws for high-entropy text,
/// identical for a dot run. If E[distinct] ≈ 32 even on high-entropy text, every
/// warp-step touches ~32 distinct rows no matter how states are numbered ⇒
/// relabeling cannot shrink the working set ⇒ the scatter lever is REFUTED. If
/// ≪ 32, a few hot states dominate ⇒ their rows stay L1-resident ⇒ scatter is
/// not the limiter on that text. Measured on BOTH a sparse (dot-heavy) and a
/// high-entropy buffer because scatter is a property of the TEXT, not the
/// automaton. CPU-only (no GPU).
#[test]
fn warp_state_scatter_bounds_the_relabeling_lever_at_keyhog_scale() {
    const WARP: i32 = 32;
    let owned = large_catalog();
    let patterns: Vec<&[u8]> = owned.iter().map(|p| p.as_slice()).collect();
    let ac = classic_ac_compile(&patterns);
    let state_count = ac.dfa.state_count as usize;
    let transitions = &ac.dfa.transitions;

    let walk_hist = |buf: &[u8]| -> (Vec<u64>, u64) {
        let mut hist = vec![0u64; state_count];
        let mut state = 0u32;
        for &b in buf {
            state = transitions[state as usize * 256 + b as usize];
            hist[state as usize] += 1;
        }
        (hist, buf.len() as u64)
    };
    // E[distinct states among WARP i.i.d. samples of the visit distribution].
    let expected_distinct = |hist: &[u64], total: u64| -> f64 {
        if total == 0 {
            return 0.0;
        }
        let total = total as f64;
        hist.iter()
            .filter(|&&c| c > 0)
            .map(|&c| {
                let p = c as f64 / total;
                1.0 - (1.0 - p).powi(WARP)
            })
            .sum()
    };
    // How many distinct states cover `frac` of all visits (concentration).
    let states_covering = |hist: &[u64], total: u64, frac: f64| -> usize {
        let mut v: Vec<u64> = hist.iter().copied().filter(|&c| c > 0).collect();
        v.sort_unstable_by(|a, b| b.cmp(a));
        let target = (total as f64 * frac) as u64;
        let (mut acc, mut n) = (0u64, 0usize);
        for c in v {
            acc += c;
            n += 1;
            if acc >= target {
                break;
            }
        }
        n
    };

    // Buffer A: the sparse planted corpus (mostly b'.').
    let sparse = build_haystack(&patterns);
    // Buffer B: deterministic high-entropy base62 — the worst case for scatter.
    let body: &[u8] = b"0123456789abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ";
    let mut dense: Vec<u8> = Vec::with_capacity(FILE_LEN);
    for i in 0..FILE_LEN {
        let idx = i.wrapping_mul(2_654_435_761).wrapping_add(i >> 3) % body.len();
        dense.push(body[idx]);
    }

    let (sparse_hist, sparse_total) = walk_hist(&sparse);
    let (dense_hist, dense_total) = walk_hist(&dense);
    let sparse_visited = sparse_hist.iter().filter(|&&c| c > 0).count();
    let dense_visited = dense_hist.iter().filter(|&&c| c > 0).count();
    let sparse_hot90 = states_covering(&sparse_hist, sparse_total, 0.90);
    let dense_hot90 = states_covering(&dense_hist, dense_total, 0.90);
    let sparse_ed = expected_distinct(&sparse_hist, sparse_total);
    let dense_ed = expected_distinct(&dense_hist, dense_total);
    eprintln!(
        "   sparse/realistic: states visited {sparse_visited}/{state_count}, hot (90%) {sparse_hot90}, \
         E[distinct among 32 lanes] = {sparse_ed:.2}/32",
    );
    eprintln!(
        "high-entropy base62: states visited {dense_visited}/{state_count}, hot (90%) {dense_hot90}, \
         E[distinct among 32 lanes] = {dense_ed:.2}/32",
    );

    // --- Pinned regression facts (deterministic catalog + buffers) ---
    assert_eq!(dense_visited, 14, "high-entropy state-visit set drifted");
    assert_eq!(dense_hot90, 7, "high-entropy hot-state (90%) count drifted");
    assert_eq!(sparse_visited, 13_199, "sparse buffer must exercise every state");
    assert_eq!(sparse_hot90, 7_512, "sparse hot-state (90%) count drifted");
    assert!(
        (12.5..13.0).contains(&sparse_ed),
        "sparse E[distinct among 32] drifted: {sparse_ed}",
    );
    assert!(
        (5.7..6.0).contains(&dense_ed),
        "high-entropy E[distinct among 32] drifted: {dense_ed}",
    );

    // DECISION ENCODED — the relabeling/scatter lever is REFUTED:
    //
    // A warp's 32 lanes touch only ~6 (high-entropy) to ~13 (realistic) DISTINCT
    // states per step, NOT 32 — they CLUSTER onto a few hot states, so there is no
    // 32-wide scatter for a fixed relabeling to coalesce. And the hot set on
    // realistic text is 7512 states (~2 MB of rows) — far too large to pack into
    // L1 by any relabeling. So the real scale mechanism is the GLOBAL hot-state set
    // growing with catalog size (8 patterns: tiny → 2048 patterns: 7512 hot),
    // NOT per-warp transition-read scatter. If a future build pushes the per-warp
    // distinct count toward the warp width (>= 24/32), this assert fires so the
    // relabeling lever is reconsidered with fresh evidence.
    assert!(
        sparse_ed < 24.0 && dense_ed < 24.0,
        "a warp's lanes cluster on few hot states (refuting the 32-wide-scatter premise); \
         if E[distinct] approaches the 32 warp width the relabeling lever must be revisited \
         (sparse {sparse_ed:.2}, dense {dense_ed:.2})",
    );
}

/// `CombinedDispatcher::calibrate_seg_len` must MEASURE the per-device window
/// optimum and leave the batch tiled at a FAST, CONSERVING geometry — the
/// autoroute-honest answer to "the optimum shifts by device." We deliberately
/// upload at the whole-file `u32::MAX` floor (the ~0.01x-HS correctness default a
/// naive caller would inherit); calibration must move off it. Asserts the
/// selection is the fastest COMPLETE candidate (real value, not shape), every
/// candidate's measurement is exposed (decision inputs visible — never a silent
/// pick), and the chosen geometry reproduces the CPU oracle exactly while beating
/// Hyperscan. Device-robust: it pins the CONTRACT, not a specific seg_len.
#[test]
#[ignore = "live GPU; run with --ignored --nocapture"]
fn calibrate_seg_len_lands_on_a_fast_conserving_geometry_on_this_device() {
    let backend = WgpuBackend::new()
        .expect("Fix: live GPU required (missing GPU is a configuration bug, not a fallback)");

    let patterns = catalog();
    let max_pattern_len = patterns
        .iter()
        .map(|p| p.len())
        .max()
        .expect("non-empty catalog") as u32;
    let buf = build_haystack(&patterns);
    let ac = classic_ac_compile(&patterns);
    let oracle: BTreeSet<(u32, u32)> = classic_ac_scan(&ac, &buf).into_iter().collect();
    assert!(!oracle.is_empty(), "fixture must plant matches");

    let hit_capacity: u32 = 1 << 20;
    // Upload at the WHOLE-FILE floor on purpose: calibration must improve on it.
    let mut batch = CombinedBatch::upload(
        backend.device_queue(),
        &[BatchFile::new(0xC0FFEE, 0, buf.clone())],
        &ac.dfa.transitions,
        &ac.dfa.output_offsets,
        &ac.dfa.output_records,
        ac.dfa.state_count,
        max_pattern_len,
        u32::MAX,
        hit_capacity,
    )
    .expect("Fix: CombinedBatch upload must succeed for a valid automaton");

    let config = BatchDispatchConfig {
        workgroup_size_x: 64,
        worker_groups: 2048,
        hit_capacity,
        timeout: Duration::from_secs(120),
        ..Default::default()
    };
    let mut dispatcher = CombinedDispatcher::new(backend.clone(), config);

    let cal = dispatcher
        .calibrate_seg_len(&mut batch, DEFAULT_SEG_LEN_CANDIDATES, 3)
        .expect("Fix: at least one default candidate must dispatch completely on a live GPU");

    // Every candidate measured — the decision inputs are fully visible.
    assert_eq!(
        cal.measurements.len(),
        DEFAULT_SEG_LEN_CANDIDATES.len(),
        "calibration must report a measurement for every candidate it tried"
    );
    assert!(
        DEFAULT_SEG_LEN_CANDIDATES.contains(&cal.chosen),
        "chosen seg_len {} must be one of the candidates",
        cal.chosen
    );

    // The chosen geometry is COMPLETE and is the FASTEST complete one measured.
    let chosen_m = cal
        .measurements
        .iter()
        .find(|m| m.seg_len == cal.chosen)
        .expect("chosen seg_len must appear in the measurements");
    assert!(
        chosen_m.complete && chosen_m.dropped_hits == 0,
        "chosen geometry must be complete (clean drain, 0 dropped); got {chosen_m:?}"
    );
    for m in &cal.measurements {
        if m.complete {
            assert!(
                chosen_m.wall_time <= m.wall_time,
                "chosen seg_len={} ({:?}) is not the fastest complete geometry; \
                 seg_len={} ran in {:?}",
                cal.chosen,
                chosen_m.wall_time,
                m.seg_len,
                m.wall_time
            );
        }
    }

    // The batch is left tiled at the winner: a dispatch now must CONSERVE the
    // full oracle hit set with 0 dropped — calibration never trades recall.
    let mut hits: Vec<HitRecord> = Vec::new();
    let summary = dispatcher
        .dispatch_into(&batch, &mut hits)
        .expect("dispatch at the calibrated geometry must succeed");
    let found: BTreeSet<(u32, u32)> = hits
        .iter()
        .map(|h| (h.rule_idx, h.match_offset))
        .collect();
    assert_eq!(
        found, oracle,
        "the calibrated geometry must reproduce the classic_ac_scan oracle exactly"
    );
    assert_eq!(summary.dropped_hits, 0, "calibrated dispatch must drop nothing");

    // And it must beat Hyperscan — calibration off the whole-file floor is the
    // whole point. (Device-robust: we assert the HS floor, not an exact seg_len.)
    let chosen_gbps = FILE_LEN as f64 / chosen_m.wall_time.as_secs_f64() / 1e9;
    assert!(
        chosen_gbps > HS_FLOOR_GBPS,
        "calibrated seg_len={} ran at {chosen_gbps:.3} GB/s, must beat Hyperscan {HS_FLOOR_GBPS} GB/s",
        cal.chosen
    );
    eprintln!(
        "calibrated seg_len={} ⇒ {chosen_gbps:.3} GB/s ({:.2}x Hyperscan); measurements: {:?}",
        cal.chosen,
        chosen_gbps / HS_FLOOR_GBPS,
        cal.measurements
    );
}

/// Calibration must hold at keyhog's REAL catalog scale (thousands of literals,
/// ~280k planted matches), not just the 32-literal smoke catalog. The combined-AC
/// geometry collapse is O(input) and rule-count-independent, so the calibrator's
/// measure-and-pick must still land on a fast geometry that reproduces the large
/// oracle EXACTLY — no dropped hits even as the automaton grows to thousands of
/// states and the hit set approaches the ring capacity. Same contract as the
/// smaller calibrator test, exercised at the scale that actually matters.
#[test]
#[ignore = "live GPU; run with --ignored --nocapture"]
fn calibrate_seg_len_holds_at_keyhog_catalog_scale() {
    let backend = WgpuBackend::new()
        .expect("Fix: live GPU required (missing GPU is a configuration bug, not a fallback)");

    let owned = large_catalog();
    let patterns: Vec<&[u8]> = owned.iter().map(|p| p.as_slice()).collect();
    let max_pattern_len = patterns
        .iter()
        .map(|p| p.len())
        .max()
        .expect("non-empty catalog") as u32;
    let buf = build_haystack(&patterns);
    let ac = classic_ac_compile(&patterns);
    let oracle: BTreeSet<(u32, u32)> = classic_ac_scan(&ac, &buf).into_iter().collect();
    assert!(
        oracle.len() > 10_000,
        "keyhog-scale fixture must plant a large oracle; got {}",
        oracle.len()
    );

    let hit_capacity: u32 = 1 << 20;
    // Start at the whole-file floor; calibration must move us off it even at scale.
    let mut batch = CombinedBatch::upload(
        backend.device_queue(),
        &[BatchFile::new(0xC0FFEE, 0, buf.clone())],
        &ac.dfa.transitions,
        &ac.dfa.output_offsets,
        &ac.dfa.output_records,
        ac.dfa.state_count,
        max_pattern_len,
        u32::MAX,
        hit_capacity,
    )
    .expect("Fix: CombinedBatch upload must succeed for a valid automaton");

    let config = BatchDispatchConfig {
        workgroup_size_x: 64,
        worker_groups: 2048,
        hit_capacity,
        timeout: Duration::from_secs(120),
        ..Default::default()
    };
    let mut dispatcher = CombinedDispatcher::new(backend.clone(), config);

    let cal = dispatcher
        .calibrate_seg_len(&mut batch, DEFAULT_SEG_LEN_CANDIDATES, 3)
        .expect("Fix: a default candidate must dispatch completely at keyhog scale");

    // The pick is the fastest COMPLETE geometry measured.
    let chosen_m = cal
        .measurements
        .iter()
        .find(|m| m.seg_len == cal.chosen)
        .expect("chosen seg_len must appear in the measurements");
    assert!(
        chosen_m.complete && chosen_m.dropped_hits == 0,
        "chosen geometry must be complete at scale (clean drain, 0 dropped); got {chosen_m:?}"
    );
    for m in &cal.measurements {
        if m.complete {
            assert!(
                chosen_m.wall_time <= m.wall_time,
                "chosen seg_len={} ({:?}) is not the fastest complete; seg_len={} ran in {:?}",
                cal.chosen,
                chosen_m.wall_time,
                m.seg_len,
                m.wall_time
            );
        }
    }

    // The calibrated geometry must reproduce the LARGE oracle exactly — calibration
    // must never trade recall for speed, even when the hit set is ~280k.
    let mut hits: Vec<HitRecord> = Vec::new();
    let summary = dispatcher
        .dispatch_into(&batch, &mut hits)
        .expect("dispatch at the calibrated geometry must succeed");
    let found: BTreeSet<(u32, u32)> = hits
        .iter()
        .map(|h| (h.rule_idx, h.match_offset))
        .collect();
    assert_eq!(
        found, oracle,
        "the calibrated keyhog-scale geometry must conserve every one of the {} oracle matches",
        oracle.len()
    );
    assert_eq!(summary.dropped_hits, 0, "calibrated dispatch must drop nothing at scale");

    let chosen_gbps = FILE_LEN as f64 / chosen_m.wall_time.as_secs_f64() / 1e9;
    assert!(
        chosen_gbps > HS_FLOOR_GBPS,
        "calibrated keyhog-scale seg_len={} ran at {chosen_gbps:.3} GB/s, must beat Hyperscan {HS_FLOOR_GBPS} GB/s",
        cal.chosen
    );
    eprintln!(
        "keyhog-scale calibrated seg_len={} ⇒ {chosen_gbps:.3} GB/s ({:.2}x HS) over {} patterns / {} matches",
        cal.chosen,
        chosen_gbps / HS_FLOOR_GBPS,
        patterns.len(),
        oracle.len()
    );
}
