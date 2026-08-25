use vyre::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

use crate::region::wrap_anonymous;
use crate::scan::builders::load_packed_byte_expr;
use crate::scan::dfa::CompiledDfa;

use super::super::super::count_program::{
    suffix3_bloom_bit_index_expr, CLASSIC_AC_SUFFIX2_MASK_WORDS, CLASSIC_AC_SUFFIX3_BLOOM_WORDS,
};
use super::super::{
    bounded_ranges_presence_and_positions_by_region_nodes,
    bounded_ranges_presence_by_region_nodes, bounded_ranges_presence_nodes,
    bounded_ranges_scan_nodes,
};

/// The suffix2/suffix3 candidate-gating body shared by the match-emitting scan
/// program and the presence-bitmap program. Both run the IDENTICAL prefilter
/// cascade (byte end-mask → suffix2 → suffix3 bloom) and only differ in the
/// `replay_nodes` they execute at an accepted candidate position. Extracted so
/// the two output modes cannot drift in their candidate logic.
#[allow(clippy::too_many_arguments)]
fn suffix3_prefilter_body(
    haystack: &str,
    haystack_len: &str,
    candidate_end_mask: &str,
    candidate_suffix2_mask: &str,
    candidate_suffix3_bloom: &str,
    replay_nodes: Vec<Node>,
) -> Vec<Node> {
    let i = Expr::var("i");
    let candidate_byte = load_packed_byte_expr(haystack, i.clone());
    let previous_byte =
        load_packed_byte_expr(haystack, Expr::saturating_sub(i.clone(), Expr::u32(1)));
    let previous2_byte =
        load_packed_byte_expr(haystack, Expr::saturating_sub(i.clone(), Expr::u32(2)));
    let suffix2_index = Expr::bitor(
        Expr::shl(Expr::var("previous_byte"), Expr::u32(8)),
        Expr::var("candidate_byte"),
    );
    let suffix3_index = Expr::bitor(
        Expr::bitor(
            Expr::shl(Expr::var("previous2_byte"), Expr::u32(16)),
            Expr::shl(Expr::var("previous_byte"), Expr::u32(8)),
        ),
        Expr::var("candidate_byte"),
    );
    let suffix3_bit_index = suffix3_bloom_bit_index_expr(Expr::var("suffix3_index"));

    vec![
        Node::let_bind("i", Expr::InvocationId { axis: 0 }),
        Node::if_then(
            Expr::lt(i.clone(), Expr::load(haystack_len, Expr::u32(0))),
            vec![
                Node::let_bind("candidate_byte", candidate_byte),
                Node::let_bind(
                    "candidate_word",
                    Expr::load(
                        candidate_end_mask,
                        Expr::shr(Expr::var("candidate_byte"), Expr::u32(5)),
                    ),
                ),
                Node::let_bind(
                    "candidate_bit",
                    Expr::shl(
                        Expr::u32(1),
                        Expr::bitand(Expr::var("candidate_byte"), Expr::u32(31)),
                    ),
                ),
                Node::if_then(
                    Expr::ne(
                        Expr::bitand(Expr::var("candidate_word"), Expr::var("candidate_bit")),
                        Expr::u32(0),
                    ),
                    vec![Node::if_then_else(
                        Expr::eq(i.clone(), Expr::u32(0)),
                        replay_nodes.clone(),
                        vec![
                            Node::let_bind("previous_byte", previous_byte),
                            Node::let_bind("suffix2_index", suffix2_index),
                            Node::let_bind(
                                "suffix2_word",
                                Expr::load(
                                    candidate_suffix2_mask,
                                    Expr::shr(Expr::var("suffix2_index"), Expr::u32(5)),
                                ),
                            ),
                            Node::let_bind(
                                "suffix2_bit",
                                Expr::shl(
                                    Expr::u32(1),
                                    Expr::bitand(Expr::var("suffix2_index"), Expr::u32(31)),
                                ),
                            ),
                            Node::if_then(
                                Expr::ne(
                                    Expr::bitand(
                                        Expr::var("suffix2_word"),
                                        Expr::var("suffix2_bit"),
                                    ),
                                    Expr::u32(0),
                                ),
                                vec![Node::if_then_else(
                                    Expr::eq(i.clone(), Expr::u32(1)),
                                    replay_nodes.clone(),
                                    vec![
                                        Node::let_bind("previous2_byte", previous2_byte),
                                        Node::let_bind("suffix3_index", suffix3_index),
                                        Node::let_bind("suffix3_bit_index", suffix3_bit_index),
                                        Node::let_bind(
                                            "suffix3_word",
                                            Expr::load(
                                                candidate_suffix3_bloom,
                                                Expr::shr(
                                                    Expr::var("suffix3_bit_index"),
                                                    Expr::u32(5),
                                                ),
                                            ),
                                        ),
                                        Node::let_bind(
                                            "suffix3_bit",
                                            Expr::shl(
                                                Expr::u32(1),
                                                Expr::bitand(
                                                    Expr::var("suffix3_bit_index"),
                                                    Expr::u32(31),
                                                ),
                                            ),
                                        ),
                                        Node::if_then(
                                            Expr::ne(
                                                Expr::bitand(
                                                    Expr::var("suffix3_word"),
                                                    Expr::var("suffix3_bit"),
                                                ),
                                                Expr::u32(0),
                                            ),
                                            replay_nodes,
                                        ),
                                    ],
                                )],
                            ),
                        ],
                    )],
                ),
            ],
        ),
    ]
}

