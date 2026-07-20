//! GPU-side hit-buffer append and compaction helpers.
//!
//! Hit counts are sparse relative to the `rules x files` search space, so the
//! matching pipeline appends only the live tuples into a flat u32 buffer:
//! `(rule_id, file_id, span_start, span_len)`.

use vyre::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};
use vyre_foundation::execution_plan::fusion::{fuse_programs_vec, FusionError};

use crate::region::wrap_anonymous;

const EMIT_HIT_OP_ID: &str = "vyre-libs::matching::emit_hit";
const COMPACT_HITS_OP_ID: &str = "vyre-libs::matching::compact_hits";
const DEFAULT_LANES: u32 = 4;
const DEFAULT_MAX_HITS: u32 = 4;

/// Observable output buffer carrying the number of dropped hits.
pub const HIT_BUFFER_OVERFLOW_COUNT: &str = "hit_buffer_overflow_count";
/// Observable output buffer carrying the compacted live-hit length.
pub const HIT_BUFFER_LIVE_LENGTH: &str = "hit_buffer_live_length";

/// Emit one hit tuple per active lane into a compacted GPU append buffer.
///
/// The default shape is sized for the harness. Use
/// [`emit_hit_with_layout`] when the caller needs a larger lane/count budget.
#[must_use]
pub fn emit_hit(
    rule_id: &str,
    file_id: &str,
    span_start: &str,
    span_len: &str,
    out_hits: &str,
    out_cursor: &str,
) -> Program {
    emit_hit_with_layout(
        rule_id,
        file_id,
        span_start,
        span_len,
        out_hits,
        out_cursor,
        DEFAULT_LANES,
        DEFAULT_MAX_HITS,
    )
}

/// Emit one hit tuple per active lane into a compacted GPU append buffer.
///
/// `lane_count` is the number of per-lane inputs supplied by the caller.
/// `max_hits` is the tuple capacity of `out_hits`; the backing buffer stores
/// four `u32` values per hit.
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn emit_hit_with_layout(
    rule_id: &str,
    file_id: &str,
    span_start: &str,
    span_len: &str,
    out_hits: &str,
    out_cursor: &str,
    lane_count: u32,
    max_hits: u32,
) -> Program {
    let lane = Expr::var("lane");
    let base = Expr::mul(lane.clone(), Expr::u32(4));
    let max_capacity = Expr::div(Expr::buf_len(out_hits), Expr::u32(4));
    let body = vec![
        Node::let_bind("lane", Expr::InvocationId { axis: 0 }),
        Node::if_then(
            Expr::lt(lane.clone(), Expr::buf_len(rule_id)),
            vec![Node::if_then(
                Expr::lt(lane.clone(), max_capacity.clone()),
                vec![
                    Node::store(out_hits, base.clone(), Expr::load(rule_id, lane.clone())),
                    Node::store(
                        out_hits,
                        Expr::add(base.clone(), Expr::u32(1)),
                        Expr::load(file_id, lane.clone()),
                    ),
                    Node::store(
                        out_hits,
                        Expr::add(base.clone(), Expr::u32(2)),
                        Expr::load(span_start, lane.clone()),
                    ),
                    Node::store(
                        out_hits,
                        Expr::add(base, Expr::u32(3)),
                        Expr::load(span_len, lane),
                    ),
                ],
            )],
        ),
        Node::if_then(
            Expr::eq(Expr::var("lane"), Expr::u32(0)),
            vec![
                Node::store(
                    out_cursor,
                    Expr::u32(0),
                    Expr::min(Expr::buf_len(rule_id), max_capacity.clone()),
                ),
                Node::if_then(
                    Expr::lt(max_capacity.clone(), Expr::buf_len(rule_id)),
                    vec![Node::store(
                        HIT_BUFFER_OVERFLOW_COUNT,
                        Expr::u32(0),
                        Expr::sub(Expr::buf_len(rule_id), max_capacity),
                    )],
                ),
            ],
        ),
    ];

    Program::wrapped(
        vec![
            BufferDecl::storage(rule_id, 0, BufferAccess::ReadOnly, DataType::U32)
                .with_count(lane_count),
            BufferDecl::storage(file_id, 1, BufferAccess::ReadOnly, DataType::U32)
                .with_count(lane_count),
            BufferDecl::storage(span_start, 2, BufferAccess::ReadOnly, DataType::U32)
                .with_count(lane_count),
            BufferDecl::storage(span_len, 3, BufferAccess::ReadOnly, DataType::U32)
                .with_count(lane_count),
            BufferDecl::output(out_hits, 4, DataType::U32).with_count(max_hits.saturating_mul(4)),
            BufferDecl::read_write(out_cursor, 5, DataType::U32).with_count(1),
            BufferDecl::read_write(HIT_BUFFER_OVERFLOW_COUNT, 6, DataType::U32).with_count(1),
        ],
        [64, 1, 1],
        vec![wrap_anonymous(EMIT_HIT_OP_ID, body)],
    )
}

