//! P7.3  -  Megakernel multi-tenant scheduler contract tests.
//!
//! The megakernel consults a per-slot `tenant_id` and the control
//! buffer's tenant-mask table before executing each claimed slot.
//! These tests assert the contract's shape is locked for 0.6 so
//! community runtime crates (multi-tenant serving, priority-queued
//! dispatch) can pin against it.
//!
//! Execution-level tests live in concrete drivers (needs a live
//! adapter). These assertions cover the IR-shape contract + the
//! slot-header encoding that every tenant-aware scheduler walks.

#![allow(clippy::assertions_on_constants)]

use vyre_runtime::megakernel::{
    self, control, opcode, slot, ARG0_WORD, OPCODE_WORD, PRIORITY_WORD, SLOT_WORDS, STATUS_WORD,
    TENANT_WORD,
};

#[test]
fn slot_layout_is_stable() {
    // SLOT_WORDS frozen at 16. Any bump breaks published pipeline
    // caches pinned against existing fingerprints.
    assert_eq!(SLOT_WORDS, 16);
    assert_eq!(STATUS_WORD, 0);
    assert_eq!(OPCODE_WORD, 1);
    assert_eq!(TENANT_WORD, 2);
    assert_eq!(PRIORITY_WORD, 3);
    assert_eq!(ARG0_WORD, 4);
}

#[test]
fn control_layout_is_stable() {
    assert_eq!(control::SHUTDOWN, 0);
    assert_eq!(control::DONE_COUNT, 1);
    assert_eq!(control::TENANT_BASE, 2);
    assert!(control::TENANT_BASE < control::TENANT_QUOTA_BASE);
    assert!(control::TENANT_QUOTA_BASE < control::TENANT_FAIRNESS_BASE);
    assert!(
        control::TENANT_FAIRNESS_BASE + control::TENANT_FAIRNESS_SLOTS <= control::METRICS_BASE
    );
    assert!(control::METRICS_BASE + control::METRICS_SLOTS <= control::EPOCH);
    assert_eq!(control::PRIORITY_OFFSETS_BASE, control::EPOCH + 1);
    assert_eq!(control::PRIORITY_OFFSETS_SLOTS, 6);
    assert!(
        control::PRIORITY_OFFSETS_BASE + control::PRIORITY_OFFSETS_SLOTS
            <= control::PRIORITY_STARVATION_COUNTER
    );
    assert!(control::PRIORITY_STARVATION_COUNTER < control::PRIORITY_FAIRNESS_BASE);
    assert!(
        control::PRIORITY_FAIRNESS_BASE + control::PRIORITY_FAIRNESS_SLOTS
            < control::OBSERVABLE_BASE
    );
}

#[test]
fn slot_states_are_stable() {
    assert_eq!(slot::EMPTY, 0);
    assert_eq!(slot::PUBLISHED, 1);
    assert_eq!(slot::CLAIMED, 2);
    assert_eq!(slot::DONE, 3);
}

#[test]
fn opcode_discriminants_frozen() {
    assert_eq!(opcode::NOP, 0);
    assert_eq!(opcode::STORE_U32, 1);
    assert_eq!(opcode::ATOMIC_ADD, 2);
    assert_eq!(opcode::PRINTF, 0x0000_FFFE);
    assert_eq!(opcode::SHUTDOWN, u32::MAX);
}

#[test]
fn encode_control_populates_tenant_table_with_all_lanes_allowed() {
    // Default tenant-mask = !0u32 (every lane allowed). Multi-tenant
    // schedulers flip specific bits to 0 to revoke a tenant's slot.
    let ctrl = megakernel::Megakernel::encode_control(false, 4, 4).unwrap();
    // Tenant table lives at word control::TENANT_BASE+1 .. OBSERVABLE_BASE.
    let tt_word_start = (control::TENANT_BASE + 1) as usize;
    for i in 0..4 {
        let off = (tt_word_start + i) * 4;
        let word = u32::from_le_bytes(ctrl[off..off + 4].try_into().unwrap());
        assert_eq!(word, u32::MAX, "tenant {i} default must be all-lanes-on");
    }
}

#[test]
fn priority_offsets_and_fairness_counters_do_not_alias() {
    use vyre_runtime::megakernel::scheduler::{write_default_priority_offsets, PRIORITY_LEVELS};

    let mut ctrl = megakernel::Megakernel::encode_control(false, 40, 4).unwrap();
    write_default_priority_offsets(&mut ctrl, 40).expect("priority offsets must encode");

    for i in 0..=PRIORITY_LEVELS {
        let offset_word = control::PRIORITY_OFFSETS_BASE + i;
        assert!(
            offset_word < control::PRIORITY_STARVATION_COUNTER,
            "priority offset word {offset_word} must stay below scheduler counters"
        );
        assert!(
            offset_word < control::PRIORITY_FAIRNESS_BASE,
            "priority offset word {offset_word} must not alias priority fairness"
        );
    }

    for pri in 0..control::PRIORITY_FAIRNESS_SLOTS {
        let off = ((control::PRIORITY_FAIRNESS_BASE + pri) as usize) * 4;
        ctrl[off..off + 4].copy_from_slice(&(pri + 1).to_le_bytes());
    }

    let offsets = vyre_runtime::megakernel::scheduler::default_priority_offsets(40);
    for (i, expected) in offsets.iter().copied().enumerate() {
        let off = ((control::PRIORITY_OFFSETS_BASE as usize) + i) * 4;
        let word = u32::from_le_bytes(ctrl[off..off + 4].try_into().unwrap());
        assert_eq!(
            word, expected,
            "priority offset {i} must survive fairness counter writes"
        );
    }
}

#[test]
fn encode_control_keeps_fairness_counters_cold_at_boot() {
    let ctrl = megakernel::Megakernel::encode_control(false, 40, 4).unwrap();
    for i in 0..control::TENANT_FAIRNESS_SLOTS {
        let off = ((control::TENANT_FAIRNESS_BASE + i) as usize) * 4;
        let word = u32::from_le_bytes(ctrl[off..off + 4].try_into().unwrap());
        assert_eq!(word, 0, "tenant fairness counter {i} must start cold");
    }
    for i in 0..control::PRIORITY_FAIRNESS_SLOTS {
        let off = ((control::PRIORITY_FAIRNESS_BASE + i) as usize) * 4;
        let word = u32::from_le_bytes(ctrl[off..off + 4].try_into().unwrap());
        assert_eq!(word, 0, "priority fairness counter {i} must start cold");
    }
}

#[test]
fn publish_slot_stamps_tenant_id_at_word_2() {
    let mut ring = megakernel::Megakernel::encode_empty_ring(2).unwrap();
    megakernel::Megakernel::publish_slot(&mut ring, 1, 7, opcode::STORE_U32, &[42, 3]).unwrap();
    // Slot 1 starts at byte offset SLOT_WORDS * 4.
    let base = (SLOT_WORDS as usize) * 4;
    let tenant = u32::from_le_bytes(ring[base + 2 * 4..base + 2 * 4 + 4].try_into().unwrap());
    assert_eq!(tenant, 7, "tenant id must land in slot word 2");
}

#[test]
fn sharded_megakernel_scales_with_workgroup_size() {
    // Building a sharded program at wg_x=1024 must produce a program
    // whose workgroup_size matches. Community schedulers that
    // slice slots across workgroups pin against this.
    let program = megakernel::build_program_sharded(1024, &[]);
    assert_eq!(program.workgroup_size(), [1024, 1, 1]);
}
