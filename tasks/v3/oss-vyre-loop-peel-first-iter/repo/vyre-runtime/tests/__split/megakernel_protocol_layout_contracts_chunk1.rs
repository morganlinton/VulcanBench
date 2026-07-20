// Megakernel protocol layout contracts  -  exact byte/word placement and non-overlap.
//
// Covers:
// - Priority offsets / fairness non-overlap
// - Control min words
// - Observable base placement
// - Slot publish / read bounds
// - Epoch / done counter placement
// - Packed slot overflow behavior

// `#![allow(clippy::assertions_on_constants)]` was moved to the parent
// `megakernel_protocol_layout_contracts.rs` because inner attributes
// cannot ride an `include!`-d chunk.

use vyre_runtime::megakernel::{
    protocol::{self, control, opcode, slot, ARG0_WORD, ARGS_PER_SLOT, SLOT_WORDS},
    scheduler::{self, PRIORITY_LEVELS},
    Megakernel,
};
use vyre_runtime::PipelineError;

fn write_word(bytes: &mut [u8], word_idx: usize, value: u32) {
    let off = word_idx * 4;
    bytes[off..off + 4].copy_from_slice(&value.to_le_bytes());
}

fn read_word(bytes: &[u8], word_idx: usize) -> u32 {
    let off = word_idx * 4;
    u32::from_le_bytes(bytes[off..off + 4].try_into().unwrap())
}

// ---------------------------------------------------------------------------
// 1. Priority offsets / fairness non-overlap
// ---------------------------------------------------------------------------

#[test]
fn priority_offsets_do_not_overlap_tenant_fairness() {
    let tenant_fairness_end = control::TENANT_FAIRNESS_BASE + control::TENANT_FAIRNESS_SLOTS;
    assert!(
        control::PRIORITY_OFFSETS_BASE >= tenant_fairness_end,
        "priority offsets must start at or after tenant fairness ends ({tenant_fairness_end})"
    );
}

#[test]
fn priority_offsets_do_not_overlap_metrics() {
    let metrics_end = control::METRICS_BASE + control::METRICS_SLOTS;
    assert!(
        control::PRIORITY_OFFSETS_BASE >= metrics_end,
        "priority offsets must start at or after metrics end ({metrics_end})"
    );
}

#[test]
fn priority_starvation_counter_is_after_offsets() {
    assert_eq!(
        control::PRIORITY_STARVATION_COUNTER,
        control::PRIORITY_OFFSETS_BASE + control::PRIORITY_OFFSETS_SLOTS,
        "starvation counter must immediately follow priority offsets"
    );
}

#[test]
fn priority_fairness_base_is_after_starvation_counter() {
    assert_eq!(
        control::PRIORITY_FAIRNESS_BASE,
        control::PRIORITY_STARVATION_COUNTER + 1,
        "priority fairness base must immediately follow starvation counter"
    );
}

#[test]
fn priority_fairness_does_not_overlap_observable_base() {
    let priority_fairness_end = control::PRIORITY_FAIRNESS_BASE + control::PRIORITY_FAIRNESS_SLOTS;
    assert!(
        control::OBSERVABLE_BASE >= priority_fairness_end,
        "observable base must start at or after priority fairness ends ({priority_fairness_end})"
    );
}

#[test]
fn priority_offsets_slots_equals_levels_plus_sentinel() {
    assert_eq!(
        control::PRIORITY_OFFSETS_SLOTS,
        PRIORITY_LEVELS + 1,
        "priority offsets must include one sentinel word for total slot count"
    );
}

#[test]
fn write_default_priority_offsets_preserves_epoch_word() {
    let mut control = Megakernel::encode_control(false, 1, 0).unwrap();
    write_word(&mut control, control::EPOCH as usize, 0xABCD_1234);
    scheduler::write_default_priority_offsets(&mut control, 64).unwrap();
    assert_eq!(
        read_word(&control, control::EPOCH as usize),
        0xABCD_1234,
        "epoch word must be untouched after writing priority offsets"
    );
}

#[test]
fn write_default_priority_offsets_preserves_done_count() {
    let mut control = Megakernel::encode_control(false, 1, 0).unwrap();
    write_word(&mut control, control::DONE_COUNT as usize, 99);
    scheduler::write_default_priority_offsets(&mut control, 64).unwrap();
    assert_eq!(
        read_word(&control, control::DONE_COUNT as usize),
        99,
        "done count must be untouched after writing priority offsets"
    );
}

