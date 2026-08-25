//! DEFLATE stored-block inflate primitive body.

use std::sync::Arc;

use vyre_foundation::ir::model::expr::{GeneratorRef, Ident};
use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

/// Canonical op id for stored-block inflate.
pub const INFLATE_STORED_OP_ID: &str = "vyre-primitives::decode::inflate_stored";
/// Fixed-Huffman block diagnostic.
pub const FIXED_HUFFMAN_REJECT: &str = "Fix: vyre-primitives::decode::inflate_stored accepts raw DEFLATE stored blocks only; route BTYPE=1 input to a compressed-block decoder.";
/// Dynamic-Huffman block diagnostic.
pub const DYNAMIC_HUFFMAN_REJECT: &str = "Fix: vyre-primitives::decode::inflate_stored accepts raw DEFLATE stored blocks only; route BTYPE=2 input to a dynamic-Huffman decoder.";
/// Reserved BTYPE diagnostic.
pub const RESERVED_BTYPE_FIX: &str =
    "Fix: reject reserved DEFLATE BTYPE=3 inputs before dispatching vyre-primitives::decode::inflate_stored.";
/// Stored block LEN/NLEN diagnostic.
pub const STORED_HEADER_FIX: &str =
    "Fix: validate LEN/NLEN before copying a stored DEFLATE block in vyre-primitives::decode::inflate_stored.";
/// Number of u32 byte lanes occupied by the stored-block header.
pub const INFLATE_STORED_HEADER_WORDS: u32 = 5;
/// Canonical workgroup shape for stored-block inflate compositions.
pub const INFLATE_STORED_WORKGROUP_SIZE: [u32; 3] = [64, 1, 1];

/// Emit canonical stored-block header decode nodes.
///
/// Defines `header`, `btype`, `len`, and `nlen` in the caller's region.
#[must_use]
pub fn inflate_stored_header_nodes(input: &str) -> Vec<Node> {
    vec![
        Node::let_bind("header", Expr::load(input, Expr::u32(0))),
        Node::let_bind(
            "btype",
            Expr::bitand(Expr::shr(Expr::var("header"), Expr::u32(1)), Expr::u32(0x3)),
        ),
        Node::let_bind(
            "len",
            Expr::bitor(
                Expr::load(input, Expr::u32(1)),
                Expr::shl(Expr::load(input, Expr::u32(2)), Expr::u32(8)),
            ),
        ),
        Node::let_bind(
            "nlen",
            Expr::bitor(
                Expr::load(input, Expr::u32(3)),
                Expr::shl(Expr::load(input, Expr::u32(4)), Expr::u32(8)),
            ),
        ),
    ]
}

/// Expression asserting the stored-block LEN/NLEN complement contract.
#[must_use]
pub fn inflate_stored_len_is_valid_expr() -> Expr {
    Expr::eq(
        Expr::var("nlen"),
        Expr::bitxor(Expr::var("len"), Expr::u32(0xFFFF)),
    )
}

/// Expression loading payload byte lane `index` after the stored-block header.
#[must_use]
pub fn inflate_stored_payload_expr(input: &str, index: Expr) -> Expr {
    Expr::load(
        input,
        Expr::add(Expr::u32(INFLATE_STORED_HEADER_WORDS), index),
    )
}

/// Trap node for a BTYPE=0 block whose LEN/NLEN header is invalid.
#[must_use]
pub fn inflate_stored_invalid_len_trap_node() -> Node {
    Node::if_then(
        Expr::ne(
            Expr::var("nlen"),
            Expr::bitxor(Expr::var("len"), Expr::u32(0xFFFF)),
        ),
        vec![Node::trap(Expr::u32(0), STORED_HEADER_FIX)],
    )
}

/// Trap nodes for non-stored DEFLATE BTYPE values.
#[must_use]
pub fn inflate_stored_non_stored_trap_nodes() -> [Node; 3] {
    [
        Node::if_then(
            Expr::eq(Expr::var("btype"), Expr::u32(1)),
            vec![Node::trap(Expr::u32(1), FIXED_HUFFMAN_REJECT)],
        ),
        Node::if_then(
            Expr::eq(Expr::var("btype"), Expr::u32(2)),
            vec![Node::trap(Expr::u32(2), DYNAMIC_HUFFMAN_REJECT)],
        ),
        Node::if_then(
            Expr::eq(Expr::var("btype"), Expr::u32(3)),
            vec![Node::trap(Expr::u32(3), RESERVED_BTYPE_FIX)],
        ),
    ]
}

