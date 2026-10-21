use vyre::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

use crate::region::wrap_anonymous;
use crate::scan::builders::{append_match, append_match_subgroup, load_packed_byte};
use crate::scan::dfa::CompiledDfa;

#[cfg(any(test, feature = "cpu-parity"))]
use super::ClassicAcAutomaton;

#[path = "bounded_ranges/prefilter.rs"]
mod prefilter;

pub use prefilter::{
    build_ac_bounded_ranges_prefilter_program, build_ac_bounded_ranges_prefilter_program_ext,
    build_ac_bounded_ranges_suffix3_prefilter_program,
    build_ac_bounded_ranges_suffix3_prefilter_program_ext,
    classic_ac_bounded_ranges_prefilter_program, classic_ac_bounded_ranges_prefilter_program_ext,
    classic_ac_bounded_ranges_suffix3_prefilter_program,
    classic_ac_bounded_ranges_suffix3_prefilter_program_ext,
    classic_ac_bounded_ranges_suffix3_presence_and_positions_by_region_program_ext,
    classic_ac_bounded_ranges_suffix3_presence_by_region_program_ext,
    classic_ac_bounded_ranges_suffix3_presence_program_ext, presence_bitmap_words,
    presence_by_region_words, try_build_ac_bounded_ranges_prefilter_program,
    try_build_ac_bounded_ranges_prefilter_program_ext,
    try_build_ac_bounded_ranges_suffix3_prefilter_program,
    try_build_ac_bounded_ranges_suffix3_prefilter_program_ext,
    try_build_ac_bounded_ranges_suffix3_presence_and_positions_by_region_program,
    try_build_ac_bounded_ranges_suffix3_presence_by_region_program,
    try_build_ac_bounded_ranges_suffix3_presence_program,
};

/// Build a Program that scans `haystack` for any AC match and emits
/// `(pattern_id, start, end)` triples through the canonical
/// [`append_match`] hit buffer. Pairs with
/// [`pack_haystack_u32`](crate::scan::dispatch_io::pack_haystack_u32):
/// each invocation `i` corresponds to byte position `i` of the
/// **unpacked** haystack, but loads from the packed u32 buffer via
/// [`load_packed_byte`].
///
/// Buffer layout (bindings 0..7):
///
/// | binding | name | access | element shape |
/// |---|---|---|---|
/// | 0 | `haystack`        | ReadOnly  | packed u32, 4 bytes / word |
/// | 1 | `transitions`     | ReadOnly  | `state_count * 256` u32    |
/// | 2 | `output_offsets`  | ReadOnly  | `state_count + 1` u32      |
/// | 3 | `output_records`  | ReadOnly  | `output_records_len` u32   |
/// | 4 | `pattern_lengths` | ReadOnly  | `pattern_count` u32        |
/// | 5 | `haystack_len`    | ReadOnly  | 1 u32 (byte length)        |
/// | 6 | `match_count`     | ReadWrite | 1 u32 (atomic)             |
/// | 7 | `matches`         | Output    | `max_matches * 3` u32      |
///
/// Each invocation `i` replays the suffix window
/// `haystack[max(0, i+1-max_pattern_len)..=i]` from state 0, then
/// emits every `(pid, start, end)` triple that accepts at `i`. The
/// scan window cap is the only difference from the unbounded walk:
/// `max_pattern_len` must be greater than or equal to the longest
/// entry in `pattern_lengths`, or matches longer than the window are
/// invisible because the walk never sees their first byte.
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn classic_ac_bounded_ranges_program(
    haystack: &str,
    transitions: &str,
    output_offsets: &str,
    output_records: &str,
    pattern_lengths: &str,
    haystack_len: &str,
    match_count: &str,
    matches: &str,
    state_count: u32,
    output_records_len: u32,
    pattern_count: u32,
    max_matches: u32,
    max_pattern_len: u32,
) -> Program {
    classic_ac_bounded_ranges_program_ext(
        haystack,
        transitions,
        output_offsets,
        output_records,
        pattern_lengths,
        haystack_len,
        match_count,
        matches,
        state_count,
        output_records_len,
        pattern_count,
        max_matches,
        max_pattern_len,
        true,
    )
}

