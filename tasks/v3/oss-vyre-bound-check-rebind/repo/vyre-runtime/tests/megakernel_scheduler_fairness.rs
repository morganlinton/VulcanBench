//! Integration tests for the megakernel scheduler fairness accounting.

#![allow(clippy::assertions_on_constants)]
use vyre_runtime::megakernel::{control, priority_scan_body};

#[test]
fn scheduler_priority_scan_includes_fairness_accounting() {
    let nodes = priority_scan_body(256);

    // Check for tenant fairness check (64 is TENANT_FAIRNESS_BASE)
    let has_tenant_fairness = nodes.iter().any(|node| {
        let s = format!("{:?}", node);
        s.contains("64") || s.contains("TENANT_FAIRNESS_BASE")
    });

    // Check for priority fairness telemetry (129 is PRIORITY_FAIRNESS_BASE)
    let has_priority_fairness = nodes.iter().any(|node| {
        let s = format!("{:?}", node);
        s.contains("129") || s.contains("PRIORITY_FAIRNESS_BASE")
    });

    // Check for offset-based scanning to reduce contention
    let has_offset_scan = nodes.iter().any(|node| {
        let s = format!("{:?}", node);
        s.contains("lane_id") && s.contains("Mod")
    });

    assert!(
        has_tenant_fairness,
        "Priority scan must include tenant fairness accounting"
    );
    assert!(
        has_priority_fairness,
        "Priority scan must include priority fairness telemetry"
    );
    assert!(
        has_offset_scan,
        "Priority scan should use offset-based probing to reduce contention"
    );
}

#[test]
fn control_buffer_layout_has_room_for_fairness() {
    assert!(control::TENANT_FAIRNESS_BASE >= 64);
    assert!(control::PRIORITY_OFFSETS_BASE > control::EPOCH);
    assert!(
        control::PRIORITY_OFFSETS_BASE + control::PRIORITY_OFFSETS_SLOTS
            <= control::PRIORITY_STARVATION_COUNTER
    );
    assert!(control::PRIORITY_FAIRNESS_BASE > control::PRIORITY_STARVATION_COUNTER);
    assert!(control::OBSERVABLE_BASE >= 160);
}