/// Build a bounded-window AC ranges program with byte, suffix2, and suffix3
/// candidate filters before match-emitting replay.
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn classic_ac_bounded_ranges_suffix3_prefilter_program(
    haystack: &str,
    transitions: &str,
    output_offsets: &str,
    output_records: &str,
    pattern_lengths: &str,
    haystack_len: &str,
    match_count: &str,
    candidate_end_mask: &str,
    candidate_suffix2_mask: &str,
    candidate_suffix3_bloom: &str,
    matches: &str,
    state_count: u32,
    output_records_len: u32,
    pattern_count: u32,
    max_matches: u32,
    max_pattern_len: u32,
) -> Program {
    classic_ac_bounded_ranges_suffix3_prefilter_program_ext(
        haystack,
        transitions,
        output_offsets,
        output_records,
        pattern_lengths,
        haystack_len,
        match_count,
        candidate_end_mask,
        candidate_suffix2_mask,
        candidate_suffix3_bloom,
        matches,
        state_count,
        output_records_len,
        pattern_count,
        max_matches,
        max_pattern_len,
        true,
    )
}

/// Variant of [`classic_ac_bounded_ranges_suffix3_prefilter_program`] with
/// explicit control over subgroup match-append coalescing.
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn classic_ac_bounded_ranges_suffix3_prefilter_program_ext(
    haystack: &str,
    transitions: &str,
    output_offsets: &str,
    output_records: &str,
    pattern_lengths: &str,
    haystack_len: &str,
    match_count: &str,
    candidate_end_mask: &str,
    candidate_suffix2_mask: &str,
    candidate_suffix3_bloom: &str,
    matches: &str,
    state_count: u32,
    output_records_len: u32,
    pattern_count: u32,
    max_matches: u32,
    max_pattern_len: u32,
    use_subgroup_coalesce: bool,
) -> Program {
    let scan_nodes = bounded_ranges_scan_nodes(
        haystack,
        transitions,
        output_offsets,
        output_records,
        pattern_lengths,
        match_count,
        matches,
        max_pattern_len,
        use_subgroup_coalesce,
    );
    let body = suffix3_prefilter_body(
        haystack,
        haystack_len,
        candidate_end_mask,
        candidate_suffix2_mask,
        candidate_suffix3_bloom,
        scan_nodes,
    );

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
            BufferDecl::storage(candidate_end_mask, 7, BufferAccess::ReadOnly, DataType::U32)
                .with_count(8),
            BufferDecl::storage(
                candidate_suffix2_mask,
                8,
                BufferAccess::ReadOnly,
                DataType::U32,
            )
            .with_count(CLASSIC_AC_SUFFIX2_MASK_WORDS as u32),
            BufferDecl::storage(
                candidate_suffix3_bloom,
                9,
                BufferAccess::ReadOnly,
                DataType::U32,
            )
            .with_count(CLASSIC_AC_SUFFIX3_BLOOM_WORDS as u32),
            BufferDecl::output(matches, 10, DataType::U32)
                .with_count(max_matches.saturating_mul(3)),
        ],
        [128, 1, 1],
        vec![wrap_anonymous(
            "vyre-libs::matching::classic_ac_bounded_ranges_suffix3_prefilter",
            body,
        )],
    )
}

/// Number of u32 words a presence bitmap needs for `pattern_count` patterns.
#[must_use]
pub fn presence_bitmap_words(pattern_count: u32) -> u32 {
    pattern_count.div_ceil(32).max(1)
}

