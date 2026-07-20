use super::*;

#[test]
fn decode_window_opcodes_groups_ticketed_slots() {
    let control = Megakernel::try_encode_control(false, 1, 0).unwrap();
    let mut ring = Megakernel::try_encode_empty_ring(4).unwrap();
    let window_opcode = 0xF101;
    Megakernel::publish_slot(
        &mut ring,
        0,
        3,
        window_opcode,
        &[7, WindowClass::Required.into_wire(), 42],
    )
    .unwrap();
    Megakernel::publish_slot(
        &mut ring,
        1,
        3,
        window_opcode,
        &[7, WindowClass::Lookahead.into_wire(), 99],
    )
    .unwrap();
    Megakernel::publish_slot(
        &mut ring,
        2,
        3,
        window_opcode,
        &[7, WindowClass::Required.into_wire(), 123],
    )
    .unwrap();
    let telemetry = RingTelemetry::decode_with_window_opcodes(&control, &ring, &[window_opcode]);
    assert_eq!(telemetry.windows.len(), 1);
    let window = &telemetry.windows[0];
    assert_eq!(window.ticket, 7);
    assert_eq!(window.tenant_id, 3);
    assert_eq!(window.opcode, window_opcode);
    assert_eq!(window.required_slots, 2);
    assert_eq!(window.lookahead_slots, 1);
    assert_eq!(window.published, 3);
    assert!(window.is_active());
    assert_eq!(telemetry.active_windows().len(), 1);
    assert_eq!(telemetry.active_slots_for_opcode(window_opcode).len(), 3);
    assert_eq!(
        telemetry
            .active_slots_for_opcode_iter(window_opcode)
            .count(),
        3
    );
    let mut active_windows = Vec::with_capacity(4);
    let mut active_slots = Vec::with_capacity(4);
    let windows_ptr = active_windows.as_ptr();
    let slots_ptr = active_slots.as_ptr();
    telemetry.active_windows_into(&mut active_windows);
    telemetry.active_slots_for_opcode_into(window_opcode, &mut active_slots);
    assert_eq!(active_windows.len(), 1);
    assert_eq!(active_slots.len(), 3);
    assert_eq!(active_windows.as_ptr(), windows_ptr);
    assert_eq!(active_slots.as_ptr(), slots_ptr);
}

#[test]
fn decode_window_opcodes_matches_dense_bitmap_opcodes() {
    let control = Megakernel::try_encode_control(false, 1, 0).unwrap();
    let mut ring = Megakernel::try_encode_empty_ring(4).unwrap();
    let first_window_opcode = 3u32;
    let second_window_opcode = 9u32;
    Megakernel::publish_slot(
        &mut ring,
        0,
        3,
        first_window_opcode,
        &[11, WindowClass::Required.into_wire(), 42],
    )
    .unwrap();
    Megakernel::publish_slot(
        &mut ring,
        1,
        3,
        second_window_opcode,
        &[11, WindowClass::Lookahead.into_wire(), 99],
    )
    .unwrap();
    let telemetry = RingTelemetry::decode_with_window_opcodes(
        &control,
        &ring,
        &[first_window_opcode, second_window_opcode],
    );
    assert_eq!(telemetry.windows.len(), 2);
    assert_eq!(
        telemetry.active_slots_for_opcode(first_window_opcode).len(),
        1
    );
    assert_eq!(
        telemetry.active_slots_for_opcode(second_window_opcode).len(),
        1
    );
}

