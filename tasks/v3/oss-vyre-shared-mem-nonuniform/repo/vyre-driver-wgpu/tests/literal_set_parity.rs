//! WGPU parity coverage for the shared literal-set matcher.

#![allow(deprecated)]
use vyre_driver_wgpu::WgpuBackend;
use vyre_libs::scan::literal_set::{GpuLiteralSet, Match};

#[test]
fn literal_set_parity_abc() {
    let patterns: &[&[u8]] = &[b"abc", b"bc"];
    let engine = GpuLiteralSet::compile(patterns);
    let haystack = b"zabc";

    let reference_matches = engine.reference_scan(haystack);
    assert_eq!(reference_matches.len(), 2);
    assert_eq!(reference_matches[0], Match::new(0, 1, 4));
    assert_eq!(reference_matches[1], Match::new(1, 2, 4));

    let backend = WgpuBackend::new().expect("Fix: literal_set parity requires a live GPU");
    let gpu_matches = engine.scan(&backend, haystack, 10_000).unwrap();
    assert_eq!(gpu_matches, reference_matches);
}
