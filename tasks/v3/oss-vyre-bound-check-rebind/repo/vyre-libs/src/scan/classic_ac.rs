//! Classic Aho-Corasick scanner with flat `output_links`.
//!
//! Unlike the single-accept DFA in `super::dfa::dfa_compile`, this
//! module precomputes **all** pattern ids that match at each state
//! (including patterns reachable via failure links) into a flat
//! `output_offsets` + `output_records` array.  At scan time the
//! match loop is **O(matches)**, not **O(states × n)**.
//!
//! Build-time complexity is still dominated by the dense transition
//! table (O(states × alphabet)), but the per-state pattern list is
//! built in one BFS pass using dynamic programming on the failure
//! links.

use vyre::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

use crate::region::wrap_anonymous;
use crate::scan::dfa::{dfa_compile, CompiledDfa};

#[path = "classic_ac/bounded_ranges.rs"]
mod bounded_ranges;

#[path = "classic_ac/count_program.rs"]
mod count_program;

#[cfg(any(test, feature = "cpu-parity"))]
pub use bounded_ranges::classic_ac_bounded_ranges_scan;
pub use bounded_ranges::{
    build_ac_bounded_ranges_prefilter_program, build_ac_bounded_ranges_prefilter_program_ext,
    build_ac_bounded_ranges_program, build_ac_bounded_ranges_program_ext,
    build_ac_bounded_ranges_suffix3_prefilter_program,
    build_ac_bounded_ranges_suffix3_prefilter_program_ext,
    classic_ac_bounded_ranges_prefilter_program, classic_ac_bounded_ranges_prefilter_program_ext,
    classic_ac_bounded_ranges_program, classic_ac_bounded_ranges_program_ext,
    classic_ac_bounded_ranges_suffix3_prefilter_program,
    classic_ac_bounded_ranges_suffix3_prefilter_program_ext,
    classic_ac_bounded_ranges_suffix3_presence_and_positions_by_region_program_ext,
    classic_ac_bounded_ranges_suffix3_presence_by_region_program_ext,
    classic_ac_bounded_ranges_suffix3_presence_program_ext, presence_bitmap_words,
    presence_by_region_words, try_build_ac_bounded_ranges_prefilter_program,
    try_build_ac_bounded_ranges_prefilter_program_ext, try_build_ac_bounded_ranges_program,
    try_build_ac_bounded_ranges_program_ext, try_build_ac_bounded_ranges_suffix3_prefilter_program,
    try_build_ac_bounded_ranges_suffix3_prefilter_program_ext,
    try_build_ac_bounded_ranges_suffix3_presence_and_positions_by_region_program,
    try_build_ac_bounded_ranges_suffix3_presence_by_region_program,
    try_build_ac_bounded_ranges_suffix3_presence_program,
};

pub use count_program::{
    build_ac_bounded_count_prefilter_program, build_ac_bounded_count_program,
    build_ac_bounded_count_suffix2_prefilter_program,
    build_ac_bounded_count_suffix3_prefilter_program, classic_ac_bounded_count_prefilter_program,
    classic_ac_bounded_count_program, classic_ac_bounded_count_suffix2_prefilter_program,
    classic_ac_bounded_count_suffix3_prefilter_program, classic_ac_candidate_end_byte_mask_words,
    classic_ac_candidate_suffix2_mask_words, classic_ac_candidate_suffix3_bloom_words,
    classic_ac_suffix3_bloom_contains, CLASSIC_AC_SUFFIX2_MASK_WORDS,
    CLASSIC_AC_SUFFIX3_BLOOM_WORDS,
};

/// A classic AC automaton with precomputed flat output links.
///
/// Wraps a [`CompiledDfa`] and exposes the Reference oracle scan plus
/// a vyre IR Program for GPU dispatch.
#[derive(Debug, Clone)]
pub struct ClassicAcAutomaton {
    /// Underlying compiled DFA (transitions + flat output links).
    pub dfa: CompiledDfa,
}

/// Build a [`ClassicAcAutomaton`] from a list of byte patterns.
///
/// # Panics
///
/// Panics with an actionable message on DFA budget exhaustion.
/// Use `super::dfa::dfa_compile_with_budget` for structured
/// error handling.
#[must_use]
pub fn classic_ac_compile(patterns: &[&[u8]]) -> ClassicAcAutomaton {
    let dfa = dfa_compile(patterns);
    ClassicAcAutomaton { dfa }
}

