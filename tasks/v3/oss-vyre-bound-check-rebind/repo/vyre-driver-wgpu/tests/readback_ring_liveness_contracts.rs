//! Readback-ring state behavior and liveness contracts.
//!
//! Guarantees:
//! - Slot lifecycle: Free → Pending → Ready → Free.
//! - Freed slots become reusable after readiness polling and collection.
//! - Returned indices cycle modulo ring capacity.
//! - Data round-trips byte-for-byte through the staging slot.
//! - Extreme requested sizes are clamped and remain functional.
//!
//! Failure-liveness: if a `map_async` callback receives an error, the slot is
//! reset to Free and the caller receives a structured error. Integration tests
//! cover the normal liveness path without faking GPU absence or forcing device loss.

mod common;
use common::acquire_live_backend as live_backend;

use vyre_driver_wgpu::runtime::readback_ring::{ReadbackRing, ReadbackRingSet};

fn make_source_buffer(device: &wgpu::Device, queue: &wgpu::Queue, data: &[u8]) -> wgpu::Buffer {
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("readback ring test source"),
        size: aligned_copy_len(data.len() as u64),
        usage: wgpu::BufferUsages::COPY_SRC | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    if !data.is_empty() {
        let mut padded = vec![0; aligned_copy_len(data.len() as u64) as usize];
        padded[..data.len()].copy_from_slice(data);
        queue.write_buffer(&buffer, 0, &padded);
    }
    buffer
}

fn aligned_copy_len(byte_len: u64) -> u64 {
    if byte_len == 0 {
        4
    } else {
        byte_len
            .checked_add(3)
            .map(|len| len & !3)
            .expect("Fix: test readback length must not overflow alignment")
    }
}

fn wait_for_device(device: &wgpu::Device) {
    let _ = device.poll(wgpu::PollType::wait());
}

// ------------------------------------------------------------------
// 1. Live GPU required
// ------------------------------------------------------------------

#[test]
fn live_gpu_required() {
    let _backend = live_backend();
}

// ------------------------------------------------------------------
// 2. Round-trip data integrity
// ------------------------------------------------------------------

#[test]
fn round_trip_preserves_bytes() {
    let backend = live_backend();
    let dq = backend.device_queue();
    let device = &dq.0;
    let queue = &dq.1;

    let data: Vec<u8> = (0..64u8).collect();
    let src = make_source_buffer(device, queue, &data);
    let ring = ReadbackRing::new(device, 4, data.len() as u64).unwrap();

    let idx = ring
        .submit_readback(device, queue, &src, 0, data.len() as u64)
        .unwrap();

    wait_for_device(device);
    let collected = ring
        .collect_slot(device, idx)
        .expect("slot collection must not fail")
        .expect("slot must be ready after poll(Wait)");
    assert_eq!(collected, data, "readback data must match source exactly");
}

#[test]
fn collect_into_reuses_destination_capacity_and_trims_to_request() {
    let backend = live_backend();
    let dq = backend.device_queue();
    let device = &dq.0;
    let queue = &dq.1;

    let data: Vec<u8> = (0..20u8).collect();
    let src = make_source_buffer(device, queue, &data);
    let ring = ReadbackRing::new(device, 4, 64).unwrap();
    let mut out = Vec::with_capacity(128);
    let original_capacity = out.capacity();

    let idx = ring
        .submit_readback(device, queue, &src, 0, data.len() as u64)
        .unwrap();

    wait_for_device(device);
    let len = ring
        .collect_slot_into(device, idx, &mut out)
        .expect("slot collection must not fail")
        .expect("slot must be ready after poll(Wait)");

    assert_eq!(len, data.len(), "reported readback length must be exact");
    assert_eq!(out, data, "readback bytes must match requested payload");
    assert_eq!(
        out.capacity(),
        original_capacity,
        "collect_slot_into must reuse caller-owned allocation"
    );
}

