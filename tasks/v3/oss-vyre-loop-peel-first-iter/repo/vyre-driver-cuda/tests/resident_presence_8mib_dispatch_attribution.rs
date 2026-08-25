//! 8 MiB CUDA region-presence dispatch ATTRIBUTION (perf diagnosis, `#[ignore]`).
//!
//! keyhog's `gpu_vs_hs_8mib` bench measures GPU region-presence at 20.55 ms vs
//! Hyperscan 18.52 ms (1.11x SLOWER) on the sparse 8 MiB corpus; the gap is
//! exactly the phase-1 difference (GPU dispatch ~5 ms vs HS phase-1 ~3.5 ms — the
//! shared 15 ms phase-2 cancels). To know whether a VYRE change can close that
//! 1.5 ms phase-1 gap, the 5 ms dispatch must be attributed: how much is the GPU
//! KERNEL (`device_ns`, irreducible without a kernel rewrite) vs host-side
//! staging/upload/readback (cuttable with resident tables + overlap)?
//!
//! This drives the REAL region-presence path on the CUDA backend at 8 MiB and
//! prints, for both the borrowed path (keyhog's — re-uploads the immutable tables
//! every scan) and the resident path (tables uploaded once):
//!   - total per-scan host wall (the whole `scan_*` call),
//!   - the dispatch wall (`TimedDispatchResult::wall_ns`),
//!   - the GPU kernel time (`TimedDispatchResult::device_ns`, when the backend
//!     reports it).
//! The borrowed−resident delta is the per-scan table re-upload keyhog pays; the
//! resident `device_ns` is the kernel floor the phase-1 lever cannot beat without
//! a kernel rewrite. This is a DIAGNOSIS harness, not a parity gate — it asserts
//! the scan is real (planted hit present, sane hit count) so the numbers aren't
//! measuring an empty/degenerate scan, then prints the attribution.
//!
//! Run:
//!   cargo test -p vyre-driver-cuda --test resident_presence_8mib_dispatch_attribution \
//!     --release -- --ignored --nocapture

use std::time::Instant;

use vyre_driver_cuda::{CudaBackend, CudaBackendRegistration};
use vyre_libs::scan::GpuLiteralSet;

const HAYSTACK_BYTES: usize = 8 * 1024 * 1024;
const ITERS: usize = 20;

/// ~900 distinct, varied-prefix literals so the compiled DFA is comparable in
/// scale to keyhog's ~895-detector catalog (the transition table size — and thus
/// the borrowed re-upload cost — tracks the state count). Each literal is a
/// base-26 encoding of its index across the first 4 bytes plus a fixed-ish tail,
/// giving broad prefix fan-out rather than one shared stem.
fn synth_literals() -> Vec<Vec<u8>> {
    let mut lits = Vec::with_capacity(900);
    for i in 0u32..900 {
        let mut s = Vec::with_capacity(10);
        let mut v = i;
        for _ in 0..4 {
            s.push(b'a' + (v % 26) as u8);
            v /= 26;
        }
        // Distinct tail so no literal is a prefix of another (keeps each an
        // accepting terminal): "_k" + two index digits.
        s.extend_from_slice(b"_k");
        s.push(b'0' + (i % 10) as u8);
        s.push(b'0' + ((i / 10) % 10) as u8);
        lits.push(s);
    }
    lits
}

/// 8 MiB sparse haystack: a repeating non-matching filler with a HANDFUL of
/// planted literal occurrences, mirroring the canonical corpus (the suffix3
/// prefilter passes few positions → the kernel does little replay work, so the
/// dispatch cost is dominated by the whole-input prefilter pass + host overhead,
/// not by candidate replay). Returns the haystack and the planted literal so the
/// caller can assert the scan actually fired.
fn sparse_8mib_with_plants(planted: &[u8]) -> Vec<u8> {
    // Filler that contains none of the synth literals (digits + spaces only).
    let filler: &[u8] = b"0123456789 0123456789 0123456789 0123456789 \n";
    let mut h = Vec::with_capacity(HAYSTACK_BYTES + planted.len() * 64);
    while h.len() < HAYSTACK_BYTES {
        h.extend_from_slice(filler);
        // Plant the literal roughly every 256 KiB so a few dozen real hits exist.
        if h.len() % (256 * 1024) < filler.len() {
            h.extend_from_slice(b" ");
            h.extend_from_slice(planted);
            h.extend_from_slice(b" ");
        }
    }
    h.truncate(HAYSTACK_BYTES);
    h
}

fn median(mut v: Vec<f64>) -> f64 {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    v[v.len() / 2]
}

/// Count set bits across a presence bitmap row (region 0 spans `words` u32s).
fn popcount(words: &[u32]) -> u32 {
    words.iter().map(|w| w.count_ones()).sum()
}