/// Build a suffix3-prefiltered bounded-ranges AC PRESENCE program: same candidate
/// gating + DFA replay as the match-emitting scan, but each accepted pattern sets
/// one idempotent bit in a `presence_bitmap_words(pattern_count)`-word read-write
/// bitmap (binding 6, replacing the `match_count` + `matches` buffers) via
/// `atomic_or`. The inputs at bindings 0-5 and 7-9 (haystack, DFA tables, prefilter
/// masks) are byte-identical to the scan program, so a resident integration can
/// share the uploaded static tables. There is NO match-triple output and the entire
/// readback is the small bitmap — removing the dense-workload output bottleneck.
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn classic_ac_bounded_ranges_suffix3_presence_program_ext(
    haystack: &str,
    transitions: &str,
    output_offsets: &str,
    output_records: &str,
    pattern_lengths: &str,
    haystack_len: &str,
    presence: &str,
    candidate_end_mask: &str,
    candidate_suffix2_mask: &str,
    candidate_suffix3_bloom: &str,
    state_count: u32,
    output_records_len: u32,
    pattern_count: u32,
    max_pattern_len: u32,
) -> Program {
    let presence_words = presence_bitmap_words(pattern_count);
    let replay_nodes = bounded_ranges_presence_nodes(
        haystack,
        transitions,
        output_offsets,
        output_records,
        presence,
        max_pattern_len,
    );
    let body = suffix3_prefilter_body(
        haystack,
        haystack_len,
        candidate_end_mask,
        candidate_suffix2_mask,
        candidate_suffix3_bloom,
        replay_nodes,
    );

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
            BufferDecl::read_write(presence, 6, DataType::U32).with_count(presence_words),
            BufferDecl::storage(candidate_end_mask, 7, BufferAccess::ReadOnly, DataType::U32)
                .with_count(8),
            BufferDecl::storage(
                candidate_suffix2_mask,
                8,
                BufferAccess::ReadOnly,
                DataType::U32,
            )
            .with_count(CLASSIC_AC_SUFFIX2_MASK_WORDS as u32),
            BufferDecl::storage(
                candidate_suffix3_bloom,
                9,
                BufferAccess::ReadOnly,
                DataType::U32,
            )
            .with_count(CLASSIC_AC_SUFFIX3_BLOOM_WORDS as u32),
        ],
        [128, 1, 1],
        vec![wrap_anonymous(
            "vyre-libs::matching::classic_ac_bounded_ranges_suffix3_presence",
            body,
        )],
    )
}

/// Build the suffix3-prefiltered PRESENCE program for a compiled DFA.
///
/// # Errors
/// Returns an actionable error when DFA output-record metadata exceeds the u32
/// GPU buffer-count ABI.
pub fn try_build_ac_bounded_ranges_suffix3_presence_program(
    dfa: &CompiledDfa,
    pattern_count: u32,
) -> Result<Program, String> {
    let output_records_len = u32::try_from(dfa.output_records.len()).map_err(|source| {
        format!(
            "AC bounded-ranges suffix3 presence DFA output record count {} exceeds u32 GPU buffer metadata: {source}. Fix: shard the pattern set or lower the DFA budget before dispatch.",
            dfa.output_records.len()
        )
    })?;
    Ok(classic_ac_bounded_ranges_suffix3_presence_program_ext(
        "haystack",
        "transitions",
        "output_offsets",
        "output_records",
        "pattern_lengths",
        "haystack_len",
        "presence",
        "candidate_end_mask",
        "candidate_suffix2_mask",
        "candidate_suffix3_bloom",
        dfa.state_count,
        output_records_len,
        pattern_count,
        dfa.max_pattern_len,
    ))
}

/// `ceil(log2(n))` for the binary-search iteration count, with a floor of 1 so a
/// 1- or 2-region program still runs one narrowing step.
#[must_use]
fn ceil_log2(n: u32) -> u32 {
    match n {
        0 | 1 => 1,
        _ => (32 - (n - 1).leading_zeros()).max(1),
    }
}

/// Number of u32 words a per-region presence bitmap needs for `region_count`
/// regions of `pattern_count` patterns each: `region_count × presence_words`.
#[must_use]
pub fn presence_by_region_words(pattern_count: u32, max_regions: u32) -> u32 {
    presence_bitmap_words(pattern_count).saturating_mul(max_regions.max(1))
}