#[test]
fn control_regions_are_strictly_ordered() {
    assert!(control::SHUTDOWN < control::DONE_COUNT);
    assert!(control::DONE_COUNT < control::TENANT_BASE);
    assert!(control::TENANT_BASE < control::TENANT_QUOTA_BASE);
    assert!(control::TENANT_QUOTA_BASE < control::TENANT_FAIRNESS_BASE);
    assert!(control::TENANT_FAIRNESS_BASE < control::METRICS_BASE);
    assert!(control::METRICS_BASE < control::EPOCH);
    assert!(control::EPOCH < control::PRIORITY_OFFSETS_BASE);
    assert!(control::PRIORITY_OFFSETS_BASE < control::PRIORITY_STARVATION_COUNTER);
    assert!(control::PRIORITY_STARVATION_COUNTER < control::PRIORITY_FAIRNESS_BASE);
    assert!(control::PRIORITY_FAIRNESS_BASE < control::OBSERVABLE_BASE);
}

// ---------------------------------------------------------------------------
// 2. Control min words
// ---------------------------------------------------------------------------

#[test]
fn control_min_words_covers_all_fixed_regions() {
    let last_fixed = control::OBSERVABLE_BASE;
    assert!(
        protocol::CONTROL_MIN_WORDS >= last_fixed,
        "CONTROL_MIN_WORDS ({}) must cover all fixed regions up to OBSERVABLE_BASE ({last_fixed})",
        protocol::CONTROL_MIN_WORDS
    );
}

#[test]
fn encode_control_zero_observables_equals_control_min_words_in_bytes() {
    let ctrl = Megakernel::encode_control(false, 0, 0).unwrap();
    assert_eq!(
        ctrl.len(),
        (protocol::CONTROL_MIN_WORDS as usize) * 4,
        "control with zero observables must be exactly CONTROL_MIN_WORDS * 4 bytes"
    );
}

#[test]
fn encode_control_huge_tenant_count_saturates_fixed_tables() {
    let ctrl = protocol::try_encode_control(false, u32::MAX, 0)
        .expect("huge tenant count must saturate fixed tenant tables without host overflow");
    assert_eq!(
        ctrl.len(),
        (protocol::CONTROL_MIN_WORDS as usize) * 4,
        "tenant count must not enlarge the fixed control buffer"
    );
    assert_eq!(
        read_word(&ctrl, control::TENANT_QUOTA_BASE as usize),
        1_000_000,
        "quota table should be initialized through its fixed capacity"
    );
    assert_eq!(
        read_word(&ctrl, (control::TENANT_FAIRNESS_BASE - 1) as usize),
        1_000_000,
        "quota initialization must saturate at the fairness boundary"
    );
}

#[test]
fn encode_control_nonzero_observables_exceeds_min_words() {
    let ctrl = Megakernel::encode_control(false, 0, 8).unwrap();
    let expected = (control::OBSERVABLE_BASE as usize + 8) * 4;
    assert_eq!(
        ctrl.len(),
        expected,
        "control with 8 observables must be (OBSERVABLE_BASE + 8) * 4 bytes"
    );
    assert!(
        ctrl.len() > (protocol::CONTROL_MIN_WORDS as usize) * 4,
        "control with observables must exceed CONTROL_MIN_WORDS bytes"
    );
}

#[test]
fn control_byte_len_observable_zero_returns_min_words() {
    let bytes = protocol::control_byte_len(0).expect("control_byte_len(0) must succeed");
    assert_eq!(bytes, (protocol::CONTROL_MIN_WORDS as usize) * 4);
}

#[test]
fn control_byte_len_rejects_observable_count_above_protocol_cap() {
    assert_eq!(
        protocol::control_byte_len(protocol::MAX_OBSERVABLE_SLOTS + 1),
        None,
        "oversized observable regions must reject before allocating host memory"
    );
}

#[test]
fn ring_byte_len_rejects_slot_count_above_protocol_cap() {
    assert_eq!(
        protocol::ring_byte_len(protocol::MAX_RING_SLOTS + 1),
        None,
        "oversized rings must reject before allocating host memory"
    );
}