/// Clamp the live prefix of `out_hits` to `min(cursor, max_capacity)` and
/// return the resulting hit count via [`HIT_BUFFER_LIVE_LENGTH`].
#[must_use]
pub fn compact_hits(out_hits: &str, out_cursor: &str, max_capacity: u32) -> Program {
    compact_hits_with_layout(out_hits, out_cursor, max_capacity, max_capacity)
}

/// Clamp the live prefix of `out_hits` to `min(cursor, max_capacity)` using an
/// explicit backing-hit-buffer size.
#[must_use]
pub fn compact_hits_with_layout(
    out_hits: &str,
    out_cursor: &str,
    hit_capacity: u32,
    max_capacity: u32,
) -> Program {
    let body = vec![
        Node::let_bind("lane", Expr::InvocationId { axis: 0 }),
        Node::if_then(
            Expr::eq(Expr::var("lane"), Expr::u32(0)),
            vec![
                Node::let_bind("cursor", Expr::load(out_cursor, Expr::u32(0))),
                Node::let_bind(
                    "buffer_cap",
                    Expr::div(Expr::buf_len(out_hits), Expr::u32(4)),
                ),
                Node::let_bind(
                    "live_len",
                    Expr::min(
                        Expr::var("cursor"),
                        Expr::min(Expr::u32(max_capacity), Expr::var("buffer_cap")),
                    ),
                ),
                Node::store(HIT_BUFFER_LIVE_LENGTH, Expr::u32(0), Expr::var("live_len")),
            ],
        ),
    ];

    Program::wrapped(
        vec![
            BufferDecl::storage(out_hits, 0, BufferAccess::ReadOnly, DataType::U32)
                .with_count(hit_capacity.saturating_mul(4)),
            BufferDecl::storage(out_cursor, 1, BufferAccess::ReadOnly, DataType::U32).with_count(1),
            BufferDecl::output(HIT_BUFFER_LIVE_LENGTH, 2, DataType::U32).with_count(1),
        ],
        [1, 1, 1],
        vec![wrap_anonymous(COMPACT_HITS_OP_ID, body)],
    )
}

/// FUSE-6: [`emit_hit_with_layout`] immediately followed by [`compact_hits_with_layout`]
/// in one fused program (one dispatch), preserving buffer names.
///
/// Workgroups are unified to `[64, 1, 1]`; the compact phase only runs meaningful
/// work on lane 0, matching the standalone [`compact_hits`] contract.
///
/// **Reference interpreter:** the fused program declares two `BufferDecl::output`
/// regions (hits + live length), which violates V022’s single-output rule for
/// `vyre_reference::reference_eval`. For CPU parity checks, run [`emit_hit`] and
/// [`compact_hits`] as two programs; use this fused builder for GPU megakernels.
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn emit_hit_then_compact_with_layout(
    rule_id: &str,
    file_id: &str,
    span_start: &str,
    span_len: &str,
    out_hits: &str,
    out_cursor: &str,
    lane_count: u32,
    max_hits: u32,
) -> Result<Program, FusionError> {
    let emit = emit_hit_with_layout(
        rule_id, file_id, span_start, span_len, out_hits, out_cursor, lane_count, max_hits,
    );
    let compact = compact_hits_with_layout(out_hits, out_cursor, max_hits, max_hits);
    fuse_programs_vec(vec![emit, compact])
}