/// Reference oracle: walk the DFA byte-by-byte and emit **every**
/// pattern id that matches at each offset.
///
/// Returns a vector of `(pattern_id, end_offset)` pairs.  `end_offset`
/// is the byte position (0-based, inclusive) where the match ends.
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn classic_ac_scan(ac: &ClassicAcAutomaton, haystack: &[u8]) -> Vec<(u32, u32)> {
    let dfa = &ac.dfa;
    let mut state = 0u32;
    let mut out = Vec::new();
    for (pos, &b) in haystack.iter().enumerate() {
        state = dfa.transitions[(state as usize) * 256 + (b as usize)];
        let begin = dfa.output_offsets[state as usize] as usize;
        let end = dfa.output_offsets[state as usize + 1] as usize;
        for &pattern_id in &dfa.output_records[begin..end] {
            out.push((pattern_id, pos as u32));
        }
    }
    out
}

/// Reference oracle that returns a per-position **count** of matches.
///
/// `counts[i]` = number of patterns that match ending at byte `i`.
/// This is the oracle shape used by the companion GPU emit when the
/// caller only needs cardinality, not the individual pattern ids.
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn classic_ac_scan_counts(ac: &ClassicAcAutomaton, haystack: &[u8]) -> Vec<u32> {
    let dfa = &ac.dfa;
    let mut state = 0u32;
    let mut out = Vec::with_capacity(haystack.len());
    for &b in haystack {
        state = dfa.transitions[(state as usize) * 256 + (b as usize)];
        let begin = dfa.output_offsets[state as usize] as usize;
        let end = dfa.output_offsets[state as usize + 1] as usize;
        out.push((end - begin) as u32);
    }
    out
}

