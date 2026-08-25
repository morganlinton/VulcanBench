//! Hex decode primitive body.

use std::sync::{Arc, OnceLock};

use vyre_foundation::ir::model::expr::{GeneratorRef, Ident};
use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

/// Canonical op id for ASCII hex decode.
pub const HEX_DECODE_OP_ID: &str = "vyre-primitives::decode::hex_decode";
/// Number of words in the ASCII hex decode lookup table.
pub const HEX_DECODE_TABLE_WORDS: u32 = 256;
/// Canonical hex decode workgroup size.
pub const HEX_WORKGROUP_SIZE: [u32; 3] = [64, 1, 1];

static HEX_DECODE_TABLE: OnceLock<[u32; 256]> = OnceLock::new();

/// Return the canonical 256-entry ASCII hex decode table by value.
#[must_use]
pub fn hex_decode_table() -> [u32; 256] {
    *hex_decode_table_ref()
}

/// Process-wide canonical ASCII hex decode table.
///
/// The table is immutable after construction. Dispatch setup and CPU oracles
/// should use this reference when they do not need an owned copy.
#[must_use]
pub fn hex_decode_table_ref() -> &'static [u32; 256] {
    HEX_DECODE_TABLE.get_or_init(build_hex_decode_table)
}

fn build_hex_decode_table() -> [u32; 256] {
    let mut table = [0u32; 256];
    let mut byte = b'0';
    while byte <= b'9' {
        table[byte as usize] = u32::from(byte - b'0');
        byte += 1;
    }
    byte = b'A';
    while byte <= b'F' {
        table[byte as usize] = u32::from(byte - b'A' + 10);
        byte += 1;
    }
    byte = b'a';
    while byte <= b'f' {
        table[byte as usize] = u32::from(byte - b'a' + 10);
        byte += 1;
    }
    table
}

/// Number of decoded byte slots produced by an even-length hex input.
#[must_use]
pub const fn hex_decoded_capacity(input_len: u32) -> u32 {
    input_len / 2
}

fn nibble_expr(byte: Expr, table: &str) -> Expr {
    Expr::load(table, Expr::bitand(byte, Expr::u32(0xFF)))
}

/// Decode one hex byte pair into a single u32 byte value.
#[must_use]
pub fn hex_decode_pair_expr(input: &str, table: &str, pair: Expr) -> Expr {
    let in_base = Expr::mul(pair, Expr::u32(2));
    let hi = nibble_expr(Expr::load(input, in_base.clone()), table);
    let lo = nibble_expr(Expr::load(input, Expr::add(in_base, Expr::u32(1))), table);
    Expr::bitor(Expr::shl(hi, Expr::u32(4)), lo)
}

/// Build the reusable hex decode body.
#[must_use]
pub fn hex_decode_body(input: &str, output: &str, table: &str, input_len: u32) -> Vec<Node> {
    if input_len % 2 != 0 {
        return vec![Node::trap(
            Expr::u32(input_len),
            "Fix: hex_decode requires an even input_len; reject the dangling nibble upstream",
        )];
    }
    let output_len = hex_decoded_capacity(input_len);
    vec![
        Node::let_bind("pair", Expr::InvocationId { axis: 0 }),
        Node::if_then(
            Expr::lt(Expr::var("pair"), Expr::u32(output_len)),
            vec![Node::store(
                output,
                Expr::var("pair"),
                hex_decode_pair_expr(input, table, Expr::var("pair")),
            )],
        ),
    ]
}

/// Wrap the hex decode body as a child of `parent_op_id`.
#[must_use]
pub fn hex_decode_child(
    parent_op_id: &str,
    input: &str,
    output: &str,
    table: &str,
    input_len: u32,
) -> Node {
    Node::Region {
        generator: Ident::from(HEX_DECODE_OP_ID),
        source_region: Some(GeneratorRef {
            name: parent_op_id.to_string(),
        }),
        body: Arc::new(hex_decode_body(input, output, table, input_len)),
    }
}

/// Standalone hex decode program for primitive-level conformance.
#[must_use]
pub fn hex_decode(input: &str, output: &str, table: &str, input_len: u32) -> Program {
    Program::wrapped(
        vec![
            BufferDecl::storage(input, 0, BufferAccess::ReadOnly, DataType::U32)
                .with_count(input_len),
            BufferDecl::output(output, 1, DataType::U32)
                .with_count(hex_decoded_capacity(input_len)),
            BufferDecl::storage(table, 2, BufferAccess::ReadOnly, DataType::U32)
                .with_count(HEX_DECODE_TABLE_WORDS),
        ],
        HEX_WORKGROUP_SIZE,
        vec![Node::Region {
            generator: Ident::from(HEX_DECODE_OP_ID),
            source_region: None,
            body: Arc::new(hex_decode_body(input, output, table, input_len)),
        }],
    )
}

/// CPU oracle for the primitive hex decode contract.
///
/// Invalid nibbles clamp to zero through the table, matching the GPU body.
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn hex_decode_reference_packed(input: &[u8]) -> Vec<u32> {
    assert!(input.len() % 2 == 0, "hex input must contain byte pairs");
    let table = hex_decode_table_ref();
    input
        .chunks_exact(2)
        .map(|pair| {
            let hi = table[usize::from(pair[0])];
            let lo = table[usize::from(pair[1])];
            (hi << 4) | lo
        })
        .collect()
}

#[cfg(feature = "inventory-registry")]
inventory::submit! {
    crate::harness::OpEntry::new(
        HEX_DECODE_OP_ID,
        || hex_decode("input", "output", "table", 6),
        Some(|| vec![vec![
            crate::wire::pack_u32_slice(&[
                u32::from(b'4'),
                u32::from(b'D'),
                u32::from(b'6'),
                u32::from(b'1'),
                u32::from(b'6'),
                u32::from(b'E'),
            ]),
            vec![0; 12],
            crate::wire::pack_u32_slice(hex_decode_table_ref()),
        ]]),
        Some(|| vec![vec![crate::wire::pack_u32_slice(&[0x4D, 0x61, 0x6E])]]),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reference_decodes_upper_lower_and_invalid_nibbles() {
        assert_eq!(
            hex_decode_reference_packed(b"4D6aZ1"),
            vec![0x4D, 0x6A, 0x01]
        );
    }

    #[test]
    fn hex_decode_table_ref_matches_value_api_and_reuses_allocation() {
        let first = hex_decode_table_ref();
        let second = hex_decode_table_ref();
        assert!(
            std::ptr::eq(first, second),
            "Fix: hex decode setup must reuse the immutable primitive table instead of rebuilding it per dispatch."
        );
        assert_eq!(*first, hex_decode_table());
    }

    #[test]
    fn odd_length_lowers_to_trap_not_silent_truncation() {
        let body = hex_decode_body("input", "output", "table", 3);
        assert!(matches!(body.as_slice(), [Node::Trap { .. }]));
    }

    #[test]
    fn standalone_program_is_single_primitive_region() {
        let program = hex_decode("input", "output", "table", 6);
        let [Node::Region { generator, .. }] = program.entry() else {
            panic!("expected one primitive hex decode region");
        };
        assert_eq!(generator.as_str(), HEX_DECODE_OP_ID);
    }
}