#[test]
#[ignore = "perf diagnosis; run explicitly with --ignored on a CUDA host"]
fn region_presence_8mib_dispatch_attribution_cuda() {
    let backend = match CudaBackend::acquire() {
        Ok(b) => CudaBackendRegistration::new(b),
        Err(e) => {
            eprintln!("no CUDA backend ({e}); skipping 8MiB dispatch attribution");
            return;
        }
    };

    let literals = synth_literals();
    let lit_refs: Vec<&[u8]> = literals.iter().map(|v| v.as_slice()).collect();
    // Plant literal index 0 (its byte form) so the scan has real, assertable hits.
    let planted = literals[0].clone();
    let matcher = GpuLiteralSet::compile(&lit_refs);
    let pattern_count = lit_refs.len() as u32;
    let words = pattern_count.div_ceil(32).max(1) as usize;

    let haystack = sparse_8mib_with_plants(&planted);
    assert_eq!(haystack.len(), HAYSTACK_BYTES, "8 MiB haystack");
    // Single region spanning the whole 8 MiB, matching keyhog's borrowed-single-chunk.
    let region_starts = [0u32];

    // --- Ground truth + real-scan gate: borrowed presence-by-region. ---
    let borrowed_bitmap = matcher
        .scan_presence_by_region(&backend, &haystack, &region_starts)
        .expect("borrowed CUDA presence-by-region scan");
    assert_eq!(borrowed_bitmap.len(), words, "one region → one presence row");
    let hits = popcount(&borrowed_bitmap);
    assert!(
        hits >= 1,
        "the planted literal must register at least one present bit (got {hits}); \
         a zero-hit scan would make the timing meaningless"
    );
    // Bit 0 (the planted literal index 0) MUST be set.
    assert_eq!(
        borrowed_bitmap[0] & 1,
        1,
        "planted literal (pattern 0) must be marked present in region 0"
    );

    // --- Borrowed path timing (keyhog's path: re-uploads tables every scan). ---
    let mut borrowed_wall_ms = Vec::with_capacity(ITERS);
    for _ in 0..ITERS + 1 {
        let t = Instant::now();
        let bm = matcher
            .scan_presence_by_region(&backend, &haystack, &region_starts)
            .expect("borrowed scan");
        let dt = t.elapsed().as_secs_f64() * 1e3;
        assert_eq!(popcount(&bm), hits, "borrowed hit count must be stable");
        borrowed_wall_ms.push(dt);
    }
    borrowed_wall_ms.remove(0); // drop warm-up

    // --- Resident path timing (tables uploaded once; per-scan transfer = haystack). ---
    let session = matcher
        .prepare_resident_presence(&backend, haystack.len() + 64, 2)
        .expect("prepare resident region-presence session on CUDA");
    let mut out = Vec::new();
    let mut scratch = Vec::new();
    let mut resident_call_ms = Vec::with_capacity(ITERS);
    let mut resident_dispatch_ms = Vec::with_capacity(ITERS);
    let mut resident_kernel_ms = Vec::with_capacity(ITERS);
    let mut kernel_reported = false;
    for _ in 0..ITERS + 1 {
        let t = Instant::now();
        let timed = session
            .scan_into_timed(&backend, &haystack, &region_starts, 0, &mut out, &mut scratch)
            .expect("resident timed scan");
        let call_ms = t.elapsed().as_secs_f64() * 1e3;
        assert_eq!(out, borrowed_bitmap, "resident bitmap must equal borrowed");
        resident_call_ms.push(call_ms);
        resident_dispatch_ms.push(timed.wall_ns as f64 / 1e6);
        if let Some(dev) = timed.device_ns {
            kernel_reported = true;
            resident_kernel_ms.push(dev as f64 / 1e6);
        }
    }
    resident_call_ms.remove(0);
    resident_dispatch_ms.remove(0);
    if !resident_kernel_ms.is_empty() {
        resident_kernel_ms.remove(0);
    }
    session.free(&backend).expect("free resident session");

    let borrowed_med = median(borrowed_wall_ms);
    let res_call_med = median(resident_call_ms);
    let res_dispatch_med = median(resident_dispatch_ms);

    eprintln!("=== 8 MiB CUDA region-presence dispatch attribution ===");
    eprintln!(
        "detectors={pattern_count}  haystack=8 MiB  region_count=1  hits={hits}  iters={ITERS}"
    );
    eprintln!("borrowed scan_presence_by_region (keyhog path): median wall {borrowed_med:.3} ms");
    eprintln!("resident scan_into_timed: median total-call {res_call_med:.3} ms");
    eprintln!("resident dispatch (TimedDispatchResult.wall): median {res_dispatch_med:.3} ms");
    if kernel_reported {
        let res_kernel_med = median(resident_kernel_ms);
        eprintln!("resident GPU KERNEL (device_ns): median {res_kernel_med:.3} ms");
        eprintln!(
            "  -> dispatch host overhead (wall-device): {:.3} ms",
            res_dispatch_med - res_kernel_med
        );
        eprintln!(
            "  -> staging+decode (call-dispatch): {:.3} ms",
            res_call_med - res_dispatch_med
        );
        eprintln!(
            "VERDICT: HS phase-1 ~3.5 ms. Kernel floor = {res_kernel_med:.3} ms. \
             Phase-1 win in vyre is {} (kernel {} 3.5 ms).",
            if res_kernel_med < 3.5 { "POSSIBLE" } else { "NOT possible without a kernel rewrite" },
            if res_kernel_med < 3.5 { "<" } else { ">=" },
        );
    } else {
        eprintln!("(backend did not report device_ns; only wall times available)");
    }
    eprintln!(
        "borrowed - resident_call = {:.3} ms (per-scan table re-upload keyhog pays)",
        borrowed_med - res_call_med
    );
}
