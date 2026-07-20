//! Subgroup-cooperative DFA scan for multi-string workloads.
//!
//! This is the Innovation I.9 sibling of [`super::aho_corasick`]: lanes in one
//! subgroup forward DFA state with [`Expr::SubgroupShuffle`] instead of
//! replaying the whole prefix independently.

use vyre::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

use crate::region::wrap_anonymous;

const OP_ID: &str = "vyre-libs::matching::cooperative_dfa";
const ALPHABET_SIZE: u32 = 256;

// Forwarding alias to the canonical packer in `scan::dispatch_io`.
// Was a private inline copy with identical body - removed so the
// LE-byte packing format has a single source of truth.
use crate::scan::dispatch_io::pack_u32_slice as pack_u32;

fn correction_lane(local_lane: Expr, offset: u32) -> Expr {
    Expr::select(
        Expr::lt(local_lane.clone(), Expr::u32(offset)),
        Expr::u32(0),
        Expr::sub(local_lane, Expr::u32(offset)),
    )
}

fn transition_expr(transitions: &str, state: Expr, byte: Expr) -> Expr {
    Expr::load(
        transitions,
        Expr::add(Expr::mul(state, Expr::u32(ALPHABET_SIZE)), byte),
    )
}

fn fixture_case() -> (Vec<u32>, super::CompiledDfa, Vec<u32>) {
    let compiled = super::dfa_compile(&[b"a"]);
    let input = b"banana"
        .iter()
        .map(|&byte| u32::from(byte))
        .collect::<Vec<_>>();
    let expected = vec![0, 1, 0, 1, 0, 1];
    (input, compiled, expected)
}

/// Build the cooperative DFA scan body as a `Vec<Node>` so it can be
/// inlined into fused decode→scan programs.
///
/// `store_value` is the [`Expr`] written to `matches[idx]`; callers that
/// want `aho_corasick` semantics (store `accept[state]` directly) pass
/// `Expr::var("accepting")`, while callers that want a boolean mask pass
/// `Expr::select(Expr::ne(Expr::var("accepting"), Expr::u32(0)), Expr::u32(1), Expr::u32(0))`.
#[must_use]
pub fn cooperative_dfa_scan_body_with_store(
    input: &str,
    transitions: &str,
    accept_mask: &str,
    matches: &str,
    subgroup_size: u32,
    store_value: Expr,
) -> Vec<Node> {
    let idx = Expr::InvocationId { axis: 0 };
    let local_lane = Expr::LocalId { axis: 0 };
    let effective_subgroup = subgroup_size.max(1);
    let round_count = effective_subgroup.ilog2();

    let mut lane_body = vec![
        Node::let_bind(
            "safe_idx",
            Expr::select(Expr::var("in_bounds"), idx.clone(), Expr::u32(0)),
        ),
        Node::let_bind("byte", Expr::load(input, Expr::var("safe_idx"))),
        Node::assign(
            "state",
            transition_expr(transitions, Expr::var("state"), Expr::var("byte")),
        ),
    ];

    for round in 0..round_count {
        let offset = 1u32 << round;
        let shuffled_name = format!("forwarded_state_{round}");
        lane_body.push(Node::let_bind(
            shuffled_name.as_str(),
            Expr::SubgroupShuffle {
                value: Box::new(Expr::var("state")),
                lane: Box::new(correction_lane(local_lane.clone(), offset)),
            },
        ));
        lane_body.push(Node::assign(
            "state",
            transition_expr(
                transitions,
                Expr::var(shuffled_name.as_str()),
                Expr::var("byte"),
            ),
        ));
    }

    lane_body.push(Node::let_bind(
        "accepting",
        Expr::load(accept_mask, Expr::var("state")),
    ));
    lane_body.push(Node::if_then(
        Expr::var("in_bounds"),
        vec![Node::Store {
            buffer: matches.into(),
            index: idx.clone(),
            value: store_value,
        }],
    ));

    let mut body = vec![
        Node::let_bind("idx", idx),
        Node::let_bind(
            "in_bounds",
            Expr::lt(Expr::var("idx"), Expr::buf_len(input)),
        ),
        Node::let_bind("state", Expr::u32(0)),
    ];
    body.extend(lane_body);
    body
}

