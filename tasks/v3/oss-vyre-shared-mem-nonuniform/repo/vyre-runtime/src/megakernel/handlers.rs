//! Built-in opcode handler bodies  -  STORE_U32, ATOMIC_ADD, PRINTF, SHUTDOWN.
//!
//! Each function returns a `Vec<Node>` that executes when the opcode
//! matches in the claimed-slot dispatch. Variables `arg0`, `arg1`,
//! `arg2`, `slot_base` are in scope.

use vyre_foundation::ir::{Expr, Node};

use super::ir_util::{atomic_load_relaxed, atomic_store_relaxed};
use super::protocol::{control, debug, opcode, ARGS_PER_SLOT};

/// Caller-supplied opcode extension wired into the megakernel at
/// bootstrap. The `body` executes when `opcode` matches; within the
/// body, variables `slot_base`, `opcode`, `arg0..arg2` and the
/// buffers `control`/`ring_buffer`/`debug_log` are in scope.
#[derive(Debug, Clone)]
pub struct OpcodeHandler {
    /// Discriminant matched against `ring_buffer[slot_base + OPCODE_WORD]`.
    pub opcode: u32,
    /// IR nodes executed when the match lands.
    pub body: Vec<Node>,
}

/// Wrap a body in `if opcode == discriminant { body }`.
pub(crate) fn opcode_if(op: u32, body: Vec<Node>) -> Node {
    Node::if_then(Expr::eq(Expr::var("opcode"), Expr::u32(op)), body)
}

pub(crate) fn store_u32_body() -> Vec<Node> {
    vec![atomic_store_relaxed(
        "store_u32_prev",
        "control",
        Expr::var("arg1"),
        Expr::var("arg0"),
    )]
}

pub(crate) fn atomic_add_body() -> Vec<Node> {
    vec![Node::let_bind(
        "atomic_add_prev",
        Expr::atomic_add("control", Expr::var("arg1"), Expr::var("arg0")),
    )]
}

pub(crate) fn shutdown_body() -> Vec<Node> {
    vec![Node::let_bind(
        "shutdown_prev",
        Expr::atomic_exchange("control", Expr::u32(control::SHUTDOWN), Expr::u32(1)),
    )]
}

pub(crate) fn printf_body() -> Vec<Node> {
    // Reserve 4 u32 words at debug_log[cursor..cursor+4] atomically,
    // then write (fmt_id=arg0, arg1, arg2, slot_base) into them.
    // atomic_add returns the pre-increment value  -  our reservation
    // base.
    vec![
        Node::let_bind(
            "printf_base",
            Expr::add(
                Expr::atomic_add(
                    "debug_log",
                    Expr::u32(debug::CURSOR_WORD),
                    Expr::u32(debug::RECORD_WORDS),
                ),
                Expr::u32(debug::RECORDS_BASE),
            ),
        ),
        Node::if_then(
            Expr::le(
                Expr::add(Expr::var("printf_base"), Expr::u32(debug::RECORD_WORDS)),
                Expr::u32(debug::BUFFER_WORDS),
            ),
            vec![
                atomic_store_relaxed(
                    "printf_fmt_prev",
                    "debug_log",
                    Expr::var("printf_base"),
                    Expr::var("arg0"),
                ),
                atomic_store_relaxed(
                    "printf_arg1_prev",
                    "debug_log",
                    Expr::add(Expr::var("printf_base"), Expr::u32(1)),
                    Expr::var("arg1"),
                ),
                atomic_store_relaxed(
                    "printf_arg2_prev",
                    "debug_log",
                    Expr::add(Expr::var("printf_base"), Expr::u32(2)),
                    Expr::var("arg2"),
                ),
                atomic_store_relaxed(
                    "printf_slot_prev",
                    "debug_log",
                    Expr::add(Expr::var("printf_base"), Expr::u32(3)),
                    Expr::var("slot_base"),
                ),
            ],
        ),
    ]
}

// --- V6.4 new opcode bodies ---

