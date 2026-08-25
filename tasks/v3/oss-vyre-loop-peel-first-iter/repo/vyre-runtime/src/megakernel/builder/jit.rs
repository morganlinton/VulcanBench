use super::persistent_lane_prologue;
use super::{
    claimed_slot_bindings, direct_slot_base_binding, process_io_requests, slot_tenant_id_load,
    tenant_authorized_claim_body, wrap_persistent_megakernel_program,
};
use crate::megakernel::ir_util::atomic_load_relaxed;
use crate::megakernel::protocol::{control, slot, STATUS_WORD};
use vyre_foundation::ir::{Expr, Node, Program};

/// Build the JIT Megakernel IR where payload processor logic is fused into the body stream.
#[must_use]
pub fn build_program_jit(workgroup_size_x: u32, payload_processor: &[Node]) -> Program {
    build_program_jit_slots(workgroup_size_x, workgroup_size_x.max(1), payload_processor)
}

/// Build the JIT megakernel IR for an explicit number of ring slots.
#[must_use]
pub fn build_program_jit_slots(
    workgroup_size_x: u32,
    slot_count: u32,
    payload_processor: &[Node],
) -> Program {
    wrap_persistent_megakernel_program(
        workgroup_size_x,
        slot_count,
        persistent_body_jit(workgroup_size_x, payload_processor),
    )
}

fn execute_slot_body_jit(payload_processor: &[Node]) -> Vec<Node> {
    vec![
        Node::let_bind(
            "status_index",
            Expr::add(Expr::var("slot_base"), Expr::u32(STATUS_WORD)),
        ),
        Node::let_bind(
            "observed_status",
            atomic_load_relaxed("ring_buffer", Expr::var("status_index")),
        ),
        Node::if_then(
            Expr::eq(Expr::var("observed_status"), Expr::u32(slot::PUBLISHED)),
            tenant_authorized_claim_body(
                slot_tenant_id_load(),
                claimed_slot_body_jit(payload_processor),
            ),
        ),
    ]
}

// ---- JIT variant ----

/// The JIT body that runs once per iteration per lane.
#[must_use]
pub fn persistent_body_jit(workgroup_size_x: u32, payload_processor: &[Node]) -> Vec<Node> {
    let mut body = persistent_lane_prologue(workgroup_size_x);
    if let Some(body_capacity) = body.len().checked_add(3) {
        let _ = vyre_foundation::allocation::try_reserve_vec_to_capacity(&mut body, body_capacity);
    }
    body.push(direct_slot_base_binding());
    body.push(Node::Block(execute_slot_body_jit(payload_processor)));
    body.push(Node::Block(process_io_requests()));
    body
}

/// Fallible JIT body builder with explicit staging-allocation reporting.
pub(super) fn try_persistent_body_jit(
    workgroup_size_x: u32,
    payload_processor: &[Node],
) -> Result<Vec<Node>, String> {
    let mut body = persistent_lane_prologue(workgroup_size_x);
    let body_capacity = body.len().checked_add(3).ok_or_else(|| {
        "megakernel JIT body node reservation overflowed usize. Fix: reduce fused payload/body staging before building the JIT megakernel."
            .to_string()
    })?;
    vyre_foundation::allocation::try_reserve_vec_to_capacity(&mut body, body_capacity).map_err(|error| {
        format!(
            "megakernel JIT body node reservation failed: {error}. Fix: reduce fused payload/body staging before building the JIT megakernel."
        )
    })?;
    body.push(direct_slot_base_binding());
    body.push(Node::Block(execute_slot_body_jit(payload_processor)));
    body.push(Node::Block(process_io_requests()));
    Ok(body)
}

fn claimed_slot_body_jit(payload_processor: &[Node]) -> Vec<Node> {
    let mut nodes = claimed_slot_bindings();

    // Wire the statically JIT-compiled rule/payload evaluation graph.
    nodes.extend(payload_processor.iter().cloned());

    nodes.push(Node::let_bind(
        "done_prev",
        Expr::atomic_add("control", Expr::u32(control::DONE_COUNT), Expr::u32(1)),
    ));
    nodes.push(Node::store(
        "ring_buffer",
        Expr::var("status_index"),
        Expr::u32(slot::DONE),
    ));
    nodes
}
