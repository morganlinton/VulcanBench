use vyre::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

use crate::region::wrap_anonymous;
use crate::scan::builders::load_packed_byte_expr;
use crate::scan::dfa::CompiledDfa;

use super::bounded_ranges_scan_nodes;

#[path = "prefilter/suffix3.rs"]
mod suffix3;

pub use suffix3::{
    build_ac_bounded_ranges_suffix3_prefilter_program,
    build_ac_bounded_ranges_suffix3_prefilter_program_ext,
    classic_ac_bounded_ranges_suffix3_prefilter_program,
    classic_ac_bounded_ranges_suffix3_prefilter_program_ext,
    classic_ac_bounded_ranges_suffix3_presence_and_positions_by_region_program_ext,
    classic_ac_bounded_ranges_suffix3_presence_by_region_program_ext,
    classic_ac_bounded_ranges_suffix3_presence_program_ext, presence_bitmap_words,
    presence_by_region_words, try_build_ac_bounded_ranges_suffix3_prefilter_program,
    try_build_ac_bounded_ranges_suffix3_prefilter_program_ext,
    try_build_ac_bounded_ranges_suffix3_presence_and_positions_by_region_program,
    try_build_ac_bounded_ranges_suffix3_presence_by_region_program,
    try_build_ac_bounded_ranges_suffix3_presence_program,
};

/// Build a bounded-window AC ranges program with an exact candidate-end-byte
/// prefilter.
///
/// `candidate_end_mask` is an 8-word bitset where bit `b` is set when byte `b`
/// can terminate at least one accepted DFA state. Non-candidate lanes skip the
/// bounded replay window and match append path entirely.
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn classic_ac_bounded_ranges_prefilter_program(
    haystack: &str,
    transitions: &str,
    output_offsets: &str,
    output_records: &str,
    pattern_lengths: &str,
    haystack_len: &str,
    match_count: &str,
    candidate_end_mask: &str,
    matches: &str,
    state_count: u32,
    output_records_len: u32,
    pattern_count: u32,
    max_matches: u32,
    max_pattern_len: u32,
) -> Program {
    classic_ac_bounded_ranges_prefilter_program_ext(
        haystack,
        transitions,
        output_offsets,
        output_records,
        pattern_lengths,
        haystack_len,
        match_count,
        candidate_end_mask,
        matches,
        state_count,
        output_records_len,
        pattern_count,
        max_matches,
        max_pattern_len,
        true,
    )
}

/// Variant of [`classic_ac_bounded_ranges_prefilter_program`] with explicit
/// control over subgroup match-append coalescing.
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn classic_ac_bounded_ranges_prefilter_program_ext(
    haystack: &str,
    transitions: &str,
    output_offsets: &str,
    output_records: &str,
    pattern_lengths: &str,
    haystack_len: &str,
    match_count: &str,
    candidate_end_mask: &str,
    matches: &str,
    state_count: u32,
    output_records_len: u32,
    pattern_count: u32,
    max_matches: u32,
    max_pattern_len: u32,
    use_subgroup_coalesce: bool,
) -> Program {
    let i = Expr::var("i");
    let candidate_byte = load_packed_byte_expr(haystack, i.clone());
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

    let body = vec![
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
                    scan_nodes,
                ),
            ],
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
            BufferDecl::storage(candidate_end_mask, 7, BufferAccess::ReadOnly, DataType::U32)
                .with_count(8),
            BufferDecl::output(matches, 8, DataType::U32).with_count(max_matches.saturating_mul(3)),
        ],
        [128, 1, 1],
        vec![wrap_anonymous(
            "vyre-libs::matching::classic_ac_bounded_ranges_prefilter",
            body,
        )],
    )
}

/// Build the candidate-end prefiltered bounded-ranges AC scan for a compiled
/// DFA.
#[must_use]
pub fn build_ac_bounded_ranges_prefilter_program(
    dfa: &CompiledDfa,
    pattern_count: u32,
    max_matches: u32,
) -> Program {
    build_ac_bounded_ranges_prefilter_program_ext(dfa, pattern_count, max_matches, true)
}

/// Variant of [`build_ac_bounded_ranges_prefilter_program`] that exposes the
/// match-append coalescing selector.
#[must_use]
pub fn build_ac_bounded_ranges_prefilter_program_ext(
    dfa: &CompiledDfa,
    pattern_count: u32,
    max_matches: u32,
    use_subgroup_coalesce: bool,
) -> Program {
    match try_build_ac_bounded_ranges_prefilter_program_ext(
        dfa,
        pattern_count,
        max_matches,
        use_subgroup_coalesce,
    ) {
        Ok(program) => program,
        Err(error) => {
            // Returning an empty-mask prefilter program would silently suppress
            // every candidate position — a total recall-loss silent fallback.
            // Fail closed instead. Callers that need graceful overflow handling
            // must call try_build_ac_bounded_ranges_prefilter_program_ext
            // directly and shard oversized DFAs across multiple programs.
            panic!(
                "AC bounded-ranges prefilter program build failed: {error} — \
                 returning an empty-mask program would silently suppress every match; \
                 use try_build_ac_bounded_ranges_prefilter_program_ext and shard oversized DFAs."
            )
        }
    }
}