/// LOAD_U32: copy `control[arg0]` into `control[OBSERVABLE_BASE + arg1]`.
pub(crate) fn load_u32_body() -> Vec<Node> {
    vec![atomic_store_relaxed(
        "load_u32_observable_prev",
        "control",
        Expr::add(Expr::u32(control::OBSERVABLE_BASE), Expr::var("arg1")),
        atomic_load_relaxed("control", Expr::var("arg0")),
    )]
}

/// COMPARE_SWAP: CAS on `control[arg0]`, expected=arg1, desired=arg2.
/// Write the previous value (before CAS) to `control[OBSERVABLE_BASE + arg0]`
/// so the host can detect success (prev == expected means swap happened).
pub(crate) fn compare_swap_body() -> Vec<Node> {
    vec![
        Node::let_bind(
            "cas_prev",
            Expr::atomic_compare_exchange(
                "control",
                Expr::var("arg0"),
                Expr::var("arg1"),
                Expr::var("arg2"),
            ),
        ),
        atomic_store_relaxed(
            "cas_observable_prev",
            "control",
            Expr::add(Expr::u32(control::OBSERVABLE_BASE), Expr::var("arg0")),
            Expr::var("cas_prev"),
        ),
    ]
}

/// MEMCPY: copy `control[arg0..arg0+arg2]` → `control[arg1..arg1+arg2]`.
/// Sequential loop  -  fine for small copies within the control buffer.
pub(crate) fn memcpy_body() -> Vec<Node> {
    vec![Node::loop_for(
        "copy_i",
        Expr::u32(0),
        Expr::var("arg2"),
        vec![atomic_store_relaxed(
            "memcpy_dst_prev",
            "control",
            Expr::add(Expr::var("arg1"), Expr::var("copy_i")),
            atomic_load_relaxed("control", Expr::add(Expr::var("arg0"), Expr::var("copy_i"))),
        )],
    )]
}

/// BATCH_FENCE: atomically increment `control[EPOCH]` and write the
/// user-tag (`arg1`) to `control[OBSERVABLE_BASE]`.
pub(crate) fn batch_fence_body() -> Vec<Node> {
    vec![
        Node::let_bind(
            "epoch_prev",
            Expr::atomic_add("control", Expr::u32(control::EPOCH), Expr::u32(1)),
        ),
        atomic_store_relaxed(
            "fence_observable_prev",
            "control",
            Expr::u32(control::OBSERVABLE_BASE),
            Expr::var("arg1"),
        ),
    ]
}

