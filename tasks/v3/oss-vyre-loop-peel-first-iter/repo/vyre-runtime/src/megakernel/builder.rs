//! IR program builders  -  construct the megakernel `Program` from vyre IR.
//!
//! Two flavours:
//! - **Interpreted** (`build_program_sharded`)  -  If-tree opcode dispatch.
//! - **JIT** (`build_program_jit`)  -  payload processor fused directly.

use std::sync::Arc;

use vyre_foundation::ir::{BufferDecl, DataType, Expr, Node, Program};

use super::handlers::{claimed_slot_bindings, claimed_slot_body, load_miss_body, OpcodeHandler};
use super::io::{
    io_word, IO_DESTINATION_CAPABILITY_TABLE, IO_QUEUE_DMA_TAG, IO_SLOT_COUNT, IO_SLOT_WORDS,
    IO_SOURCE_CAPABILITY_TABLE,
};
use super::ir_util::atomic_load_relaxed;
use super::protocol::*;
use super::workspace_adapter::MegakernelWorkspaceAdapter;
mod cache;
mod jit;
mod priority;
pub use jit::{build_program_jit, build_program_jit_slots, persistent_body_jit};
pub use priority::{
    build_program_priority, build_program_priority_slots, persistent_body_priority,
    persistent_body_priority_slots,
};

/// Build the default megakernel IR (256 lanes × 1 workgroup, no custom opcodes).
#[must_use]
pub fn build_program() -> Program {
    build_program_sharded(256, &[])
}

/// Build the megakernel IR with a custom workgroup size and optional
/// custom opcodes.
///
/// Buffers are declared with concrete `with_count(...)` sizes so the
/// backend readback layer allocates the right static staging size  -  a
/// `count=0` default reads back 4 bytes regardless of how much the
/// kernel wrote.
#[must_use]
pub fn build_program_sharded(workgroup_size_x: u32, opcodes: &[OpcodeHandler]) -> Program {
    build_program_sharded_slots(workgroup_size_x, workgroup_size_x.max(1), opcodes)
}

/// Build the megakernel IR for an explicit number of ring slots.
///
/// This is the production sharded ABI: `slot_count` sizes the ring buffer,
/// while `workgroup_size_x` controls lanes per workgroup. Dispatch must launch
/// `slot_count / workgroup_size_x` workgroups so every slot has an owning lane.
#[must_use]
pub fn build_program_sharded_slots(
    workgroup_size_x: u32,
    slot_count: u32,
    opcodes: &[OpcodeHandler],
) -> Program {
    build_program_sharded_slots_with_io(workgroup_size_x, slot_count, opcodes, false)
}

/// Build the sharded megakernel IR as a shared immutable template.
///
/// Empty opcode sets use the thread-local template cache directly, allowing
/// compile paths to avoid cloning the cached Program before wrapping it in
/// `Arc` again.
#[must_use]
pub fn build_program_sharded_slots_shared(
    workgroup_size_x: u32,
    slot_count: u32,
    opcodes: &[OpcodeHandler],
) -> Arc<Program> {
    if opcodes.is_empty() {
        return cache::cached_empty_sharded_program_shared(workgroup_size_x, slot_count, false);
    }
    Arc::new(build_program_sharded_slots(
        workgroup_size_x,
        slot_count,
        opcodes,
    ))
}

/// Build the sharded megakernel IR with a consumer-owned resident workspace.
#[must_use]
pub fn build_program_sharded_with_workspace_adapter(
    workgroup_size_x: u32,
    slot_count: u32,
    opcodes: &[OpcodeHandler],
    adapter: &impl MegakernelWorkspaceAdapter,
) -> Program {
    wrap_persistent_megakernel_program_with_buffers(
        default_buffers_with_workspace_adapter(slot_count, adapter),
        workgroup_size_x,
        persistent_body_with_workspace_adapter(workgroup_size_x, opcodes, adapter),
    )
}

/// Build a finite one-pass sharded megakernel IR for host-submitted batches.
///
/// Unlike [`build_program_sharded_slots`], this program does not wrap the body
/// in `Node::forever`; each lane attempts to drain its owning slot once and the
/// dispatch returns. Use this for synchronous batch APIs that need a completion
/// report from the same queue submission.
#[must_use]
pub fn build_program_sharded_once_slots(
    workgroup_size_x: u32,
    slot_count: u32,
    opcodes: &[OpcodeHandler],
) -> Program {
    if opcodes.is_empty() {
        return cache::cached_empty_sharded_once_program(workgroup_size_x, slot_count);
    }
    wrap_megakernel_program(
        workgroup_size_x,
        slot_count,
        persistent_body_with_io(workgroup_size_x, opcodes, false),
    )
}