/// Variant of [`classic_ac_bounded_ranges_program`] with explicit
/// control over the match-append strategy.
///
/// Set `use_subgroup_coalesce = true` for `append_match_subgroup`
/// (Innovation I.17, one atomic per subgroup leader, the default).
/// Set `false` for the simpler `append_match` (one atomic per lane
/// per hit). Use the `false` variant on backends whose IR lowering
/// can't emit `subgroup_ballot`/`subgroup_shuffle`.
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn classic_ac_bounded_ranges_program_ext(
    haystack: &str,
    transitions: &str,
    output_offsets: &str,
    output_records: &str,
    pattern_lengths: &str,
    haystack_len: &str,
    match_count: &str,
    matches: &str,
    state_count: u32,
    output_records_len: u32,
    pattern_count: u32,
    max_matches: u32,
    max_pattern_len: u32,
    use_subgroup_coalesce: bool,
) -> Program {
    let max_pattern_len = max_pattern_len.max(1);
    let i = Expr::var("i");
    let walk_body = vec![
        Node::let_bind("i", Expr::InvocationId { axis: 0 }),
        Node::if_then(
            Expr::lt(i.clone(), Expr::load(haystack_len, Expr::u32(0))),
            bounded_ranges_scan_nodes(
                haystack,
                transitions,
                output_offsets,
                output_records,
                pattern_lengths,
                match_count,
                matches,
                max_pattern_len,
                use_subgroup_coalesce,
            ),
        ),
    ];

    Program::wrapped(
        vec![
            BufferDecl::storage(haystack, 0, BufferAccess::ReadOnly, DataType::U32),
            BufferDecl::storage(transitions, 1, BufferAccess::ReadOnly, DataType::U32)
                .with_count(state_count.saturating_mul(256)),
            BufferDecl::storage(output_offsets, 2, BufferAccess::ReadOnly, DataType::U32)
                .with_count(state_count.saturating_add(1)),
            BufferDecl::storage(output_records, 3, BufferAccess::ReadOnly, DataType::U32)
                .with_count(output_records_len),
            BufferDecl::storage(pattern_lengths, 4, BufferAccess::ReadOnly, DataType::U32)
                .with_count(pattern_count),
            BufferDecl::storage(haystack_len, 5, BufferAccess::ReadOnly, DataType::U32)
                .with_count(1),
            BufferDecl::read_write(match_count, 6, DataType::U32).with_count(1),
            BufferDecl::output(matches, 7, DataType::U32).with_count(max_matches.saturating_mul(3)),
        ],
        [128, 1, 1],
        vec![wrap_anonymous(
            "vyre-libs::matching::classic_ac_bounded_ranges",
            walk_body,
        )],
    )
}

#[allow(clippy::too_many_arguments)]
fn bounded_ranges_scan_nodes(
    haystack: &str,
    transitions: &str,
    output_offsets: &str,
    output_records: &str,
    pattern_lengths: &str,
    match_count: &str,
    matches: &str,
    max_pattern_len: u32,
    use_subgroup_coalesce: bool,
) -> Vec<Node> {
    let max_pattern_len = max_pattern_len.max(1);
    let i = Expr::var("i");
    let end = Expr::add(i.clone(), Expr::u32(1));
    let scan_start = Expr::select(
        Expr::lt(i.clone(), Expr::u32(max_pattern_len - 1)),
        Expr::u32(0),
        Expr::sub(end.clone(), Expr::u32(max_pattern_len)),
    );
    let (load_step_byte, step_byte) = load_packed_byte(haystack, Expr::var("step"));

    vec![
        Node::let_bind("state", Expr::u32(0)),
        Node::let_bind("scan_start", scan_start),
        Node::let_bind("scan_end", end),
        Node::loop_for(
            "step",
            Expr::var("scan_start"),
            Expr::var("scan_end"),
            vec![
                load_step_byte,
                Node::assign(
                    "state",
                    Expr::load(
                        transitions,
                        Expr::add(Expr::mul(Expr::var("state"), Expr::u32(256)), step_byte),
                    ),
                ),
            ],
        ),
        Node::let_bind("out_begin", Expr::load(output_offsets, Expr::var("state"))),
        Node::let_bind(
            "out_end",
            Expr::load(output_offsets, Expr::add(Expr::var("state"), Expr::u32(1))),
        ),
        Node::loop_for("out_idx", Expr::var("out_begin"), Expr::var("out_end"), {
            let mut body = vec![
                Node::let_bind(
                    "pattern_id",
                    Expr::load(output_records, Expr::var("out_idx")),
                ),
                Node::let_bind(
                    "pat_len",
                    Expr::load(pattern_lengths, Expr::var("pattern_id")),
                ),
                Node::let_bind(
                    "match_start",
                    Expr::select(
                        Expr::lt(Expr::var("scan_end"), Expr::var("pat_len")),
                        Expr::u32(0),
                        Expr::sub(Expr::var("scan_end"), Expr::var("pat_len")),
                    ),
                ),
            ];
            if use_subgroup_coalesce {
                body.extend(append_match_subgroup(
                    matches,
                    match_count,
                    Expr::var("pattern_id"),
                    Expr::var("match_start"),
                    Expr::var("scan_end"),
                    Expr::bool(true),
                ));
            } else {
                body.push(append_match(
                    matches,
                    match_count,
                    Expr::var("pattern_id"),
                    Expr::var("match_start"),
                    Expr::var("scan_end"),
                ));
            }
            body
        }),
    ]
}

