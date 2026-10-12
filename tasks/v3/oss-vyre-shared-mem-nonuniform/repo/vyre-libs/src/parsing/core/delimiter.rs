//! Bracket matching  -  Tier 3 wrapper over the
//! Tier 2.5 [`vyre_primitives::matching::bracket_match::bracket_match`] primitive.
//!
//! Migrated per `docs/primitives-tier.md` Step 2 +
//! `docs/lego-block-rule.md`. The IR-builder + CPU reference live in
//! `vyre-primitives-matching`.
//! Parser dialects (`parse-c`, `parse-rust`, `parse-go`,
//! `parse-python` for f-strings) consume the exact same scanner.

use vyre::ir::Program;

use crate::region::tag_program;
use vyre_primitives::matching::bracket_match::bracket_match as primitive_bracket_match;
#[cfg(any(test, feature = "cpu-parity"))]
pub use vyre_primitives::matching::bracket_match::cpu_ref;
pub use vyre_primitives::matching::bracket_match::{
    pack_u32, CLOSE_BRACE, MATCH_NONE, OPEN_BRACE, OTHER,
};
use vyre_primitives::parsing::core_delimiter_match as primitive_core_delimiter;

/// Tier-3 parser-facing bracket-match op id.
pub const OP_ID: &str = "vyre-libs::parsing::bracket_match";
/// Tier-3 parser-facing generic delimiter-depth op id.
pub const CORE_DELIMITER_OP_ID: &str = "vyre-libs::parsing::core_delimiter_match";

/// Parser-facing bracket matcher composed from the Tier 2.5 primitive.
#[must_use]
pub fn bracket_match(
    kinds: &str,
    stack: &str,
    match_pairs: &str,
    n: u32,
    max_depth: u32,
) -> Program {
    tag_program(
        OP_ID,
        primitive_bracket_match(kinds, stack, match_pairs, n, max_depth),
    )
}

/// Parser-facing delimiter-depth scanner composed from the Tier 2.5 primitive.
#[must_use]
pub fn core_delimiter_match(
    tok_types: &str,
    tok_depths: &str,
    tok_count: u32,
    open_tok_id: u32,
    close_tok_id: u32,
) -> Program {
    tag_program(
        CORE_DELIMITER_OP_ID,
        primitive_core_delimiter::core_delimiter_match(
            tok_types,
            tok_depths,
            tok_count,
            open_tok_id,
            close_tok_id,
        ),
    )
}

fn fixture_inputs() -> Vec<Vec<Vec<u8>>> {
    vec![vec![
        pack_u32(&[OPEN_BRACE, OPEN_BRACE, CLOSE_BRACE, CLOSE_BRACE]),
        vec![0u8; 4 * 4],
    ]]
}

fn fixture_outputs() -> Vec<Vec<Vec<u8>>> {
    // max_depth == n selects the parallel matcher, which writes match_pairs
    // directly and leaves caller-supplied stack scratch untouched.
    vec![vec![pack_u32(&[0, 0, 0, 0]), pack_u32(&[3, 2, 1, 0])]]
}

inventory::submit! {
    crate::harness::OpEntry {
        id: OP_ID,
        build: || bracket_match("kinds", "stack", "match_pairs", 4, 4),
        test_inputs: Some(fixture_inputs),
        expected_output: Some(fixture_outputs),
        category: Some("parsing"),
    }
}

inventory::submit! {
    crate::harness::OpEntry {
        id: CORE_DELIMITER_OP_ID,
        build: || {
            core_delimiter_match("tok_types", "tok_depths", 8, 12, 13)
        },
        test_inputs: Some(|| {
            let tokens: [u32; 8] = [12, 12, 0, 0, 0, 13, 13, 0];
            let bytes = vyre_primitives::wire::pack_u32_slice(&tokens);
            vec![vec![bytes, vec![0u8; 4 * 8]]]
        }),
        expected_output: Some(|| {
            let depths: [u32; 8] = [1, 2, 2, 2, 2, 1, 0, 0];
            let bytes = vyre_primitives::wire::pack_u32_slice(&depths);
            vec![vec![bytes]]
        }),
        category: Some("parsing"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vyre_reference::value::Value;

    fn run(kinds: &[u32], max_depth: u32) -> Vec<u32> {
        let n = kinds.len().max(1) as u32;
        let program = bracket_match("kinds", "stack", "match_pairs", n, max_depth);
        let inputs = vec![
            Value::Bytes(pack_u32(kinds).into()),
            Value::Bytes(vec![0u8; (max_depth as usize) * 4].into()),
            Value::Bytes(pack_u32(&vec![MATCH_NONE; n as usize]).into()),
        ];
        let outputs = vyre_reference::reference_eval(&program, &inputs)
            .expect("Fix: bracket_match must run; restore this invariant before continuing.");
        vyre_primitives::wire::decode_u32_le_bytes_all(&outputs[1].to_bytes())
    }

    #[test]
    fn balanced_single_pair() {
        assert_eq!(
            run(&[OPEN_BRACE, OTHER, CLOSE_BRACE], 3),
            vec![2, MATCH_NONE, 0]
        );
    }

    #[test]
    fn nested_pairs() {
        assert_eq!(
            run(&[OPEN_BRACE, OPEN_BRACE, CLOSE_BRACE, CLOSE_BRACE], 4),
            vec![3, 2, 1, 0]
        );
    }

    #[test]
    fn unbalanced_extra_open() {
        assert_eq!(
            run(&[OPEN_BRACE, OPEN_BRACE, CLOSE_BRACE], 3),
            vec![MATCH_NONE, 2, 1]
        );
    }

    #[test]
    fn unbalanced_extra_close() {
        assert_eq!(
            run(&[CLOSE_BRACE, OPEN_BRACE, CLOSE_BRACE], 3),
            vec![MATCH_NONE, 2, 1]
        );
    }

    #[test]
    fn depth_cap_truncates_extra_opens() {
        assert_eq!(
            run(
                &[
                    OPEN_BRACE,
                    OPEN_BRACE,
                    OPEN_BRACE,
                    CLOSE_BRACE,
                    CLOSE_BRACE,
                    CLOSE_BRACE
                ],
                2,
            ),
            vec![4, 3, MATCH_NONE, 1, 0, MATCH_NONE],
        );
    }
}
