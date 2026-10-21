use super::*;

#[test]
fn telemetry_recommendation_promotes_hot_opcodes_and_requeue_pressure() {
    let mut control = Megakernel::try_encode_control(false, 1, 0).unwrap();
    for opcode_idx in 0..8u32 {
        let off = ((control::METRICS_BASE + opcode_idx) as usize) * 4;
        control[off..off + 4].copy_from_slice(&1u32.to_le_bytes());
    }
    let mut ring = Megakernel::try_encode_empty_ring(4).unwrap();
    let status_off = (STATUS_WORD as usize) * 4;
    ring[status_off..status_off + 4].copy_from_slice(&slot::REQUEUE.to_le_bytes());
    let telemetry = RingTelemetry::decode(&control, &ring);
    let rec = telemetry
        .recommend_launch(MegakernelLaunchRequest::direct(4096, 64, 256))
        .expect("Fix: telemetry launch recommendation must accept valid limits");
    assert_eq!(rec.execution_mode, MegakernelExecutionMode::Jit);
    assert!(rec.promote_hot_opcodes);
    assert!(rec.age_priority_work);
    assert_eq!(telemetry.priority_accounting().requeue_count, 1);
}

#[test]
fn runtime_counters_report_queue_idle_fairness_and_drain() {
    let mut control = Megakernel::try_encode_control(false, 7, 0).unwrap();
    let tenant_a = (control::TENANT_FAIRNESS_BASE as usize) * 4;
    let tenant_b = ((control::TENANT_FAIRNESS_BASE + 1) as usize) * 4;
    let priority_a = (control::PRIORITY_FAIRNESS_BASE as usize) * 4;
    let done_count = (control::DONE_COUNT as usize) * 4;
    control[done_count..done_count + 4].copy_from_slice(&7u32.to_le_bytes());
    control[tenant_a..tenant_a + 4].copy_from_slice(&3u32.to_le_bytes());
    control[tenant_b..tenant_b + 4].copy_from_slice(&9u32.to_le_bytes());
    control[priority_a..priority_a + 4].copy_from_slice(&5u32.to_le_bytes());

    let mut ring = Megakernel::try_encode_empty_ring(4).unwrap();
    Megakernel::publish_slot(&mut ring, 2, 11, opcode::ATOMIC_ADD, &[1, 2, 3]).unwrap();
    let slot_status =
        |slot_idx: usize| slot_idx * (SLOT_WORDS as usize) * 4 + (STATUS_WORD as usize) * 4;
    let requeue = slot_status(0);
    ring[requeue..requeue + 4].copy_from_slice(&slot::REQUEUE.to_le_bytes());
    let done = slot_status(1);
    ring[done..done + 4].copy_from_slice(&slot::DONE.to_le_bytes());

    let counters = RingTelemetry::decode(&control, &ring).runtime_counters();
    assert_eq!(counters.total_slots, 4);
    assert_eq!(counters.queue_depth, 2);
    assert_eq!(counters.gpu_idle_slots, 1);
    assert_eq!(counters.gpu_idle_ppm, 250_000);
    assert_eq!(counters.frontier_density_bps, 5_000);
    assert_eq!(counters.occupancy_proxy_bps, 7_500);
    assert_eq!(counters.drained_slots, 7);
    assert_eq!(counters.unreclaimed_done_slots, 1);
    assert_eq!(counters.tenant_fairness_total, 12);
    assert_eq!(counters.tenant_fairness_skew, 6);
    assert_eq!(counters.priority_fairness_total, 5);
    assert_eq!(counters.requeue_slots, 1);
}