/// Build the reusable stored-block inflate body.
#[must_use]
pub fn inflate_stored_body(input: &str, output: &str, inflated_len_buffer: &str) -> Vec<Node> {
    let mut body = vec![
        Node::let_bind("lane", Expr::InvocationId { axis: 0 }),
        Node::if_then(
            Expr::eq(Expr::var("lane"), Expr::u32(0)),
            vec![Node::store(inflated_len_buffer, Expr::u32(0), Expr::u32(0))],
        ),
    ];
    body.extend(inflate_stored_header_nodes(input));
    body.extend([Node::if_then(
        Expr::eq(Expr::var("btype"), Expr::u32(0)),
        vec![
            Node::if_then(
                inflate_stored_len_is_valid_expr(),
                vec![
                    Node::if_then(
                        Expr::eq(Expr::var("lane"), Expr::u32(0)),
                        vec![Node::store(
                            inflated_len_buffer,
                            Expr::u32(0),
                            Expr::var("len"),
                        )],
                    ),
                    Node::if_then(
                        Expr::lt(Expr::var("lane"), Expr::var("len")),
                        vec![Node::store(
                            output,
                            Expr::var("lane"),
                            inflate_stored_payload_expr(input, Expr::var("lane")),
                        )],
                    ),
                ],
            ),
            inflate_stored_invalid_len_trap_node(),
        ],
    )]);
    body.extend(inflate_stored_non_stored_trap_nodes());
    body
}

/// Wrap the stored-block inflate body as a child of `parent_op_id`.
#[must_use]
pub fn inflate_stored_child(
    parent_op_id: &str,
    input: &str,
    output: &str,
    inflated_len_buffer: &str,
) -> Node {
    Node::Region {
        generator: Ident::from(INFLATE_STORED_OP_ID),
        source_region: Some(GeneratorRef {
            name: parent_op_id.to_string(),
        }),
        body: Arc::new(inflate_stored_body(input, output, inflated_len_buffer)),
    }
}

/// Standalone stored-block inflate program for primitive-level conformance.
#[must_use]
pub fn inflate_stored(
    input: &str,
    output: &str,
    inflated_len_buffer: &str,
    input_len: u32,
) -> Program {
    Program::wrapped(
        vec![
            BufferDecl::storage(input, 0, BufferAccess::ReadOnly, DataType::U32)
                .with_count(input_len),
            BufferDecl::output(output, 1, DataType::U32).with_count(input_len),
            BufferDecl::read_write(inflated_len_buffer, 2, DataType::U32).with_count(1),
        ],
        INFLATE_STORED_WORKGROUP_SIZE,
        vec![Node::Region {
            generator: Ident::from(INFLATE_STORED_OP_ID),
            source_region: None,
            body: Arc::new(inflate_stored_body(input, output, inflated_len_buffer)),
        }],
    )
}

#[cfg(feature = "inventory-registry")]
inventory::submit! {
    crate::harness::OpEntry::new(
        INFLATE_STORED_OP_ID,
        || inflate_stored("input", "output", "inflated_len", 10),
        Some(|| vec![vec![
            crate::wire::pack_u32_slice(&[
                0x01,
                0x05,
                0x00,
                0xFA,
                0xFF,
                u32::from(b'h'),
                u32::from(b'e'),
                u32::from(b'l'),
                u32::from(b'l'),
                u32::from(b'o'),
            ]),
            vec![0; 40],
            vec![0; 4],
        ]]),
        Some(|| vec![vec![
            crate::wire::pack_u32_slice(&[
                u32::from(b'h'),
                u32::from(b'e'),
                u32::from(b'l'),
                u32::from(b'l'),
                u32::from(b'o'),
                0,
                0,
                0,
                0,
                0,
            ]),
            crate::wire::pack_u32_slice(&[5]),
        ]]),
    )
}

// ---------------------------------------------------------------------------
// CPU reference implementation
// ---------------------------------------------------------------------------

/// Result of a CPU stored-block inflate.
#[derive(Debug, PartialEq, Eq)]
pub struct CpuInflateResult {
    /// Inflated data bytes (one per u32 slot, low 8 bits).
    pub data: Vec<u32>,
    /// Number of data bytes inflated.
    pub inflated_len: u32,
}