/// Bindings 0-11 of the region-presence program, shared BYTE-IDENTICALLY by the
/// presence-only program ([`classic_ac_bounded_ranges_suffix3_presence_by_region_program_ext`])
/// and the fused presence+positions program
/// ([`classic_ac_bounded_ranges_suffix3_presence_and_positions_by_region_program_ext`],
/// which appends its match-counter binding 12 + triple-output binding 13). One
/// source of truth keeps the shared static-table / region-attribution ABI identical
/// across both builders — a binding added or resized here reaches both at once.
#[allow(clippy::too_many_arguments)]
fn presence_by_region_base_buffer_decls(
    haystack: &str,
    transitions: &str,
    output_offsets: &str,
    output_records: &str,
    pattern_lengths: &str,
    haystack_len: &str,
    presence: &str,
    candidate_end_mask: &str,
    candidate_suffix2_mask: &str,
    candidate_suffix3_bloom: &str,
    region_starts: &str,
    region_base: &str,
    state_count: u32,
    output_records_len: u32,
    pattern_count: u32,
    total_presence_words: u32,
) -> Vec<BufferDecl> {
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
        BufferDecl::storage(haystack_len, 5, BufferAccess::ReadOnly, DataType::U32).with_count(1),
        BufferDecl::read_write(presence, 6, DataType::U32).with_count(total_presence_words),
        BufferDecl::storage(candidate_end_mask, 7, BufferAccess::ReadOnly, DataType::U32)
            .with_count(8),
        BufferDecl::storage(candidate_suffix2_mask, 8, BufferAccess::ReadOnly, DataType::U32)
            .with_count(CLASSIC_AC_SUFFIX2_MASK_WORDS as u32),
        BufferDecl::storage(candidate_suffix3_bloom, 9, BufferAccess::ReadOnly, DataType::U32)
            .with_count(CLASSIC_AC_SUFFIX3_BLOOM_WORDS as u32),
        // Region start offsets (ascending, region_starts[0] == 0). Dynamic length:
        // the kernel reads region_count via buf_len(region_starts).
        BufferDecl::storage(region_starts, 10, BufferAccess::ReadOnly, DataType::U32),
        // Shard base offset (1 u32) added to each local candidate position so a
        // sharded dispatch attributes against the whole-batch region table (0 for
        // the single-dispatch path).
        BufferDecl::storage(region_base, 11, BufferAccess::ReadOnly, DataType::U32).with_count(1),
    ]
}

/// Region-attributed variant of [`classic_ac_bounded_ranges_suffix3_presence_program_ext`]:
/// the presence bitmap (binding 6) is `max_regions × presence_bitmap_words(pattern_count)`
/// words, and a `region_starts` table (binding 10, the ascending file start
/// offsets of the coalesced buffer with `region_starts[0] == 0`) maps each hit to
/// its region row. Same candidate gating + DFA replay + idempotent `atomic_or` as
/// the global presence program, so it keeps the dense-input scan ceiling; the only
/// added per-hit work is a `ceil(log2(max_regions))`-iteration binary search. One
/// compiled program serves any batch with `region_count <= max_regions` (the live
/// count is read from `buf_len(region_starts)`).
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn classic_ac_bounded_ranges_suffix3_presence_by_region_program_ext(
    haystack: &str,
    transitions: &str,
    output_offsets: &str,
    output_records: &str,
    pattern_lengths: &str,
    haystack_len: &str,
    presence: &str,
    candidate_end_mask: &str,
    candidate_suffix2_mask: &str,
    candidate_suffix3_bloom: &str,
    region_starts: &str,
    region_base: &str,
    state_count: u32,
    output_records_len: u32,
    pattern_count: u32,
    max_pattern_len: u32,
    max_regions: u32,
) -> Program {
    let presence_words = presence_bitmap_words(pattern_count);
    let total_presence_words = presence_by_region_words(pattern_count, max_regions);
    let replay_nodes = bounded_ranges_presence_by_region_nodes(
        haystack,
        transitions,
        output_offsets,
        output_records,
        presence,
        region_starts,
        region_base,
        max_pattern_len,
        presence_words,
        ceil_log2(max_regions),
    );
    let body = suffix3_prefilter_body(
        haystack,
        haystack_len,
        candidate_end_mask,
        candidate_suffix2_mask,
        candidate_suffix3_bloom,
        replay_nodes,
    );

    Program::wrapped(
        presence_by_region_base_buffer_decls(
            haystack,
            transitions,
            output_offsets,
            output_records,
            pattern_lengths,
            haystack_len,
            presence,
            candidate_end_mask,
            candidate_suffix2_mask,
            candidate_suffix3_bloom,
            region_starts,
            region_base,
            state_count,
            output_records_len,
            pattern_count,
            total_presence_words,
        ),
        [128, 1, 1],
        vec![wrap_anonymous(
            "vyre-libs::matching::classic_ac_bounded_ranges_suffix3_presence_by_region",
            body,
        )],
    )
}

