//! UTF-8 shape counters over a precomputed byte histogram.

use std::sync::Arc;

use vyre_foundation::ir::model::expr::{GeneratorRef, Ident};
use vyre_foundation::ir::{BinOp, BufferAccess, BufferDecl, DataType, Expr, Node, Program};

/// Canonical op id for UTF-8 histogram shape counting.
pub const UTF8_SHAPE_COUNTS_OP_ID: &str = "vyre-primitives::text::utf8_shape_counts";

fn saturating_add_expr(left: Expr, right: Expr) -> Expr {
    Expr::BinOp {
        op: BinOp::SaturatingAdd,
        left: Box::new(left),
        right: Box::new(right),
    }
}

fn saturating_const_mul_expr(value: Expr, factor: u32) -> Expr {
    match factor {
        0 => Expr::u32(0),
        1 => value,
        _ => Expr::select(
            Expr::gt(value.clone(), Expr::u32(u32::MAX / factor)),
            Expr::u32(u32::MAX),
            Expr::mul(value, Expr::u32(factor)),
        ),
    }
}

/// Build a body that assigns continuation and expected-continuation counts.
#[must_use]
pub fn utf8_shape_counts_body(
    histogram: &str,
    continuation_var: &str,
    expected_var: &str,
) -> Vec<Node> {
    vec![
        Node::assign(continuation_var, Expr::u32(0)),
        Node::assign(expected_var, Expr::u32(0)),
        Node::loop_for(
            "i",
            Expr::u32(0x80),
            Expr::u32(0xF5),
            vec![
                Node::let_bind("byte_count", Expr::load(histogram, Expr::var("i"))),
                Node::if_then(
                    Expr::lt(Expr::var("i"), Expr::u32(0xC0)),
                    vec![Node::assign(
                        continuation_var,
                        saturating_add_expr(Expr::var(continuation_var), Expr::var("byte_count")),
                    )],
                ),
                Node::if_then(
                    Expr::and(
                        Expr::gt(Expr::var("i"), Expr::u32(0xC1)),
                        Expr::lt(Expr::var("i"), Expr::u32(0xE0)),
                    ),
                    vec![Node::assign(
                        expected_var,
                        saturating_add_expr(Expr::var(expected_var), Expr::var("byte_count")),
                    )],
                ),
                Node::if_then(
                    Expr::and(
                        Expr::gt(Expr::var("i"), Expr::u32(0xDF)),
                        Expr::lt(Expr::var("i"), Expr::u32(0xF0)),
                    ),
                    vec![Node::assign(
                        expected_var,
                        saturating_add_expr(
                            Expr::var(expected_var),
                            saturating_const_mul_expr(Expr::var("byte_count"), 2),
                        ),
                    )],
                ),
                Node::if_then(
                    Expr::gt(Expr::var("i"), Expr::u32(0xEF)),
                    vec![Node::assign(
                        expected_var,
                        saturating_add_expr(
                            Expr::var(expected_var),
                            saturating_const_mul_expr(Expr::var("byte_count"), 3),
                        ),
                    )],
                ),
            ],
        ),
    ]
}

/// Wrap the UTF-8 shape counter body as a child of `parent_op_id`.
#[must_use]
pub fn utf8_shape_counts_child(
    parent_op_id: &str,
    histogram: &str,
    continuation_var: &str,
    expected_var: &str,
) -> Node {
    Node::Region {
        generator: Ident::from(UTF8_SHAPE_COUNTS_OP_ID),
        source_region: Some(GeneratorRef {
            name: parent_op_id.to_string(),
        }),
        body: Arc::new(utf8_shape_counts_body(
            histogram,
            continuation_var,
            expected_var,
        )),
    }
}

/// Standalone UTF-8 shape counter program for primitive-level conformance.
#[must_use]
pub fn utf8_shape_counts(histogram: &str, out: &str) -> Program {
    let mut body = vec![
        Node::let_bind("continuation", Expr::u32(0)),
        Node::let_bind("expected", Expr::u32(0)),
    ];
    body.extend(utf8_shape_counts_body(
        histogram,
        "continuation",
        "expected",
    ));
    body.push(Node::store(out, Expr::u32(0), Expr::var("continuation")));
    body.push(Node::store(out, Expr::u32(1), Expr::var("expected")));
    Program::wrapped(
        vec![
            BufferDecl::storage(histogram, 0, BufferAccess::ReadOnly, DataType::U32)
                .with_count(256),
            BufferDecl::output(out, 1, DataType::U32)
                .with_count(2)
                .with_output_byte_range(0..8),
        ],
        [1, 1, 1],
        vec![Node::Region {
            generator: Ident::from(UTF8_SHAPE_COUNTS_OP_ID),
            source_region: None,
            body: Arc::new(body),
        }],
    )
}

/// Reference oracle for [`utf8_shape_counts`].
#[must_use]
pub(crate) fn utf8_shape_counts_from_histogram(histogram: &[u32; 256]) -> (u32, u32) {
    let continuation = histogram[0x80..0xC0]
        .iter()
        .fold(0u32, |acc, &count| acc.saturating_add(count));
    let expected = histogram[0xC2..0xE0]
        .iter()
        .fold(0u32, |acc, &count| acc.saturating_add(count));
    let expected = histogram[0xE0..0xF0].iter().fold(expected, |acc, &count| {
        acc.saturating_add(count.saturating_mul(2))
    });
    let expected = histogram[0xF0..0xF5].iter().fold(expected, |acc, &count| {
        acc.saturating_add(count.saturating_mul(3))
    });
    (continuation, expected)
}

/// Reference oracle for [`utf8_shape_counts`].
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn reference_utf8_shape_counts(histogram: &[u32; 256]) -> (u32, u32) {
    utf8_shape_counts_from_histogram(histogram)
}

#[cfg(feature = "inventory-registry")]
inventory::submit! {
    crate::harness::OpEntry::new(
        UTF8_SHAPE_COUNTS_OP_ID,
        || utf8_shape_counts("histogram", "out"),
        Some(|| {
            let mut histogram = vec![0u8; 256 * 4];
            for (slot, value) in [(0xC3usize, 2u32), (0xA9usize, 2u32)] {
                histogram[slot * 4..slot * 4 + 4].copy_from_slice(&value.to_le_bytes());
            }
            vec![vec![histogram, vec![0; 8]]]
        }),
        Some(|| vec![vec![[2u32.to_le_bytes(), 2u32.to_le_bytes()].concat()]]),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reference_counts_continuation_and_expected() {
        let mut histogram = [0u32; 256];
        histogram[0xC3] = 2;
        histogram[0xA9] = 2;
        assert_eq!(reference_utf8_shape_counts(&histogram), (2, 2));
    }

    #[test]
    fn reference_saturates_three_byte_expected_count() {
        let mut histogram = [0u32; 256];
        histogram[0xE0] = u32::MAX / 2 + 1;
        assert_eq!(reference_utf8_shape_counts(&histogram), (0, u32::MAX));
    }

    #[test]
    fn reference_saturates_four_byte_expected_count() {
        let mut histogram = [0u32; 256];
        histogram[0xF0] = u32::MAX / 3 + 1;
        assert_eq!(reference_utf8_shape_counts(&histogram), (0, u32::MAX));
    }
}
