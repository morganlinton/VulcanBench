//! Contract tests for compact megakernel sketch telemetry.
#![cfg(feature = "legacy-infallible")]

use vyre_runtime::megakernel::{
    control, opcode, slot, CountMinSketch, Megakernel, RingTelemetry, SLOT_WORDS, STATUS_WORD,
};

fn write_slot_status(ring: &mut [u8], slot_idx: usize, status: u32) {
    let off = slot_idx * (SLOT_WORDS as usize) * 4 + (STATUS_WORD as usize) * 4;
    ring[off..off + 4].copy_from_slice(&status.to_le_bytes());
}

fn write_control_word(control: &mut [u8], word_idx: u32, value: u32) {
    let off = word_idx as usize * 4;
    control[off..off + 4].copy_from_slice(&value.to_le_bytes());
}

#[test]
fn count_min_sketch_estimates_inserted_keys_without_under_counting() {
    let mut sketch = CountMinSketch::new(4, 128).expect("valid sketch dimensions");
    sketch.add(7, 3);
    sketch.add(7, 2);
    sketch.add(99, 4);

    assert_eq!(sketch.depth(), 4);
    assert_eq!(sketch.width(), 128);
    assert!(
        sketch.estimate(7) >= 5,
        "Count-Min estimate must never under-count inserted key 7"
    );
    assert!(
        sketch.estimate(99) >= 4,
        "Count-Min estimate must never under-count inserted key 99"
    );
}

#[test]
fn count_min_sketch_rejects_invalid_dimensions() {
    let depth_err = CountMinSketch::new(0, 64).expect_err("depth zero is invalid");
    assert!(
        depth_err.to_string().contains("depth") || depth_err.to_string().contains("Fix:"),
        "depth-zero error: {depth_err}"
    );
    let width_err = CountMinSketch::new(4, 0).expect_err("width zero is invalid");
    assert!(
        width_err.to_string().contains("width") || width_err.to_string().contains("Fix:"),
        "width-zero error: {width_err}"
    );
    let overflow_err = CountMinSketch::new(usize::MAX, 2).expect_err("overflow is invalid");
    assert!(
        overflow_err.to_string().contains("overflow"),
        "dimension-overflow error: {overflow_err}"
    );
}

#[test]
fn count_min_sketch_merges_matching_shapes_and_rejects_shape_drift() {
    let mut left = CountMinSketch::new(4, 64).expect("valid left sketch");
    let mut right = CountMinSketch::new(4, 64).expect("valid right sketch");
    left.add(11, 2);
    right.add(11, 5);
    left.merge(&right).expect("matching sketches merge");
    assert!(
        left.estimate(11) >= 7,
        "merged sketch must preserve both contributing counts"
    );

    let drift = CountMinSketch::new(3, 64).expect("valid drift sketch");
    assert!(
        left.merge(&drift).is_err(),
        "shape drift must fail loudly instead of corrupting telemetry"
    );
}

#[test]
fn ring_telemetry_sketch_tracks_hot_opcodes_tenants_statuses_and_metrics() {
    let mut control = Megakernel::encode_control(false, 3, 0).unwrap();
    write_control_word(&mut control, control::METRICS_BASE + opcode::ATOMIC_ADD, 9);
    write_control_word(&mut control, control::METRICS_BASE + opcode::STORE_U32, 2);

    let mut ring = Megakernel::encode_empty_ring(6).unwrap();
    Megakernel::publish_slot(&mut ring, 0, 1, opcode::ATOMIC_ADD, &[1, 2, 3]).unwrap();
    Megakernel::publish_slot(&mut ring, 1, 1, opcode::ATOMIC_ADD, &[4, 5, 6]).unwrap();
    Megakernel::publish_slot(&mut ring, 2, 2, opcode::STORE_U32, &[7, 8, 9]).unwrap();
    Megakernel::publish_slot(&mut ring, 3, 2, opcode::DFA_STEP, &[10, 11, 12]).unwrap();
    write_slot_status(&mut ring, 1, slot::CLAIMED);
    write_slot_status(&mut ring, 3, slot::DONE);

    let telemetry = RingTelemetry::decode(&control, &ring);
    let sketch = telemetry
        .sketch(4, 128)
        .expect("valid telemetry sketch dimensions");

    assert_eq!(sketch.total_slots, 6);
    assert_eq!(sketch.active_slots, 3);
    assert!(
        sketch.ring_opcode.estimate(opcode::ATOMIC_ADD) >= 2,
        "ring opcode sketch must preserve both ATOMIC_ADD slots"
    );
    assert!(
        sketch.active_opcode.estimate(opcode::DFA_STEP) == 0,
        "terminal DONE slots must not be counted as active work"
    );
    assert!(
        sketch.tenant.estimate(1) >= 2,
        "tenant sketch must expose tenant pressure"
    );
    assert!(
        sketch.status.estimate(slot::PUBLISHED) >= 2,
        "status sketch must expose published work pressure"
    );
    assert!(
        sketch.dispatch_metrics.estimate(opcode::ATOMIC_ADD) >= 9,
        "dispatch metric sketch must include control-buffer opcode counters"
    );
}