#[test]
fn telemetry_launch_recommendation_uses_frontier_density_for_topology() {
    let control = Megakernel::try_encode_control(false, 1, 0).unwrap();
    let mut ring = Megakernel::try_encode_empty_ring(8).unwrap();
    Megakernel::publish_slot(&mut ring, 0, 7, opcode::ATOMIC_ADD, &[1, 2, 3]).unwrap();
    Megakernel::publish_slot(&mut ring, 1, 7, opcode::ATOMIC_ADD, &[1, 2, 3]).unwrap();
    Megakernel::publish_slot(&mut ring, 2, 7, opcode::ATOMIC_ADD, &[1, 2, 3]).unwrap();
    Megakernel::publish_slot(&mut ring, 3, 7, opcode::ATOMIC_ADD, &[1, 2, 3]).unwrap();

    let telemetry = RingTelemetry::decode(&control, &ring);
    let rec = telemetry
        .recommend_launch(MegakernelLaunchRequest::direct(8, 64, 256))
        .expect("Fix: telemetry launch recommendation must accept valid limits");

    assert_eq!(telemetry.runtime_counters().frontier_density_bps, 5_000);
    assert_eq!(rec.topology, MegakernelDispatchTopology::DenseFrontier);
}

#[test]
fn telemetry_decode_into_reports_caller_owned_capacity_evidence() {
    let control = Megakernel::try_encode_control(false, 1, 0).unwrap();
    let mut ring = Megakernel::try_encode_empty_ring(2).unwrap();
    Megakernel::publish_slot(&mut ring, 0, 7, opcode::ATOMIC_ADD, &[11, 0, 0]).unwrap();
    Megakernel::publish_slot(&mut ring, 1, 7, opcode::ATOMIC_ADD, &[11, 1, 0]).unwrap();
    let mut telemetry = RingTelemetry::default();
    let mut scratch = TelemetryDecodeScratch::new();

    RingTelemetry::try_decode_with_window_opcodes_into(
        &control,
        &ring,
        &[opcode::ATOMIC_ADD, opcode::ATOMIC_ADD],
        &mut telemetry,
        &mut scratch,
    )
    .expect("Fix: strict telemetry decode should accept aligned ring/control snapshots");
    let evidence = telemetry.decode_capacity_evidence(&scratch);

    assert_eq!(
        evidence.schema_version,
        TELEMETRY_DECODE_CAPACITY_SCHEMA_VERSION
    );
    assert_eq!(evidence.decoded_slot_count, 2);
    assert!(evidence.slot_output_capacity >= 2);
    assert_eq!(evidence.decoded_window_count, 1);
    assert!(evidence.window_output_capacity >= 1);
    assert!(evidence.window_opcode_scratch_capacity >= 2);
    assert!(evidence.window_accumulator_scratch_capacity >= 2);
    assert!(evidence.uses_caller_owned_scratch);
    assert!(evidence.is_complete());
}

#[test]
fn launch_recommendation_rejects_route_window_demand_overflow_without_panic() {
    let telemetry = RingTelemetry {
        windows: vec![WindowTelemetry {
            ticket: 1,
            tenant_id: 1,
            opcode: opcode::ATOMIC_ADD,
            required_slots: u32::MAX,
            lookahead_slots: 1,
            published: 0,
            claimed: 0,
            done: 0,
            wait_io: 0,
            yield_count: 0,
            requeue: 0,
            fault: 0,
        }],
        ..RingTelemetry::default()
    };

    let error = telemetry
        .recommend_launch(MegakernelLaunchRequest::direct(4096, 64, 256))
        .expect_err("Fix: route-window demand overflow must not panic during launch recommendation");
    assert!(
        error.to_string().contains("route-window slot demand overflowed"),
        "Fix: launch recommendation overflow errors must identify route-window demand: {error}"
    );
}

#[test]
fn metrics_and_observable_regions_remain_non_overlapping_in_snapshot() {
    let mut control = Megakernel::try_encode_control(false, 1, 4).unwrap();
    let metric_off = (control::METRICS_BASE as usize) * 4;
    control[metric_off..metric_off + 4].copy_from_slice(&0xAA55AA55u32.to_le_bytes());
    let observable_off = (control::OBSERVABLE_BASE as usize) * 4;
    control[observable_off..observable_off + 4].copy_from_slice(&0x11223344u32.to_le_bytes());

    let ring = Megakernel::try_encode_empty_ring(1).unwrap();
    let telemetry = RingTelemetry::decode(&control, &ring);
    assert!(
        telemetry.control.metrics.contains(&(0, 0xAA55AA55)),
        "metrics decoder must preserve metric slot 0 value"
    );
    assert_eq!(
        Megakernel::read_observable(&control, 0),
        0x11223344,
        "observable reads must not alias metric region words"
    );
}
