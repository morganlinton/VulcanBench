//! Aho-Corasick multi-pattern scanner (companion to [`dfa_compile`]).
//!
//! Consumes a transition table built by [`super::dfa_compile`] and
//! scans `haystack` for any of the compiled patterns. Emits `1` at
//! `matches[i]` whenever the automaton accepts at position `i`.
//!
//! Layout assumptions (see `dfa_compile::CompiledDfa`):
//!
//! ```text
//! transitions[state * 256 + byte] = next_state
//! accept[state]                    = 0 unless state accepts
//! ```
//!
//! One invocation per haystack byte; each invocation walks only the
//! suffix window that can still affect matches ending at that byte.
//! Callers that know the longest pattern length should use
//! [`aho_corasick_bounded`] directly.

use vyre::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

use crate::region::wrap_anonymous;

/// Build a Program that scans `haystack` (u32 per byte) for any
/// accepting state of a pre-built DFA. Buffers:
///
/// - `haystack`: ReadOnly, `u32` per byte.
/// - `transitions`: ReadOnly, `u32`  -  `state * 256 + byte → next`.
/// - `accept`: ReadOnly, `u32`  -  accept table indexed by state.
/// - `matches`: ReadWrite, `u32`  -  one slot per haystack byte, set
///   to `accept[state]` (pattern_id + 1) when the automaton accepts
///   at that offset.
#[must_use]
pub fn aho_corasick(
    haystack: &str,
    transitions: &str,
    accept: &str,
    matches: &str,
    haystack_len: u32,
    state_count: u32,
) -> Program {
    aho_corasick_bounded(
        haystack,
        transitions,
        accept,
        matches,
        haystack_len,
        state_count,
        haystack_len.max(1),
    )
}

/// Build a Program that scans `haystack` using a bounded suffix window.
///
/// `max_pattern_len` is the longest pattern encoded in the transition table.
/// A match ending at byte `i` can depend only on bytes
/// `i + 1 - max_pattern_len ..= i`, so this avoids replaying the whole prefix
/// for every output position while preserving exact AC semantics.
#[must_use]
pub fn aho_corasick_bounded(
    haystack: &str,
    transitions: &str,
    accept: &str,
    matches: &str,
    haystack_len: u32,
    state_count: u32,
    max_pattern_len: u32,
) -> Program {
    let max_pattern_len = max_pattern_len.max(1);
    let i = Expr::var("i");
    let end = Expr::add(i.clone(), Expr::u32(1));
    let start = Expr::select(
        Expr::lt(i.clone(), Expr::u32(max_pattern_len - 1)),
        Expr::u32(0),
        Expr::sub(end.clone(), Expr::u32(max_pattern_len)),
    );
    let body = vec![
        Node::let_bind("i", Expr::InvocationId { axis: 0 }),
        Node::if_then(
            Expr::lt(i.clone(), Expr::buf_len(haystack)),
            vec![
                Node::let_bind("state", Expr::u32(0)),
                Node::let_bind("scan_start", start),
                Node::loop_for(
                    "step",
                    Expr::var("scan_start"),
                    end,
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
                Node::Store {
                    buffer: matches.into(),
                    index: i,
                    value: Expr::load(accept, Expr::var("state")),
                },
            ],
        ),
    ];

    Program::wrapped(
        vec![
            BufferDecl::storage(haystack, 0, BufferAccess::ReadOnly, DataType::U32)
                .with_count(haystack_len),
            BufferDecl::storage(transitions, 1, BufferAccess::ReadOnly, DataType::U32)
                .with_count(state_count.saturating_mul(256)),
            BufferDecl::storage(accept, 2, BufferAccess::ReadOnly, DataType::U32)
                .with_count(state_count),
            BufferDecl::output(matches, 3, DataType::U32).with_count(haystack_len),
        ],
        [64, 1, 1],
        vec![wrap_anonymous("vyre-libs::matching::aho_corasick", body)],
    )
}

inventory::submit! {
    crate::harness::OpEntry {
        id: "vyre-libs::matching::aho_corasick",
        build: || {
            let patterns: [&[u8]; 1] = [b"abra"];
            let compiled = crate::scan::dfa::dfa_compile(&patterns);
            aho_corasick_bounded("haystack", "transitions", "accept", "matches", 11, compiled.accept.len() as u32, compiled.max_pattern_len)
        },
        test_inputs: Some(|| {
            let patterns: [&[u8]; 1] = [b"abra"];
            let compiled = crate::scan::dfa::dfa_compile(&patterns);
            let haystack = b"abracadabra";

            vec![vec![
                crate::test_support::byte_pack::u32_bytes(&haystack.iter().map(|&b| u32::from(b)).collect::<Vec<_>>()),
                crate::test_support::byte_pack::u32_bytes(&compiled.transitions),
                crate::test_support::byte_pack::u32_bytes(&compiled.accept),
            ]]
        }),
        expected_output: Some(|| vec![
            vec![
                vec![0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00,
                     0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                     0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, ],
            ],
        ]),
        category: Some("scan"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vyre::ir::Node;

    fn first_loop_bounds(nodes: &[Node]) -> Option<(&Expr, &Expr)> {
        for node in nodes {
            match node {
                Node::Loop { from, to, .. } => return Some((from, to)),
                Node::If {
                    then, otherwise, ..
                } => {
                    if let Some(bounds) = first_loop_bounds(then) {
                        return Some(bounds);
                    }
                    if let Some(bounds) = first_loop_bounds(otherwise) {
                        return Some(bounds);
                    }
                }
                Node::Block(body) => {
                    if let Some(bounds) = first_loop_bounds(body) {
                        return Some(bounds);
                    }
                }
                Node::Region { body, .. } => {
                    if let Some(bounds) = first_loop_bounds(body) {
                        return Some(bounds);
                    }
                }
                _ => {}
            }
        }
        None
    }

    #[test]
    fn bounded_aho_corasick_scans_suffix_window_instead_of_whole_prefix() {
        let program =
            aho_corasick_bounded("haystack", "transitions", "accept", "matches", 64, 8, 4);
        let (from, to) = first_loop_bounds(program.entry())
            .expect("Fix: bounded Aho-Corasick must emit one DFA-walk loop");
        assert!(
            matches!(from, Expr::Var(name) if name.as_ref() == "scan_start"),
            "bounded Aho-Corasick must start from the max-pattern suffix window"
        );
        assert!(
            matches!(to, Expr::BinOp { .. }),
            "bounded Aho-Corasick must end at i + 1"
        );
    }
}
