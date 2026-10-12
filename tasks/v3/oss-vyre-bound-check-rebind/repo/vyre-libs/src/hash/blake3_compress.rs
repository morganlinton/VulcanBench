//! BLAKE3 compression function as a Cat-A composition.
//!
//! Computes one invocation of the BLAKE3 permutation: takes an
//! 8-word chaining value, 16-word message block, counter, and flags,
//! returning an 8-word chaining output. Tree-hashing and chunking
//! compose on top of this primitive.
//!
//! Buffers:
//! - `chaining_in`: ReadOnly, 8 u32. Previous chaining value (or IV).
//! - `message`: ReadOnly, 16 u32. Message block (padded as needed).
//! - `params`: ReadOnly, 4 u32  -  `[counter_lo, counter_hi, block_len, flags]`.
//! - `chaining_out`: ReadWrite, 8 u32. Output chaining value.
//!
//! Single-invocation dispatch: one workgroup of [1, 1, 1] runs the
//! 7-round permutation in-place on 16 local state words. Parallel
//! tree hashing composes this compression primitive over chunk states.
//!
//! Migration 3 moved this op from `vyre-libs::crypto::blake3_compress`
//! to `vyre-libs::hash::blake3_compress`.

use vyre::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};
use vyre_foundation::ir::model::expr::GeneratorRef;
use vyre_primitives::hash::blake3::{blake3_round, BLAKE3_ROUND_OP_ID, MSG_SCHEDULE};

use crate::buffer_names::scoped_generic_name;
use crate::region::{wrap_anonymous, wrap_child};

const OP_ID: &str = "vyre-libs::hash::blake3_compress";
const FAMILY_PREFIX: &str = "hash_blake3_compress";
const BLAKE3_COMPRESS_BODY_NODE_COUNT: usize = 8 + 4 + 4 + 16 + MSG_SCHEDULE.len() + 8;

fn scoped_chaining_in(name: &str) -> String {
    scoped_generic_name(
        FAMILY_PREFIX,
        "chaining_in",
        name,
        &["cv_in", "chaining_in", "input"],
    )
}

fn scoped_message(name: &str) -> String {
    scoped_generic_name(FAMILY_PREFIX, "message", name, &["msg", "message"])
}

fn scoped_params(name: &str) -> String {
    scoped_generic_name(FAMILY_PREFIX, "params", name, &["params"])
}

fn scoped_chaining_out(name: &str) -> String {
    scoped_generic_name(
        FAMILY_PREFIX,
        "chaining_out",
        name,
        &["cv_out", "chaining_out", "output", "out"],
    )
}

/// BLAKE3 IV constants (same as BLAKE2s initial hash values).
pub(crate) const IV: [u32; 8] = [
    0x6A09_E667,
    0xBB67_AE85,
    0x3C6E_F372,
    0xA54F_F53A,
    0x510E_527F,
    0x9B05_688C,
    0x1F83_D9AB,
    0x5BE0_CD19,
];

