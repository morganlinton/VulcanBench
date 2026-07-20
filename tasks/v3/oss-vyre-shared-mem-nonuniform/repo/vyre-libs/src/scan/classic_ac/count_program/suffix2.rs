use vyre::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

use crate::region::wrap_anonymous;
use crate::scan::builders::load_packed_byte_expr;
use crate::scan::dfa::CompiledDfa;

use super::count_scan_nodes;

/// Number of u32 words in the 65,536-bit two-byte AC suffix mask.
pub const CLASSIC_AC_SUFFIX2_MASK_WORDS: usize = 2048;

/// Build a bounded-window AC count program with a two-byte suffix prefilter.
///
/// `candidate_suffix2_mask` is a 65,536-bit mask keyed by
/// `(previous_byte << 8) | current_byte`. A lane replays the bounded DFA only
/// when the byte pair can end at least one match. Byte zero still uses the
/// single-byte candidate mask so one-byte patterns and offset-zero matches are
/// preserved.
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn classic_ac_bounded_count_suffix2_prefilter_program(
    haystack: &str,
    transitions: &str,
    output_offsets: &str,
    candidate_end_mask: &str,
    candidate_suffix2_mask: &str,
    haystack_len: &str,
    match_count: &str,
    state_count: u32,
    max_pattern_len: u32,
) -> Program {
    let i = Expr::var("i");
    let candidate_byte = load_packed_byte_expr(haystack, i.clone());
    let previous_byte = load_packed_byte_expr(haystack, Expr::sub(i.clone(), Expr::u32(1)));
    let suffix2_index = Expr::bitor(
        Expr::shl(Expr::var("previous_byte"), Expr::u32(8)),
        Expr::var("candidate_byte"),
    );
    let scan_nodes = count_scan_nodes(
        haystack,
        transitions,
        output_offsets,
        match_count,
        max_pattern_len,
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
                    vec![Node::if_then_else(
                        Expr::eq(i.clone(), Expr::u32(0)),
                        scan_nodes.clone(),
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
                                scan_nodes,
                            ),
                        ],
                    )],
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
            BufferDecl::storage(candidate_end_mask, 3, BufferAccess::ReadOnly, DataType::U32)
                .with_count(8),
            BufferDecl::storage(
                candidate_suffix2_mask,
                4,
                BufferAccess::ReadOnly,
                DataType::U32,
            )
            .with_count(CLASSIC_AC_SUFFIX2_MASK_WORDS as u32),
            BufferDecl::storage(haystack_len, 5, BufferAccess::ReadOnly, DataType::U32)
                .with_count(1),
            BufferDecl::read_write(match_count, 6, DataType::U32).with_count(1),
        ],
        [128, 1, 1],
        vec![wrap_anonymous(
            "vyre-libs::matching::classic_ac_bounded_count_suffix2_prefilter",
            body,
        )],
    )
}

/// Derive the two-byte suffix mask consumed by the suffix2 count prefilter.
#[must_use]
pub fn classic_ac_candidate_suffix2_mask_words(
    dfa: &CompiledDfa,
) -> [u32; CLASSIC_AC_SUFFIX2_MASK_WORDS] {
    let mut mask = [0_u32; CLASSIC_AC_SUFFIX2_MASK_WORDS];
    let states = valid_dfa_states(dfa);
    for state in 0..states {
        let row = state * 256;
        for previous in 0..256 {
            let mid = dfa.transitions[row + previous] as usize;
            if mid >= states {
                continue;
            }
            let mid_row = mid * 256;
            for byte in 0..256 {
                let next = dfa.transitions[mid_row + byte] as usize;
                if state_accepts(dfa, next) {
                    let suffix = (previous << 8) | byte;
                    mask[suffix / 32] |= 1_u32 << (suffix % 32);
                }
            }
        }
    }
    mask
}

fn valid_dfa_states(dfa: &CompiledDfa) -> usize {
    (dfa.state_count as usize)
        .min(dfa.output_offsets.len().saturating_sub(1))
        .min(dfa.transitions.len() / 256)
}

fn state_accepts(dfa: &CompiledDfa, state: usize) -> bool {
    state + 1 < dfa.output_offsets.len()
        && dfa.output_offsets[state] != dfa.output_offsets[state + 1]
}