/// CPU reference: inflate a DEFLATE stored block (BTYPE=0).
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn inflate_stored_reference_bytes(input: &[u8]) -> Result<CpuInflateResult, &'static str> {
    if input.len() < 5 {
        return Err(STORED_HEADER_FIX);
    }
    let btype = (input[0] >> 1) & 0x3;
    match btype {
        0 => {
            let len = u16::from_le_bytes([input[1], input[2]]);
            let nlen = u16::from_le_bytes([input[3], input[4]]);
            if nlen != !len {
                return Err(STORED_HEADER_FIX);
            }
            let len_usize = usize::from(len);
            let header_words = INFLATE_STORED_HEADER_WORDS as usize;
            if input.len() < header_words + len_usize {
                return Err(STORED_HEADER_FIX);
            }
            Ok(CpuInflateResult {
                data: input[header_words..][..len_usize]
                    .iter()
                    .map(|&byte| u32::from(byte))
                    .collect(),
                inflated_len: u32::from(len),
            })
        }
        1 => Err(FIXED_HUFFMAN_REJECT),
        2 => Err(DYNAMIC_HUFFMAN_REJECT),
        _ => Err(RESERVED_BTYPE_FIX),
    }
}

/// CPU reference over one-byte-per-u32 packed lanes.
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn inflate_stored_reference_words(input: &[u32]) -> Result<CpuInflateResult, &'static str> {
    let bytes = input
        .iter()
        .map(|word| (word & 0xFF) as u8)
        .collect::<Vec<_>>();
    inflate_stored_reference_bytes(&bytes)
}

/// Compatibility CPU reference: inflate a DEFLATE stored block (BTYPE=0).
///
/// Input words: `[header, len_lo, len_hi, nlen_lo, nlen_hi, data0, data1, ...]`.
/// Each word carries one byte in its low 8 bits. Returns the decoded data
/// and the inflated length. Returns `None` if the input is not a valid
/// stored block (wrong BTYPE or LEN/NLEN mismatch).
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn cpu_inflate_stored(input: &[u32]) -> Option<CpuInflateResult> {
    inflate_stored_reference_words(input).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inflate_stored_hello() {
        // Matches the inventory test case exactly.
        let input = [
            0x01, // BFINAL=1, BTYPE=00
            0x05,
            0x00, // LEN = 5
            0xFA,
            0xFF, // NLEN = 0xFFFA (= 5 ^ 0xFFFF)
            b'h' as u32,
            b'e' as u32,
            b'l' as u32,
            b'l' as u32,
            b'o' as u32,
        ];
        let result = cpu_inflate_stored(&input).unwrap();
        assert_eq!(result.inflated_len, 5);
        assert_eq!(
            result.data,
            vec![
                b'h' as u32,
                b'e' as u32,
                b'l' as u32,
                b'l' as u32,
                b'o' as u32
            ]
        );
    }

    #[test]
    fn inflate_stored_empty_block() {
        let input = [
            0x01, // BFINAL=1, BTYPE=00
            0x00, 0x00, // LEN = 0
            0xFF, 0xFF, // NLEN = 0xFFFF
        ];
        let result = cpu_inflate_stored(&input).unwrap();
        assert_eq!(result.inflated_len, 0);
        assert!(result.data.is_empty());
    }

    #[test]
    fn inflate_stored_rejects_fixed_huffman() {
        let input = [
            0x03, // BFINAL=1, BTYPE=01 (fixed Huffman)
            0x00, 0x00, 0xFF, 0xFF,
        ];
        assert!(cpu_inflate_stored(&input).is_none());
    }

    #[test]
    fn inflate_stored_rejects_len_nlen_mismatch() {
        let input = [
            0x01, // BFINAL=1, BTYPE=00
            0x05,
            0x00, // LEN = 5
            0x00,
            0x00, // NLEN = 0 (wrong!)
            b'x' as u32,
            b'x' as u32,
            b'x' as u32,
            b'x' as u32,
            b'x' as u32,
        ];
        assert!(cpu_inflate_stored(&input).is_none());
    }

    #[test]
    fn inflate_stored_rejects_truncated_payload() {
        let input = [0x01, 0x05, 0x00, 0xFA, 0xFF, b'h' as u32, b'e' as u32];
        assert_eq!(
            inflate_stored_reference_words(&input),
            Err(STORED_HEADER_FIX)
        );
        assert!(cpu_inflate_stored(&input).is_none());
    }
}
