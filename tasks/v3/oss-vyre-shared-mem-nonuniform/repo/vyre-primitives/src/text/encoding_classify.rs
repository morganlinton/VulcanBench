//! Encoding classifier over a precomputed 256-bin byte histogram.

use std::sync::Arc;

use vyre_foundation::ir::model::expr::{GeneratorRef, Ident};
use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

use crate::reduce::range_counts::range_counts_u32_child;
use crate::text::utf8_shape_counts::{utf8_shape_counts_child, utf8_shape_counts_from_histogram};

/// Canonical op id for histogram-based encoding classification.
pub const ENCODING_CLASSIFY_OP_ID: &str = "vyre-primitives::text::encoding_classify";
/// Single-result workgroup for standalone histogram classification.
pub const ENCODING_CLASSIFY_WORKGROUP_SIZE: [u32; 3] = [1, 1, 1];

/// Encoding-id for pure ASCII input.
pub const ENC_ASCII: u32 = 0;
/// Encoding-id for UTF-8 input.
pub const ENC_UTF8: u32 = 1;
/// Encoding-id for UTF-16 little-endian input.
pub const ENC_UTF16LE: u32 = 2;
/// Encoding-id for UTF-16 big-endian input.
pub const ENC_UTF16BE: u32 = 3;
/// Encoding-id for ISO-8859-1 / Windows-1252-like high-byte input.
pub const ENC_ISO8859_1: u32 = 4;
/// Encoding-id for unknown or binary input.
pub const ENC_BINARY: u32 = 255;

/// Build the reusable classifier body.
#[must_use]
pub fn encoding_classify_body(histogram: &str, output: &str, count: u32) -> Vec<Node> {
    vec![Node::if_then(
        Expr::eq(Expr::InvocationId { axis: 0 }, Expr::u32(0)),
        {
            let mut body = vec![
                Node::let_bind("null_count", Expr::load(histogram, Expr::u32(0))),
                Node::let_bind("ascii_count", Expr::u32(0)),
                range_counts_u32_child(ENCODING_CLASSIFY_OP_ID, histogram, "ascii_count", 0, 128),
                Node::let_bind(
                    "high_count",
                    Expr::sub(Expr::u32(count), Expr::var("ascii_count")),
                ),
                Node::let_bind("enc_id", Expr::u32(ENC_BINARY)),
                Node::if_then(
                    Expr::eq(Expr::var("high_count"), Expr::u32(0)),
                    vec![Node::assign("enc_id", Expr::u32(ENC_ASCII))],
                ),
                Node::if_then(
                    Expr::gt(
                        Expr::var("null_count"),
                        Expr::div(Expr::u32(count), Expr::u32(8)),
                    ),
                    vec![Node::assign("enc_id", Expr::u32(ENC_UTF16LE))],
                ),
                Node::let_bind("continuation", Expr::u32(0)),
                Node::let_bind("expected_continuation", Expr::u32(0)),
                utf8_shape_counts_child(
                    ENCODING_CLASSIFY_OP_ID,
                    histogram,
                    "continuation",
                    "expected_continuation",
                ),
            ];

            body.push(Node::if_then(
                Expr::and(
                    Expr::gt(Expr::var("high_count"), Expr::u32(0)),
                    Expr::lt(
                        Expr::abs_diff(
                            Expr::var("continuation"),
                            Expr::var("expected_continuation"),
                        ),
                        Expr::div(Expr::u32(count.saturating_add(19)), Expr::u32(20)),
                    ),
                ),
                vec![Node::assign("enc_id", Expr::u32(ENC_UTF8))],
            ));
            body.push(Node::if_then(
                Expr::and(
                    Expr::gt(Expr::var("high_count"), Expr::u32(0)),
                    Expr::ne(Expr::var("enc_id"), Expr::u32(ENC_UTF8)),
                ),
                vec![Node::if_then(
                    Expr::ne(Expr::var("enc_id"), Expr::u32(ENC_UTF16LE)),
                    vec![Node::assign("enc_id", Expr::u32(ENC_ISO8859_1))],
                )],
            ));
            body.push(Node::store(output, Expr::u32(0), Expr::var("enc_id")));
            body
        },
    )]
}