/// Emit the bounded-window DFA replay for a single candidate position, writing a
/// per-pattern PRESENCE bit instead of an `(id,start,end)` match triple.
///
/// Innovation: match-DENSE literal sets (a source-code prefilter fires ~1 hit per
/// 30 bytes) make the triple-append path output-bound — every hit takes an atomic
/// counter increment + three global stores, and the host reads back tens of
/// thousands of triples. Measured on a 5090 that collapses a 676 MB/s scan kernel
/// to 4.5 MB/s. But a prefilter consumer (e.g. a downstream scanner's `collect_triggered_patterns`)
/// only needs to know WHICH patterns fired, not where. Setting a presence bit is
/// IDEMPOTENT, so concurrent lanes hitting the same pattern need no counter and no
/// per-hit serialization — just an `atomic_or` into a ~`ceil(patterns/32)`-word
/// bitmap that is the entire readback. This keeps the kernel near the scan ceiling
/// on dense inputs. `pattern_lengths` / `match_start` are unused (no positions).
fn bounded_ranges_presence_nodes(
    haystack: &str,
    transitions: &str,
    output_offsets: &str,
    output_records: &str,
    presence: &str,
    max_pattern_len: u32,
) -> Vec<Node> {
    let max_pattern_len = max_pattern_len.max(1);
    let i = Expr::var("i");
    let end = Expr::add(i.clone(), Expr::u32(1));
    let scan_start = Expr::select(
        Expr::lt(i.clone(), Expr::u32(max_pattern_len - 1)),
        Expr::u32(0),
        Expr::sub(end.clone(), Expr::u32(max_pattern_len)),
    );
    let (load_step_byte, step_byte) = load_packed_byte(haystack, Expr::var("step"));

    vec![
        Node::let_bind("state", Expr::u32(0)),
        Node::let_bind("scan_start", scan_start),
        Node::let_bind("scan_end", end),
        Node::loop_for(
            "step",
            Expr::var("scan_start"),
            Expr::var("scan_end"),
            vec![
                load_step_byte,
                Node::assign(
                    "state",
                    Expr::load(
                        transitions,
                        Expr::add(Expr::mul(Expr::var("state"), Expr::u32(256)), step_byte),
                    ),
                ),
            ],
        ),
        Node::let_bind("out_begin", Expr::load(output_offsets, Expr::var("state"))),
        Node::let_bind(
            "out_end",
            Expr::load(output_offsets, Expr::add(Expr::var("state"), Expr::u32(1))),
        ),
        Node::loop_for("out_idx", Expr::var("out_begin"), Expr::var("out_end"), {
            vec![
                Node::let_bind(
                    "pattern_id",
                    Expr::load(output_records, Expr::var("out_idx")),
                ),
                // presence[pattern_id >> 5] |= 1u32 << (pattern_id & 31).
                // Bind the (discarded) previous value so the atomic RMW is emitted
                // as a side-effecting statement (same idiom as `append_match`'s
                // `_vyre_match_slot`).
                Node::let_bind(
                    "_vyre_presence_prev",
                    Expr::atomic_or(
                        presence,
                        Expr::shr(Expr::var("pattern_id"), Expr::u32(5)),
                        Expr::shl(
                            Expr::u32(1),
                            Expr::bitand(Expr::var("pattern_id"), Expr::u32(31)),
                        ),
                    ),
                ),
            ]
        }),
    ]
}