/// Build a Program that scans `input` for accepting DFA states using
/// shuffle-based correction rounds.
///
/// Buffers:
/// - `input`: ReadOnly, `u32` per byte.
/// - `transitions`: ReadOnly, `u32`  -  `state * 256 + byte -> next`.
/// - `accept_mask`: ReadOnly, `u32`  -  non-zero where the DFA accepts.
/// - `matches`: output `u32`  -  `1` where a match ends, else `0`.
#[must_use]
pub fn cooperative_dfa_scan(
    input: &str,
    transitions: &str,
    accept_mask: &str,
    matches: &str,
    input_len: u32,
    state_count: u32,
    subgroup_size: u32,
) -> Program {
    let effective_subgroup = subgroup_size.max(1);
    let body = cooperative_dfa_scan_body_with_store(
        input,
        transitions,
        accept_mask,
        matches,
        subgroup_size,
        Expr::select(
            Expr::ne(Expr::var("accepting"), Expr::u32(0)),
            Expr::u32(1),
            Expr::u32(0),
        ),
    );
    Program::wrapped(
        vec![
            BufferDecl::storage(input, 0, BufferAccess::ReadOnly, DataType::U32)
                .with_count(input_len),
            BufferDecl::storage(transitions, 1, BufferAccess::ReadOnly, DataType::U32)
                .with_count(state_count.saturating_mul(ALPHABET_SIZE)),
            BufferDecl::storage(accept_mask, 2, BufferAccess::ReadOnly, DataType::U32)
                .with_count(state_count),
            BufferDecl::output(matches, 3, DataType::U32).with_count(input_len),
        ],
        [effective_subgroup, 1, 1],
        vec![wrap_anonymous(OP_ID, body)],
    )
}

/// Canonical sequential CPU witness for the cooperative DFA match buffer.
#[must_use]
#[cfg(test)]
pub(crate) fn cpu_ref_cooperative_dfa(
    input: &[u32],
    transitions: &[u32],
    accept_mask: &[u32],
    state_count: u32,
    alphabet_size: u32,
) -> Vec<u32> {
    let row_width = alphabet_size as usize;
    let Some(expected_transitions) = (state_count as usize).checked_mul(row_width) else {
        panic!(
            "cpu_ref_cooperative_dfa: malformed table (state_count={state_count}, \
             alphabet_size={alphabet_size}, transitions.len()={}, accept_mask.len()={}); \
             fix the caller before running parity",
            transitions.len(),
            accept_mask.len()
        );
    };
    if row_width == 0
        || transitions.len() != expected_transitions
        || accept_mask.len() < state_count as usize
    {
        panic!(
            "cpu_ref_cooperative_dfa: malformed table (state_count={state_count}, \
             alphabet_size={alphabet_size}, transitions.len()={}, accept_mask.len()={}); \
             fix the caller before running parity",
            transitions.len(),
            accept_mask.len()
        );
    }

    let mut state = 0u32;
    let mut matches = Vec::with_capacity(input.len());
    for &symbol in input {
        if symbol >= alphabet_size || state >= state_count {
            state = 0;
            matches.push(0);
            continue;
        }
        let offset = (state as usize) * row_width + symbol as usize;
        state = transitions[offset];
        matches.push(u32::from(
            (state as usize) < accept_mask.len() && accept_mask[state as usize] != 0,
        ));
    }
    matches
}

fn fixture_inputs() -> Vec<Vec<Vec<u8>>> {
    let (input, compiled, _) = fixture_case();
    vec![vec![
        pack_u32(&input),
        pack_u32(&compiled.transitions),
        pack_u32(&compiled.accept),
    ]]
}