#[test]
fn recorded_copy_ticket_collects_from_dispatch_encoder_slot() {
    let backend = live_backend();
    let dq = backend.device_queue();
    let device = &dq.0;
    let queue = &dq.1;

    let data: Vec<u8> = (0..31u8).collect();
    let src = make_source_buffer(device, queue, &data);
    let ring = ReadbackRing::new(device, 4, data.len() as u64).unwrap();
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("readback ring recorded-copy test"),
    });
    let ticket = ring
        .record_copy(device, &mut encoder, &src, 0, data.len() as u64)
        .expect("Fix: recording into a free readback-ring slot must succeed");

    queue.submit(std::iter::once(encoder.finish()));
    let (receiver, ready) = ring
        .arm_ticket(&ticket)
        .expect("Fix: arming a valid readback-ring ticket must succeed");
    wait_for_device(device);
    assert!(
        ready.load(std::sync::atomic::Ordering::Acquire),
        "readback-ring ticket must report ready after device wait"
    );
    receiver
        .recv_timeout(std::time::Duration::from_secs(30))
        .expect("Fix: readback-ring map callback must fire after device wait")
        .expect("Fix: readback-ring recorded copy must map successfully");

    let seen = ring
        .with_mapped_ticket(&ticket, |bytes| {
            assert_eq!(bytes, data, "recorded-copy ticket must expose exact bytes");
            Ok(bytes.len())
        })
        .expect("Fix: ready readback-ring ticket must collect successfully");
    assert_eq!(seen, data.len(), "visitor return value must be preserved");
}

#[test]
fn submit_rejects_readback_larger_than_slot_capacity() {
    let backend = live_backend();
    let dq = backend.device_queue();
    let device = &dq.0;
    let queue = &dq.1;

    let payload = vec![0xA5u8; 32];
    let src = make_source_buffer(device, queue, &payload);
    let ring = ReadbackRing::new(device, 4, 16).unwrap();

    let err = ring
        .submit_readback(device, queue, &src, 0, payload.len() as u64)
        .expect_err("oversized readback must fail before recording invalid GPU copy");
    assert!(
        err.to_string().contains("exceeds ring slot capacity"),
        "error must explain the slot-capacity contract: {err}"
    );
}

#[test]
fn collect_slot_into_reuses_destination_and_preserves_unaligned_len() {
    let backend = live_backend();
    let dq = backend.device_queue();
    let device = &dq.0;
    let queue = &dq.1;

    let data = [0xAAu8, 0xBB, 0xCC];
    let src = make_source_buffer(device, queue, &data);
    let ring = ReadbackRing::new(device, 4, data.len() as u64).unwrap();
    let idx = ring
        .submit_readback(device, queue, &src, 0, data.len() as u64)
        .unwrap();

    wait_for_device(device);
    let mut out = Vec::with_capacity(64);
    out.extend_from_slice(&[0x11; 32]);
    let capacity = out.capacity();
    let copied = ring
        .collect_slot_into(device, idx, &mut out)
        .expect("slot collection into caller buffer must not fail")
        .expect("slot must be ready after poll(Wait)");

    assert_eq!(copied, data.len(), "reported byte count must be exact");
    assert_eq!(out, data, "unaligned readback must not expose padding");
    assert_eq!(
        out.capacity(),
        capacity,
        "collect_slot_into must reuse caller-owned allocation when capacity is sufficient"
    );
}

// ------------------------------------------------------------------
// 3. Slot state lifecycle
// ------------------------------------------------------------------

#[test]
fn slot_state_lifecycle() {
    let backend = live_backend();
    let dq = backend.device_queue();
    let device = &dq.0;
    let queue = &dq.1;

    let data = [0x01u8, 0x02, 0x03, 0x04];
    let src = make_source_buffer(device, queue, &data);
    let ring = ReadbackRing::new(device, 4, data.len() as u64).unwrap();

    let idx = ring
        .submit_readback(device, queue, &src, 0, data.len() as u64)
        .unwrap();

    assert!(
        ring.collect_slot(device, idx)
            .expect("pending slot collection must not fail")
            .is_none(),
        "slot must be pending immediately after submit"
    );

    wait_for_device(device);
    let collected = ring
        .collect_slot(device, idx)
        .expect("slot collection must not fail")
        .expect("slot must be ready after poll(Wait)");
    assert_eq!(collected, data, "collected data must match source");

    assert!(
        ring.collect_slot(device, idx)
            .expect("free slot collection must not fail")
            .is_none(),
        "slot must be free after successful collection"
    );
}