/// Region-attributed counterpart of [`bounded_ranges_presence_nodes`]: write the
/// presence bit into a per-REGION bitmap row instead of one global bitmap.
///
/// Innovation: a coalesced-batch consumer packs N independent
/// files into one haystack and needs to know which patterns fired *in each file*,
/// not just somewhere in the batch. The triple-append path gives exact spans the
/// consumer then reduces to a per-file trigger set on the host — paying the dense
/// per-hit atomic-counter serialization + large triple readback measured to
/// collapse a 554 MB/s scan to 4.4 MB/s. This builder keeps the idempotent
/// `atomic_or` (no counter, stays near the scan ceiling) but indexes it by region:
/// the candidate end position `i` is mapped to its region via a bounded binary
/// search over `region_starts` (ascending file start offsets in the coalesced
/// buffer; `region_starts[0]` MUST be 0), then the bit lands in
/// `presence[region * presence_words + (pattern_id >> 5)]`. The readback is the
/// `region_count × presence_words` bitmap the consumer wanted directly — no host
/// reduction, no span materialization.
///
/// `log2_max_regions` fixed binary-search iterations bound the region lookup
/// (`ceil(log2(max_regions))`); `presence_words` is the per-region row stride.
/// The kernel reads the live `region_count` from `buf_len(region_starts)`, so one
/// compiled program serves any batch with `region_count <= max_regions`. A match
/// never spans a region boundary (the consumer inserts separator bytes between
/// files), so attributing by the end position `i` equals attributing by the start.
#[allow(clippy::too_many_arguments)]
fn bounded_ranges_presence_by_region_nodes(
    haystack: &str,
    transitions: &str,
    output_offsets: &str,
    output_records: &str,
    presence: &str,
    region_starts: &str,
    region_base: &str,
    max_pattern_len: u32,
    presence_words: u32,
    log2_max_regions: u32,
) -> Vec<Node> {
    let max_pattern_len = max_pattern_len.max(1);
    let i = Expr::var("i");
    let end = Expr::add(i.clone(), Expr::u32(1));
    let scan_start = Expr::select(
        Expr::lt(i.clone(), Expr::u32(max_pattern_len - 1)),
        Expr::u32(0),
        Expr::sub(end.clone(), Expr::u32(max_pattern_len)),
    );
    let (load_step_byte, step_byte) = load_packed_byte(haystack, Expr::var("step"));

    // Region lookup + presence writes, run ONLY when this candidate has matches
    // (`out_begin < out_end`). `region = largest r with region_starts[r] <= pos`
    // where `pos = i + region_base` is the GLOBAL byte position: a sharded
    // dispatch scans a slice with local positions `i` but attributes against the
    // whole batch's `region_starts` by adding the shard's base offset (0 for the
    // single-dispatch path). The binary search is fixed-iteration; `select`
    // evaluates both arms, so the `rs_mid - 1` arm can underflow to u32::MAX on
    // the rejected branch — it is discarded (rs_mid == 0 only when rs_lo == rs_hi
    // == 0, where region_starts[0] == 0 <= pos forces the `cond` arm), harmless.
    let mut region_and_emit =
        region_search_prologue_nodes(region_starts, region_base, presence_words, log2_max_regions);
    region_and_emit.push(Node::loop_for(
        "out_idx",
        Expr::var("out_begin"),
        Expr::var("out_end"),
        vec![
            Node::let_bind(
                "pattern_id",
                Expr::load(output_records, Expr::var("out_idx")),
            ),
            // presence[rs_base + (pattern_id >> 5)] |= 1u32 << (pattern_id & 31).
            Node::let_bind(
                "_vyre_presence_prev",
                Expr::atomic_or(
                    presence,
                    Expr::add(
                        Expr::var("rs_base"),
                        Expr::shr(Expr::var("pattern_id"), Expr::u32(5)),
                    ),
                    Expr::shl(
                        Expr::u32(1),
                        Expr::bitand(Expr::var("pattern_id"), Expr::u32(31)),
                    ),
                ),
            ),
        ],
    ));

    vec![
        Node::let_bind("state", Expr::u32(0)),
        Node::let_bind("scan_start", scan_start),
        Node::let_bind("scan_end", end),
        Node::loop_for(
            "step",
            Expr::var("scan_start"),
            Expr::var("scan_end"),
            vec![
                load_step_byte,
                Node::assign(
                    "state",
                    Expr::load(
                        transitions,
                        Expr::add(Expr::mul(Expr::var("state"), Expr::u32(256)), step_byte),
                    ),
                ),
            ],
        ),
        Node::let_bind("out_begin", Expr::load(output_offsets, Expr::var("state"))),
        Node::let_bind(
            "out_end",
            Expr::load(output_offsets, Expr::add(Expr::var("state"), Expr::u32(1))),
        ),
        Node::if_then(
            Expr::lt(Expr::var("out_begin"), Expr::var("out_end")),
            region_and_emit,
        ),
    ]
}

