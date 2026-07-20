//! API-boundary regression tests for production scan paths.

mod support;

#[test]
fn scan_layer_does_not_export_cpu_named_execution_paths() {
    support::assert_no_cpu_named_api_exports(
        "src/scan",
        "scan",
        &["scan_cpu"],
        "scan-layer CPU-named APIs must be explicit reference/parity internals",
    );
}