/// LOAD_MISS: GPU-initiated DMA request to the IO queue.
///
/// Reads the consumer's `resource_id` from `arg0` and `prefetch_flag` from
/// `arg1`, scans the IO queue for an empty slot, writes a READ request,
/// and spins until the host/runtime marks it OK. vyre is opaque to the
/// resource identifier  -  it's just a u32 the consumer uses to look up
/// the source and destination of the read.
pub(crate) fn load_miss_body() -> Vec<Node> {
    let io_slot_count = super::io::IO_SLOT_COUNT;
    let io_slot_words = super::io::IO_SLOT_WORDS;

    vec![
        Node::let_bind(
            "resource_id",
            Expr::load(
                "ring_buffer",
                Expr::add(
                    Expr::var("slot_base"),
                    Expr::u32(super::protocol::ARG0_WORD),
                ),
            ),
        ),
        Node::let_bind(
            "prefetch_flag",
            Expr::load(
                "ring_buffer",
                Expr::add(
                    Expr::var("slot_base"),
                    Expr::u32(super::protocol::ARG0_WORD + 1),
                ),
            ),
        ),
        // Scan for an empty IO slot.
        Node::let_bind("found_io_slot", Expr::u32(io_slot_count)),
        Node::loop_for(
            "scan_i",
            Expr::u32(0),
            Expr::u32(io_slot_count),
            vec![
                Node::if_then(
                    Expr::ne(Expr::var("found_io_slot"), Expr::u32(io_slot_count)),
                    vec![], // already found, skip remaining scan iterations
                ),
                Node::if_then(
                    Expr::eq(Expr::var("found_io_slot"), Expr::u32(io_slot_count)),
                    vec![
                        Node::let_bind(
                            "scan_base",
                            Expr::mul(Expr::var("scan_i"), Expr::u32(io_slot_words)),
                        ),
                        Node::let_bind(
                            "scan_status",
                            Expr::load(
                                "io_queue",
                                Expr::add(
                                    Expr::var("scan_base"),
                                    Expr::u32(super::io::io_word::STATUS),
                                ),
                            ),
                        ),
                        Node::if_then(
                            Expr::eq(
                                Expr::var("scan_status"),
                                Expr::u32(super::protocol::slot::EMPTY),
                            ),
                            vec![Node::assign("found_io_slot", Expr::var("scan_i"))],
                        ),
                    ],
                ),
            ],
        ),
        // If a slot was found, write the DMA request and poll for completion.
        Node::if_then(
            Expr::ne(Expr::var("found_io_slot"), Expr::u32(io_slot_count)),
            vec![
                Node::let_bind(
                    "io_base",
                    Expr::mul(Expr::var("found_io_slot"), Expr::u32(io_slot_words)),
                ),
                Node::store(
                    "io_queue",
                    Expr::add(Expr::var("io_base"), Expr::u32(super::io::io_word::OP_TYPE)),
                    Expr::u32(super::io::io_op::READ),
                ),
                Node::store(
                    "io_queue",
                    Expr::add(
                        Expr::var("io_base"),
                        Expr::u32(super::io::io_word::SRC_HANDLE),
                    ),
                    Expr::var("resource_id"),
                ),
                Node::store(
                    "io_queue",
                    Expr::add(
                        Expr::var("io_base"),
                        Expr::u32(super::io::io_word::DST_HANDLE),
                    ),
                    Expr::var("resource_id"),
                ),
                Node::store(
                    "io_queue",
                    Expr::add(
                        Expr::var("io_base"),
                        Expr::u32(super::io::io_word::OFFSET_LO),
                    ),
                    Expr::u32(0),
                ),
                Node::store(
                    "io_queue",
                    Expr::add(
                        Expr::var("io_base"),
                        Expr::u32(super::io::io_word::OFFSET_HI),
                    ),
                    Expr::u32(0),
                ),
                Node::store(
                    "io_queue",
                    Expr::add(
                        Expr::var("io_base"),
                        Expr::u32(super::io::io_word::BYTE_COUNT),
                    ),
                    Expr::u32(0),
                ),
                Node::store(
                    "io_queue",
                    Expr::add(Expr::var("io_base"), Expr::u32(super::io::io_word::TAG)),
                    Expr::var("resource_id"),
                ),
                // Publish the request.
                Node::store(
                    "io_queue",
                    Expr::add(Expr::var("io_base"), Expr::u32(super::io::io_word::STATUS)),
                    Expr::u32(super::protocol::slot::PUBLISHED),
                ),
                // Poll until the host/runtime marks it OK.
                Node::let_bind("poll_done", Expr::u32(0)),
                Node::let_bind("poll_max_iters", Expr::u32(u32::MAX)),
                Node::loop_for(
                    "poll_i",
                    Expr::u32(0),
                    Expr::var("poll_max_iters"),
                    vec![
                        Node::if_then(
                            Expr::eq(Expr::var("poll_done"), Expr::u32(1)),
                            vec![], // skip once done
                        ),
                        Node::if_then(
                            Expr::ne(Expr::var("poll_done"), Expr::u32(1)),
                            vec![
                                Node::let_bind(
                                    "poll_status",
                                    Expr::load(
                                        "io_queue",
                                        Expr::add(
                                            Expr::var("io_base"),
                                            Expr::u32(super::io::io_word::STATUS),
                                        ),
                                    ),
                                ),
                                Node::if_then(
                                    Expr::eq(
                                        Expr::var("poll_status"),
                                        Expr::u32(super::io::io_status::OK),
                                    ),
                                    vec![
                                        Node::store(
                                            "io_queue",
                                            Expr::add(
                                                Expr::var("io_base"),
                                                Expr::u32(super::io::io_word::STATUS),
                                            ),
                                            Expr::u32(super::protocol::slot::EMPTY),
                                        ),
                                        Node::assign("poll_done", Expr::u32(1)),
                                    ],
                                ),
                            ],
                        ),
                    ],
                ),
            ],
        ),
    ]
}