/// Default-layout [`emit_hit_then_compact_with_layout`].
#[must_use]
pub fn emit_hit_then_compact(
    rule_id: &str,
    file_id: &str,
    span_start: &str,
    span_len: &str,
    out_hits: &str,
    out_cursor: &str,
) -> Result<Program, FusionError> {
    emit_hit_then_compact_with_layout(
        rule_id,
        file_id,
        span_start,
        span_len,
        out_hits,
        out_cursor,
        DEFAULT_LANES,
        DEFAULT_MAX_HITS,
    )
}

fn emit_hit_inputs() -> Vec<Vec<Vec<u8>>> {
    vec![vec![
        pack_words(&[7, 9, 11, 13]),
        pack_words(&[101, 103, 107, 109]),
        pack_words(&[5, 9, 13, 17]),
        pack_words(&[2, 4, 6, 8]),
        pack_words(&[0]),
        pack_words(&[0]),
    ]]
}

fn emit_hit_expected_output() -> Vec<Vec<Vec<u8>>> {
    vec![vec![
        pack_words(&[7, 101, 5, 2, 9, 103, 9, 4, 11, 107, 13, 6, 13, 109, 17, 8]),
        pack_words(&[4]),
        pack_words(&[0]),
    ]]
}

fn compact_hits_inputs() -> Vec<Vec<Vec<u8>>> {
    vec![vec![
        pack_words(&[7, 101, 5, 2, 9, 103, 9, 4, 11, 107, 13, 6, 13, 109, 17, 8]),
        pack_words(&[7]),
    ]]
}

fn compact_hits_expected_output() -> Vec<Vec<Vec<u8>>> {
    vec![vec![pack_words(&[DEFAULT_MAX_HITS])]]
}

// Forwarding alias to the canonical packer in `scan::dispatch_io`.
// Was a private inline copy with identical body - removed so the
// LE-byte packing format has a single source of truth.
use crate::scan::dispatch_io::pack_u32_slice as pack_words;

#[cfg(test)]
mod emit_then_compact_tests {
    use super::*;

    #[test]
    fn fused_program_builds() {
        let fused = emit_hit_then_compact(
            "rule_id",
            "file_id",
            "span_start",
            "span_len",
            "out_hits",
            "out_cursor",
        )
        .expect("Fix: emit_hit and compact_hits must fuse");

        // Assert concrete properties of the fused Program to verify correct building
        assert_eq!(fused.workgroup_size(), [64, 1, 1]);
        assert_eq!(fused.buffers().len(), 8);
        assert_eq!(fused.buffers()[0].name(), "rule_id");
        assert_eq!(fused.buffers()[4].name(), "out_hits");
        assert_eq!(fused.buffers()[7].name(), "hit_buffer_live_length");
    }
}

inventory::submit! {
    crate::harness::OpEntry::new(
        EMIT_HIT_OP_ID,
        || emit_hit(
            "rule_id",
            "file_id",
            "span_start",
            "span_len",
            "out_hits",
            "out_cursor",
        ),
        Some(emit_hit_inputs),
        Some(emit_hit_expected_output),
    )
}

inventory::submit! {
    crate::harness::OpEntry::new(
        COMPACT_HITS_OP_ID,
        || compact_hits("out_hits", "out_cursor", DEFAULT_MAX_HITS),
        Some(compact_hits_inputs),
        Some(compact_hits_expected_output),
    )
}