/// Shared-Arc variant of [`build_program_sharded_once_slots`] for hot runtime
/// dispatchers that must not clone the megakernel template every launch.
#[must_use]
pub fn build_program_sharded_once_slots_shared(
    workgroup_size_x: u32,
    slot_count: u32,
    opcodes: &[OpcodeHandler],
) -> Arc<Program> {
    if opcodes.is_empty() {
        return cache::cached_empty_sharded_once_program_shared(workgroup_size_x, slot_count);
    }
    Arc::new(build_program_sharded_once_slots(
        workgroup_size_x,
        slot_count,
        opcodes,
    ))
}

/// Build a finite one-pass megakernel that reports completion through the
/// control buffer only.
///
/// Ring, debug, and IO buffers remain read-write device buffers, but their
/// host readback ranges are empty. This is the hot dispatcher path: completion
/// is already accumulated into control, so reading back the full ring/debug/IO
/// surfaces is redundant launch latency.
#[must_use]
pub fn build_program_sharded_once_slots_control_report_shared(
    workgroup_size_x: u32,
    slot_count: u32,
    opcodes: &[OpcodeHandler],
) -> Arc<Program> {
    if opcodes.is_empty() {
        return cache::cached_empty_sharded_once_control_report_program_shared(
            workgroup_size_x,
            slot_count,
        );
    }
    let mut buffers = default_buffers(slot_count);
    for buffer in buffers.iter_mut().skip(1) {
        buffer.output_byte_range = Some(0..0);
    }
    Arc::new(optimize_megakernel_program(Program::wrapped(
        buffers,
        [workgroup_size_x, 1, 1],
        persistent_body_with_io(workgroup_size_x, opcodes, false),
    )))
}

/// Build the megakernel IR without the IO polling sidecar.
///
/// This is the dispatch path for host-provided [`super::MegakernelWorkItem`]
/// queues. It keeps the executable kernel free of `AsyncLoad` nodes until the
/// runtime scheduler owns a concrete async-lowering pass.
#[must_use]
pub fn build_program_sharded_no_io(workgroup_size_x: u32, opcodes: &[OpcodeHandler]) -> Program {
    build_program_sharded_slots(workgroup_size_x, workgroup_size_x.max(1), opcodes)
}

/// Build the megakernel IR with the experimental IO polling sidecar.
///
/// The returned Program contains `AsyncLoad` nodes and must be lowered through
/// a runtime scheduler pass before reaching a concrete backend lowering path.
#[must_use]
pub fn build_program_sharded_with_io_polling(
    workgroup_size_x: u32,
    opcodes: &[OpcodeHandler],
) -> Program {
    build_program_sharded_slots_with_io(workgroup_size_x, workgroup_size_x.max(1), opcodes, true)
}

/// Build the megakernel IR with a self-loading load-miss handler.
///
/// The persistent loop is extended with an [`opcode::LOAD_MISS`] handler.
/// When the GPU sees this opcode it scans the IO queue for an empty slot,
/// writes a DMA-read request, and polls until the host/runtime marks it
/// complete. The `arg0` field of the slot is the consumer's opaque
/// resource identifier; vyre does not interpret it.
#[must_use]
#[cfg(any(test, feature = "legacy-infallible"))]
pub fn build_program_with_self_loading_miss_handler(
    workgroup_size_x: u32,
    slot_count: u32,
    opcodes: &[OpcodeHandler],
) -> Program {
    match try_build_program_with_self_loading_miss_handler(workgroup_size_x, slot_count, opcodes) {
        Ok(program) => program,
        Err(error) => panic!("{error}"),
    }
}

