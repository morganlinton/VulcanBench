use super::*;

#[test]
fn cuda_resident_queue_delta_enqueues_only_new_discoveries() {
    let backend = live_backend();
    let dispatcher = CudaOptimizerDispatcher::new(&backend);
    let node_count = 16u32;
    let active_queue_capacity = 8u32;
    let next_queue_capacity = 8u32;
    let active_queue = vec![0u32, 4, 0, 0, 0, 0, 0, 0];
    let active_len = vec![2u32];
    let edge_offsets = vec![0, 3, 3, 3, 3, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6];
    let edge_targets = vec![1, 2, 3, 2, 5, 6];
    let edge_kind_mask = vec![1, 1, 2, 1, 1, 1];
    let accumulator_seed = pack_nodes(&[0, 4], node_count);
    let (expected_accumulator, expected_next_queue, expected_next_len) =
        csr_queue_delta_enqueue_cpu(
            &active_queue,
            active_len[0],
            &edge_offsets,
            &edge_targets,
            &edge_kind_mask,
            &accumulator_seed,
            node_count,
            next_queue_capacity as usize,
            1,
        );

    let active_queue_handle = dispatcher
        .alloc_resident(active_queue.len() * std::mem::size_of::<u32>())
        .expect("Fix: active_queue resident allocation failed.");
    let active_len_handle = dispatcher
        .alloc_resident(std::mem::size_of::<u32>())
        .expect("Fix: active_len resident allocation failed.");
    let edge_offsets_handle = dispatcher
        .alloc_resident(edge_offsets.len() * std::mem::size_of::<u32>())
        .expect("Fix: edge_offsets resident allocation failed.");
    let edge_targets_handle = dispatcher
        .alloc_resident(edge_targets.len() * std::mem::size_of::<u32>())
        .expect("Fix: edge_targets resident allocation failed.");
    let edge_kind_handle = dispatcher
        .alloc_resident(edge_kind_mask.len() * std::mem::size_of::<u32>())
        .expect("Fix: edge_kind_mask resident allocation failed.");
    let accumulator_handle = dispatcher
        .alloc_resident(accumulator_seed.len() * std::mem::size_of::<u32>())
        .expect("Fix: accumulator resident allocation failed.");
    let next_queue_handle = dispatcher
        .alloc_resident(next_queue_capacity as usize * std::mem::size_of::<u32>())
        .expect("Fix: next_queue resident allocation failed.");
    let next_len_handle = dispatcher
        .alloc_resident(std::mem::size_of::<u32>())
        .expect("Fix: next_len resident allocation failed.");

    let program = csr_queue_delta_enqueue(
        "active_queue",
        "active_len",
        "edge_offsets",
        "edge_targets",
        "edge_kind_mask",
        "accumulator",
        "next_queue",
        "next_len",
        node_count,
        edge_targets.len() as u32,
        active_queue_capacity,
        next_queue_capacity,
        1,
    );
    let handles = [
        active_queue_handle,
        active_len_handle,
        edge_offsets_handle,
        edge_targets_handle,
        edge_kind_handle,
        accumulator_handle,
        next_queue_handle,
        next_len_handle,
    ];
    let steps = [ResidentDispatchStep {
        program: &program,
        handle_ids: &handles,
        grid_override: Some([active_queue_capacity.div_ceil(256).max(1), 1, 1]),
    }];

    let zero_next_queue = vec![0u8; next_queue_capacity as usize * std::mem::size_of::<u32>()];
    let active_queue_bytes = u32_bytes(&active_queue);
    let active_len_bytes = u32_bytes(&active_len);
    let edge_offsets_bytes = u32_bytes(&edge_offsets);
    let edge_targets_bytes = u32_bytes(&edge_targets);
    let edge_kind_bytes = u32_bytes(&edge_kind_mask);
    let accumulator_bytes = u32_bytes(&accumulator_seed);
    let zero_next_len = u32_bytes(&[0]);
    let uploads = [
        (active_queue_handle, active_queue_bytes.as_slice()),
        (active_len_handle, active_len_bytes.as_slice()),
        (edge_offsets_handle, edge_offsets_bytes.as_slice()),
        (edge_targets_handle, edge_targets_bytes.as_slice()),
        (edge_kind_handle, edge_kind_bytes.as_slice()),
        (accumulator_handle, accumulator_bytes.as_slice()),
        (next_queue_handle, zero_next_queue.as_slice()),
        (next_len_handle, zero_next_len.as_slice()),
    ];
    let read_ranges = [
        ResidentReadRange {
            handle_id: accumulator_handle,
            byte_offset: 0,
            byte_len: accumulator_seed.len() * std::mem::size_of::<u32>(),
        },
        ResidentReadRange {
            handle_id: next_queue_handle,
            byte_offset: 0,
            byte_len: next_queue_capacity as usize * std::mem::size_of::<u32>(),
        },
        ResidentReadRange {
            handle_id: next_len_handle,
            byte_offset: 0,
            byte_len: std::mem::size_of::<u32>(),
        },
    ];

    backend.reset_telemetry();
    let outputs = dispatcher
        .upload_resident_many_sequence_read_ranges(&uploads, &steps, &read_ranges)
        .expect("Fix: resident queue delta enqueue sequence failed.");

    assert_eq!(bytes_u32(&outputs[0]), expected_accumulator);
    let observed_next_len = bytes_u32(&outputs[2])[0];
    assert_eq!(observed_next_len, expected_next_len);
    let mut observed_queue = bytes_u32(&outputs[1]);
    observed_queue.truncate(observed_next_len as usize);
    observed_queue.sort_unstable();
    let mut expected_queue = expected_next_queue;
    expected_queue.sort_unstable();
    assert_eq!(observed_queue, expected_queue);

    let telemetry = backend.telemetry_snapshot();
    assert_eq!(
        telemetry.kernel_launches, 1,
        "Fix: queue delta enqueue must be one resident kernel over active nodes."
    );
    assert_eq!(
        telemetry.readback_bytes,
        (accumulator_seed.len() * std::mem::size_of::<u32>()
            + next_queue_capacity as usize * std::mem::size_of::<u32>()
            + std::mem::size_of::<u32>()) as u64
    );

    for handle in [
        active_queue_handle,
        active_len_handle,
        edge_offsets_handle,
        edge_targets_handle,
        edge_kind_handle,
        accumulator_handle,
        next_queue_handle,
        next_len_handle,
    ] {
        dispatcher
            .free_resident(handle)
            .expect("Fix: resident queue delta cleanup failed.");
    }
}