#[test]
fn debug_log_byte_len_rejects_record_count_above_protocol_cap() {
    assert_eq!(
        protocol::debug_log_byte_len(protocol::MAX_DEBUG_RECORDS + 1),
        None,
        "oversized debug logs must reject before allocating host memory"
    );
}

#[test]
fn strict_metrics_reader_rejects_less_than_full_metrics_window() {
    // Metrics window needs METRICS_BASE .. METRICS_BASE+METRICS_SLOTS words.
    let short = vec![0u8; (control::METRICS_BASE as usize) * 4];
    let err = Megakernel::try_read_metrics(&short)
        .expect_err("control buffer ending at metrics base must reject metrics read");
    assert!(err.to_string().contains("Fix:"));
}

// ---------------------------------------------------------------------------
// 3. Observable base
// ---------------------------------------------------------------------------

#[test]
fn observable_base_is_at_word_160() {
    assert_eq!(
        control::OBSERVABLE_BASE,
        160,
        "observable base must be exactly word 160 per ABI contract"
    );
}

#[test]
fn observable_base_matches_control_min_words() {
    assert_eq!(control::OBSERVABLE_BASE, 160);
    assert_eq!(protocol::CONTROL_MIN_WORDS, 160);
    assert_eq!(
        control::OBSERVABLE_BASE,
        protocol::CONTROL_MIN_WORDS,
        "observable base must equal CONTROL_MIN_WORDS when no extra observables are allocated"
    );
}

#[test]
fn read_observable_uses_correct_byte_offset() {
    let mut ctrl = Megakernel::encode_control(false, 0, 4).unwrap();
    let base_word = control::OBSERVABLE_BASE as usize;
    write_word(&mut ctrl, base_word, 0x1111_1111);
    write_word(&mut ctrl, base_word + 1, 0x2222_2222);
    write_word(&mut ctrl, base_word + 3, 0x4444_4444);

    assert_eq!(Megakernel::read_observable(&ctrl, 0), 0x1111_1111);
    assert_eq!(Megakernel::read_observable(&ctrl, 1), 0x2222_2222);
    assert_eq!(Megakernel::read_observable(&ctrl, 3), 0x4444_4444);
}

#[test]
fn strict_observable_rejects_index_at_exact_buffer_end() {
    let ctrl = Megakernel::encode_control(false, 0, 2).unwrap();
    let err = Megakernel::try_read_observable(&ctrl, 2)
        .expect_err("observable index 2 is at word OBSERVABLE_BASE+2 == buffer end / 4");
    assert!(err.to_string().contains("Fix:"));
}

#[test]
fn strict_observable_accepts_last_valid_index() {
    let mut ctrl = Megakernel::encode_control(false, 0, 2).unwrap();
    write_word(&mut ctrl, (control::OBSERVABLE_BASE + 1) as usize, 0xBEEF);
    let val =
        Megakernel::try_read_observable(&ctrl, 1).expect("index 1 must be valid for 2 observables");
    assert_eq!(val, 0xBEEF);
}

#[test]
fn observable_does_not_alias_metrics_region() {
    let metrics_end = control::METRICS_BASE + control::METRICS_SLOTS;
    assert!(
        control::OBSERVABLE_BASE >= metrics_end,
        "observable region must not alias metrics region ending at {metrics_end}"
    );
}

#[test]
fn observable_does_not_alias_epoch_word() {
    assert_ne!(
        control::OBSERVABLE_BASE,
        control::EPOCH,
        "observable base must not alias epoch word"
    );
    assert!(
        control::OBSERVABLE_BASE > control::EPOCH,
        "observable base must come after epoch word"
    );
}

#[test]
fn observable_does_not_alias_priority_regions() {
    let pri_end = control::PRIORITY_FAIRNESS_BASE + control::PRIORITY_FAIRNESS_SLOTS;
    assert!(
        control::OBSERVABLE_BASE >= pri_end,
        "observable base must not alias priority scheduler regions"
    );
}

// ---------------------------------------------------------------------------
// 4. Slot publish / read bounds
// ---------------------------------------------------------------------------

#[test]
fn slot_publish_first_slot_is_index_zero() {
    let mut ring = Megakernel::encode_empty_ring(4).unwrap();
    Megakernel::publish_slot(&mut ring, 0, 0, opcode::NOP, &[])
        .expect("slot index 0 must always be publishable in a non-empty ring");
}