/// Build the region-attributed suffix3 PRESENCE program for a compiled DFA, sized
/// for up to `max_regions` coalesced files.
///
/// # Errors
/// Returns an actionable error when DFA output-record metadata exceeds the u32
/// GPU buffer-count ABI.
pub fn try_build_ac_bounded_ranges_suffix3_presence_by_region_program(
    dfa: &CompiledDfa,
    pattern_count: u32,
    max_regions: u32,
) -> Result<Program, String> {
    let output_records_len = u32::try_from(dfa.output_records.len()).map_err(|source| {
        format!(
            "AC bounded-ranges suffix3 region-presence DFA output record count {} exceeds u32 GPU buffer metadata: {source}. Fix: shard the pattern set or lower the DFA budget before dispatch.",
            dfa.output_records.len()
        )
    })?;
    Ok(
        classic_ac_bounded_ranges_suffix3_presence_by_region_program_ext(
            "haystack",
            "transitions",
            "output_offsets",
            "output_records",
            "pattern_lengths",
            "haystack_len",
            "presence",
            "candidate_end_mask",
            "candidate_suffix2_mask",
            "candidate_suffix3_bloom",
            "region_starts",
            "region_base",
            dfa.state_count,
            output_records_len,
            pattern_count,
            dfa.max_pattern_len,
            max_regions,
        ),
    )
}

/// FUSED region-presence + match-positions program: one suffix3-gated bounded-ranges
/// scan that writes BOTH the per-region presence bitmap (binding 6, `atomic_or`, like
/// [`classic_ac_bounded_ranges_suffix3_presence_by_region_program_ext`]) AND the
/// `(pattern_id, start, end)` match triples (bindings 12 `match_count` + 13 `matches`,
/// like [`classic_ac_bounded_ranges_suffix3_prefilter_program_ext`]).
///
/// Bindings 0-11 are byte-identical to the presence-by-region program (so an
/// integration can share every uploaded static table and the `region_starts`/
/// `region_base` inputs); bindings 12-13 add the match counter + triple output. A
/// consumer that today dispatches the presence-by-region scan AND a separate
/// position scan over the same haystack collapses both into ONE dispatch with this
/// program — the expensive candidate gate + DFA replay runs once. The two outputs
/// are recall-identical to the separate scans by construction (same candidate set,
/// same `output_records` iteration).
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn classic_ac_bounded_ranges_suffix3_presence_and_positions_by_region_program_ext(
    haystack: &str,
    transitions: &str,
    output_offsets: &str,
    output_records: &str,
    pattern_lengths: &str,
    haystack_len: &str,
    presence: &str,
    candidate_end_mask: &str,
    candidate_suffix2_mask: &str,
    candidate_suffix3_bloom: &str,
    region_starts: &str,
    region_base: &str,
    match_count: &str,
    matches: &str,
    state_count: u32,
    output_records_len: u32,
    pattern_count: u32,
    max_pattern_len: u32,
    max_regions: u32,
    max_matches: u32,
) -> Program {
    let presence_words = presence_bitmap_words(pattern_count);
    let total_presence_words = presence_by_region_words(pattern_count, max_regions);
    let replay_nodes = bounded_ranges_presence_and_positions_by_region_nodes(
        haystack,
        transitions,
        output_offsets,
        output_records,
        pattern_lengths,
        presence,
        region_starts,
        region_base,
        match_count,
        matches,
        max_pattern_len,
        presence_words,
        ceil_log2(max_regions),
    );
    let body = suffix3_prefilter_body(
        haystack,
        haystack_len,
        candidate_end_mask,
        candidate_suffix2_mask,
        candidate_suffix3_bloom,
        replay_nodes,
    );

    let mut buffers = presence_by_region_base_buffer_decls(
        haystack,
        transitions,
        output_offsets,
        output_records,
        pattern_lengths,
        haystack_len,
        presence,
        candidate_end_mask,
        candidate_suffix2_mask,
        candidate_suffix3_bloom,
        region_starts,
        region_base,
        state_count,
        output_records_len,
        pattern_count,
        total_presence_words,
    );
    // Match counter (binding 12) + triple output (binding 13): the position half of
    // the fused output. `append_match` bounds writes to `buf_len(matches) / 3 ==
    // max_matches`.
    buffers.push(BufferDecl::read_write(match_count, 12, DataType::U32).with_count(1));
    buffers.push(
        BufferDecl::output(matches, 13, DataType::U32).with_count(max_matches.saturating_mul(3)),
    );

    Program::wrapped(
        buffers,
        [128, 1, 1],
        vec![wrap_anonymous(
            "vyre-libs::matching::classic_ac_bounded_ranges_suffix3_presence_and_positions_by_region",
            body,
        )],
    )
}

