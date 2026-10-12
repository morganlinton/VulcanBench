use super::{
    claimed_slot_body, execute_already_claimed_slot_body, persistent_lane_prologue,
    process_io_requests, wrap_persistent_megakernel_program,
};
use super::{Expr, Node, OpcodeHandler, Program};

// ---- Priority-aware variant ----

/// Build a priority-aware megakernel IR.
///
/// Unlike `build_program_sharded` where each lane owns exactly one slot,
/// the priority variant has workers scan across priority-partitioned ring
/// regions, claiming the highest-priority PUBLISHED slot available. This
/// ensures latency-sensitive work (CRITICAL, HIGH) is processed before
/// background tasks (LOW, IDLE).
///
/// The control buffer is extended with `PRIORITY_OFFSETS_BASE..+6` words
/// that the host sets to define partition boundaries. The host can
/// dynamically resize partitions by updating these offsets between batches.
#[must_use]
pub fn build_program_priority(workgroup_size_x: u32, opcodes: &[OpcodeHandler]) -> Program {
    build_program_priority_slots(workgroup_size_x, workgroup_size_x.max(1), opcodes)
}

/// Build a priority-aware megakernel IR for an explicit ring slot count.
#[must_use]
pub fn build_program_priority_slots(
    workgroup_size_x: u32,
    slot_count: u32,
    opcodes: &[OpcodeHandler],
) -> Program {
    wrap_persistent_megakernel_program(
        workgroup_size_x,
        slot_count.max(1),
        persistent_body_priority_slots(workgroup_size_x, slot_count.max(1), opcodes),
    )
}

/// Priority-aware loop body. Replaces the per-lane 1:1 slot mapping
/// with the scheduler's priority scan.
#[must_use]
pub fn persistent_body_priority(workgroup_size_x: u32, opcodes: &[OpcodeHandler]) -> Vec<Node> {
    persistent_body_priority_slots(workgroup_size_x, workgroup_size_x.max(1), opcodes)
}

/// Priority-aware loop body for an explicit ring slot count.
#[must_use]
pub fn persistent_body_priority_slots(
    workgroup_size_x: u32,
    slot_count: u32,
    opcodes: &[OpcodeHandler],
) -> Vec<Node> {
    use crate::megakernel::scheduler;

    let slot_count = slot_count.max(1);
    let mut body = persistent_lane_prologue(workgroup_size_x);

    // -- Priority scan: find and claim the best available slot. --------
    body.extend(scheduler::priority_scan_body(slot_count));

    // -- If claimed, execute the slot. ---------------------------------
    body.push(Node::if_then(
        Expr::ne(Expr::var("claimed_slot_base"), Expr::u32(u32::MAX)),
        {
            // Rebind `slot_base` to the claimed slot so downstream
            // handler code works unchanged.
            let mut exec = vec![Node::let_bind("slot_base", Expr::var("claimed_slot_base"))];
            exec.extend(execute_already_claimed_slot_body(
                Expr::var("claimed_tenant"),
                claimed_slot_body(opcodes),
            ));
            exec
        },
    ));

    // -- IO poll (same as base variant). --------------------------------
    body.push(Node::Block(process_io_requests()));

    body
}
