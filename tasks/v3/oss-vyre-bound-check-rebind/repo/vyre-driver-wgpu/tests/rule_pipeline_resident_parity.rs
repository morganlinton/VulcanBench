//! Real-GPU parity + amortization coverage for [`ResidentRulePipeline`].
//!
//! The resident session uploads the NFA transition/epsilon tables once and then
//! transfers only the haystack per scan, versus [`RulePipeline::scan`] which
//! re-uploads the tables on every dispatch. This test runs both paths against a
//! live WGPU adapter and asserts:
//!
//!   1. **Parity** — the resident match set is byte-identical to the borrowed
//!      `scan` (and to the CPU `reference_scan`) for every haystack. This is the
//!      recall guarantee: switching keyhog's megascan path to the resident
//!      session must not drop or invent a single match.
//!   2. **Stability across reuse** — repeated scans on one session return the
//!      same matches, proving the resident tables survive (and the per-scan
//!      counter reset works) across dispatches.
//!
//! Fails loudly if no GPU adapter is present — these machines must have one.

use std::time::Instant;

use vyre_driver_wgpu::WgpuBackend;
use vyre_foundation::match_result::Match;
use vyre_libs::scan::{build_rule_pipeline, ResidentRulePipeline};

/// A pattern set large enough that the lane-major transition table is non-trivial
/// (this is the table the resident path avoids re-uploading every scan).
const PATTERNS: &[&str] = &[
    "abc", "abd", "bcd", "cde", "def", "key", "token", "secret", "passwd", "AKIA",
];

const MAX_MATCHES: u32 = 10_000;

fn borrowed_then_reference(
    pipeline: &vyre_libs::scan::RulePipeline,
    backend: &WgpuBackend,
    haystack: &[u8],
) -> Vec<Match> {
    let borrowed = pipeline
        .scan(backend, haystack, MAX_MATCHES)
        .expect("borrowed RulePipeline::scan dispatch");
    let reference = pipeline.reference_scan(haystack);
    assert_eq!(
        borrowed,
        reference,
        "borrowed GPU scan must match the CPU reference for {:?}",
        String::from_utf8_lossy(haystack)
    );
    borrowed
}

#[test]
fn resident_rule_pipeline_matches_borrowed_on_real_gpu() {
    let backend =
        WgpuBackend::new().expect("Fix: resident RulePipeline parity requires a live GPU adapter");

    let haystacks: &[&[u8]] = &[
        b"zabcd",
        b"the api key=secret and the token=AKIAEXAMPLE passwd=def",
        b"no matches here at all, just prose without any pattern bytes",
        b"",
        b"abcabcabc def def secret secret token",
    ];

    // Size the resident haystack buffer to the largest haystack under test.
    let capacity = haystacks.iter().map(|h| h.len()).max().unwrap_or(0).max(1);
    let pipeline = build_rule_pipeline(PATTERNS, "input", "hits", capacity as u32);

    let session: ResidentRulePipeline = pipeline
        .prepare_resident(&backend, capacity, MAX_MATCHES)
        .expect("Fix: WGPU backend must support resident allocation");

    let mut scratch = Vec::new();
    let mut resident_matches = Vec::new();

    for haystack in haystacks {
        let expected = borrowed_then_reference(&pipeline, &backend, haystack);

        session
            .scan_into(&backend, haystack, &mut resident_matches, &mut scratch)
            .expect("resident scan dispatch");

        assert_eq!(
            resident_matches,
            expected,
            "resident scan diverged from borrowed/reference for {:?}",
            String::from_utf8_lossy(haystack)
        );
    }

    // Stability: re-scan the busiest haystack many times on the same session and
    // confirm the resident tables + counter reset keep producing the same set.
    let busy: &[u8] = b"abcabcabc def def secret secret token AKIA passwd key";
    let expected = borrowed_then_reference(&pipeline, &backend, busy);
    for round in 0..32 {
        session
            .scan_into(&backend, busy, &mut resident_matches, &mut scratch)
            .expect("resident re-scan dispatch");
        assert_eq!(
            resident_matches, expected,
            "resident scan drifted on reuse round {round}"
        );
    }

    session.free(&backend).expect("free resident resources");
}

/// Amortization signal (not a hard gate — printed under `--nocapture`). Times a
/// batch of identical scans through the borrowed path (re-uploads tables each
/// call) versus a resident session (tables uploaded once). The resident path
/// should not be slower; on large pattern sets it is materially faster because
/// the per-scan host→device transfer drops from `tables + haystack` to
/// `haystack`. Kept `#[ignore]` so CI time isn't spent on a measurement.
#[test]
#[ignore = "measurement, run with --ignored --nocapture"]
fn resident_rule_pipeline_amortizes_table_upload() {
    let backend =
        WgpuBackend::new().expect("Fix: GPU adapter required for the amortization measurement");

    let haystack = b"abcabcabc def def secret secret token AKIA passwd key cde bcd".repeat(64);
    let pipeline = build_rule_pipeline(PATTERNS, "input", "hits", haystack.len() as u32);
    const ROUNDS: usize = 200;

    // Warm-up both paths (adapter/pipeline cold-start).
    let _ = pipeline.scan(&backend, &haystack, MAX_MATCHES).unwrap();
    let session = pipeline
        .prepare_resident(&backend, haystack.len(), MAX_MATCHES)
        .unwrap();
    let mut scratch = Vec::new();
    let mut matches = Vec::new();
    session
        .scan_into(&backend, &haystack, &mut matches, &mut scratch)
        .unwrap();

    let t0 = Instant::now();
    for _ in 0..ROUNDS {
        let m = pipeline.scan(&backend, &haystack, MAX_MATCHES).unwrap();
        std::hint::black_box(&m);
    }
    let borrowed_ms = t0.elapsed().as_secs_f64() * 1000.0;

    let t1 = Instant::now();
    for _ in 0..ROUNDS {
        session
            .scan_into(&backend, &haystack, &mut matches, &mut scratch)
            .unwrap();
        std::hint::black_box(&matches);
    }
    let resident_ms = t1.elapsed().as_secs_f64() * 1000.0;

    eprintln!(
        "resident amortization: borrowed {borrowed_ms:.1} ms vs resident {resident_ms:.1} ms over {ROUNDS} scans ({:.2}x)",
        borrowed_ms / resident_ms.max(1e-9)
    );
    session.free(&backend).unwrap();
}