/// Build the fused region-presence + match-positions suffix3 program for a compiled
/// DFA, sized for up to `max_regions` coalesced files and `max_matches` triples.
///
/// # Errors
/// Returns an actionable error when DFA output-record metadata exceeds the u32 GPU
/// buffer-count ABI.
pub fn try_build_ac_bounded_ranges_suffix3_presence_and_positions_by_region_program(
    dfa: &CompiledDfa,
    pattern_count: u32,
    max_regions: u32,
    max_matches: u32,
) -> Result<Program, String> {
    let output_records_len = u32::try_from(dfa.output_records.len()).map_err(|source| {
        format!(
            "AC bounded-ranges suffix3 region-presence+positions DFA output record count {} exceeds u32 GPU buffer metadata: {source}. Fix: shard the pattern set or lower the DFA budget before dispatch.",
            dfa.output_records.len()
        )
    })?;
    Ok(
        classic_ac_bounded_ranges_suffix3_presence_and_positions_by_region_program_ext(
            "haystack",
            "transitions",
            "output_offsets",
            "output_records",
            "pattern_lengths",
            "haystack_len",
            "presence",
            "candidate_end_mask",
            "candidate_suffix2_mask",
            "candidate_suffix3_bloom",
            "region_starts",
            "region_base",
            "match_count",
            "matches",
            dfa.state_count,
            output_records_len,
            pattern_count,
            dfa.max_pattern_len,
            max_regions,
            max_matches,
        ),
    )
}

/// Build the suffix-prefiltered bounded-ranges AC scan for a compiled DFA.
#[must_use]
pub fn build_ac_bounded_ranges_suffix3_prefilter_program(
    dfa: &CompiledDfa,
    pattern_count: u32,
    max_matches: u32,
) -> Program {
    build_ac_bounded_ranges_suffix3_prefilter_program_ext(dfa, pattern_count, max_matches, true)
}

/// Variant of [`build_ac_bounded_ranges_suffix3_prefilter_program`] that
/// exposes the match-append coalescing selector.
#[must_use]
pub fn build_ac_bounded_ranges_suffix3_prefilter_program_ext(
    dfa: &CompiledDfa,
    pattern_count: u32,
    max_matches: u32,
    use_subgroup_coalesce: bool,
) -> Program {
    match try_build_ac_bounded_ranges_suffix3_prefilter_program_ext(
        dfa,
        pattern_count,
        max_matches,
        use_subgroup_coalesce,
    ) {
        Ok(program) => program,
        Err(error) => {
            // Returning an empty-rejecting program would silently drop every
            // match without the caller knowing — a total recall-loss silent
            // fallback (Law 10). Fail closed instead. Callers that need graceful
            // overflow handling call try_build_ac_bounded_ranges_suffix3_prefilter_program_ext
            // directly and shard oversized DFAs across multiple programs.
            panic!(
                "vyre-libs AC bounded-ranges suffix3 prefilter program build failed: {error} — \
                 returning an empty rejecting automaton would silently drop every match; \
                 use try_build_ac_bounded_ranges_suffix3_prefilter_program_ext and shard oversized DFAs."
            )
        }
    }
}

/// Fallible variant of [`build_ac_bounded_ranges_suffix3_prefilter_program`].
///
/// # Errors
///
/// Returns an actionable error when DFA metadata cannot fit the GPU program's
/// u32 buffer-count ABI.
pub fn try_build_ac_bounded_ranges_suffix3_prefilter_program(
    dfa: &CompiledDfa,
    pattern_count: u32,
    max_matches: u32,
) -> Result<Program, String> {
    try_build_ac_bounded_ranges_suffix3_prefilter_program_ext(dfa, pattern_count, max_matches, true)
}