/// Build a vyre `Program` that scans `haystack` and appends every
/// matching pattern id to `matches` via an atomic slot counter.
///
/// Buffers (bindings 0..5):
///
/// | binding | name | access | count |
/// |---|---|---|---|
/// | 0 | `haystack` | ReadOnly | `haystack_len` |
/// | 1 | `transitions` | ReadOnly | `state_count * 256` |
/// | 2 | `output_offsets` | ReadOnly | `state_count + 1` |
/// | 3 | `output_records` | ReadOnly | `output_records_len` |
/// | 4 | `match_count` | ReadWrite | 1 (atomic) |
/// | 5 | `matches` | ReadWrite | `max_matches` |
///
/// Each invocation `i` replays the DFA from state 0 through
/// `haystack[0..=i]`, then atomically claims slots in `matches` and
/// writes every pattern id for the final state.
///
/// The serial replay is still O(n²) total work  -  the fix here is
/// the **match-emission loop**, which is O(matches) thanks to the
/// flat `output_links`.
#[must_use]
pub fn classic_ac_program(
    haystack: &str,
    transitions: &str,
    output_offsets: &str,
    output_records: &str,
    match_count: &str,
    matches: &str,
    haystack_len: u32,
    state_count: u32,
    output_records_len: u32,
    max_matches: u32,
) -> Program {
    let i = Expr::var("i");

    // body executed per invocation i:
    //   walk DFA from 0 through haystack[0..=i]
    //   for each pattern in output_links[state]:
    //       slot = atomic_add(match_count, 0, 1)
    //       if slot < max_matches:
    //           matches[slot] = pattern_id
    let walk_body = vec![
        Node::let_bind("i", Expr::InvocationId { axis: 0 }),
        Node::if_then(
            Expr::lt(i.clone(), Expr::buf_len(haystack)),
            vec![
                // Walk the automaton from state 0 through haystack[0..=i].
                Node::let_bind("state", Expr::u32(0)),
                Node::loop_for(
                    "step",
                    Expr::u32(0),
                    Expr::add(Expr::var("i"), Expr::u32(1)),
                    vec![Node::assign(
                        "state",
                        Expr::load(
                            transitions,
                            Expr::add(
                                Expr::mul(Expr::var("state"), Expr::u32(256)),
                                Expr::load(haystack, Expr::var("step")),
                            ),
                        ),
                    )],
                ),
                // Emit every pattern in the flat output_links[state].
                Node::let_bind("out_begin", Expr::load(output_offsets, Expr::var("state"))),
                Node::let_bind(
                    "out_end",
                    Expr::load(output_offsets, Expr::add(Expr::var("state"), Expr::u32(1))),
                ),
                Node::loop_for(
                    "out_idx",
                    Expr::var("out_begin"),
                    Expr::var("out_end"),
                    vec![
                        Node::let_bind(
                            "pattern_id",
                            Expr::load(output_records, Expr::var("out_idx")),
                        ),
                        Node::let_bind(
                            "slot",
                            Expr::atomic_add(match_count, Expr::u32(0), Expr::u32(1)),
                        ),
                        Node::if_then(
                            Expr::lt(Expr::var("slot"), Expr::u32(max_matches)),
                            vec![Node::Store {
                                buffer: matches.into(),
                                index: Expr::var("slot"),
                                value: Expr::var("pattern_id"),
                            }],
                        ),
                    ],
                ),
            ],
        ),
    ];

    Program::wrapped(
        vec![
            BufferDecl::storage(haystack, 0, BufferAccess::ReadOnly, DataType::U32)
                .with_count(haystack_len),
            BufferDecl::storage(transitions, 1, BufferAccess::ReadOnly, DataType::U32)
                .with_count(state_count.saturating_mul(256)),
            BufferDecl::storage(output_offsets, 2, BufferAccess::ReadOnly, DataType::U32)
                .with_count(state_count.saturating_add(1)),
            BufferDecl::storage(output_records, 3, BufferAccess::ReadOnly, DataType::U32)
                .with_count(output_records_len),
            BufferDecl::storage(match_count, 4, BufferAccess::ReadWrite, DataType::U32)
                .with_count(1),
            BufferDecl::storage(matches, 5, BufferAccess::ReadWrite, DataType::U32)
                .with_count(max_matches),
        ],
        [64, 1, 1],
        vec![wrap_anonymous("vyre-libs::matching::classic_ac", walk_body)],
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::byte_pack::bytes_to_u32 as decode_u32_words;

    #[test]
    fn single_pattern_matches() {
        let ac = classic_ac_compile(&[b"abc"]);
        let matches = classic_ac_scan(&ac, b"xxabcxx");
        assert_eq!(matches, vec![(0, 4)]);
    }

    #[test]
    fn overlapping_patterns_report_all() {
        // Patterns "he", "she", "his", "hers" on "ushers".
        let ac = classic_ac_compile(&[b"he", b"she", b"his", b"hers"]);
        let matches = classic_ac_scan(&ac, b"ushers");
        // "she" ends at position 3, "he" ends at position 3,
        // "hers" ends at position 5.
        assert!(matches.contains(&(1, 3)), "must match she");
        assert!(matches.contains(&(0, 3)), "must match he");
        assert!(matches.contains(&(3, 5)), "must match hers");
    }

    #[test]
    fn nested_suffix_patterns_all_reported() {
        // a, aa, aaa, aaaa on "aaaa".
        let ac = classic_ac_compile(&[b"a", b"aa", b"aaa", b"aaaa"]);
        let matches = classic_ac_scan(&ac, b"aaaa");
        // Position 0: "a" (1 char)
        // Position 1: "a", "aa"
        // Position 2: "a", "aa", "aaa"
        // Position 3: "a", "aa", "aaa", "aaaa"
        let expected = vec![
            (0, 0),
            (0, 1),
            (1, 1),
            (0, 2),
            (1, 2),
            (2, 2),
            (0, 3),
            (1, 3),
            (2, 3),
            (3, 3),
        ];
        assert_eq!(matches, expected);
    }

    #[test]
    fn adversarial_failure_chain_is_linear_in_matches() {
        // Patterns a, aa, aaa, ... up to length 128.
        // This creates a long failure-link chain.
        let patterns: Vec<Vec<u8>> = (1..=128).map(|i| vec![b'a'; i]).collect();
        let pattern_refs: Vec<&[u8]> = patterns.iter().map(|p| p.as_slice()).collect();
        let ac = classic_ac_compile(&pattern_refs);
        let haystack = vec![b'a'; 128];
        let matches = classic_ac_scan(&ac, &haystack);

        // At position i (0-based) there are i+1 matches.
        // Total matches = 128 * 129 / 2 = 8256.
        assert_eq!(matches.len(), 8256);

        // Verify the last position has all 128 patterns.
        let last_pos_matches: Vec<u32> = matches
            .iter()
            .filter(|&&(_, pos)| pos == 127)
            .map(|&(pid, _)| pid)
            .collect();
        assert_eq!(last_pos_matches.len(), 128);
        for (expected_pid, &actual_pid) in last_pos_matches.iter().enumerate() {
            assert_eq!(actual_pid, expected_pid as u32);
        }
    }

    #[test]
    fn empty_haystack_yields_no_matches() {
        let ac = classic_ac_compile(&[b"abc"]);
        assert!(classic_ac_scan(&ac, b"").is_empty());
    }

    #[test]
    fn empty_pattern_list_yields_no_matches() {
        let ac = classic_ac_compile(&[]);
        assert!(classic_ac_scan(&ac, b"anything").is_empty());
    }

    #[test]
    fn gpu_emit_matches_cpu_reference() {
        let patterns: [&[u8]; 4] = [b"he", b"she", b"his", b"hers"];
        let ac = classic_ac_compile(&patterns);
        let haystack = b"ushers";
        let cpu = classic_ac_scan(&ac, haystack);

        // Build the vyre IR program.
        let program = classic_ac_program(
            "haystack",
            "transitions",
            "output_offsets",
            "output_records",
            "match_count",
            "matches",
            haystack.len() as u32,
            ac.dfa.state_count,
            ac.dfa.output_records.len() as u32,
            1024,
        );

        let inputs = vec![
            vyre_reference::value::Value::from(crate::test_support::byte_pack::u32_bytes(
                &haystack.iter().map(|&b| u32::from(b)).collect::<Vec<_>>(),
            )),
            vyre_reference::value::Value::from(crate::test_support::byte_pack::u32_bytes(
                &ac.dfa.transitions,
            )),
            vyre_reference::value::Value::from(crate::test_support::byte_pack::u32_bytes(
                &ac.dfa.output_offsets,
            )),
            vyre_reference::value::Value::from(crate::test_support::byte_pack::u32_bytes(
                &ac.dfa.output_records,
            )),
            vyre_reference::value::Value::from(crate::test_support::byte_pack::u32_bytes(&[0u32])),
            vyre_reference::value::Value::from(vec![0u8; 1024 * 4]),
        ];

        let outputs = vyre_reference::reference_eval(&program, &inputs).expect(
            "Fix: classic_ac_program must execute; restore this invariant before continuing.",
        );

        let match_count = decode_u32_words(&outputs[0].to_bytes())[0];
        let gpu_matches_raw = decode_u32_words(&outputs[1].to_bytes());
        let gpu_matches: Vec<u32> = gpu_matches_raw[..match_count as usize].to_vec();

        // CPU gives (pattern_id, end_pos); GPU gives just pattern_id
        // because each invocation writes its own patterns.  The order
        // is nondeterministic (atomic scheduling), so sort both.
        let mut cpu_ids: Vec<u32> = cpu.iter().map(|&(pid, _)| pid).collect();
        cpu_ids.sort_unstable();

        let mut gpu_ids = gpu_matches;
        gpu_ids.sort_unstable();

        assert_eq!(
            cpu_ids, gpu_ids,
            "GPU emit must agree with Reference oracle on pattern ids"
        );
    }

    #[test]
    fn gpu_emit_does_not_overflow_when_max_matches_is_small() {
        let ac = classic_ac_compile(&[b"a", b"aa", b"aaa"]);
        let haystack = b"aaa";
        let program = classic_ac_program(
            "haystack",
            "transitions",
            "output_offsets",
            "output_records",
            "match_count",
            "matches",
            haystack.len() as u32,
            ac.dfa.state_count,
            ac.dfa.output_records.len() as u32,
            2, // only allow 2 stored matches
        );

        let inputs = vec![
            vyre_reference::value::Value::from(crate::test_support::byte_pack::u32_bytes(
                &haystack.iter().map(|&b| u32::from(b)).collect::<Vec<_>>(),
            )),
            vyre_reference::value::Value::from(crate::test_support::byte_pack::u32_bytes(
                &ac.dfa.transitions,
            )),
            vyre_reference::value::Value::from(crate::test_support::byte_pack::u32_bytes(
                &ac.dfa.output_offsets,
            )),
            vyre_reference::value::Value::from(crate::test_support::byte_pack::u32_bytes(
                &ac.dfa.output_records,
            )),
            vyre_reference::value::Value::from(crate::test_support::byte_pack::u32_bytes(&[0u32])),
            vyre_reference::value::Value::from(vec![0u8; 2 * 4]),
        ];

        let outputs = vyre_reference::reference_eval(&program, &inputs).expect(
            "Fix: classic_ac_program must execute; restore this invariant before continuing.",
        );

        let match_count = decode_u32_words(&outputs[0].to_bytes())[0];
        // Total matches = 1 + 2 + 3 = 6, but only 2 slots.
        assert_eq!(match_count, 6, "match_count must reflect total discoveries");
        let stored = decode_u32_words(&outputs[1].to_bytes());
        assert_eq!(stored.len(), 2, "only 2 slots allocated");
    }

    #[test]
    fn bounded_ranges_builder_exposes_checked_metadata_variant() {
        // Inspect only the production section — everything before the file's own
        // `#[cfg(test)]` block. That test module legitimately uses .expect()/
        // .unwrap(), which must not trip the "builder must not panic" guard below
        // (the guard concerns the production builder, not its tests).
        let full = include_str!("classic_ac/bounded_ranges.rs");
        let production = full.split("\n#[cfg(test)]").next().unwrap_or(full);

        assert!(
            production.contains("try_build_ac_bounded_ranges_program_ext"),
            "Fix: AC bounded-ranges builder must expose a fallible metadata sizing path."
        );
        assert!(
            !production.contains("dfa.output_records.len() as u32"),
            "Fix: AC bounded-ranges builder must not narrow output record counts with unchecked casts."
        );
        assert!(
            production.contains("u32::try_from(dfa.output_records.len())"),
            "Fix: AC bounded-ranges output record count must use checked conversion."
        );
        assert!(
            !production.contains(".expect(") && !production.contains(".unwrap("),
            "Fix: AC bounded-ranges production builder must not panic."
        );
    }
}
