//! Test crate.

#![allow(deprecated)]
use proptest::prelude::*;
use std::collections::BTreeSet;
use vyre::DispatchConfig;
use vyre::VyreBackend;
use vyre_foundation::optimizer::pre_lowering::optimize;
use vyre_libs::scan::dispatch_io::pack_u32_slice as pack_words;
use vyre_libs::scan::{compact_hits_with_layout, emit_hit_with_layout};
use vyre_reference::value::Value;

fn unpack_words(bytes: &[u8]) -> Vec<u32> {
    bytes
        .chunks_exact(4)
        .map(|chunk| u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
        .collect()
}

fn run_emit_reference(
    rule_ids: &[u32],
    file_ids: &[u32],
    span_starts: &[u32],
    span_lens: &[u32],
    max_hits: u32,
) -> Vec<Vec<u8>> {
    let program = emit_hit_with_layout(
        "rule_id",
        "file_id",
        "span_start",
        "span_len",
        "out_hits",
        "out_cursor",
        rule_ids.len() as u32,
        max_hits,
    );
    let inputs = vec![
        Value::Bytes(pack_words(rule_ids).into()),
        Value::Bytes(pack_words(file_ids).into()),
        Value::Bytes(pack_words(span_starts).into()),
        Value::Bytes(pack_words(span_lens).into()),
        Value::Bytes(vec![0u8; (max_hits * 4 * 4) as usize].into()),
        Value::Bytes(pack_words(&[0]).into()),
        Value::Bytes(pack_words(&[0]).into()),
    ];
    vyre_reference::reference_eval(&program, &inputs)
        .expect("Fix: hit-buffer reference run must succeed")
        .into_iter()
        .map(|value| value.to_bytes())
        .collect()
}

#[test]
fn overflow_records_drop_not_ub() {
    let outputs = run_emit_reference(&[7, 9], &[101, 103], &[5, 9], &[2, 4], 1);
    assert_eq!(unpack_words(&outputs[0]), vec![7, 101, 5, 2]);
    assert_eq!(unpack_words(&outputs[1]), vec![2]);
    assert_eq!(unpack_words(&outputs[2]), vec![1]);
}

type HitTuple = (u32, u32, u32, u32);
type EmitSimulation = (usize, usize, BTreeSet<HitTuple>);

fn simulate_emit_schedule(
    hits: &[HitTuple],
    schedule: &[usize],
    max_hits: usize,
) -> EmitSimulation {
    let mut cursor = 0usize;
    let mut overflow = 0usize;
    let mut stored = BTreeSet::new();
    for &lane in schedule {
        let slot = cursor;
        cursor += 1;
        if slot < max_hits {
            stored.insert(hits[lane]);
        } else {
            overflow += 1;
        }
    }
    (cursor, overflow, stored)
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 128,
        ..ProptestConfig::default()
    })]

    #[test]
    fn cuckoo_free_parallel_lanes_no_lost_hits(
        lane_count in 1usize..8,
        schedule_keys in proptest::collection::vec(any::<u32>(), 1..8),
    ) {
        prop_assume!(schedule_keys.len() == lane_count);
        let mut order = schedule_keys
            .iter()
            .copied()
            .enumerate()
            .collect::<Vec<_>>();
        order.sort_by_key(|(idx, key)| (*key, *idx));
        let schedule = order.into_iter().map(|(idx, _)| idx).collect::<Vec<_>>();

        let hits = (0..lane_count)
            .map(|lane| {
                let lane = lane as u32;
                (100 + lane, 200 + lane, 300 + lane, 400 + lane)
            })
            .collect::<Vec<_>>();
        let (cursor, overflow, stored) = simulate_emit_schedule(&hits, &schedule, lane_count);
        prop_assert_eq!(cursor, lane_count);
        prop_assert_eq!(overflow, 0);
        prop_assert_eq!(stored.len(), lane_count);
        for tuple in hits {
            prop_assert!(stored.contains(&tuple));
        }
    }
}

#[test]
fn host_readback_prefix_matches_cursor() {
    let backend = vyre_driver_wgpu::WgpuBackend::new()
        .expect("Fix: GPU backend is required for hit-buffer readback on this machine");
    let emit_program = optimize(emit_hit_with_layout(
        "rule_id",
        "file_id",
        "span_start",
        "span_len",
        "out_hits",
        "out_cursor",
        3,
        4,
    ));
    let wgpu_hits = backend
        .dispatch(
            &emit_program,
            &[
                pack_words(&[7, 9, 11]),
                pack_words(&[101, 103, 107]),
                pack_words(&[5, 9, 13]),
                pack_words(&[2, 4, 6]),
                vec![0u8; 16 * 4],
                pack_words(&[0]),
                pack_words(&[0]),
            ],
            &DispatchConfig::default(),
        )
        .expect("Fix: wgpu emit_hit dispatch must succeed");
    let reference_outputs =
        run_emit_reference(&[7, 9, 11], &[101, 103, 107], &[5, 9, 13], &[2, 4, 6], 4);
    let cursor = unpack_words(&reference_outputs[1])[0];
    let overflow = unpack_words(&reference_outputs[2])[0];
    assert_eq!(cursor, 3);
    assert_eq!(overflow, 0);

    let compact_program = optimize(compact_hits_with_layout("out_hits", "out_cursor", 4, 4));
    let compact_outputs = backend
        .dispatch(
            &compact_program,
            &[
                reference_outputs[0].clone(),
                reference_outputs[1].clone(),
                pack_words(&[0]),
            ],
            &DispatchConfig::default(),
        )
        .expect("Fix: wgpu compact_hits dispatch must succeed");
    let live_len = unpack_words(&compact_outputs[0])[0];
    assert_eq!(live_len, cursor);

    let prefix_words = unpack_words(&wgpu_hits[0])[..(live_len as usize * 4)].to_vec();
    assert_eq!(
        prefix_words,
        vec![7, 101, 5, 2, 9, 103, 9, 4, 11, 107, 13, 6,]
    );
}
