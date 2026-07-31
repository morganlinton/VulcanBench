//! Fused multi-hash primitive.
//!
//! Computes CRC-32, FNV-1a32, and Adler-32 in one serial byte walk. This is
//! the primitive authority for the fused checksum body; higher-tier crates may
//! rename buffers or stamp parent op ids, but should not rebuild this loop.

use std::sync::Arc;

use vyre_foundation::ir::model::expr::Ident;
use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

use super::adler32::{
    adler32, adler32_finalize_expr, adler32_initial_a_expr, adler32_initial_b_expr,
    adler32_update_byte_nodes,
};
use super::crc32::{crc32, crc32_finalize_expr, crc32_initial_expr, crc32_update_byte_nodes};
use super::fnv1a::{fnv1a32, fnv1a32_initial_expr, fnv1a32_update_byte_node};

/// Stable Tier 2.5 op id for the fused CRC-32/FNV-1a32/Adler-32 walker.
pub const MULTI_HASH_OP_ID: &str = "vyre-primitives::hash::multi_hash";

/// CPU reference for the fused multi-hash contract.
#[must_use]
pub fn multi_hash_reference(bytes: &[u8]) -> (u32, u32, u32) {
    (crc32(bytes), fnv1a32(bytes), adler32(bytes))
}

/// Build a Program that computes CRC-32, FNV-1a32, and Adler-32 over
/// `input[0..n]` in a single walk.
///
/// `input[i]` packs one byte per u32 slot. The three results are packed into
/// one output buffer: `out[0] = crc32`, `out[1] = fnv1a32`,
/// `out[2] = adler32`.
#[must_use]
pub fn multi_hash_program(input: &str, out: &str, n: u32) -> Program {
    Program::wrapped(
        vec![
            BufferDecl::storage(input, 0, BufferAccess::ReadOnly, DataType::U32).with_count(n),
            BufferDecl::output(out, 1, DataType::U32).with_count(3),
        ],
        [1, 1, 1],
        vec![Node::Region {
            generator: Ident::from(MULTI_HASH_OP_ID),
            source_region: None,
            body: Arc::new(multi_hash_body(input, out, n)),
        }],
    )
}

fn multi_hash_body(input: &str, out: &str, n: u32) -> Vec<Node> {
    vec![Node::if_then(
        Expr::eq(Expr::InvocationId { axis: 0 }, Expr::u32(0)),
        vec![
            Node::let_bind("crc", crc32_initial_expr()),
            Node::let_bind("fnv", fnv1a32_initial_expr()),
            Node::let_bind("a", adler32_initial_a_expr()),
            Node::let_bind("b", adler32_initial_b_expr()),
            Node::loop_for("i", Expr::u32(0), Expr::u32(n), {
                let mut nodes = vec![Node::let_bind(
                    "byte",
                    Expr::bitand(Expr::load(input, Expr::var("i")), Expr::u32(0xFF)),
                )];
                nodes.extend(crc32_update_byte_nodes("crc", "crc_bit", Expr::var("byte")));
                nodes.push(fnv1a32_update_byte_node("fnv", Expr::var("byte")));
                nodes.extend(adler32_update_byte_nodes("a", "b", Expr::var("byte")));
                nodes
            }),
            Node::store(out, Expr::u32(0), crc32_finalize_expr(Expr::var("crc"))),
            Node::store(out, Expr::u32(1), Expr::var("fnv")),
            Node::store(
                out,
                Expr::u32(2),
                adler32_finalize_expr(Expr::var("a"), Expr::var("b")),
            ),
        ],
    )]
}

#[cfg(feature = "inventory-registry")]
inventory::submit! {
    crate::harness::OpEntry::new(
        MULTI_HASH_OP_ID,
        || multi_hash_program("input", "out", 3),
        Some(|| vec![vec![crate::wire::pack_bytes_as_u32_slice(b"abc")]]),
        Some(|| vec![vec![crate::wire::pack_u32_slice(&[
            0x3524_41c2,
            0x1a47_e90b,
            0x024D_0127,
        ])]]),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reference_matches_constituent_hashes() {
        assert_eq!(
            multi_hash_reference(b"abc"),
            (0x3524_41c2, 0x1a47_e90b, 0x024D_0127)
        );
    }

    #[test]
    fn standalone_program_is_single_multi_hash_region() {
        let program = multi_hash_program("input", "out", 3);
        let [Node::Region { generator, .. }] = program.entry() else {
            panic!("expected one primitive multi_hash region");
        };
        assert_eq!(generator.as_str(), MULTI_HASH_OP_ID);
        assert_eq!(program.buffers()[1].count(), 3);
    }

    #[test]
    fn generated_body_masks_high_input_bits_once_before_updates() {
        let program = multi_hash_program("input", "out", 4);
        let rendered = format!("{:?}", program.entry());
        assert!(
            rendered.contains("255"),
            "Fix: fused multi_hash must mask u32 byte slots before every checksum update."
        );
    }
}
