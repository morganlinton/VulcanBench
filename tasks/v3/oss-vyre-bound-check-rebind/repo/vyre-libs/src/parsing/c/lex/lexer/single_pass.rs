//! Single-pass C lexer composition.
//!
//! Composes `c11_lexer` (raw-byte -> token stream) and
//! `c11_lex_digraphs` (digraph + line-splice rewrite over the token stream)
//! into one Region whose body is the concatenation of both Programs' entry
//! trees. The optimizer and megakernel planner can see the producer/consumer
//! chain directly instead of receiving two opaque top-level Programs.
//!
//! The composition is straight-line at the IR level: the token
//! buffers (`tok_types`, `tok_starts`, `tok_lens`) are produced
//! by the lex pass and consumed by the digraph pass through the
//! same buffer-decl table, so the megakernel scheduler sees a
//! producer/consumer chain it can fuse without alias proof.
//!
//! Soundness: the composed Program has the same observable buffer contract as
//! the two-Program sequence and preserves the component bodies in order.

use rustc_hash::FxHashSet;
use std::sync::Arc;
use vyre::ir::{BufferDecl, Node, Program};
use vyre_foundation::ir::model::expr::GeneratorRef;

use super::core::{c11_lexer, c11_lexer_regular};
use super::digraphs::c11_lex_digraphs;
use crate::region::wrap;

const OP_ID: &str = "vyre-libs::parsing::c11_lex_single_pass";
const REGULAR_OP_ID: &str = "vyre-libs::parsing::c11_lex_regular_single_pass";

/// Build a Program that runs the C11 lexer + digraph passes back-
/// to-back inside a single Region. Buffer table is the union of
/// both passes' buffers; the body is the concatenation of both
/// pass entry trees.
///
/// The fused Region's `generator` ident is
/// `vyre-libs::parsing::c11_lex_single_pass`, giving downstream passes one
/// auditable op boundary for the lex+digraph composition.
#[must_use]
pub fn c11_lex_single_pass(
    haystack: &str,
    out_tok_types: &str,
    out_tok_starts: &str,
    out_tok_lens: &str,
    out_counts: &str,
    haystack_len: u32,
    digraph_capacity: u32,
) -> Program {
    let lex_program = c11_lexer(
        haystack,
        out_tok_types,
        out_tok_starts,
        out_tok_lens,
        out_counts,
        haystack_len,
    );
    let digraph_program = c11_lex_digraphs(
        out_tok_types,
        out_tok_starts,
        out_tok_lens,
        digraph_capacity,
    );

    let workgroup_size = lex_program.workgroup_size();
    let mut buffers: Vec<BufferDecl> = lex_program.buffers().to_vec();
    let mut seen_names: FxHashSet<Arc<str>> = buffers
        .iter()
        .map(|buffer| Arc::clone(&buffer.name))
        .collect();
    for buffer in digraph_program.buffers() {
        if seen_names.insert(Arc::clone(&buffer.name)) {
            buffers.push(buffer.clone());
        }
    }

    // Each sub-program emits Let bindings at its own outer scope. If
    // we flatten both into one Vec, overlapping names trigger V032
    // duplicate-sibling-let. Wrap each in its own Block so the
    // bindings live in disjoint scopes and the combined body sees
    // only the two opaque Block nodes.
    let combined_body = vec![
        Node::Block(lex_program.into_entry_vec()),
        Node::Block(digraph_program.into_entry_vec()),
    ];

    Program::wrapped(
        buffers,
        workgroup_size,
        vec![wrap(
            OP_ID,
            combined_body,
            Some(GeneratorRef {
                name: OP_ID.to_string(),
            }),
        )],
    )
}

#[must_use]
/// Build the regular-C fast lexer plus digraph rewrite as one IR region.
///
/// This variant is only for callers that have already proven the source does
/// not require generic C lexer features such as comments, preprocessor
/// directives, string/char literal scanning, or complex numeric literals.
/// It preserves the same output buffer contract as [`c11_lex_single_pass`].
pub fn c11_lex_regular_single_pass(
    haystack: &str,
    out_tok_types: &str,
    out_tok_starts: &str,
    out_tok_lens: &str,
    out_counts: &str,
    haystack_len: u32,
    digraph_capacity: u32,
) -> Program {
    let lex_program = c11_lexer_regular(
        haystack,
        out_tok_types,
        out_tok_starts,
        out_tok_lens,
        out_counts,
        haystack_len,
    );
    let digraph_program = c11_lex_digraphs(
        out_tok_types,
        out_tok_starts,
        out_tok_lens,
        digraph_capacity,
    );

    let workgroup_size = lex_program.workgroup_size();
    let mut buffers: Vec<BufferDecl> = lex_program.buffers().to_vec();
    let mut seen_names: FxHashSet<Arc<str>> = buffers
        .iter()
        .map(|buffer| Arc::clone(&buffer.name))
        .collect();
    for buffer in digraph_program.buffers() {
        if seen_names.insert(Arc::clone(&buffer.name)) {
            buffers.push(buffer.clone());
        }
    }

    // Each sub-program emits Let bindings at its own outer scope. If
    // we flatten both into one Vec, overlapping names trigger V032
    // duplicate-sibling-let. Wrap each in its own Block so the
    // bindings live in disjoint scopes and the combined body sees
    // only the two opaque Block nodes.
    let combined_body = vec![
        Node::Block(lex_program.into_entry_vec()),
        Node::Block(digraph_program.into_entry_vec()),
    ];

    Program::wrapped(
        buffers,
        workgroup_size,
        vec![wrap(
            REGULAR_OP_ID,
            combined_body,
            Some(GeneratorRef {
                name: REGULAR_OP_ID.to_string(),
            }),
        )],
    )
}