/// Build the two-byte-suffix prefiltered AC count-only program for a compiled DFA.
#[must_use]
pub fn build_ac_bounded_count_suffix2_prefilter_program(dfa: &CompiledDfa) -> Program {
    classic_ac_bounded_count_suffix2_prefilter_program(
        "haystack",
        "transitions",
        "output_offsets",
        "candidate_end_mask",
        "candidate_suffix2_mask",
        "haystack_len",
        "match_count",
        dfa.state_count,
        dfa.max_pattern_len,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scan::classic_ac::{
        classic_ac_candidate_end_byte_mask_words, classic_ac_compile, classic_ac_scan_counts,
    };
    use crate::scan::{pack_haystack_u32, pack_u32_slice};

    fn suffix2_candidate(
        previous: u8,
        current: u8,
        mask: &[u32; CLASSIC_AC_SUFFIX2_MASK_WORDS],
    ) -> bool {
        let suffix = ((previous as usize) << 8) | current as usize;
        (mask[suffix / 32] & (1_u32 << (suffix % 32))) != 0
    }

    fn decode_u32(bytes: &[u8]) -> Vec<u32> {
        bytes
            .chunks_exact(4)
            .map(|chunk| u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
            .collect()
    }

    fn with_reference_dispatch_lanes(program: Program, lanes: u32) -> Program {
        let buffers = program
            .buffers()
            .iter()
            .cloned()
            .map(|buffer| {
                if buffer.name() == "match_count" {
                    buffer.with_count(lanes.max(1)).with_output_byte_range(0..4)
                } else {
                    buffer
                }
            })
            .collect();
        program.with_rewritten_buffers(buffers)
    }

    #[test]
    fn suffix2_mask_marks_only_pairs_that_can_finish_matches() {
        let ac = classic_ac_compile(&[b"ab", b"cab", b"tool"]);
        let mask = classic_ac_candidate_suffix2_mask_words(&ac.dfa);

        assert!(suffix2_candidate(b'a', b'b', &mask));
        assert!(suffix2_candidate(b'o', b'l', &mask));
        assert!(!suffix2_candidate(b'x', b'b', &mask));
        assert!(!suffix2_candidate(b't', b'o', &mask));
    }

    #[test]
    fn suffix2_prefilter_reference_eval_matches_cpu_count() {
        let patterns: [&[u8]; 4] = [b"ab", b"cab", b"token", b"BEGIN"];
        let haystack = b"zzzzab zzzzcab zzzBEGIN zztoken zzz";
        let ac = classic_ac_compile(&patterns);
        let expected = classic_ac_scan_counts(&ac, haystack).iter().sum::<u32>();
        let program = with_reference_dispatch_lanes(
            build_ac_bounded_count_suffix2_prefilter_program(&ac.dfa),
            haystack.len() as u32,
        );
        let inputs = vec![
            vyre_reference::value::Value::from(pack_haystack_u32(haystack)),
            vyre_reference::value::Value::from(pack_u32_slice(&ac.dfa.transitions)),
            vyre_reference::value::Value::from(pack_u32_slice(&ac.dfa.output_offsets)),
            vyre_reference::value::Value::from(pack_u32_slice(
                &classic_ac_candidate_end_byte_mask_words(&ac.dfa),
            )),
            vyre_reference::value::Value::from(pack_u32_slice(
                &classic_ac_candidate_suffix2_mask_words(&ac.dfa),
            )),
            vyre_reference::value::Value::from(pack_u32_slice(&[haystack.len() as u32])),
            vyre_reference::value::Value::from(vec![0_u8; haystack.len() * 4]),
        ];
        let outputs = vyre_reference::reference_eval(&program, &inputs).expect(
            "Fix: suffix2 prefiltered AC bounded count program should evaluate in reference backend.",
        );

        assert_eq!(decode_u32(&outputs[0].to_bytes()), vec![expected]);
    }

    #[test]
    fn suffix2_prefilter_program_has_compact_stable_shape() {
        let ac = classic_ac_compile(&[b"Authorization: Bearer ", b"token", b"tok"]);
        let program = build_ac_bounded_count_suffix2_prefilter_program(&ac.dfa);

        assert_eq!(program.workgroup_size(), [128, 1, 1]);
        assert_eq!(program.buffers().len(), 7);
        assert_eq!(program.buffers()[4].name(), "candidate_suffix2_mask");
        assert_eq!(
            program.buffers()[4].count,
            CLASSIC_AC_SUFFIX2_MASK_WORDS as u32
        );
        assert_eq!(program.buffers()[6].name(), "match_count");
        assert_eq!(program.buffers()[6].count, 1);
    }
}
