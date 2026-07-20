use super::*;

#[test]
fn sketch_into_reuses_counter_storage() {
    let control = Megakernel::try_encode_control(false, 1, 0).unwrap();
    let mut ring = Megakernel::try_encode_empty_ring(4).unwrap();
    Megakernel::publish_slot(&mut ring, 1, 9, opcode::ATOMIC_ADD, &[5, 7, 11]).unwrap();
    let telemetry = RingTelemetry::decode(&control, &ring);
    let mut scratch = SketchTelemetryScratch::new(3, 16).unwrap();

    telemetry.sketch_into(3, 16, &mut scratch).unwrap();
    let ring_ptr = scratch.ring_opcode.counters().as_ptr();
    let active_ptr = scratch.active_opcode.counters().as_ptr();
    let tenant_ptr = scratch.tenant.counters().as_ptr();
    let status_ptr = scratch.status.counters().as_ptr();
    let metrics_ptr = scratch.dispatch_metrics.counters().as_ptr();
    let first_active = scratch.active_slots;

    telemetry.sketch_into(3, 16, &mut scratch).unwrap();

    assert_eq!(scratch.ring_opcode.counters().as_ptr(), ring_ptr);
    assert_eq!(scratch.active_opcode.counters().as_ptr(), active_ptr);
    assert_eq!(scratch.tenant.counters().as_ptr(), tenant_ptr);
    assert_eq!(scratch.status.counters().as_ptr(), status_ptr);
    assert_eq!(scratch.dispatch_metrics.counters().as_ptr(), metrics_ptr);
    assert_eq!(scratch.total_slots, 4);
    assert_eq!(scratch.active_slots, first_active);
    assert!(scratch.ring_opcode.estimate(opcode::ATOMIC_ADD) >= 1);
}

#[test]
fn watchdog_health_flags_active_queue_without_drain_progress() {
    let mut previous_control = Megakernel::try_encode_control(false, 7, 0).unwrap();
    let done_count = (control::DONE_COUNT as usize) * 4;
    previous_control[done_count..done_count + 4].copy_from_slice(&7u32.to_le_bytes());
    let previous_ring = Megakernel::try_encode_empty_ring(2).unwrap();
    let previous = RingTelemetry::decode(&previous_control, &previous_ring);

    let mut current_control = previous_control.clone();
    let mut current_ring = Megakernel::try_encode_empty_ring(2).unwrap();
    Megakernel::publish_slot(&mut current_ring, 0, 7, opcode::ATOMIC_ADD, &[1, 2, 3]).unwrap();
    let stalled = RingTelemetry::decode(&current_control, &current_ring).health_since(&previous);
    assert_eq!(stalled.done_delta, 0);
    assert_eq!(stalled.queue_depth, 1);
    assert!(stalled.suspected_stall);

    current_control[done_count..done_count + 4].copy_from_slice(&9u32.to_le_bytes());
    let progressed = RingTelemetry::decode(&current_control, &current_ring).health_since(&previous);
    assert_eq!(progressed.done_delta, 2);
    assert!(!progressed.suspected_stall);
}

#[test]
fn watchdog_try_health_rejects_done_counter_wrap_without_panic() {
    let mut previous_control = Megakernel::try_encode_control(false, 7, 0).unwrap();
    let done_count = (control::DONE_COUNT as usize) * 4;
    previous_control[done_count..done_count + 4].copy_from_slice(&9u32.to_le_bytes());
    let previous_ring = Megakernel::try_encode_empty_ring(2).unwrap();
    let previous = RingTelemetry::decode(&previous_control, &previous_ring);

    let mut current_control = previous_control.clone();
    current_control[done_count..done_count + 4].copy_from_slice(&7u32.to_le_bytes());
    let current_ring = Megakernel::try_encode_empty_ring(2).unwrap();
    let current = RingTelemetry::decode(&current_control, &current_ring);

    let error = current
        .try_health_since(&previous)
        .expect_err("Fix: wrapped done counters must return structured watchdog errors");
    assert!(
        error.to_string().contains("moved backwards"),
        "Fix: watchdog wrap errors must identify the counter relationship: {error}"
    );
}