// ------------------------------------------------------------------
// 4. Indices cycle through ring capacity
// ------------------------------------------------------------------

#[test]
fn indices_cycle_through_ring_capacity() {
    let backend = live_backend();
    let dq = backend.device_queue();
    let device = &dq.0;
    let queue = &dq.1;

    let ring = ReadbackRing::new(device, 4, 16).unwrap();
    let mut indices = Vec::with_capacity(4);

    for i in 0..4 {
        let payload = vec![i as u8; 16];
        let src = make_source_buffer(device, queue, &payload);
        let idx = ring
            .submit_readback(device, queue, &src, 0, payload.len() as u64)
            .unwrap();
        indices.push(idx);
    }

    assert_eq!(indices, vec![0, 1, 2, 3], "indices must cycle 0..capacity");

    // Free slot 0 so it can be reused.
    wait_for_device(device);
    let _ = ring
        .collect_slot(device, 0)
        .expect("slot collection must not fail")
        .expect("slot 0 must be ready after poll");

    let payload = vec![0xABu8; 16];
    let src = make_source_buffer(device, queue, &payload);
    let idx = ring
        .submit_readback(device, queue, &src, 0, payload.len() as u64)
        .unwrap();
    assert_eq!(idx, 0, "next submit must reuse the freed slot 0");
}

// ------------------------------------------------------------------
// 5. Minimum ring size is enforced
// ------------------------------------------------------------------

#[test]
fn minimum_ring_size_is_enforced() {
    let backend = live_backend();
    let dq = backend.device_queue();
    let device = &dq.0;
    let queue = &dq.1;

    // Request size 1, which must be clamped to MIN_RING_SIZE (2).
    let ring = ReadbackRing::new(device, 1, 16).unwrap();

    let payload_a = vec![0xAAu8; 16];
    let payload_b = vec![0xBBu8; 16];
    let src_a = make_source_buffer(device, queue, &payload_a);
    let src_b = make_source_buffer(device, queue, &payload_b);

    let idx_a = ring.submit_readback(device, queue, &src_a, 0, 16).unwrap();
    let idx_b = ring.submit_readback(device, queue, &src_b, 0, 16).unwrap();

    // If the ring were size 1, idx_b would attempt to reuse slot 0 while it
    // is still mapped, causing a wgpu validation error. The fact that both
    // submits succeed proves the constructor clamped to at least 2 slots.
    assert_ne!(
        idx_a, idx_b,
        "minimum clamping must provide at least 2 distinct slots"
    );

    wait_for_device(device);
    let data_a = ring
        .collect_slot(device, idx_a)
        .expect("slot a collection must not fail")
        .expect("slot a must be ready");
    let data_b = ring
        .collect_slot(device, idx_b)
        .expect("slot b collection must not fail")
        .expect("slot b must be ready");
    assert_eq!(data_a, payload_a, "data mismatch for slot a");
    assert_eq!(data_b, payload_b, "data mismatch for slot b");
}

// ------------------------------------------------------------------
// 6. Concurrent submits are independent
// ------------------------------------------------------------------

#[test]
fn concurrent_submits_are_independent() {
    let backend = live_backend();
    let dq = backend.device_queue();
    let device = &dq.0;
    let queue = &dq.1;

    let ring = ReadbackRing::new(device, 8, 16).unwrap();
    let payloads: Vec<Vec<u8>> = (0..8)
        .map(|i| {
            let mut v = vec![0u8; 16];
            v[0] = i as u8;
            v
        })
        .collect();
    let mut indices = Vec::with_capacity(8);

    for payload in &payloads {
        let src = make_source_buffer(device, queue, payload);
        let idx = ring
            .submit_readback(device, queue, &src, 0, payload.len() as u64)
            .unwrap();
        indices.push(idx);
    }

    wait_for_device(device);
    for (i, payload) in payloads.iter().enumerate() {
        let idx = indices[i];
        let data = ring
            .collect_slot(device, idx)
            .expect("slot collection must not fail")
            .unwrap_or_else(|| panic!("slot {idx} (dispatch {i}) must be ready"));
        assert_eq!(data, *payload, "payload {i} must round-trip intact");
    }
}

