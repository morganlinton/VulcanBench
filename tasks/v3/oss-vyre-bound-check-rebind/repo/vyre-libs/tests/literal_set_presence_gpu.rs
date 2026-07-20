//! Real-GPU proof for the literal-set PRESENCE-bitmap output mode.
//!
//! 1. CORRECTNESS (the soundness gate): the GPU presence bitmap must mark EXACTLY
//!    the set of pattern ids the CPU reference scan reports — no missed pattern
//!    (recall) and no fabricated pattern (precision). Runs on whatever the wgpu
//!    backend resolves to (the RTX 5090 here); skips cleanly if no GPU.
//! 2. THROUGHPUT: on a match-DENSE haystack (the keyhog phase-1 regime, ~1 hit per
//!    few dozen bytes), compare the match-triple `scan` (atomic-append per hit +
//!    big readback) against `scan_presence` (one idempotent atomic-OR bit per hit,
//!    tiny readback). The triple path is output-bound and collapses; presence
//!    should stay near the scan ceiling.
//!
//! Run:
//!   cargo test -p vyre-libs --test literal_set_presence_gpu --release -- --nocapture

use vyre_driver_wgpu::WgpuBackend;
use vyre_libs::scan::GpuLiteralSet;

/// Dense, keyhog-like literal set: short high-frequency anchors that fire all over
/// source text, plus realistic credential prefixes.
const LITERALS: &[&[u8]] = &[
    b"key",
    b"token",
    b"secret",
    b"api",
    b"pass",
    b"auth",
    b"user",
    b"id",
    b"AKIA",
    b"ghp_",
    b"xoxb-",
    b"sk_live_",
    b"glpat-",
    b"AIza",
    b"-----BEGIN",
    b"password",
    b"private",
    b"access",
    b"client",
    b"bearer",
    b"cred",
    b"config",
    b"value",
    b"name",
    b"data",
    b"hash",
    b"sign",
    b"cert",
];

fn dense_haystack(target_bytes: usize) -> Vec<u8> {
    // A realistic-ish source/config line carrying multiple literal hits.
    let unit: &[u8] = b"api_key = \"AKIA0123token\"; secret_token: ghp_abc; user_password=value; \
                        access_id sign client_cert hash config data name bearer auth cred private\n";
    let mut out = Vec::with_capacity(target_bytes + unit.len());
    while out.len() < target_bytes {
        out.extend_from_slice(unit);
    }
    out
}

fn presence_bit(bitmap: &[u32], pattern_id: u32) -> bool {
    let w = (pattern_id >> 5) as usize;
    let b = pattern_id & 31;
    bitmap.get(w).is_some_and(|word| (word >> b) & 1 == 1)
}

#[test]
fn presence_bitmap_matches_reference_pattern_set_and_is_faster_dense() {
    let backend = match WgpuBackend::shared() {
        Ok(b) => b,
        Err(e) => {
            eprintln!("no wgpu backend ({e}); skipping GPU presence test");
            return;
        }
    };
    let matcher = GpuLiteralSet::compile(LITERALS);
    let n_patterns = LITERALS.len() as u32;

    // ---- Correctness on a smaller, varied haystack ----
    let probe = dense_haystack(64 * 1024);
    let mut expected = [false; 64];
    for m in matcher.reference_scan(&probe) {
        expected[m.pattern_id as usize] = true;
    }
    let bitmap = matcher
        .scan_presence(backend.as_ref(), &probe)
        .expect("gpu presence scan");
    for pid in 0..n_patterns {
        let got = presence_bit(&bitmap, pid);
        let want = expected[pid as usize];
        assert_eq!(
            got,
            want,
            "presence mismatch for pattern {pid} ({:?}): gpu={got} reference={want}",
            std::str::from_utf8(LITERALS[pid as usize]).unwrap_or("<bytes>")
        );
    }
    // At least some patterns must have fired, or the test proves nothing.
    assert!(
        expected.iter().any(|&p| p),
        "reference scan found no patterns; corpus/literal set is degenerate"
    );

    // ---- Throughput on a large dense haystack ----
    // Kept under the wgpu per-dimension workgroup cap: one byte-scan invocation
    // per haystack byte, workgroup_x = 128, so bytes / 128 must stay < 65535
    // (≈ 8.39 MB). 4 MiB → 32768 workgroups, comfortably inside the limit.
    const BIG: usize = 4 * 1024 * 1024;
    let big = dense_haystack(BIG);
    let mb = big.len() as f64 / 1e6;
    let max_matches: u32 = 4_000_000;

    // Warm up shader compile / first-dispatch init.
    let _ = matcher.scan(backend.as_ref(), &big[..4096], 64);
    let _ = matcher.scan_presence(backend.as_ref(), &big[..4096]);

    let t = std::time::Instant::now();
    let triples = matcher
        .scan(backend.as_ref(), &big, max_matches)
        .expect("gpu triple scan");
    let scan_ms = t.elapsed().as_secs_f64() * 1000.0;
    let scan_mbps = mb / (scan_ms / 1e3);

    let t = std::time::Instant::now();
    let presence = matcher
        .scan_presence(backend.as_ref(), &big)
        .expect("gpu presence scan");
    let presence_ms = t.elapsed().as_secs_f64() * 1000.0;
    let presence_mbps = mb / (presence_ms / 1e3);

    // CPU reference throughput for the same haystack (single thread).
    let t = std::time::Instant::now();
    let cpu = matcher.reference_scan(&big);
    let cpu_ms = t.elapsed().as_secs_f64() * 1000.0;
    let cpu_mbps = mb / (cpu_ms / 1e3);

    eprintln!("\n=== literal-set output-mode throughput on {mb:.1} MB dense haystack ===");
    eprintln!(
        "  CPU reference (1 thread) : {cpu_ms:>8.1} ms  {cpu_mbps:>9.1} MB/s  ({} matches)",
        cpu.len()
    );
    eprintln!(
        "  GPU scan (match triples) : {scan_ms:>8.1} ms  {scan_mbps:>9.1} MB/s  ({} triples)",
        triples.len()
    );
    eprintln!("  GPU scan_presence (bits) : {presence_ms:>8.1} ms  {presence_mbps:>9.1} MB/s  ({} set bits)",
        presence.iter().map(|w| w.count_ones()).sum::<u32>());
    eprintln!(
        "  presence speedup over triples: {:.1}×",
        presence_mbps / scan_mbps.max(1e-9)
    );
    eprintln!(
        "  presence vs CPU              : {:.1}×",
        presence_mbps / cpu_mbps.max(1e-9)
    );

    // The whole point: on a dense haystack the compact presence output must beat
    // the triple-append path. (Correctness is asserted above; this pins the win.)
    assert!(
        presence_mbps > scan_mbps,
        "presence ({presence_mbps:.1} MB/s) was not faster than triple scan ({scan_mbps:.1} MB/s) on a dense haystack — the output-mode lever regressed"
    );
}
