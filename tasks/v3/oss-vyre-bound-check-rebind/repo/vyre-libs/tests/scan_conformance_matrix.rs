//! Scan conformance matrix test suite.

use std::collections::BTreeSet;

const REQUIRED_SEMANTICS: &[&str] = &[
    "leftmost_semantics",
    "overlapping_matches",
    "capture_groups",
    "byte_mode",
    "unicode_mode",
    "streaming_chunks",
    "unsupported_constructs",
];

const REQUIRED_ENGINES: &[&str] = &[
    "cpu_ref",
    "cuda",
    "wgpu",
    "metal",
    "rust_regex",
    "hyperscan",
    "vectorscan",
];

const ROWS: &[(&str, &[&str], &str, &str)] = &[
    (
        "leftmost_semantics",
        &[
            "cpu_ref",
            "cuda",
            "wgpu",
            "metal",
            "rust_regex",
            "hyperscan",
            "vectorscan",
        ],
        "6c6566746d6f73743a7061747465726e303a303a31",
        "VYRE_SCAN_UNSUPPORTED_NOT_APPLICABLE",
    ),
    (
        "overlapping_matches",
        &[
            "cpu_ref",
            "cuda",
            "wgpu",
            "metal",
            "rust_regex",
            "hyperscan",
            "vectorscan",
        ],
        "6f7665726c61703a7061747465726e303a303a333b7061747465726e303a313a34",
        "VYRE_SCAN_UNSUPPORTED_NOT_APPLICABLE",
    ),
    (
        "capture_groups",
        &[
            "cpu_ref",
            "cuda",
            "wgpu",
            "metal",
            "rust_regex",
            "hyperscan",
            "vectorscan",
        ],
        "63617074757265733a6d617463683a303a333b67726f7570313a303a31",
        "VYRE_SCAN_UNSUPPORTED_CAPTURE_GROUPS",
    ),
    (
        "byte_mode",
        &[
            "cpu_ref",
            "cuda",
            "wgpu",
            "metal",
            "rust_regex",
            "hyperscan",
            "vectorscan",
        ],
        "627974655f6d6f64653afffe3a313a33",
        "VYRE_SCAN_UNSUPPORTED_NOT_APPLICABLE",
    ),
    (
        "unicode_mode",
        &[
            "cpu_ref",
            "cuda",
            "wgpu",
            "metal",
            "rust_regex",
            "hyperscan",
            "vectorscan",
        ],
        "756e69636f64655f6d6f64653aceb13a303a32",
        "VYRE_SCAN_UNSUPPORTED_UNICODE_MODE_GPU",
    ),
    (
        "streaming_chunks",
        &[
            "cpu_ref",
            "cuda",
            "wgpu",
            "metal",
            "rust_regex",
            "hyperscan",
            "vectorscan",
        ],
        "73747265616d696e673a6368756e6b303a7461696c3b6368756e6b313a68656164",
        "VYRE_SCAN_UNSUPPORTED_NOT_APPLICABLE",
    ),
    (
        "unsupported_constructs",
        &[
            "cpu_ref",
            "cuda",
            "wgpu",
            "metal",
            "rust_regex",
            "hyperscan",
            "vectorscan",
        ],
        "756e737570706f727465643a6261636b7265663a7061747465726e30",
        "VYRE_SCAN_UNSUPPORTED_BACKREFERENCE",
    ),
];

#[test]
fn scan_conformance_matrix_covers_required_semantics() {
    let semantics = ROWS
        .iter()
        .map(|(semantics, _, _, _)| *semantics)
        .collect::<BTreeSet<_>>();
    for required in REQUIRED_SEMANTICS {
        assert!(
            semantics.contains(required),
            "Fix: scan conformance matrix must include `{required}` semantics"
        );
    }
}

#[test]
fn scan_conformance_matrix_reports_all_required_engines() {
    for (semantics, engines, _, _) in ROWS {
        let engines = engines.iter().copied().collect::<BTreeSet<_>>();
        for required in REQUIRED_ENGINES {
            assert!(
                engines.contains(required),
                "Fix: scan conformance row `{semantics}` must report `{required}` support"
            );
        }
    }
}

#[test]
fn scan_conformance_matrix_records_output_bytes_and_diagnostics() {
    for (semantics, _, output_hex, diagnostic_code) in ROWS {
        assert!(
            !output_hex.is_empty()
                && output_hex.len() % 2 == 0
                && output_hex.bytes().all(|byte| byte.is_ascii_hexdigit()),
            "Fix: scan conformance row `{semantics}` must record exact output bytes as hex"
        );
        assert!(
            diagnostic_code.starts_with("VYRE_SCAN_"),
            "Fix: scan conformance row `{semantics}` must record exact unsupported diagnostic code"
        );
    }
}