/// Fallible variant of [`build_program_with_self_loading_miss_handler`].
pub fn try_build_program_with_self_loading_miss_handler(
    workgroup_size_x: u32,
    slot_count: u32,
    opcodes: &[OpcodeHandler],
) -> Result<Program, String> {
    let mut extended = Vec::new();
    let extended_len = opcodes.len().checked_add(1).ok_or_else(|| {
        "megakernel self-loading opcode extension count overflowed usize. Fix: split opcode handler sets before building the megakernel."
            .to_string()
    })?;
    vyre_foundation::allocation::try_reserve_vec_to_capacity(&mut extended, extended_len).map_err(|error| {
        format!(
            "megakernel self-loading opcode extension allocation failed: {error}. Fix: split opcode handler sets before building the megakernel."
        )
    })?;
    extended.extend_from_slice(opcodes);
    extended.push(OpcodeHandler {
        opcode: super::protocol::opcode::LOAD_MISS,
        body: load_miss_body(),
    });
    Ok(wrap_persistent_megakernel_program(
        workgroup_size_x,
        slot_count,
        persistent_body_with_io(workgroup_size_x, &extended, false),
    ))
}

fn build_program_sharded_slots_with_io(
    workgroup_size_x: u32,
    slot_count: u32,
    opcodes: &[OpcodeHandler],
    include_io_polling: bool,
) -> Program {
    if opcodes.is_empty() {
        return cache::cached_empty_sharded_program(
            workgroup_size_x,
            slot_count,
            include_io_polling,
        );
    }
    wrap_persistent_megakernel_program(
        workgroup_size_x,
        slot_count,
        persistent_body_with_io(workgroup_size_x, opcodes, include_io_polling),
    )
}

fn wrap_persistent_megakernel_program(
    workgroup_size_x: u32,
    slot_count: u32,
    body: Vec<Node>,
) -> Program {
    wrap_megakernel_program(workgroup_size_x, slot_count, vec![Node::forever(body)])
}

fn wrap_persistent_megakernel_program_with_buffers(
    buffers: Vec<BufferDecl>,
    workgroup_size_x: u32,
    body: Vec<Node>,
) -> Program {
    optimize_megakernel_program(Program::wrapped(
        buffers,
        [workgroup_size_x, 1, 1],
        vec![Node::forever(body)],
    ))
}

fn wrap_megakernel_program(workgroup_size_x: u32, slot_count: u32, body: Vec<Node>) -> Program {
    optimize_megakernel_program(Program::wrapped(
        default_buffers(slot_count),
        [workgroup_size_x, 1, 1],
        body,
    ))
}

fn optimize_megakernel_program(program: Program) -> Program {
    // Barrier elision is infallible (its working buffers are sized by the bounded
    // IR node count), so there is no staging-failure fallback that could silently
    // ship the un-elided program — the pass always runs (Law 10).
    let (program, _) = super::planner::elide_value_flow_barriers(program);
    vyre_foundation::optimizer::pre_lowering::optimize(program)
}

/// Reserve sizes for the megakernel's four host-visible buffers. All
/// four go through the static-readback path so every buffer needs
/// a concrete `count` (u32 elements). The numbers mirror the wire
/// layout in `protocol.rs`:
///
/// - **control**: 128 u32 words covers SHUTDOWN, DONE_COUNT, EPOCH,
///   METRICS_BASE..METRICS_BASE+METRICS_SLOTS, OBSERVABLE_BASE, and
///   the 32-entry tenant-mask table.
/// - **ring_buffer**: `slot_count` slots × `SLOT_WORDS`.
///   `slot_count` must match host-published ring bytes and dispatch geometry.
/// - **debug_log**: cursor word + `debug::RECORD_CAPACITY` × 4-word records.
/// - **io_queue**: 64 slots × 8 words (source, destination,
///   offset_low, offset_high, size, status, tag, pad).
fn default_buffers(slot_count: u32) -> Vec<BufferDecl> {
    let ring_slots = slot_count.max(1);
    let control = BufferDecl::read_write("control", 0, DataType::U32).with_count(CONTROL_MIN_WORDS);
    let ring_buffer = BufferDecl::read_write("ring_buffer", 1, DataType::U32)
        .with_count(ring_slots.saturating_mul(SLOT_WORDS));
    let debug_log =
        BufferDecl::read_write("debug_log", 2, DataType::U32).with_count(debug::BUFFER_WORDS);
    let io_queue = BufferDecl::read_write("io_queue", 3, DataType::U32).with_count(64 * 8);
    vec![control, ring_buffer, debug_log, io_queue]
}