/// Wrap the classifier body as a child of `parent_op_id`.
#[must_use]
pub fn encoding_classify_child(
    parent_op_id: &str,
    histogram: &str,
    output: &str,
    count: u32,
) -> Node {
    Node::Region {
        generator: Ident::from(ENCODING_CLASSIFY_OP_ID),
        source_region: Some(GeneratorRef {
            name: parent_op_id.to_string(),
        }),
        body: Arc::new(encoding_classify_body(histogram, output, count)),
    }
}

/// Standalone classifier program for primitive-level conformance.
#[must_use]
pub fn encoding_classify(histogram: &str, output: &str, count: u32) -> Program {
    Program::wrapped(
        vec![
            BufferDecl::storage(histogram, 0, BufferAccess::ReadOnly, DataType::U32)
                .with_count(256),
            BufferDecl::output(output, 1, DataType::U32)
                .with_count(1)
                .with_output_byte_range(0..4),
        ],
        ENCODING_CLASSIFY_WORKGROUP_SIZE,
        vec![Node::Region {
            generator: Ident::from(ENCODING_CLASSIFY_OP_ID),
            source_region: None,
            body: Arc::new(encoding_classify_body(histogram, output, count)),
        }],
    )
}

/// Reference oracle for [`encoding_classify`].
#[must_use]
pub fn classify_from_histogram(histogram: &[u32; 256], count: u32) -> u32 {
    if count == 0 {
        return ENC_ASCII;
    }
    let null_count = histogram[0];
    let ascii_count: u32 = histogram[0..128].iter().sum();
    let high_count = count - ascii_count;

    if null_count > count / 8 {
        return ENC_UTF16LE;
    }
    if high_count == 0 {
        return ENC_ASCII;
    }

    let (continuation, expected_continuation) = utf8_shape_counts_from_histogram(histogram);

    let tolerance = count.saturating_add(19) / 20;
    if continuation.abs_diff(expected_continuation) < tolerance {
        return ENC_UTF8;
    }

    ENC_ISO8859_1
}

#[cfg(feature = "inventory-registry")]
inventory::submit! {
    crate::harness::OpEntry::new(
        ENCODING_CLASSIFY_OP_ID,
        || encoding_classify("histogram", "encoding", 5),
        Some(|| {
            let mut histogram = vec![0u8; 256 * 4];
            for (slot, value) in [(b'H' as usize, 1u32), (b'e' as usize, 1), (b'l' as usize, 2), (b'o' as usize, 1)] {
                histogram[slot * 4..slot * 4 + 4].copy_from_slice(&value.to_le_bytes());
            }
            vec![vec![histogram, vec![0; 4]]]
        }),
        Some(|| vec![vec![ENC_ASCII.to_le_bytes().to_vec()]]),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_ascii_histogram() {
        let mut histogram = [0u32; 256];
        histogram[usize::from(b'H')] = 1;
        histogram[usize::from(b'e')] = 1;
        histogram[usize::from(b'l')] = 2;
        histogram[usize::from(b'o')] = 1;
        assert_eq!(classify_from_histogram(&histogram, 5), ENC_ASCII);
    }

    #[test]
    fn classifies_utf8_shape() {
        let mut histogram = [0u32; 256];
        histogram[0xC3] = 2;
        histogram[0xA9] = 2;
        assert_eq!(classify_from_histogram(&histogram, 4), ENC_UTF8);
    }

    #[test]
    fn program_uses_single_result_workgroup() {
        let program = encoding_classify("histogram", "encoding", 0);
        assert_eq!(program.workgroup_size(), ENCODING_CLASSIFY_WORKGROUP_SIZE);
    }
}
