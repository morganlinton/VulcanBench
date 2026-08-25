use super::*;

#[test]
fn decode_empty_ring_counts_slots() {
    let control = Megakernel::try_encode_control(false, 1, 0).unwrap();
    let ring = Megakernel::try_encode_empty_ring(4).unwrap();
    let telemetry = RingTelemetry::decode(&control, &ring);
    assert_eq!(telemetry.occupancy.empty, 4);
    assert_eq!(telemetry.occupancy.published, 0);
    assert_eq!(telemetry.slots.len(), 4);
    assert!(telemetry.windows.is_empty());
}

#[test]
fn strict_decode_rejects_trailing_partial_slot() {
    let control = Megakernel::try_encode_control(false, 1, 0).unwrap();
    let mut ring = Megakernel::try_encode_empty_ring(1).unwrap();
    ring.push(0);
    let err = RingTelemetry::try_decode(&control, &ring)
        .expect_err("Fix: strict telemetry must reject malformed ring snapshots");
    assert!(matches!(err, PipelineError::Backend(_)));
}

#[test]
fn strict_decode_rejects_misaligned_control_snapshot() {
    let mut control = Megakernel::try_encode_control(false, 1, 0).unwrap();
    control.push(0xFF);
    let ring = Megakernel::try_encode_empty_ring(1).unwrap();
    let err = RingTelemetry::try_decode(&control, &ring)
        .expect_err("Fix: strict telemetry must reject malformed control snapshots");
    assert!(matches!(err, PipelineError::Backend(_)));
}

#[test]
fn control_try_decode_rejects_short_snapshot_without_panic() {
    let err = ControlSnapshot::try_decode(&[])
        .expect_err("Fix: strict control telemetry decode must reject missing control words");
    assert!(
        err.to_string().contains("control snapshot"),
        "Fix: strict control decode errors must explain the malformed control buffer: {err}"
    );
}

#[test]
fn strict_decode_into_rejects_trailing_partial_slot_without_mutating_output() {
    let control = Megakernel::try_encode_control(false, 1, 0).unwrap();
    let mut ring = Megakernel::try_encode_empty_ring(1).unwrap();
    ring.push(0);
    let mut telemetry = RingTelemetry::default();
    let mut scratch = TelemetryDecodeScratch::new();

    let err = RingTelemetry::try_decode_with_window_opcodes_into(
        &control,
        &ring,
        &[],
        &mut telemetry,
        &mut scratch,
    )
    .expect_err("Fix: strict telemetry decode_into must reject partial ring slots");

    assert!(
        err.to_string().contains("whole ring slots"),
        "Fix: strict telemetry decode_into errors must explain partial ring slots: {err}"
    );
    assert!(telemetry.slots.is_empty());
}

#[test]
fn decode_published_slot_reads_prefix() {
    let control = Megakernel::try_encode_control(false, 1, 0).unwrap();
    let mut ring = Megakernel::try_encode_empty_ring(2).unwrap();
    Megakernel::publish_slot(&mut ring, 1, 9, opcode::ATOMIC_ADD, &[5, 7, 11]).unwrap();
    let telemetry = RingTelemetry::decode(&control, &ring);
    let slot = &telemetry.slots[1];
    assert_eq!(slot.status, RingStatus::Published);
    assert_eq!(slot.tenant_id, 9);
    assert_eq!(slot.opcode, opcode::ATOMIC_ADD);
    assert_eq!(slot.args_prefix, [5, 7, 11]);
}