/// Fallible variant of [`build_ac_bounded_ranges_suffix3_prefilter_program_ext`].
///
/// # Errors
///
/// Returns an actionable error when DFA metadata cannot fit the GPU program's
/// u32 buffer-count ABI.
pub fn try_build_ac_bounded_ranges_suffix3_prefilter_program_ext(
    dfa: &CompiledDfa,
    pattern_count: u32,
    max_matches: u32,
    use_subgroup_coalesce: bool,
) -> Result<Program, String> {
    let output_records_len = u32::try_from(dfa.output_records.len()).map_err(|source| {
        format!(
            "AC bounded-ranges suffix3 prefilter DFA output record count {} exceeds u32 GPU buffer metadata: {source}. Fix: shard the pattern set or lower the DFA budget before dispatch.",
            dfa.output_records.len()
        )
    })?;
    Ok(classic_ac_bounded_ranges_suffix3_prefilter_program_ext(
        "haystack",
        "transitions",
        "output_offsets",
        "output_records",
        "pattern_lengths",
        "haystack_len",
        "match_count",
        "candidate_end_mask",
        "candidate_suffix2_mask",
        "candidate_suffix3_bloom",
        "matches",
        dfa.state_count,
        output_records_len,
        pattern_count,
        max_matches,
        dfa.max_pattern_len,
        use_subgroup_coalesce,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scan::classic_ac::{
        classic_ac_bounded_ranges_scan, classic_ac_candidate_end_byte_mask_words,
        classic_ac_candidate_suffix2_mask_words, classic_ac_candidate_suffix3_bloom_words,
        classic_ac_compile,
    };
    use crate::scan::{pack_haystack_u32, pack_u32_slice};

    #[test]
    fn suffix3_prefilter_builder_fails_loud_not_silent_fallback() {
        // Law 10 regression guard: the infallible suffix3 prefilter program
        // builder must not swallow a build error into an empty rejecting
        // program (which silently drops every match). The old arm logged the
        // failure and returned a degenerate empty program via a dedicated
        // helper; assert that fallback helper is gone and an explicit panic!()
        // fail-loud arm is present. (The "build failed" message string itself
        // now lives in the panic! arm, so it cannot be used as the signal.)
        let production = include_str!("suffix3.rs")
            .split("#[cfg(test)]")
            .next()
            .expect("Fix: suffix3.rs must contain a production section");
        assert!(
            !production.contains(concat!("empty_ac_bounded_ranges", "_suffix3_prefilter_program")),
            "Fix: suffix3 prefilter builder must not fall back to an empty rejecting program on error — fail loud via panic!() so callers use the try_ variant."
        );
        assert!(
            production.contains("panic!("),
            "Fix: suffix3 prefilter builder must panic!() on an unrepresentable DFA, never return an empty rejecting program."
        );
    }

    fn decode_u32(bytes: &[u8]) -> Vec<u32> {
        bytes
            .chunks_exact(4)
            .map(|chunk| u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
            .collect()
    }

    fn pattern_lengths(patterns: &[&[u8]]) -> Vec<u32> {
        patterns
            .iter()
            .map(|pattern| pattern.len() as u32)
            .collect()
    }

    fn decode_match_triples(outputs: &[vyre_reference::value::Value]) -> Vec<(u32, u32, u32)> {
        let count = decode_u32(&outputs[0].to_bytes())[0] as usize;
        let words = decode_u32(&outputs[1].to_bytes());
        words[..count.saturating_mul(3)]
            .chunks_exact(3)
            .map(|chunk| (chunk[0], chunk[1], chunk[2]))
            .collect()
    }

    #[test]
    fn bounded_ranges_suffix3_prefilter_reference_eval_matches_cpu_oracle() {
        let patterns: [&[u8]; 6] = [b"a", b"bc", b"ab", b"abcd", b"BEGIN", b"token"];
        let haystack = b"zabcd a bc BEGIN token abcdbc";
        let ac = classic_ac_compile(&patterns);
        let lengths = pattern_lengths(&patterns);
        let mut expected = classic_ac_bounded_ranges_scan(&ac, &lengths, haystack);
        expected.sort_unstable();
        let program = build_ac_bounded_ranges_suffix3_prefilter_program_ext(
            &ac.dfa,
            patterns.len() as u32,
            128,
            false,
        );
        let inputs = vec![
            vyre_reference::value::Value::from(pack_haystack_u32(haystack)),
            vyre_reference::value::Value::from(pack_u32_slice(&ac.dfa.transitions)),
            vyre_reference::value::Value::from(pack_u32_slice(&ac.dfa.output_offsets)),
            vyre_reference::value::Value::from(pack_u32_slice(&ac.dfa.output_records)),
            vyre_reference::value::Value::from(pack_u32_slice(&lengths)),
            vyre_reference::value::Value::from(pack_u32_slice(&[haystack.len() as u32])),
            vyre_reference::value::Value::from(pack_u32_slice(&[0])),
            vyre_reference::value::Value::from(pack_u32_slice(
                &classic_ac_candidate_end_byte_mask_words(&ac.dfa),
            )),
            vyre_reference::value::Value::from(pack_u32_slice(
                &classic_ac_candidate_suffix2_mask_words(&ac.dfa),
            )),
            vyre_reference::value::Value::from(pack_u32_slice(
                &classic_ac_candidate_suffix3_bloom_words(&patterns),
            )),
        ];
        let outputs = vyre_reference::reference_eval(&program, &inputs).expect(
            "Fix: suffix3 prefiltered AC bounded-ranges program should evaluate in reference backend.",
        );
        let mut actual = decode_match_triples(&outputs);
        actual.sort_unstable();

        assert_eq!(actual, expected);
    }

    #[test]
    fn bounded_ranges_suffix3_prefilter_program_has_compact_stable_shape() {
        let ac = classic_ac_compile(&[b"Authorization: Bearer ", b"token", b"tok"]);
        let program =
            build_ac_bounded_ranges_suffix3_prefilter_program_ext(&ac.dfa, 3, 1024, false);

        assert_eq!(program.workgroup_size(), [128, 1, 1]);
        assert_eq!(program.buffers().len(), 11);
        assert_eq!(program.buffers()[6].name(), "match_count");
        assert_eq!(program.buffers()[6].count, 1);
        assert_eq!(program.buffers()[7].name(), "candidate_end_mask");
        assert_eq!(program.buffers()[7].count, 8);
        assert_eq!(program.buffers()[8].name(), "candidate_suffix2_mask");
        assert_eq!(
            program.buffers()[8].count,
            CLASSIC_AC_SUFFIX2_MASK_WORDS as u32
        );
        assert_eq!(program.buffers()[9].name(), "candidate_suffix3_bloom");
        assert_eq!(
            program.buffers()[9].count,
            CLASSIC_AC_SUFFIX3_BLOOM_WORDS as u32
        );
        assert_eq!(program.buffers()[10].name(), "matches");
        assert_eq!(program.buffers()[10].count, 1024 * 3);
    }

    #[test]
    fn region_presence_program_has_region_attributed_shape() {
        let ac = classic_ac_compile(&[b"token", b"tok", b"secret"]);
        let pattern_count = 3u32;
        let max_regions = 8u32;
        let program = try_build_ac_bounded_ranges_suffix3_presence_by_region_program(
            &ac.dfa,
            pattern_count,
            max_regions,
        )
        .expect("Fix: region-presence program must build for a small DFA");

        // Bindings 0-9 match the global presence program; the per-region variant
        // adds `region_starts` at binding 10 and a `region_base` shard offset at
        // binding 11, and widens `presence` to a per-region bitmap (row stride ×
        // max_regions) instead of a single global row.
        assert_eq!(program.workgroup_size(), [128, 1, 1]);
        assert_eq!(program.buffers().len(), 12);
        assert_eq!(program.buffers()[6].name(), "presence");
        let words = presence_bitmap_words(pattern_count);
        assert_eq!(program.buffers()[6].count, words * max_regions);
        assert_eq!(
            program.buffers()[6].count,
            presence_by_region_words(pattern_count, max_regions)
        );
        assert_eq!(program.buffers()[10].name(), "region_starts");
        assert_eq!(program.buffers()[11].name(), "region_base");
        assert_eq!(program.buffers()[11].count, 1);
    }

    #[test]
    fn ceil_log2_bounds_binary_search_iterations() {
        // ceil(log2(n)) with a floor of 1: the number of narrowing steps a
        // binary search over n regions needs (n region rows → 1 index).
        assert_eq!(ceil_log2(0), 1);
        assert_eq!(ceil_log2(1), 1);
        assert_eq!(ceil_log2(2), 1);
        assert_eq!(ceil_log2(3), 2);
        assert_eq!(ceil_log2(4), 2);
        assert_eq!(ceil_log2(5), 3);
        assert_eq!(ceil_log2(8), 3);
        assert_eq!(ceil_log2(9), 4);
        assert_eq!(ceil_log2(16), 4);
        assert_eq!(ceil_log2(65536), 16);
    }
}