/// The region binary-search PROLOGUE shared verbatim by the presence-only
/// ([`bounded_ranges_presence_by_region_nodes`]) and the fused presence+positions
/// ([`bounded_ranges_presence_and_positions_by_region_nodes`]) builders. Computes
/// `rs_pos = i + region_base` (the GLOBAL byte position so a sharded dispatch
/// attributes against the whole-batch region table), binary-searches `region_starts`
/// for the largest region whose start `<= rs_pos`, and binds `rs_base = region *
/// presence_words` (the per-region presence-row offset). The caller appends its own
/// per-record emit loop after these nodes. The `rs_mid - 1` arm can underflow to
/// `u32::MAX` on the rejected `select` branch; it is discarded harmlessly
/// (`rs_mid == 0` only when `rs_lo == rs_hi == 0`, where `region_starts[0] == 0 <=
/// rs_pos` forces the `cond` arm). One source of truth for the lookup keeps the two
/// builders bit-identical by construction.
fn region_search_prologue_nodes(
    region_starts: &str,
    region_base: &str,
    presence_words: u32,
    log2_max_regions: u32,
) -> Vec<Node> {
    vec![
        Node::let_bind(
            "rs_pos",
            Expr::add(Expr::var("i"), Expr::load(region_base, Expr::u32(0))),
        ),
        Node::let_bind("rs_lo", Expr::u32(0)),
        Node::let_bind(
            "rs_hi",
            Expr::sub(Expr::buf_len(region_starts), Expr::u32(1)),
        ),
        Node::loop_for(
            "rs_step",
            Expr::u32(0),
            Expr::u32(log2_max_regions.max(1)),
            vec![
                Node::let_bind(
                    "rs_mid",
                    Expr::div(
                        Expr::add(
                            Expr::add(Expr::var("rs_lo"), Expr::var("rs_hi")),
                            Expr::u32(1),
                        ),
                        Expr::u32(2),
                    ),
                ),
                Node::let_bind(
                    "rs_cond",
                    Expr::le(
                        Expr::load(region_starts, Expr::var("rs_mid")),
                        Expr::var("rs_pos"),
                    ),
                ),
                Node::assign(
                    "rs_lo",
                    Expr::select(
                        Expr::var("rs_cond"),
                        Expr::var("rs_mid"),
                        Expr::var("rs_lo"),
                    ),
                ),
                Node::assign(
                    "rs_hi",
                    Expr::select(
                        Expr::var("rs_cond"),
                        Expr::var("rs_hi"),
                        Expr::sub(Expr::var("rs_mid"), Expr::u32(1)),
                    ),
                ),
            ],
        ),
        Node::let_bind(
            "rs_base",
            Expr::mul(Expr::var("rs_lo"), Expr::u32(presence_words.max(1))),
        ),
    ]
}

