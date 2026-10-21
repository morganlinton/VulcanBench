//! Trace context telemetry contracts test suite.

const TRACES: &str = include_str!("../../docs/optimization/TRACE_CONTEXT_TELEMETRY_CONTRACTS.toml");

#[test]
fn trace_context_telemetry_contracts_require_traceparent_and_resource_attributes() {
    for required in [
        "span_id",
        "trace_id",
        "traceparent",
        "tracestate",
        "resource_attributes",
        "span_kind",
        "parent_span",
        "privacy_class",
        "capability_digest",
    ] {
        assert!(
            TRACES.contains(required),
            "trace context telemetry contract must include {required}"
        );
    }
}