fn default_buffers_with_workspace_adapter(
    slot_count: u32,
    adapter: &impl MegakernelWorkspaceAdapter,
) -> Vec<BufferDecl> {
    let mut buffers = default_buffers(slot_count);
    buffers.push(adapter.buffer_decl());
    buffers
}

/// The body that runs once per iteration per lane. Exposed for tests
/// and downstream crates that splice additional opcodes.
#[must_use]
pub fn persistent_body(workgroup_size_x: u32, opcodes: &[OpcodeHandler]) -> Vec<Node> {
    persistent_body_with_io(workgroup_size_x, opcodes, false)
}

/// Fallible persistent body builder with explicit staging-allocation reporting.
pub fn try_persistent_body(
    workgroup_size_x: u32,
    opcodes: &[OpcodeHandler],
) -> Result<Vec<Node>, String> {
    try_persistent_body_with_io(workgroup_size_x, opcodes, false)
}

fn persistent_body_with_io(
    workgroup_size_x: u32,
    opcodes: &[OpcodeHandler],
    include_io_polling: bool,
) -> Vec<Node> {
    let mut body = persistent_lane_prologue(workgroup_size_x);
    let additional_nodes = if include_io_polling { 3 } else { 2 };
    if let Some(body_capacity) = body.len().checked_add(additional_nodes) {
        let _ = vyre_foundation::allocation::try_reserve_vec_to_capacity(&mut body, body_capacity);
    }
    body.push(direct_slot_base_binding());
    body.push(Node::Block(execute_slot_body(opcodes)));
    if include_io_polling {
        body.push(Node::Block(process_io_requests()));
    }
    body
}

fn try_persistent_body_with_io(
    workgroup_size_x: u32,
    opcodes: &[OpcodeHandler],
    include_io_polling: bool,
) -> Result<Vec<Node>, String> {
    let mut body = persistent_lane_prologue(workgroup_size_x);
    let additional_nodes = if include_io_polling { 3 } else { 2 };
    let body_capacity = body.len().checked_add(additional_nodes).ok_or_else(|| {
        "megakernel persistent body node reservation overflowed usize. Fix: reduce fused IO/body staging before building the megakernel."
            .to_string()
    })?;
    vyre_foundation::allocation::try_reserve_vec_to_capacity(&mut body, body_capacity).map_err(|error| {
        format!(
            "megakernel persistent body node reservation failed: {error}. Fix: reduce fused IO/body staging before building the megakernel."
        )
    })?;
    body.push(direct_slot_base_binding());
    body.push(Node::Block(execute_slot_body(opcodes)));
    if include_io_polling {
        body.push(Node::Block(process_io_requests()));
    }
    Ok(body)
}

fn persistent_lane_prologue(workgroup_size_x: u32) -> Vec<Node> {
    vec![
        Node::let_bind(
            "shutdown_flag",
            atomic_load_relaxed("control", Expr::u32(control::SHUTDOWN)),
        ),
        Node::if_then(
            Expr::ne(Expr::var("shutdown_flag"), Expr::u32(0)),
            vec![Node::Return],
        ),
        Node::let_bind("lane_id", lane_id_expr(workgroup_size_x)),
    ]
}

fn direct_slot_base_binding() -> Node {
    Node::let_bind(
        "slot_base",
        Expr::mul(Expr::var("lane_id"), Expr::u32(SLOT_WORDS)),
    )
}

fn slot_tenant_id_load() -> Expr {
    Expr::load(
        "ring_buffer",
        Expr::add(Expr::var("slot_base"), Expr::u32(TENANT_WORD)),
    )
}

fn tenant_authorized_body(tenant_id: Expr, authorized_body: Vec<Node>) -> Vec<Node> {
    vec![
        Node::let_bind("tenant_id", tenant_id),
        Node::let_bind(
            "tenant_base",
            atomic_load_relaxed("control", Expr::u32(control::TENANT_BASE)),
        ),
        Node::let_bind(
            "tenant_mask",
            atomic_load_relaxed(
                "control",
                Expr::add(Expr::var("tenant_base"), Expr::var("tenant_id")),
            ),
        ),
        Node::if_then(
            Expr::ne(Expr::var("tenant_mask"), Expr::u32(0)),
            authorized_body,
        ),
    ]
}

fn lane_id_expr(workgroup_size_x: u32) -> Expr {
    Expr::add(
        Expr::mul(Expr::workgroup_x(), Expr::u32(workgroup_size_x)),
        Expr::local_x(),
    )
}