/// FUSED presence-AND-positions region replay: one bounded-window DFA walk that, at
/// each accepted candidate, emits BOTH the per-region presence bit (idempotent
/// `atomic_or`, exactly as [`bounded_ranges_presence_by_region_nodes`]) AND the
/// `(pattern_id, start, end)` match triple (atomic append, exactly as
/// [`bounded_ranges_scan_nodes`]).
///
/// Innovation: a coalesced-batch consumer (a GPU phase-1 scanner) needs the per-file
/// trigger SET *and* the anchor/keyword match POSITIONS. Today it pays TWO full GPU
/// scans of the same haystack — `scan_presence_by_region` (bitmap) then a second
/// `scan_into` (triples) — because the presence bitmap carries no positions. Both
/// scans run the IDENTICAL suffix3 candidate gate + bounded DFA replay over the same
/// `output_records`; only the per-record EMISSION differs. Fusing them runs the
/// expensive walk ONCE and drives both outputs from the single `output_records`
/// loop, halving the consumer's phase-1 work. Recall-identical to the two separate
/// scans by construction: same candidate set, same walk, same iteration order — the
/// presence bits equal `scan_presence_by_region`'s and the triples equal
/// `scan_into`'s, just produced together.
#[allow(clippy::too_many_arguments)]
fn bounded_ranges_presence_and_positions_by_region_nodes(
    haystack: &str,
    transitions: &str,
    output_offsets: &str,
    output_records: &str,
    pattern_lengths: &str,
    presence: &str,
    region_starts: &str,
    region_base: &str,
    match_count: &str,
    matches: &str,
    max_pattern_len: u32,
    presence_words: u32,
    log2_max_regions: u32,
) -> Vec<Node> {
    let max_pattern_len = max_pattern_len.max(1);
    let i = Expr::var("i");
    let end = Expr::add(i.clone(), Expr::u32(1));
    let scan_start = Expr::select(
        Expr::lt(i.clone(), Expr::u32(max_pattern_len - 1)),
        Expr::u32(0),
        Expr::sub(end.clone(), Expr::u32(max_pattern_len)),
    );
    let (load_step_byte, step_byte) = load_packed_byte(haystack, Expr::var("step"));

    // Region binary search (identical to the presence-only builder), then ONE
    // `output_records` loop that emits the region presence bit AND the match triple
    // per accepted pattern. `pos = i + region_base` is the GLOBAL byte position so a
    // sharded dispatch attributes against the whole-batch region table; see
    // [`bounded_ranges_presence_by_region_nodes`] for the underflow-on-rejected-arm
    // soundness note.
    let mut region_and_emit =
        region_search_prologue_nodes(region_starts, region_base, presence_words, log2_max_regions);
    region_and_emit.push(Node::loop_for(
        "out_idx",
        Expr::var("out_begin"),
        Expr::var("out_end"),
        vec![
            Node::let_bind(
                "pattern_id",
                Expr::load(output_records, Expr::var("out_idx")),
            ),
            Node::let_bind(
                "pat_len",
                Expr::load(pattern_lengths, Expr::var("pattern_id")),
            ),
            Node::let_bind(
                "match_start",
                Expr::select(
                    Expr::lt(Expr::var("scan_end"), Expr::var("pat_len")),
                    Expr::u32(0),
                    Expr::sub(Expr::var("scan_end"), Expr::var("pat_len")),
                ),
            ),
            // presence[rs_base + (pattern_id >> 5)] |= 1u32 << (pattern_id & 31).
            Node::let_bind(
                "_vyre_presence_prev",
                Expr::atomic_or(
                    presence,
                    Expr::add(
                        Expr::var("rs_base"),
                        Expr::shr(Expr::var("pattern_id"), Expr::u32(5)),
                    ),
                    Expr::shl(
                        Expr::u32(1),
                        Expr::bitand(Expr::var("pattern_id"), Expr::u32(31)),
                    ),
                ),
            ),
            // (pattern_id, match_start, scan_end) triple append (same format as
            // the match-emitting scan). No subgroup coalesce: the CUDA backend
            // can't lower subgroup ops and the dense-hit benefit is the presence
            // bitmap's job, not this fused path's.
            append_match(
                matches,
                match_count,
                Expr::var("pattern_id"),
                Expr::var("match_start"),
                Expr::var("scan_end"),
            ),
        ],
    ));

    vec![
        Node::let_bind("state", Expr::u32(0)),
        Node::let_bind("scan_start", scan_start),
        Node::let_bind("scan_end", end),
        Node::loop_for(
            "step",
            Expr::var("scan_start"),
            Expr::var("scan_end"),
            vec![
                load_step_byte,
                Node::assign(
                    "state",
                    Expr::load(
                        transitions,
                        Expr::add(Expr::mul(Expr::var("state"), Expr::u32(256)), step_byte),
                    ),
                ),
            ],
        ),
        Node::let_bind("out_begin", Expr::load(output_offsets, Expr::var("state"))),
        Node::let_bind(
            "out_end",
            Expr::load(output_offsets, Expr::add(Expr::var("state"), Expr::u32(1))),
        ),
        Node::if_then(
            Expr::lt(Expr::var("out_begin"), Expr::var("out_end")),
            region_and_emit,
        ),
    ]
}