inventory::submit! {
    crate::harness::OpEntry {
        id: OP_ID,
        build: || c11_lex_single_pass(
            "haystack",
            "tok_types",
            "tok_starts",
            "tok_lens",
            "tok_counts",
            64,
            64,
        ),
        test_inputs: Some(single_pass_inputs),
        expected_output: Some(single_pass_expected),
        category: Some("parsing"),
    }
}

fn single_pass_inputs() -> Vec<Vec<Vec<u8>>> {
    vec![vec![
        vec![b'a'; 64 * 4],
        vec![0u8; 64 * 4],
        vec![0u8; 64 * 4],
        vec![0u8; 64 * 4],
        vec![0u8; 4],
    ]]
}

fn single_pass_expected() -> Vec<Vec<Vec<u8>>> {
    let mut out_tok_types = vec![0u8; 64 * 4];
    out_tok_types[0..4]
        .copy_from_slice(&crate::parsing::c::lex::tokens::TOK_IDENTIFIER.to_le_bytes());

    let out_tok_starts = vec![0u8; 64 * 4];

    let mut out_tok_lens = vec![0u8; 64 * 4];
    out_tok_lens[0..4].copy_from_slice(&64u32.to_le_bytes());

    let mut out_counts = vec![0u8; 4];
    out_counts.copy_from_slice(&1u32.to_le_bytes());

    vec![vec![
        out_tok_types,
        out_tok_starts,
        out_tok_lens,
        out_counts,
    ]]
}

#[cfg(test)]
mod tests {
    use super::*;
    use vyre::ir::Node;

    /// `c11_lex_single_pass` produces the same Program shape as
    /// running `c11_lexer` followed by `c11_lex_digraphs`. The
    /// fused Program's entry tree is exactly the concatenation,
    /// and its buffer table is the union (with single-name
    /// dedup).
    #[test]
    fn c11_lex_single_pass_matches_two_program_sequence() {
        let fused = c11_lex_single_pass(
            "haystack",
            "tok_types",
            "tok_starts",
            "tok_lens",
            "tok_counts",
            64,
            64,
        );
        let lex = c11_lexer(
            "haystack",
            "tok_types",
            "tok_starts",
            "tok_lens",
            "tok_counts",
            64,
        );
        let digraphs = c11_lex_digraphs("tok_types", "tok_starts", "tok_lens", 64);

        // Entry tree structure check: fused entry has exactly one
        // Region wrapping the concatenation of lex + digraph
        // bodies.
        assert_eq!(
            fused.entry().len(),
            1,
            "fused must have exactly one wrapping Region at the entry top"
        );
        let Node::Region {
            generator, body, ..
        } = &fused.entry()[0]
        else {
            panic!("fused entry's top-level must be a Region");
        };
        assert_eq!(generator.as_str(), OP_ID);
        let body_len: usize = lex.entry().len().saturating_add(digraphs.entry().len());
        assert_eq!(body.as_ref().len(), body_len);

        // Buffer table check: every lex buffer appears (in lex
        // order) and every unique digraph buffer is appended.
        let fused_names: Vec<String> = fused
            .buffers()
            .iter()
            .map(|b| b.name.as_ref().to_string())
            .collect();
        for buf in lex.buffers() {
            assert!(
                fused_names.contains(&buf.name.as_ref().to_string()),
                "fused buffer table must include lex buffer `{}`",
                buf.name
            );
        }
        for buf in digraphs.buffers() {
            assert!(
                fused_names.contains(&buf.name.as_ref().to_string()),
                "fused buffer table must include digraph buffer `{}`",
                buf.name
            );
        }
    }

    /// The fused Region's `generator` is the L1 op id, NOT the
    /// individual lex / digraph generators  -  so
    /// `region_fusion_hint` correctly treats it as a single fused
    /// arm without trying to fuse it again.
    #[test]
    fn fused_region_uses_l1_op_id() {
        let fused = c11_lex_single_pass(
            "haystack",
            "tok_types",
            "tok_starts",
            "tok_lens",
            "tok_counts",
            64,
            64,
        );
        let Node::Region { generator, .. } = &fused.entry()[0] else {
            panic!("entry must be Region");
        };
        assert_eq!(
            generator.as_str(),
            "vyre-libs::parsing::c11_lex_single_pass"
        );
    }

    /// Buffer dedup: `tok_types` etc. appear in both passes; the
    /// fused table must include each name exactly once.
    #[test]
    fn fused_buffer_table_has_no_duplicates() {
        let fused = c11_lex_single_pass(
            "haystack",
            "tok_types",
            "tok_starts",
            "tok_lens",
            "tok_counts",
            64,
            64,
        );
        let mut seen = std::collections::HashSet::new();
        for buf in fused.buffers() {
            let name = buf.name.as_ref().to_string();
            assert!(
                seen.insert(name.clone()),
                "duplicate buffer `{name}` in fused table"
            );
        }
    }
}