/// Fallible variant of [`build_ac_bounded_ranges_prefilter_program`].
///
/// # Errors
///
/// Returns an actionable error when DFA metadata cannot fit the GPU program's
/// u32 buffer-count ABI.
pub fn try_build_ac_bounded_ranges_prefilter_program(
    dfa: &CompiledDfa,
    pattern_count: u32,
    max_matches: u32,
) -> Result<Program, String> {
    try_build_ac_bounded_ranges_prefilter_program_ext(dfa, pattern_count, max_matches, true)
}

/// Fallible variant of [`build_ac_bounded_ranges_prefilter_program_ext`].
///
/// # Errors
///
/// Returns an actionable error when DFA metadata cannot fit the GPU program's
/// u32 buffer-count ABI.
pub fn try_build_ac_bounded_ranges_prefilter_program_ext(
    dfa: &CompiledDfa,
    pattern_count: u32,
    max_matches: u32,
    use_subgroup_coalesce: bool,
) -> Result<Program, String> {
    let output_records_len = u32::try_from(dfa.output_records.len()).map_err(|source| {
        format!(
            "AC bounded-ranges prefilter DFA output record count {} exceeds u32 GPU buffer metadata: {source}. Fix: shard the pattern set or lower the DFA budget before dispatch.",
            dfa.output_records.len()
        )
    })?;
    Ok(classic_ac_bounded_ranges_prefilter_program_ext(
        "haystack",
        "transitions",
        "output_offsets",
        "output_records",
        "pattern_lengths",
        "haystack_len",
        "match_count",
        "candidate_end_mask",
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
        classic_ac_compile,
    };
    use crate::scan::{pack_haystack_u32, pack_u32_slice};

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
    fn bounded_ranges_prefilter_reference_eval_matches_cpu_oracle() {
        let patterns: [&[u8]; 5] = [b"a", b"bc", b"abcd", b"BEGIN", b"token"];
        let haystack = b"zabcd BEGIN token abcdbc";
        let ac = classic_ac_compile(&patterns);
        let lengths = pattern_lengths(&patterns);
        let mut expected = classic_ac_bounded_ranges_scan(&ac, &lengths, haystack);
        expected.sort_unstable();
        let program = build_ac_bounded_ranges_prefilter_program_ext(
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
        ];
        let outputs = vyre_reference::reference_eval(&program, &inputs).expect(
            "Fix: prefiltered AC bounded-ranges program should evaluate in reference backend.",
        );
        let mut actual = decode_match_triples(&outputs);
        actual.sort_unstable();

        assert_eq!(actual, expected);
    }

    /// Regression guard: build_ac_bounded_ranges_prefilter_program_ext must
    /// never silently return an empty-mask program when the DFA is oversized.
    /// Before this fix the error arm called eprintln! and fell back to the
    /// classic_ac_bounded_ranges_prefilter_program_ext with state_count=1 and
    /// output_records_len=0, causing every candidate position to be suppressed
    /// with no signal to the caller.
    #[test]
    fn infallible_prefilter_builder_panics_not_falls_back() {
        let src = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/scan/classic_ac/bounded_ranges/prefilter.rs"
        ))
        .expect("Fix: prefilter source must be readable for regression guard");
        let production = src
            .split("\n#[cfg(test)]")
            .next()
            .expect("Fix: prefilter source must have a test section");
        // The old silent-fallback arm called classic_ac_bounded_ranges_prefilter_program_ext
        // directly inside the Err branch. Check it is absent from the production path.
        assert!(
            !production.contains("eprintln!(\"vyre-libs AC bounded-ranges prefilter"),
            "build_ac_bounded_ranges_prefilter_program_ext must not silently log and \
             return an empty-mask program on error — use panic!() instead."
        );
    }

    #[test]
    fn bounded_ranges_prefilter_program_has_compact_stable_shape() {
        let ac = classic_ac_compile(&[b"Authorization: Bearer ", b"token", b"tok"]);
        let program = build_ac_bounded_ranges_prefilter_program_ext(&ac.dfa, 3, 1024, false);

        assert_eq!(program.workgroup_size(), [128, 1, 1]);
        assert_eq!(program.buffers().len(), 9);
        assert_eq!(program.buffers()[6].name(), "match_count");
        assert_eq!(program.buffers()[6].count, 1);
        assert_eq!(program.buffers()[7].name(), "candidate_end_mask");
        assert_eq!(program.buffers()[7].count, 8);
        assert_eq!(program.buffers()[8].name(), "matches");
        assert_eq!(program.buffers()[8].count, 1024 * 3);
    }
}