#[test]
fn slot_publish_last_slot_is_count_minus_one() {
    let mut ring = Megakernel::encode_empty_ring(8).unwrap();
    Megakernel::publish_slot(&mut ring, 7, 0, opcode::NOP, &[])
        .expect("slot index slot_count - 1 must be publishable");
}

#[test]
fn slot_publish_at_count_is_rejected() {
    let mut ring = Megakernel::encode_empty_ring(8).unwrap();
    let err = Megakernel::publish_slot(&mut ring, 8, 0, opcode::NOP, &[])
        .expect_err("slot index == slot_count must be rejected");
    assert!(matches!(err, PipelineError::QueueFull { .. }));
}

#[test]
fn slot_publish_rejects_malformed_ring_not_multiple_of_slot_bytes() {
    let mut ring = vec![0u8; (SLOT_WORDS as usize * 4) + 1];
    let err = Megakernel::publish_slot(&mut ring, 0, 0, opcode::NOP, &[])
        .expect_err("ring length not a multiple of slot bytes must be rejected");
    assert!(matches!(err, PipelineError::QueueFull { .. }));
}

#[test]
fn ring_byte_len_exactly_slots_times_slot_words_times_four() {
    for slot_count in [0, 1, 4, 16, 256] {
        let expected = (slot_count * SLOT_WORDS * 4) as usize;
        assert_eq!(
            protocol::ring_byte_len(slot_count).unwrap(),
            expected,
            "ring_byte_len({slot_count}) must equal slot_count * SLOT_WORDS * 4"
        );
    }
}

#[test]
fn encode_empty_ring_produces_exact_byte_length() {
    for slot_count in [0, 1, 4, 16] {
        let ring = Megakernel::encode_empty_ring(slot_count).unwrap();
        let expected = protocol::ring_byte_len(slot_count).unwrap();
        assert_eq!(
            ring.len(),
            expected,
            "encode_empty_ring({slot_count}) must match ring_byte_len"
        );
    }
}

#[test]
fn publish_slot_args_exactly_at_budget_succeeds() {
    let mut ring = Megakernel::encode_empty_ring(1).unwrap();
    let args = vec![0u32; ARGS_PER_SLOT as usize];
    Megakernel::publish_slot(&mut ring, 0, 0, opcode::NOP, &args)
        .expect("exactly ARGS_PER_SLOT args must succeed");
}

#[test]
fn publish_slot_args_one_over_budget_fails() {
    let mut ring = Megakernel::encode_empty_ring(1).unwrap();
    let args = vec![0u32; ARGS_PER_SLOT as usize + 1];
    let err = Megakernel::publish_slot(&mut ring, 0, 0, opcode::NOP, &args)
        .expect_err("ARGS_PER_SLOT + 1 args must be rejected");
    assert!(matches!(err, PipelineError::QueueFull { .. }));
}

#[test]
fn slot_word_layout_status_is_word_0() {
    let mut ring = Megakernel::encode_empty_ring(1).unwrap();
    Megakernel::publish_slot(&mut ring, 0, 0, opcode::NOP, &[]).unwrap();
    let status = read_word(&ring, 0);
    assert_eq!(status, slot::PUBLISHED, "status must be at word 0");
}

#[test]
fn slot_word_layout_opcode_is_word_1() {
    let mut ring = Megakernel::encode_empty_ring(1).unwrap();
    Megakernel::publish_slot(&mut ring, 0, 0, opcode::STORE_U32, &[1, 2]).unwrap();
    let op = read_word(&ring, 1);
    assert_eq!(op, opcode::STORE_U32, "opcode must be at word 1");
}

#[test]
fn slot_word_layout_tenant_is_word_2() {
    let mut ring = Megakernel::encode_empty_ring(1).unwrap();
    Megakernel::publish_slot(&mut ring, 0, 7, opcode::NOP, &[]).unwrap();
    let tenant = read_word(&ring, 2);
    assert_eq!(tenant, 7, "tenant id must be at word 2");
}

#[test]
fn slot_word_layout_priority_is_word_3() {
    let mut ring = Megakernel::encode_empty_ring(1).unwrap();
    Megakernel::publish_slot(&mut ring, 0, 0, opcode::NOP, &[]).unwrap();
    let priority = read_word(&ring, 3);
    assert_eq!(
        priority,
        slot::PRIORITY_NORMAL,
        "priority must be at word 3 and default to NORMAL"
    );
}