/// Build a Program that applies one BLAKE3 compression step.
///
/// Produces the 8-word post-compression chaining value into
/// `chaining_out`. The output is `state[0..8] ^ state[8..16]` after
/// 7 rounds, per the BLAKE3 spec.
#[must_use]
pub fn blake3_compress(
    chaining_in: &str,
    message: &str,
    params: &str,
    chaining_out: &str,
) -> Program {
    let chaining_in = scoped_chaining_in(chaining_in);
    let message = scoped_message(message);
    let params = scoped_params(params);
    let chaining_out = scoped_chaining_out(chaining_out);
    let chaining_in = chaining_in.as_str();
    let message = message.as_str();
    let params = params.as_str();
    let chaining_out = chaining_out.as_str();
    // Each round is wrapped as one child Region; reserve the exact top-level
    // body size so construction is allocation-free without over-retaining
    // unused node slots.
    let mut body: Vec<Node> = Vec::with_capacity(BLAKE3_COMPRESS_BODY_NODE_COUNT);
    let parent = GeneratorRef {
        name: OP_ID.to_string(),
    };

    // -- Initialize state[0..8] = chaining_in[0..8]. -----------------
    for i in 0..8 {
        body.push(Node::let_bind(
            format!("s{i}"),
            Expr::load(chaining_in, Expr::u32(i as u32)),
        ));
    }
    // state[8..12] = IV[0..4]
    for (i, iv_word) in IV.iter().take(4).enumerate() {
        body.push(Node::let_bind(format!("s{}", i + 8), Expr::u32(*iv_word)));
    }
    // state[12..16] = params[0..4]
    for i in 0..4u32 {
        body.push(Node::let_bind(
            format!("s{}", i + 12),
            Expr::load(params, Expr::u32(i)),
        ));
    }

    // -- Bind message[0..16] = m0..m15. ------------------------------
    for i in 0..16u32 {
        body.push(Node::let_bind(
            format!("m{i}"),
            Expr::load(message, Expr::u32(i)),
        ));
    }

    // -- 7 rounds. Each round is a composed Tier 2.5 primitive. -----
    for (round_idx, perm) in MSG_SCHEDULE.iter().enumerate() {
        body.push(wrap_child(
            BLAKE3_ROUND_OP_ID,
            parent.clone(),
            blake3_round(round_idx, perm),
        ));
    }

    // -- Output: chaining_out[i] = state[i] ^ state[i + 8]. ---------
    for i in 0..8 {
        body.push(Node::Store {
            buffer: chaining_out.into(),
            index: Expr::u32(i as u32),
            value: Expr::bitxor(Expr::var(format!("s{i}")), Expr::var(format!("s{}", i + 8))),
        });
    }

    Program::wrapped(
        vec![
            BufferDecl::storage(chaining_in, 0, BufferAccess::ReadOnly, DataType::U32)
                .with_count(8),
            BufferDecl::storage(message, 1, BufferAccess::ReadOnly, DataType::U32).with_count(16),
            BufferDecl::storage(params, 2, BufferAccess::ReadOnly, DataType::U32).with_count(4),
            BufferDecl::output(chaining_out, 3, DataType::U32).with_count(8),
        ],
        [1, 1, 1],
        vec![wrap_anonymous(OP_ID, body)],
    )
}

inventory::submit! {
    crate::harness::OpEntry {
        id: OP_ID,
        build: || blake3_compress("cv_in", "msg", "params", "cv_out"),
        test_inputs: Some(|| {
            let iv: [u32; 8] = [
                0x6A09_E667, 0xBB67_AE85, 0x3C6E_F372, 0xA54F_F53A,
                0x510E_527F, 0x9B05_688C, 0x1F83_D9AB, 0x5BE0_CD19,
            ];
            let msg: [u32; 16] = [
                0x6172_6261, 0x6163_6461, 0x6172_6261, 0, 0, 0, 0, 0,
                0, 0, 0, 0, 0, 0, 0, 0,
            ];
            let params: [u32; 4] = [0, 0, 64, 0b0000_1011]; // CHUNK_START | CHUNK_END | ROOT

            vec![vec![
                crate::test_support::byte_pack::u32_bytes(&iv),
                crate::test_support::byte_pack::u32_bytes(&msg),
                crate::test_support::byte_pack::u32_bytes(&params),
            ]]
        }),
        expected_output: Some(|| vec![
            vec![
                vec![0x82, 0x5e, 0x3e, 0x45, 0xc6, 0xa8, 0x67, 0x23, 0x78, 0xcf, 0xe6, 0x40, 0x51, 0x65, 0xd4, 0x78,
                     0x8a, 0xc6, 0xee, 0xef, 0x86, 0x39, 0xc4, 0x55, 0x31, 0x4f, 0x36, 0xd0, 0xbc, 0xf1, 0x3f, 0xe5, ],
            ],
        ]),
        category: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blake3_compress_body_capacity_matches_actual_top_level_shape() {
        let program = blake3_compress("cv_in", "msg", "params", "cv_out");
        let [Node::Region { body, .. }] = program.entry() else {
            panic!("Fix: blake3_compress must remain a single provenance Region.");
        };

        assert_eq!(
            body.len(),
            BLAKE3_COMPRESS_BODY_NODE_COUNT,
            "Fix: BLAKE3 compress body reservation must stay exact as the top-level IR shape evolves."
        );
    }
}