/// Build the dispatch Program for a bounded-ranges AC scan over an
/// already-compiled DFA. Pairs with
/// [`classic_ac_bounded_ranges_program`]: identical buffer layout
/// and emit format, but the caller doesn't have to thread through
/// the eight derived count fields every time.
#[must_use]
pub fn build_ac_bounded_ranges_program(
    dfa: &CompiledDfa,
    pattern_count: u32,
    max_matches: u32,
) -> Program {
    build_ac_bounded_ranges_program_ext(dfa, pattern_count, max_matches, true)
}

/// Variant of [`build_ac_bounded_ranges_program`] that exposes the
/// `use_subgroup_coalesce` selector. Pass `false` when the program
/// is going to be dispatched on a backend that cannot lower
/// `subgroup_ballot` + `subgroup_shuffle` yet (currently
/// `vyre-driver-cuda`).
#[must_use]
pub fn build_ac_bounded_ranges_program_ext(
    dfa: &CompiledDfa,
    pattern_count: u32,
    max_matches: u32,
    use_subgroup_coalesce: bool,
) -> Program {
    match try_build_ac_bounded_ranges_program_ext(
        dfa,
        pattern_count,
        max_matches,
        use_subgroup_coalesce,
    ) {
        Ok(program) => program,
        Err(error) => {
            // Returning an empty-rejecting program would silently drop every
            // match without the caller knowing — a total recall-loss silent
            // fallback. Fail closed instead. Callers that need graceful
            // overflow handling must call try_build_ac_bounded_ranges_program_ext
            // directly and shard oversized DFAs across multiple programs.
            panic!(
                "AC bounded-ranges program build failed: {error} — \
                 returning an empty rejecting automaton would silently drop every match; \
                 use try_build_ac_bounded_ranges_program_ext and shard oversized DFAs."
            )
        }
    }
}

/// Fallible variant of [`build_ac_bounded_ranges_program`].
///
/// # Errors
///
/// Returns an actionable error when DFA metadata cannot fit the GPU program's
/// u32 buffer-count ABI.
pub fn try_build_ac_bounded_ranges_program(
    dfa: &CompiledDfa,
    pattern_count: u32,
    max_matches: u32,
) -> Result<Program, String> {
    try_build_ac_bounded_ranges_program_ext(dfa, pattern_count, max_matches, true)
}

/// Fallible variant of [`build_ac_bounded_ranges_program_ext`].
///
/// # Errors
///
/// Returns an actionable error when DFA metadata cannot fit the GPU program's
/// u32 buffer-count ABI.
pub fn try_build_ac_bounded_ranges_program_ext(
    dfa: &CompiledDfa,
    pattern_count: u32,
    max_matches: u32,
    use_subgroup_coalesce: bool,
) -> Result<Program, String> {
    let output_records_len = u32::try_from(dfa.output_records.len()).map_err(|source| {
        format!(
            "AC bounded-ranges DFA output record count {} exceeds u32 GPU buffer metadata: {source}. Fix: shard the pattern set or lower the DFA budget before dispatch.",
            dfa.output_records.len()
        )
    })?;
    Ok(classic_ac_bounded_ranges_program_ext(
        "haystack",
        "transitions",
        "output_offsets",
        "output_records",
        "pattern_lengths",
        "haystack_len",
        "match_count",
        "matches",
        dfa.state_count,
        output_records_len,
        pattern_count,
        max_matches,
        dfa.max_pattern_len,
        use_subgroup_coalesce,
    ))
}

fn empty_ac_bounded_ranges_program(max_matches: u32, use_subgroup_coalesce: bool) -> Program {
    classic_ac_bounded_ranges_program_ext(
        "haystack",
        "transitions",
        "output_offsets",
        "output_records",
        "pattern_lengths",
        "haystack_len",
        "match_count",
        "matches",
        1,
        0,
        0,
        max_matches,
        0,
        use_subgroup_coalesce,
    )
}