#[test]
fn decode_with_scratch_reuses_snapshot_storage() {
    let control = Megakernel::try_encode_control(false, 1, 0).unwrap();
    let mut ring = Megakernel::try_encode_empty_ring(4).unwrap();
    let window_opcode = 0xF101;
    Megakernel::publish_slot(
        &mut ring,
        0,
        3,
        window_opcode,
        &[7, WindowClass::Required.into_wire(), 42],
    )
    .unwrap();
    Megakernel::publish_slot(
        &mut ring,
        1,
        3,
        window_opcode,
        &[7, WindowClass::Lookahead.into_wire(), 99],
    )
    .unwrap();

    let mut telemetry = RingTelemetry {
        control: ControlSnapshot {
            metrics: Vec::with_capacity(control::METRICS_SLOTS as usize),
            tenant_fairness: Vec::with_capacity(control::TENANT_FAIRNESS_SLOTS as usize),
            priority_fairness: Vec::with_capacity(control::PRIORITY_FAIRNESS_SLOTS as usize),
            ..ControlSnapshot::default()
        },
        slots: Vec::with_capacity(4),
        windows: Vec::with_capacity(1),
        ..RingTelemetry::default()
    };
    let mut scratch = TelemetryDecodeScratch::new();

    RingTelemetry::decode_with_window_opcodes_into(
        &control,
        &ring,
        &[window_opcode],
        &mut telemetry,
        &mut scratch,
    );
    let metrics_ptr = telemetry.control.metrics.as_ptr();
    let tenant_ptr = telemetry.control.tenant_fairness.as_ptr();
    let priority_ptr = telemetry.control.priority_fairness.as_ptr();
    let slots_ptr = telemetry.slots.as_ptr();
    let windows_ptr = telemetry.windows.as_ptr();

    RingTelemetry::try_decode_with_window_opcodes_into(
        &control,
        &ring,
        &[window_opcode],
        &mut telemetry,
        &mut scratch,
    )
    .expect("Fix: scratch telemetry decode must accept valid control/ring snapshots");

    assert_eq!(telemetry.control.metrics.as_ptr(), metrics_ptr);
    assert_eq!(telemetry.control.tenant_fairness.as_ptr(), tenant_ptr);
    assert_eq!(telemetry.control.priority_fairness.as_ptr(), priority_ptr);
    assert_eq!(telemetry.slots.as_ptr(), slots_ptr);
    assert_eq!(telemetry.windows.as_ptr(), windows_ptr);
    assert_eq!(telemetry.windows.len(), 1);
    assert_eq!(telemetry.slots.len(), 4);
}

#[test]
fn decode_sorted_window_opcodes_reuses_scratch_without_resort_growth() {
    let control = Megakernel::try_encode_control(false, 1, 0).unwrap();
    let mut ring = Megakernel::try_encode_empty_ring(4).unwrap();
    let first_opcode = 0xF101;
    let second_opcode = 0xF102;
    Megakernel::publish_slot(
        &mut ring,
        0,
        3,
        first_opcode,
        &[7, WindowClass::Required.into_wire(), 42],
    )
    .unwrap();
    Megakernel::publish_slot(
        &mut ring,
        1,
        3,
        second_opcode,
        &[9, WindowClass::Lookahead.into_wire(), 99],
    )
    .unwrap();

    let mut telemetry = RingTelemetry::default();
    let mut scratch = TelemetryDecodeScratch::new();
    let sorted_unique = [first_opcode, second_opcode];
    RingTelemetry::decode_with_window_opcodes_into(
        &control,
        &ring,
        &sorted_unique,
        &mut telemetry,
        &mut scratch,
    );
    let opcode_capacity = scratch.window_opcodes.capacity();
    let window_capacity = scratch.windows.capacity();

    RingTelemetry::decode_with_window_opcodes_into(
        &control,
        &ring,
        &sorted_unique,
        &mut telemetry,
        &mut scratch,
    );

    assert_eq!(scratch.window_opcodes.capacity(), opcode_capacity);
    assert_eq!(scratch.windows.capacity(), window_capacity);
    assert_eq!(telemetry.windows.len(), 2);
    assert!(
        telemetry
            .windows
            .iter()
            .any(|window| window.opcode == first_opcode && window.ticket == 7)
    );
    assert!(
        telemetry
            .windows
            .iter()
            .any(|window| window.opcode == second_opcode && window.ticket == 9)
    );
}

#[test]
fn terminal_window_is_not_reported_as_active() {
    let control = Megakernel::try_encode_control(false, 1, 0).unwrap();
    let mut ring = Megakernel::try_encode_empty_ring(2).unwrap();
    let window_opcode = 0xF101;
    Megakernel::publish_slot(
        &mut ring,
        0,
        3,
        window_opcode,
        &[9, WindowClass::Required.into_wire(), 42],
    )
    .unwrap();
    Megakernel::publish_slot(
        &mut ring,
        1,
        3,
        window_opcode,
        &[9, WindowClass::Lookahead.into_wire(), 99],
    )
    .unwrap();
    let mut mark_done = |slot_idx: usize| {
        let start = slot_idx * (SLOT_WORDS as usize) * 4 + (STATUS_WORD as usize) * 4;
        ring[start..start + 4].copy_from_slice(&slot::DONE.to_le_bytes());
    };
    mark_done(0);
    mark_done(1);
    let telemetry = RingTelemetry::decode_with_window_opcodes(&control, &ring, &[window_opcode]);
    assert_eq!(telemetry.windows.len(), 1);
    assert!(!telemetry.windows[0].is_active());
    assert!(telemetry.active_windows().is_empty());
    assert!(telemetry.active_slots_for_opcode(window_opcode).is_empty());
}
