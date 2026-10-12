//! Adversarial overflow contracts: done-count, epoch, observable-index, and
//! control-buffer encoding boundaries.

use vyre_runtime::megakernel::{
    protocol::{self, control, control::OBSERVABLE_BASE},
    Megakernel,
};
// PipelineError intentionally not used in this file  -  all tests use Option/Result returns.

fn write_word(bytes: &mut [u8], word_idx: usize, value: u32) {
    let off = word_idx * 4;
    bytes[off..off + 4].copy_from_slice(&value.to_le_bytes());
}

// ---------------------------------------------------------------------------
// 1. Done-count / epoch word boundaries
// ---------------------------------------------------------------------------

#[test]
fn done_count_at_u32_max_decodes_correctly() {
    let mut ctrl = Megakernel::encode_control(false, 1, 0).unwrap();
    write_word(&mut ctrl, control::DONE_COUNT as usize, u32::MAX);
    assert_eq!(Megakernel::read_done_count(&ctrl), u32::MAX);
    assert_eq!(protocol::try_read_done_count(&ctrl).unwrap(), u32::MAX);
}

#[test]
fn epoch_at_u32_max_decodes_correctly() {
    let mut ctrl = Megakernel::encode_control(false, 1, 0).unwrap();
    write_word(&mut ctrl, control::EPOCH as usize, u32::MAX);
    assert_eq!(Megakernel::read_epoch(&ctrl), u32::MAX);
    assert_eq!(protocol::try_read_epoch(&ctrl).unwrap(), u32::MAX);
}

#[test]
fn done_count_and_epoch_coexist_without_alias() {
    let mut ctrl = Megakernel::encode_control(false, 1, 0).unwrap();
    write_word(&mut ctrl, control::DONE_COUNT as usize, 0x1111_1111);
    write_word(&mut ctrl, control::EPOCH as usize, 0x2222_2222);
    assert_eq!(Megakernel::read_done_count(&ctrl), 0x1111_1111);
    assert_eq!(Megakernel::read_epoch(&ctrl), 0x2222_2222);
}

// ---------------------------------------------------------------------------
// 2. Observable index overflow boundaries
// ---------------------------------------------------------------------------

#[test]
fn try_read_observable_rejects_index_that_overflows_u32_addition() {
    let ctrl = Megakernel::encode_control(false, 1, 0).unwrap();
    let bad_index = u32::MAX - OBSERVABLE_BASE + 1;
    let err = Megakernel::try_read_observable(&ctrl, bad_index)
        .expect_err("observable index causing u32 addition overflow must reject");
    assert!(err.to_string().contains("Fix:"));
}

#[test]
fn read_observable_returns_zero_for_minimal_buffer() {
    let ctrl = vec![0u8; (OBSERVABLE_BASE as usize) * 4];
    assert_eq!(Megakernel::read_observable(&ctrl, 0), 0);
}

#[test]
fn try_read_observable_accepts_max_safe_index() {
    let mut ctrl = Megakernel::encode_control(false, 1, 0).unwrap();
    // Grow the buffer to hold one observable word.
    let needed = (OBSERVABLE_BASE as usize + 1) * 4;
    if ctrl.len() < needed {
        ctrl.resize(needed, 0);
    }
    write_word(&mut ctrl, OBSERVABLE_BASE as usize, 0xBEEF);
    assert_eq!(Megakernel::try_read_observable(&ctrl, 0).unwrap(), 0xBEEF);
}

// ---------------------------------------------------------------------------
// 3. Control encoding overflow boundaries
// ---------------------------------------------------------------------------

#[test]
fn control_byte_len_overflows_when_observable_base_plus_slots_wraps() {
    let max_safe = u32::MAX - OBSERVABLE_BASE;
    assert!(protocol::control_byte_len(max_safe).is_some());
    assert!(protocol::control_byte_len(max_safe + 1).is_none());
}

#[test]
fn try_encode_control_rejects_overflow_observable_slots() {
    let err = protocol::try_encode_control(false, 1, u32::MAX)
        .expect_err("observable_slots = u32::MAX must overflow");
    assert!(err.to_string().contains("Fix:"));
}

#[test]
fn try_encode_control_rejects_exactly_one_beyond_max_safe_observable() {
    let max_safe = u32::MAX - OBSERVABLE_BASE;
    let err = protocol::try_encode_control(false, 1, max_safe + 1)
        .expect_err("one beyond max safe observable slots must overflow");
    assert!(err.to_string().contains("Fix:"));
}

#[test]
fn control_byte_len_observable_boundary_exactly_max_safe_succeeds_without_allocating() {
    let max_safe = u32::MAX - OBSERVABLE_BASE;
    let expected_words = OBSERVABLE_BASE + max_safe;
    let expected_bytes = (expected_words as usize) * core::mem::size_of::<u32>();
    assert_eq!(protocol::control_byte_len(max_safe), Some(expected_bytes));
}