fn packed_payload_byte(byte_offset: Expr) -> Expr {
    let word_offset = Expr::div(byte_offset.clone(), Expr::u32(4));
    let bit_shift = Expr::mul(Expr::rem(byte_offset, Expr::u32(4)), Expr::u32(8));
    let word = Expr::load(
        "ring_buffer",
        Expr::add(
            Expr::add(
                Expr::var("slot_base"),
                Expr::u32(super::protocol::ARG0_WORD),
            ),
            word_offset,
        ),
    );
    Expr::bitand(Expr::shr(word, bit_shift), Expr::u32(0xFF))
}

fn dispatch_opcode_body(opcodes: &[OpcodeHandler]) -> Vec<Node> {
    let mut nodes = vec![
        Node::if_then(
            Expr::lt(Expr::var("opcode"), Expr::u32(control::METRICS_SLOTS)),
            vec![Node::let_bind(
                "metric_prev",
                Expr::atomic_add(
                    "control",
                    Expr::add(Expr::u32(control::METRICS_BASE), Expr::var("opcode")),
                    Expr::u32(1),
                ),
            )],
        ),
        opcode_if(opcode::STORE_U32, store_u32_body()),
        opcode_if(opcode::ATOMIC_ADD, atomic_add_body()),
        opcode_if(opcode::LOAD_U32, load_u32_body()),
        opcode_if(opcode::COMPARE_SWAP, compare_swap_body()),
        opcode_if(opcode::MEMCPY, memcpy_body()),
        opcode_if(opcode::BATCH_FENCE, batch_fence_body()),
        opcode_if(opcode::PRINTF, printf_body()),
        opcode_if(opcode::SHUTDOWN, shutdown_body()),
    ];

    for handler in opcodes {
        nodes.push(opcode_if(handler.opcode, handler.body.clone()));
    }

    nodes
}