#[test]
fn readback_ring_slots_from_env_is_clamped_and_defaulted() {
    let invalid = ReadbackRingSet::with_requested_slots(Some("does_not_fit"));
    assert_eq!(
        invalid.slots_per_ring(),
        256,
        "invalid env values must default to 256 slots"
    );

    let low = ReadbackRingSet::with_requested_slots(Some("1"));
    assert_eq!(
        low.slots_per_ring(),
        2,
        "zero/low values must clamp to 2 slots"
    );

    let high = ReadbackRingSet::with_requested_slots(Some("1024"));
    assert_eq!(
        high.slots_per_ring(),
        256,
        "over-large values must clamp to 256 slots"
    );

    let default = ReadbackRingSet::with_requested_slots(None);
    assert_eq!(
        default.slots_per_ring(),
        256,
        "unset env defaults to 256 slots"
    );
}

#[test]
fn ring_set_reuses_rings_for_capacity_classes() {
    let backend = live_backend();
    let dq = backend.device_queue();
    let device = &dq.0;

    let ring_set = ReadbackRingSet::new();
    let ring_small_a = ring_set.ring_for(device, 16).unwrap();
    let ring_small_b = ring_set.ring_for(device, 32).unwrap();
    assert!(
        std::sync::Arc::ptr_eq(&ring_small_a, &ring_small_b),
        "readback sizes within the same capacity class must reuse the same ring"
    );

    let ring_large = ring_set.ring_for(device, 4097).unwrap();
    assert!(
        !std::sync::Arc::ptr_eq(&ring_small_a, &ring_large),
        "readback sizes that land in different capacity classes must allocate separate rings"
    );
}

#[test]
fn existing_ring_for_matches_capacity_classed_lookups() {
    let backend = live_backend();
    let dq = backend.device_queue();
    let device = &dq.0;

    let ring_set = ReadbackRingSet::new();
    let ring = ring_set.ring_for(device, 16).unwrap();
    let same = ring_set.existing_ring_for(16).unwrap();
    let miss = ring_set.existing_ring_for(4097).unwrap();

    assert!(
        same.is_some(),
        "requested capacity class must be visible via existing_ring_for immediately after creation"
    );
    assert!(
        std::sync::Arc::ptr_eq(&ring, &same.expect("ring should exist")),
        "existing_ring_for should return the identical ring instance for the same size class"
    );
    assert!(
        miss.is_none(),
        "mismatched capacity class should not report an unrelated ring"
    );
}

// ------------------------------------------------------------------
// 7. Extreme ring sizes are clamped and functional
// ------------------------------------------------------------------

#[test]
fn extreme_ring_sizes_are_clamped_and_functional() {
    let backend = live_backend();
    let dq = backend.device_queue();
    let device = &dq.0;
    let queue = &dq.1;

    for &requested_size in &[0usize, 1, 256, 300] {
        let ring = ReadbackRing::new(device, requested_size, 16).unwrap();
        let payload = vec![0xCDu8; 16];
        let src = make_source_buffer(device, queue, &payload);
        let idx = ring
            .submit_readback(device, queue, &src, 0, payload.len() as u64)
            .unwrap();

        wait_for_device(device);
        let data = ring
            .collect_slot(device, idx)
            .expect("slot collection must not fail")
            .unwrap_or_else(|| panic!("readback must succeed for requested_size={requested_size}"));
        assert_eq!(
            data, payload,
            "data mismatch for requested_size={requested_size}"
        );
    }
}