#[test]
fn cuda_resident_strided_queue_delta_matches_cpu_on_skewed_rows_and_pressure() {
    let backend = live_backend();
    let dispatcher = CudaOptimizerDispatcher::new(&backend);
    let node_count = 4096u32;
    let active_queue_capacity = 5u32;
    let active_queue = vec![0u32, 17, 2047, 0, 0];
    let active_len = vec![3u32];
    let accumulator_seed = pack_nodes(&[0, 17, 2047], node_count);
    let mut edge_offsets = Vec::with_capacity(node_count as usize + 1);
    let mut edge_targets = Vec::new();
    let mut edge_kind_mask = Vec::new();
    edge_offsets.push(0);
    for src in 0..node_count {
        let degree = match src {
            0 => 768,
            17 => 64,
            2047 => 9,
            _ => 0,
        };
        for edge in 0..degree {
            let target_base = match src {
                0 => 512,
                17 => 1536,
                2047 => 3000,
                _ => 0,
            };
            edge_targets.push(target_base + edge);
            edge_kind_mask.push(if edge % 5 == 0 { 2 } else { 1 });
        }
        edge_offsets.push(edge_targets.len() as u32);
    }
    let (expected_accumulator, full_expected_queue, full_expected_len) =
        csr_queue_delta_enqueue_cpu(
            &active_queue,
            active_len[0],
            &edge_offsets,
            &edge_targets,
            &edge_kind_mask,
            &accumulator_seed,
            node_count,
            node_count as usize,
            1,
        );
    let mut full_expected_sorted = full_expected_queue.clone();
    full_expected_sorted.sort_unstable();

    for next_queue_capacity in [full_expected_len, 32] {
        let active_queue_handle = dispatcher
            .alloc_resident(active_queue.len() * std::mem::size_of::<u32>())
            .expect("Fix: strided delta active_queue resident allocation failed.");
        let active_len_handle = dispatcher
            .alloc_resident(std::mem::size_of::<u32>())
            .expect("Fix: strided delta active_len resident allocation failed.");
        let edge_offsets_handle = dispatcher
            .alloc_resident(edge_offsets.len() * std::mem::size_of::<u32>())
            .expect("Fix: strided delta edge_offsets resident allocation failed.");
        let edge_targets_handle = dispatcher
            .alloc_resident(edge_targets.len() * std::mem::size_of::<u32>())
            .expect("Fix: strided delta edge_targets resident allocation failed.");
        let edge_kind_handle = dispatcher
            .alloc_resident(edge_kind_mask.len() * std::mem::size_of::<u32>())
            .expect("Fix: strided delta edge_kind_mask resident allocation failed.");
        let accumulator_handle = dispatcher
            .alloc_resident(accumulator_seed.len() * std::mem::size_of::<u32>())
            .expect("Fix: strided delta accumulator resident allocation failed.");
        let next_queue_handle = dispatcher
            .alloc_resident(next_queue_capacity as usize * std::mem::size_of::<u32>())
            .expect("Fix: strided delta next_queue resident allocation failed.");
        let next_len_handle = dispatcher
            .alloc_resident(std::mem::size_of::<u32>())
            .expect("Fix: strided delta next_len resident allocation failed.");

        let program = csr_queue_delta_strided_enqueue(
            "active_queue",
            "active_len",
            "edge_offsets",
            "edge_targets",
            "edge_kind_mask",
            "accumulator",
            "next_queue",
            "next_len",
            node_count,
            edge_targets.len() as u32,
            active_queue_capacity,
            next_queue_capacity,
            1,
        );
        let handles = [
            active_queue_handle,
            active_len_handle,
            edge_offsets_handle,
            edge_targets_handle,
            edge_kind_handle,
            accumulator_handle,
            next_queue_handle,
            next_len_handle,
        ];
        let steps = [ResidentDispatchStep {
            program: &program,
            handle_ids: &handles,
            grid_override: Some(csr_queue_delta_strided_dispatch_grid(active_queue_capacity)),
        }];

        let zero_next_queue = vec![0u8; next_queue_capacity as usize * std::mem::size_of::<u32>()];
        let active_queue_bytes = u32_bytes(&active_queue);
        let active_len_bytes = u32_bytes(&active_len);
        let edge_offsets_bytes = u32_bytes(&edge_offsets);
        let edge_targets_bytes = u32_bytes(&edge_targets);
        let edge_kind_bytes = u32_bytes(&edge_kind_mask);
        let accumulator_bytes = u32_bytes(&accumulator_seed);
        let zero_next_len = u32_bytes(&[0u32]);
        let uploads = [
            (active_queue_handle, active_queue_bytes.as_slice()),
            (active_len_handle, active_len_bytes.as_slice()),
            (edge_offsets_handle, edge_offsets_bytes.as_slice()),
            (edge_targets_handle, edge_targets_bytes.as_slice()),
            (edge_kind_handle, edge_kind_bytes.as_slice()),
            (accumulator_handle, accumulator_bytes.as_slice()),
            (next_queue_handle, zero_next_queue.as_slice()),
            (next_len_handle, zero_next_len.as_slice()),
        ];
        let read_ranges = [
            ResidentReadRange {
                handle_id: accumulator_handle,
                byte_offset: 0,
                byte_len: accumulator_seed.len() * std::mem::size_of::<u32>(),
            },
            ResidentReadRange {
                handle_id: next_queue_handle,
                byte_offset: 0,
                byte_len: next_queue_capacity as usize * std::mem::size_of::<u32>(),
            },
            ResidentReadRange {
                handle_id: next_len_handle,
                byte_offset: 0,
                byte_len: std::mem::size_of::<u32>(),
            },
        ];

        backend.reset_telemetry();
        let outputs = dispatcher
            .upload_resident_many_sequence_read_ranges(&uploads, &steps, &read_ranges)
            .expect("Fix: resident strided queue delta enqueue sequence failed.");

        assert_eq!(bytes_u32(&outputs[0]), expected_accumulator);
        let observed_next_len = bytes_u32(&outputs[2])[0];
        assert_eq!(observed_next_len, full_expected_len);
        let mut observed_queue = bytes_u32(&outputs[1]);
        observed_queue.truncate(next_queue_capacity.min(observed_next_len) as usize);
        observed_queue.sort_unstable();
        observed_queue.dedup();
        if next_queue_capacity >= full_expected_len {
            assert_eq!(observed_queue, full_expected_sorted);
        } else {
            assert_eq!(
                observed_queue.len(),
                next_queue_capacity as usize,
                "Fix: pressure case should fill every available next_queue slot."
            );
            assert!(
                observed_queue
                    .iter()
                    .all(|node| full_expected_sorted.binary_search(node).is_ok()),
                "Fix: pressure case stored a node outside the first-time discovery set: {:?}",
                observed_queue
            );
        }

        let telemetry = backend.telemetry_snapshot();
        assert_eq!(
            telemetry.kernel_launches, 1,
            "Fix: strided queue delta enqueue must remain one resident kernel."
        );
        assert_eq!(
            telemetry.readback_bytes,
            (accumulator_seed.len() * std::mem::size_of::<u32>()
                + next_queue_capacity as usize * std::mem::size_of::<u32>()
                + std::mem::size_of::<u32>()) as u64
        );

        for handle in [
            active_queue_handle,
            active_len_handle,
            edge_offsets_handle,
            edge_targets_handle,
            edge_kind_handle,
            accumulator_handle,
            next_queue_handle,
            next_len_handle,
        ] {
            dispatcher
                .free_resident(handle)
                .expect("Fix: resident strided queue delta cleanup failed.");
        }
    }
}