/// CPU reference for [`classic_ac_bounded_ranges_program`]. Returns
/// `(pattern_id, start, end)` triples reconstructed from
/// `output_records` plus the pattern length table.
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn classic_ac_bounded_ranges_scan(
    ac: &ClassicAcAutomaton,
    pattern_lengths: &[u32],
    haystack: &[u8],
) -> Vec<(u32, u32, u32)> {
    let dfa = &ac.dfa;
    let mut state = 0u32;
    let mut out = Vec::new();
    for (pos, &b) in haystack.iter().enumerate() {
        state = dfa.transitions[(state as usize) * 256 + (b as usize)];
        let begin = dfa.output_offsets[state as usize] as usize;
        let end_off = dfa.output_offsets[state as usize + 1] as usize;
        for &pid in &dfa.output_records[begin..end_off] {
            // Index directly so an OOB pid panics here rather than silently
            // producing a zero-length hit. A mismatch between pattern_count
            // and the actual max pid in output_records is a caller bug; the
            // GPU kernel does an unchecked load that is UB on the same input,
            // so the CPU reference must fail loud-and-early instead of clamping.
            let pat_len = pattern_lengths[pid as usize];
            let end_pos = (pos as u32).saturating_add(1);
            let start = end_pos.saturating_sub(pat_len);
            out.push((pid, start, end_pos));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scan::classic_ac::classic_ac_compile;

    /// Regression guard: build_ac_bounded_ranges_program_ext must never
    /// silently return an empty-rejecting program when the DFA is oversized.
    /// Before this fix the error arm called eprintln! and returned
    /// empty_ac_bounded_ranges_program, causing zero matches without any
    /// signal to the caller.
    #[test]
    fn infallible_ac_program_builder_panics_not_falls_back() {
        let src = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/scan/classic_ac/bounded_ranges.rs"
        ))
        .expect("Fix: bounded_ranges source must be readable for regression guard");
        let production = src
            .split("\n#[cfg(test)]")
            .next()
            .expect("Fix: bounded_ranges source must have a test section");
        // The old silent-fallback arm used eprintln! to swallow the error before
        // returning an empty-rejecting program. Check the eprintln! is gone.
        assert!(
            !production.contains("eprintln!(\"vyre-libs AC bounded-ranges program build failed"),
            "build_ac_bounded_ranges_program_ext must not silently log and return an empty \
             program on error — use panic!() so callers are forced to use \
             try_build_ac_bounded_ranges_program_ext."
        );
    }

    /// Verify try_build_ac_bounded_ranges_program_ext returns Ok for a valid
    /// small DFA, proving the success path is intact after the panic-on-error fix.
    #[test]
    fn try_build_ac_bounded_ranges_program_ext_succeeds_for_valid_dfa() {
        let ac = classic_ac_compile(&[b"abc", b"de"]);
        let result = try_build_ac_bounded_ranges_program_ext(&ac.dfa, 2, 128, false);
        assert!(
            result.is_ok(),
            "try_build must succeed for a valid small DFA: {:?}",
            result.err()
        );
        // Verify the program has the correct buffer shape for the DFA size.
        let program = result.unwrap();
        assert_eq!(
            program.workgroup_size(),
            [128, 1, 1],
            "workgroup size must be [128, 1, 1]"
        );
    }

    /// Verify the CPU reference scan panics (not silently zero-lengths) when
    /// the DFA output_records contain a pid beyond pattern_lengths.len().
    /// Before the fix, pattern_lengths.get(pid).copied().unwrap_or(0) would
    /// silently treat the OOB pid as pat_len=0, producing a zero-length match
    /// at the right position — masking the root cause of the mismatch and
    /// making parity tests impossible to detect the bug.
    #[test]
    #[should_panic]
    fn classic_ac_bounded_ranges_scan_panics_on_oob_pid() {
        use crate::scan::dfa::CompiledDfa;

        // Craft a ClassicAcAutomaton whose output_records contains pid=5
        // but we only supply pattern_lengths of length 3.
        // state 0 -b'A'-> state 1, state 1 accepts pid=5.
        let transitions: Vec<u32> = {
            let mut t = vec![0u32; 2 * 256]; // 2 states
            t[0 * 256 + b'A' as usize] = 1; // state 0 --'A'--> state 1
            // state 1 loops to 0 on all other bytes (default 0)
            t
        };
        let accept = vec![0u32, 6u32]; // state 1: accept=6 (pid=5, encoded as 5+1)
        let output_offsets = vec![0u32, 0u32, 1u32]; // state 0: [], state 1: [5]
        let output_records = vec![5u32]; // pid=5

        let dfa = CompiledDfa {
            transitions,
            accept,
            state_count: 2,
            max_pattern_len: 1,
            output_offsets,
            output_records,
        };
        let ac = crate::scan::classic_ac::ClassicAcAutomaton { dfa };
        // pattern_lengths only has 3 entries (pids 0..2) — pid=5 is OOB.
        // This must panic, not silently produce a zero-length match.
        let _result = classic_ac_bounded_ranges_scan(&ac, &[1u32, 2u32, 3u32], b"A");
    }
}