fn fixture_expected_output() -> Vec<Vec<Vec<u8>>> {
    let (_, _, expected) = fixture_case();
    vec![vec![pack_u32(&expected)]]
}

inventory::submit! {
    crate::harness::OpEntry::new(
        OP_ID,
        || {
            let (_, compiled, _) = fixture_case();
            cooperative_dfa_scan(
                "input",
                "transitions",
                "accept_mask",
                "matches",
                6,
                compiled.state_count,
                4,
            )
        },
        Some(fixture_inputs),
        Some(fixture_expected_output),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn compile_patterns(patterns: &[&[u8]]) -> (Vec<u32>, Vec<u32>, u32) {
        let compiled = super::super::dfa_compile(patterns);
        (compiled.transitions, compiled.accept, compiled.state_count)
    }

    fn encode(bytes: &[u8]) -> Vec<u32> {
        bytes.iter().map(|&byte| u32::from(byte)).collect()
    }

    #[test]
    fn cooperative_dfa_single_pattern_abc() {
        let input = encode(b"zabc");
        let (transitions, accept_mask, state_count) = compile_patterns(&[b"abc"]);
        assert_eq!(
            cpu_ref_cooperative_dfa(
                &input,
                &transitions,
                &accept_mask,
                state_count,
                ALPHABET_SIZE
            ),
            vec![0, 0, 0, 1],
        );
    }

    #[test]
    fn cooperative_dfa_overlapping_multi_pattern() {
        let input = encode(b"xabcd");
        let (transitions, accept_mask, state_count) = compile_patterns(&[b"abc", b"bcd"]);
        assert_eq!(
            cpu_ref_cooperative_dfa(
                &input,
                &transitions,
                &accept_mask,
                state_count,
                ALPHABET_SIZE
            ),
            vec![0, 0, 0, 1, 1],
        );
    }

    #[test]
    fn cooperative_dfa_empty_input() {
        let (transitions, accept_mask, state_count) = compile_patterns(&[b"abc"]);
        assert!(cpu_ref_cooperative_dfa(
            &[],
            &transitions,
            &accept_mask,
            state_count,
            ALPHABET_SIZE
        )
        .is_empty());
    }

    /// VL-001: cpu_ref_cooperative_dfa must panic (not return silent zeros) on a
    /// malformed table, so the parity oracle fails loudly rather than masking GPU
    /// divergences. This test verifies both malformed-length and zero-alphabet_size
    /// paths reach the panic arm — the oracle must never produce a plausible-looking
    /// all-zeros vector that silently accepts a GPU bug.
    #[test]
    #[should_panic(
        expected = "cpu_ref_cooperative_dfa: malformed table"
    )]
    fn cpu_ref_cooperative_dfa_panics_on_wrong_transition_length() {
        // Correct table has state_count * alphabet_size entries; trim by 1 to missize.
        let (transitions, accept_mask, state_count) = compile_patterns(&[b"abc"]);
        let short = &transitions[..transitions.len() - 1];
        // Before fix: returns vec![0; input.len()] silently, masking GPU divergences.
        // After fix: panics with an actionable message.
        let _ = cpu_ref_cooperative_dfa(&[b'a' as u32], short, &accept_mask, state_count, ALPHABET_SIZE);
    }

    /// VL-001: zero alphabet_size must also panic rather than silently return zeros,
    /// because row_width==0 is structurally invalid (no byte transitions can exist).
    #[test]
    #[should_panic(
        expected = "cpu_ref_cooperative_dfa: malformed table"
    )]
    fn cpu_ref_cooperative_dfa_panics_on_zero_alphabet_size() {
        let (transitions, accept_mask, state_count) = compile_patterns(&[b"abc"]);
        // alphabet_size=0 means row_width=0 which hits the row_width==0 branch.
        let _ = cpu_ref_cooperative_dfa(&[b'a' as u32], &transitions, &accept_mask, state_count, 0);
    }
}