fn persistent_body_with_workspace_adapter(
    workgroup_size_x: u32,
    opcodes: &[OpcodeHandler],
    adapter: &impl MegakernelWorkspaceAdapter,
) -> Vec<Node> {
    let mut body = adapter.bootstrap_nodes();
    body.extend(adapter.guard_nodes());
    body.extend(adapter.dispatch_nodes());
    body.extend(persistent_body_with_io(workgroup_size_x, opcodes, false));
    body
}

fn process_io_requests() -> Vec<Node> {
    let nodes = vec![Node::loop_for(
        "io_idx",
        Expr::u32(0),
        Expr::u32(IO_SLOT_COUNT),
        vec![
            Node::let_bind(
                "io_base",
                Expr::mul(Expr::var("io_idx"), Expr::u32(IO_SLOT_WORDS)),
            ),
            Node::let_bind(
                "io_status_idx",
                Expr::add(Expr::var("io_base"), Expr::u32(io_word::STATUS)),
            ),
            // CAS PUBLISHED -> CLAIMED
            Node::let_bind(
                "prev_io_status",
                Expr::atomic_compare_exchange(
                    "io_queue",
                    Expr::var("io_status_idx"),
                    Expr::u32(slot::PUBLISHED),
                    Expr::u32(slot::CLAIMED),
                ),
            ),
            Node::if_then(
                Expr::eq(Expr::var("prev_io_status"), Expr::u32(slot::PUBLISHED)),
                vec![
                    Node::let_bind(
                        "io_src_handle",
                        Expr::load(
                            "io_queue",
                            Expr::add(Expr::var("io_base"), Expr::u32(io_word::SRC_HANDLE)),
                        ),
                    ),
                    Node::let_bind(
                        "io_dst_handle",
                        Expr::load(
                            "io_queue",
                            Expr::add(Expr::var("io_base"), Expr::u32(io_word::DST_HANDLE)),
                        ),
                    ),
                    Node::AsyncLoad {
                        source: IO_SOURCE_CAPABILITY_TABLE.into(),
                        destination: IO_DESTINATION_CAPABILITY_TABLE.into(),
                        offset: Box::new(Expr::load(
                            "io_queue",
                            Expr::add(Expr::var("io_base"), Expr::u32(io_word::OFFSET_LO)),
                        )),
                        size: Box::new(Expr::load(
                            "io_queue",
                            Expr::add(Expr::var("io_base"), Expr::u32(io_word::BYTE_COUNT)),
                        )),
                        tag: IO_QUEUE_DMA_TAG.into(),
                    },
                    // Mark as DONE
                    Node::store(
                        "io_queue",
                        Expr::var("io_status_idx"),
                        Expr::u32(slot::DONE),
                    ),
                ],
            ),
        ],
    )];

    nodes
}

fn execute_slot_body(opcodes: &[OpcodeHandler]) -> Vec<Node> {
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
            tenant_authorized_claim_body(slot_tenant_id_load(), claimed_slot_body(opcodes)),
        ),
    ]
}

fn tenant_authorized_claim_body(tenant_id: Expr, claimed_body: Vec<Node>) -> Vec<Node> {
    tenant_authorized_body(
        tenant_id,
        vec![
            // CAS PUBLISHED -> CLAIMED after authorization. This keeps
            // disabled tenants visible to the host instead of converting
            // their slots into stuck CLAIMED work.
            Node::let_bind(
                "prev_status",
                Expr::atomic_compare_exchange(
                    "ring_buffer",
                    Expr::var("status_index"),
                    Expr::u32(slot::PUBLISHED),
                    Expr::u32(slot::CLAIMED),
                ),
            ),
            Node::if_then(
                Expr::eq(Expr::var("prev_status"), Expr::u32(slot::PUBLISHED)),
                claimed_body,
            ),
        ],
    )
}

fn execute_already_claimed_slot_body(tenant_id: Expr, claimed_body: Vec<Node>) -> Vec<Node> {
    let mut body = vec![Node::let_bind(
        "status_index",
        Expr::add(Expr::var("slot_base"), Expr::u32(STATUS_WORD)),
    )];
    body.extend(tenant_authorized_body(tenant_id, claimed_body));
    body
}

#[cfg(test)]
mod tests;