pub(crate) fn packed_slot_body(opcodes: &[OpcodeHandler]) -> Vec<Node> {
    vec![
        Node::let_bind("packed_raw_opcode_count", packed_payload_byte(Expr::u32(0))),
        Node::let_bind(
            "packed_opcode_count",
            Expr::select(
                Expr::gt(
                    Expr::var("packed_raw_opcode_count"),
                    Expr::u32(ARGS_PER_SLOT / 3),
                ),
                Expr::u32(ARGS_PER_SLOT / 3),
                Expr::var("packed_raw_opcode_count"),
            ),
        ),
        Node::let_bind(
            "packed_metadata_bytes",
            Expr::add(
                Expr::u32(2),
                Expr::mul(Expr::var("packed_opcode_count"), Expr::u32(2)),
            ),
        ),
        Node::let_bind(
            "packed_metadata_words",
            Expr::div(
                Expr::add(Expr::var("packed_metadata_bytes"), Expr::u32(3)),
                Expr::u32(4),
            ),
        ),
        Node::loop_for(
            "packed_inner_index",
            Expr::u32(0),
            Expr::var("packed_opcode_count"),
            vec![Node::block(vec![
                Node::let_bind(
                    "packed_pair_byte",
                    Expr::add(
                        Expr::u32(2),
                        Expr::mul(Expr::var("packed_inner_index"), Expr::u32(2)),
                    ),
                ),
                Node::let_bind(
                    "packed_opcode",
                    packed_payload_byte(Expr::var("packed_pair_byte")),
                ),
                Node::let_bind(
                    "packed_arg_offset",
                    packed_payload_byte(Expr::add(Expr::var("packed_pair_byte"), Expr::u32(1))),
                ),
                Node::let_bind(
                    "packed_arg_base",
                    Expr::add(
                        Expr::var("packed_metadata_words"),
                        Expr::var("packed_arg_offset"),
                    ),
                ),
                Node::assign("opcode", Expr::var("packed_opcode")),
                Node::assign(
                    "arg0",
                    Expr::load(
                        "ring_buffer",
                        Expr::add(
                            Expr::add(
                                Expr::var("slot_base"),
                                Expr::u32(super::protocol::ARG0_WORD),
                            ),
                            Expr::var("packed_arg_base"),
                        ),
                    ),
                ),
                Node::assign(
                    "arg1",
                    Expr::load(
                        "ring_buffer",
                        Expr::add(
                            Expr::add(
                                Expr::var("slot_base"),
                                Expr::u32(super::protocol::ARG0_WORD),
                            ),
                            Expr::add(Expr::var("packed_arg_base"), Expr::u32(1)),
                        ),
                    ),
                ),
                Node::assign(
                    "arg2",
                    Expr::load(
                        "ring_buffer",
                        Expr::add(
                            Expr::add(
                                Expr::var("slot_base"),
                                Expr::u32(super::protocol::ARG0_WORD),
                            ),
                            Expr::add(Expr::var("packed_arg_base"), Expr::u32(2)),
                        ),
                    ),
                ),
                Node::if_then(
                    Expr::le(
                        Expr::add(Expr::var("packed_arg_base"), Expr::u32(3)),
                        Expr::u32(ARGS_PER_SLOT),
                    ),
                    vec![Node::block(dispatch_opcode_body(opcodes))],
                ),
            ])],
        ),
    ]
}

/// Build the claimed-slot dispatch body (opcode If-tree + custom handlers).
pub(crate) fn claimed_slot_bindings() -> Vec<Node> {
    vec![
        Node::let_bind(
            "opcode",
            Expr::load(
                "ring_buffer",
                Expr::add(
                    Expr::var("slot_base"),
                    Expr::u32(super::protocol::OPCODE_WORD),
                ),
            ),
        ),
        Node::let_bind(
            "arg0",
            Expr::load(
                "ring_buffer",
                Expr::add(
                    Expr::var("slot_base"),
                    Expr::u32(super::protocol::ARG0_WORD),
                ),
            ),
        ),
        Node::let_bind(
            "arg1",
            Expr::load(
                "ring_buffer",
                Expr::add(
                    Expr::var("slot_base"),
                    Expr::u32(super::protocol::ARG0_WORD + 1),
                ),
            ),
        ),
        Node::let_bind(
            "arg2",
            Expr::load(
                "ring_buffer",
                Expr::add(
                    Expr::var("slot_base"),
                    Expr::u32(super::protocol::ARG0_WORD + 2),
                ),
            ),
        ),
    ]
}

/// Build the claimed-slot dispatch body (opcode If-tree + custom handlers).
pub(crate) fn claimed_slot_body(opcodes: &[OpcodeHandler]) -> Vec<Node> {
    let mut nodes = claimed_slot_bindings();
    nodes.push(Node::block(dispatch_opcode_body(opcodes)));
    nodes.push(opcode_if(opcode::PACKED_SLOT, packed_slot_body(opcodes)));

    // Tally progress so the host can observe done_count.
    nodes.push(Node::let_bind(
        "done_prev",
        Expr::atomic_add("control", Expr::u32(control::DONE_COUNT), Expr::u32(1)),
    ));

    // Mark slot DONE.
    nodes.push(Node::store(
        "ring_buffer",
        Expr::var("status_index"),
        Expr::u32(super::protocol::slot::DONE),
    ));

    nodes
}

#[cfg(test)]
mod tests;
