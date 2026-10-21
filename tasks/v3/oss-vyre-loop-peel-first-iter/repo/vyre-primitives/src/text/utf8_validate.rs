//! Tier 2.5 UTF-8 validator  -  single-pass byte classification with
//! structural sequence checks.
//!
//! Each invocation reads one source byte (`source[i]`, low 8 bits)
//! and writes one of four classification codes into `classes[i]`:
//!
//! - [`UTF8_ASCII`]  -  byte 0x00..0x7F, single-byte sequence
//! - [`UTF8_LEAD_2`]  -  byte 0xC0..0xDF, lead of a 2-byte sequence
//! - [`UTF8_LEAD_3`]  -  byte 0xE0..0xEF, lead of a 3-byte sequence
//! - [`UTF8_LEAD_4`]  -  byte 0xF0..0xF7, lead of a 4-byte sequence
//! - [`UTF8_CONT`]    -  byte 0x80..0xBF, continuation byte
//! - [`UTF8_INVALID`]  -  byte 0xC0/0xC1 (overlong) or ≥ 0xF8 (out of range)
//!
//! Malformed lead/continuation structure is reported as
//! [`UTF8_INVALID`] at the offending byte. Valid bytes retain the
//! shape code parser dialects need for downstream tokenization.

use std::sync::Arc;
use vyre_foundation::ir::model::expr::Ident;
use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

/// Stable op id for the registered Tier 3 wrapper.
pub const OP_ID: &str = "vyre-primitives::text::utf8_validate";
/// Byte-lane workgroup used by the UTF-8 classifier.
pub const UTF8_VALIDATE_WORKGROUP_SIZE: [u32; 3] = [256, 1, 1];
/// Dispatch grid for one UTF-8 validation pass over `n` bytes.
#[must_use]
pub const fn utf8_validate_dispatch_grid(n: u32) -> [u32; 3] {
    let blocks = n.div_ceil(UTF8_VALIDATE_WORKGROUP_SIZE[0]);
    if blocks == 0 {
        [1, 1, 1]
    } else {
        [blocks, 1, 1]
    }
}

/// 0x00..0x7F  -  single-byte ASCII.
pub const UTF8_ASCII: u32 = 0;
/// 0xC2..0xDF  -  lead of a valid 2-byte sequence.
pub const UTF8_LEAD_2: u32 = 1;
/// 0xE0..0xEF  -  lead of a 3-byte sequence.
pub const UTF8_LEAD_3: u32 = 2;
/// 0xF0..0xF7  -  lead of a 4-byte sequence.
pub const UTF8_LEAD_4: u32 = 3;
/// 0x80..0xBF  -  continuation byte.
pub const UTF8_CONT: u32 = 4;
/// 0xC0, 0xC1 (overlong) or 0xF8..0xFF (out of range)  -  invalid lead.
pub const UTF8_INVALID: u32 = 5;

/// Build a Program that validates and classifies each `source[i]`
/// byte into one of the `UTF8_*` codes above and writes the result
/// into `classes[i]`.
///
/// This compatibility entry point expects one `DataType::U32` element per
/// source byte and reads the low byte of each word. Use [`utf8_validate_u8`]
/// when the source is packed as one byte per element.
#[must_use]
pub fn utf8_validate(source: &str, classes: &str, n: u32) -> Program {
    utf8_validate_with_source_type(source, classes, n, DataType::U32)
}

/// Build a UTF-8 validation Program over a packed `DataType::U8` source.
///
/// It emits the same per-byte class stream as [`utf8_validate`] while cutting
/// source input bandwidth from four bytes per logical byte to one.
#[must_use]
pub fn utf8_validate_u8(source: &str, classes: &str, n: u32) -> Program {
    utf8_validate_with_source_type(source, classes, n, DataType::U8)
}

fn utf8_validate_with_source_type(
    source: &str,
    classes: &str,
    n: u32,
    source_type: DataType,
) -> Program {
    let idx = Expr::InvocationId { axis: 0 };
    let body = vec![Node::Region {
        generator: Ident::from(OP_ID),
        source_region: None,
        body: Arc::new(vec![
            Node::let_bind("idx", idx.clone()),
            Node::if_then(
                Expr::lt(Expr::var("idx"), Expr::u32(n)),
                vec![
                    Node::let_bind("byte", byte_expr(source, Expr::var("idx"))),
                    Node::let_bind("class", Expr::u32(UTF8_INVALID)),
                    Node::if_then(
                        Expr::lt(Expr::var("byte"), Expr::u32(0x80)),
                        vec![Node::assign("class", Expr::u32(UTF8_ASCII))],
                    ),
                    Node::if_then(
                        in_range(Expr::var("byte"), 0x80, 0xBF),
                        continuation_validation_body(source),
                    ),
                    Node::if_then(
                        in_range(Expr::var("byte"), 0xC2, 0xDF),
                        lead2_validation_body(source, n),
                    ),
                    Node::if_then(
                        in_range(Expr::var("byte"), 0xE0, 0xEF),
                        lead3_validation_body(source, n),
                    ),
                    Node::if_then(
                        in_range(Expr::var("byte"), 0xF0, 0xF4),
                        lead4_validation_body(source, n),
                    ),
                    Node::store(classes, Expr::var("idx"), Expr::var("class")),
                ],
            ),
        ]),
    }];

    let source_decl = if n == 0 {
        BufferDecl::storage(source, 0, BufferAccess::ReadOnly, source_type)
    } else {
        BufferDecl::storage(source, 0, BufferAccess::ReadOnly, source_type).with_count(n)
    };
    Program::wrapped(
        vec![
            source_decl,
            BufferDecl::output(classes, 1, DataType::U32)
                .with_count(n.max(1))
                .with_output_byte_range(0..(n as usize).saturating_mul(4)),
        ],
        UTF8_VALIDATE_WORKGROUP_SIZE,
        body,
    )
}

fn byte_expr(source: &str, index: Expr) -> Expr {
    Expr::bitand(
        Expr::cast(DataType::U32, Expr::load(source, index)),
        Expr::u32(0xFF),
    )
}

fn in_range(value: Expr, lo: u32, hi: u32) -> Expr {
    Expr::and(
        Expr::ge(value.clone(), Expr::u32(lo)),
        Expr::le(value, Expr::u32(hi)),
    )
}

fn valid_three_byte_first(lead: Expr, first: Expr) -> Expr {
    Expr::or(
        Expr::or(
            Expr::and(
                Expr::eq(lead.clone(), Expr::u32(0xE0)),
                in_range(first.clone(), 0xA0, 0xBF),
            ),
            Expr::and(
                Expr::eq(lead.clone(), Expr::u32(0xED)),
                in_range(first.clone(), 0x80, 0x9F),
            ),
        ),
        Expr::and(
            Expr::or(
                in_range(lead.clone(), 0xE1, 0xEC),
                in_range(lead, 0xEE, 0xEF),
            ),
            in_range(first, 0x80, 0xBF),
        ),
    )
}

fn valid_four_byte_first(lead: Expr, first: Expr) -> Expr {
    Expr::or(
        Expr::or(
            Expr::and(
                Expr::eq(lead.clone(), Expr::u32(0xF0)),
                in_range(first.clone(), 0x90, 0xBF),
            ),
            Expr::and(
                Expr::eq(lead.clone(), Expr::u32(0xF4)),
                in_range(first.clone(), 0x80, 0x8F),
            ),
        ),
        Expr::and(in_range(lead, 0xF1, 0xF3), in_range(first, 0x80, 0xBF)),
    )
}

fn continuation_validation_body(source: &str) -> Vec<Node> {
    vec![
        Node::if_then(
            Expr::lt(Expr::u32(0), Expr::var("idx")),
            vec![
                Node::let_bind(
                    "prev1",
                    byte_expr(source, Expr::add(Expr::var("idx"), Expr::u32(u32::MAX))),
                ),
                Node::if_then(
                    in_range(Expr::var("prev1"), 0xC2, 0xDF),
                    vec![Node::assign("class", Expr::u32(UTF8_CONT))],
                ),
                Node::if_then(
                    Expr::lt(
                        Expr::add(Expr::var("idx"), Expr::u32(1)),
                        Expr::buf_len(source),
                    ),
                    vec![
                        Node::let_bind(
                            "next1_after_cont3",
                            byte_expr(source, Expr::add(Expr::var("idx"), Expr::u32(1))),
                        ),
                        Node::if_then(
                            Expr::and(
                                valid_three_byte_first(Expr::var("prev1"), Expr::var("byte")),
                                in_range(Expr::var("next1_after_cont3"), 0x80, 0xBF),
                            ),
                            vec![Node::assign("class", Expr::u32(UTF8_CONT))],
                        ),
                    ],
                ),
                Node::if_then(
                    Expr::lt(
                        Expr::add(Expr::var("idx"), Expr::u32(2)),
                        Expr::buf_len(source),
                    ),
                    vec![
                        Node::let_bind(
                            "next1_after_cont4",
                            byte_expr(source, Expr::add(Expr::var("idx"), Expr::u32(1))),
                        ),
                        Node::let_bind(
                            "next2_after_cont4",
                            byte_expr(source, Expr::add(Expr::var("idx"), Expr::u32(2))),
                        ),
                        Node::if_then(
                            Expr::and(
                                Expr::and(
                                    valid_four_byte_first(Expr::var("prev1"), Expr::var("byte")),
                                    in_range(Expr::var("next1_after_cont4"), 0x80, 0xBF),
                                ),
                                in_range(Expr::var("next2_after_cont4"), 0x80, 0xBF),
                            ),
                            vec![Node::assign("class", Expr::u32(UTF8_CONT))],
                        ),
                    ],
                ),
            ],
        ),
        Node::if_then(
            Expr::lt(Expr::u32(1), Expr::var("idx")),
            vec![
                Node::let_bind(
                    "prev2",
                    byte_expr(source, Expr::add(Expr::var("idx"), Expr::u32(u32::MAX - 1))),
                ),
                Node::let_bind(
                    "prev1_for_3",
                    byte_expr(source, Expr::add(Expr::var("idx"), Expr::u32(u32::MAX))),
                ),
                Node::if_then(
                    valid_three_byte_first(Expr::var("prev2"), Expr::var("prev1_for_3")),
                    vec![Node::assign("class", Expr::u32(UTF8_CONT))],
                ),
                Node::if_then(
                    Expr::lt(
                        Expr::add(Expr::var("idx"), Expr::u32(1)),
                        Expr::buf_len(source),
                    ),
                    vec![
                        Node::let_bind(
                            "next1_after_cont4_mid",
                            byte_expr(source, Expr::add(Expr::var("idx"), Expr::u32(1))),
                        ),
                        Node::if_then(
                            Expr::and(
                                Expr::and(
                                    valid_four_byte_first(
                                        Expr::var("prev2"),
                                        Expr::var("prev1_for_3"),
                                    ),
                                    in_range(Expr::var("byte"), 0x80, 0xBF),
                                ),
                                in_range(Expr::var("next1_after_cont4_mid"), 0x80, 0xBF),
                            ),
                            vec![Node::assign("class", Expr::u32(UTF8_CONT))],
                        ),
                    ],
                ),
            ],
        ),
        Node::if_then(
            Expr::lt(Expr::u32(2), Expr::var("idx")),
            vec![
                Node::let_bind(
                    "prev3",
                    byte_expr(source, Expr::add(Expr::var("idx"), Expr::u32(u32::MAX - 2))),
                ),
                Node::let_bind(
                    "prev2_for_4",
                    byte_expr(source, Expr::add(Expr::var("idx"), Expr::u32(u32::MAX - 1))),
                ),
                Node::let_bind(
                    "prev1_for_4",
                    byte_expr(source, Expr::add(Expr::var("idx"), Expr::u32(u32::MAX))),
                ),
                Node::if_then(
                    Expr::and(
                        valid_four_byte_first(Expr::var("prev3"), Expr::var("prev2_for_4")),
                        in_range(Expr::var("prev1_for_4"), 0x80, 0xBF),
                    ),
                    vec![Node::assign("class", Expr::u32(UTF8_CONT))],
                ),
            ],
        ),
    ]
}

fn lead2_validation_body(source: &str, n: u32) -> Vec<Node> {
    vec![Node::if_then(
        Expr::lt(Expr::add(Expr::var("idx"), Expr::u32(1)), Expr::u32(n)),
        vec![
            Node::let_bind(
                "next1_for_2",
                byte_expr(source, Expr::add(Expr::var("idx"), Expr::u32(1))),
            ),
            Node::if_then(
                in_range(Expr::var("next1_for_2"), 0x80, 0xBF),
                vec![Node::assign("class", Expr::u32(UTF8_LEAD_2))],
            ),
        ],
    )]
}

fn lead3_validation_body(source: &str, n: u32) -> Vec<Node> {
    vec![Node::if_then(
        Expr::lt(Expr::add(Expr::var("idx"), Expr::u32(2)), Expr::u32(n)),
        vec![
            Node::let_bind(
                "next1_for_3",
                byte_expr(source, Expr::add(Expr::var("idx"), Expr::u32(1))),
            ),
            Node::let_bind(
                "next2_for_3",
                byte_expr(source, Expr::add(Expr::var("idx"), Expr::u32(2))),
            ),
            Node::if_then(
                Expr::and(
                    valid_three_byte_first(Expr::var("byte"), Expr::var("next1_for_3")),
                    in_range(Expr::var("next2_for_3"), 0x80, 0xBF),
                ),
                vec![Node::assign("class", Expr::u32(UTF8_LEAD_3))],
            ),
        ],
    )]
}

fn lead4_validation_body(source: &str, n: u32) -> Vec<Node> {
    vec![Node::if_then(
        Expr::lt(Expr::add(Expr::var("idx"), Expr::u32(3)), Expr::u32(n)),
        vec![
            Node::let_bind(
                "next1_for_4",
                byte_expr(source, Expr::add(Expr::var("idx"), Expr::u32(1))),
            ),
            Node::let_bind(
                "next2_for_4",
                byte_expr(source, Expr::add(Expr::var("idx"), Expr::u32(2))),
            ),
            Node::let_bind(
                "next3_for_4",
                byte_expr(source, Expr::add(Expr::var("idx"), Expr::u32(3))),
            ),
            Node::if_then(
                Expr::and(
                    Expr::and(
                        valid_four_byte_first(Expr::var("byte"), Expr::var("next1_for_4")),
                        in_range(Expr::var("next2_for_4"), 0x80, 0xBF),
                    ),
                    in_range(Expr::var("next3_for_4"), 0x80, 0xBF),
                ),
                vec![Node::assign("class", Expr::u32(UTF8_LEAD_4))],
            ),
        ],
    )]
}

/// Reference oracle: validate and classify each byte the same way the GPU kernel does.
#[must_use]
#[cfg(any(test, feature = "cpu-parity", feature = "text"))]
pub fn reference_utf8_validate(source: &[u8]) -> Vec<u32> {
    (0..source.len())
        .map(|idx| cpu_class_at(source, idx))
        .collect()
}

#[cfg(any(test, feature = "cpu-parity", feature = "text"))]
fn cpu_is_cont(byte: u8) -> bool {
    matches!(byte, 0x80..=0xBF)
}

#[cfg(any(test, feature = "cpu-parity", feature = "text"))]
fn cpu_valid_lead2(source: &[u8], idx: usize) -> bool {
    matches!(source[idx], 0xC2..=0xDF) && source.get(idx + 1).copied().is_some_and(cpu_is_cont)
}

#[cfg(any(test, feature = "cpu-parity", feature = "text"))]
fn cpu_valid_lead3(source: &[u8], idx: usize) -> bool {
    let Some(&b1) = source.get(idx + 1) else {
        return false;
    };
    let Some(&b2) = source.get(idx + 2) else {
        return false;
    };
    let first_ok = match source[idx] {
        0xE0 => matches!(b1, 0xA0..=0xBF),
        0xE1..=0xEC | 0xEE..=0xEF => cpu_is_cont(b1),
        0xED => matches!(b1, 0x80..=0x9F),
        _ => false,
    };
    first_ok && cpu_is_cont(b2)
}

#[cfg(any(test, feature = "cpu-parity", feature = "text"))]
fn cpu_valid_lead4(source: &[u8], idx: usize) -> bool {
    let Some(&b1) = source.get(idx + 1) else {
        return false;
    };
    let Some(&b2) = source.get(idx + 2) else {
        return false;
    };
    let Some(&b3) = source.get(idx + 3) else {
        return false;
    };
    let first_ok = match source[idx] {
        0xF0 => matches!(b1, 0x90..=0xBF),
        0xF1..=0xF3 => cpu_is_cont(b1),
        0xF4 => matches!(b1, 0x80..=0x8F),
        _ => false,
    };
    first_ok && cpu_is_cont(b2) && cpu_is_cont(b3)
}

#[cfg(any(test, feature = "cpu-parity", feature = "text"))]
fn cpu_valid_cont_position(source: &[u8], idx: usize) -> bool {
    idx.checked_sub(1).is_some_and(|lead| {
        cpu_valid_lead2(source, lead)
            || cpu_valid_lead3(source, lead)
            || cpu_valid_lead4(source, lead)
    }) || idx
        .checked_sub(2)
        .is_some_and(|lead| cpu_valid_lead3(source, lead) || cpu_valid_lead4(source, lead))
        || idx
            .checked_sub(3)
            .is_some_and(|lead| cpu_valid_lead4(source, lead))
}

#[cfg(any(test, feature = "cpu-parity", feature = "text"))]
fn cpu_class_at(source: &[u8], idx: usize) -> u32 {
    match source[idx] {
        0x00..=0x7F => UTF8_ASCII,
        0x80..=0xBF if cpu_valid_cont_position(source, idx) => UTF8_CONT,
        0x80..=0xBF => UTF8_INVALID,
        0xC2..=0xDF if cpu_valid_lead2(source, idx) => UTF8_LEAD_2,
        0xE0..=0xEF if cpu_valid_lead3(source, idx) => UTF8_LEAD_3,
        0xF0..=0xF4 if cpu_valid_lead4(source, idx) => UTF8_LEAD_4,
        _ => UTF8_INVALID,
    }
}

#[cfg(feature = "inventory-registry")]
inventory::submit! {
    crate::harness::OpEntry::new(
        OP_ID,
        || utf8_validate("source", "classes", 8),
        Some(|| {
            vec![vec![
                vec![0xC3, 0x00, 0x00, 0x00, 0xA9, 0x00, 0x00, 0x00, 0x41, 0x00, 0x00, 0x00, 0xF0, 0x00, 0x00, 0x00, 0x9F, 0x00, 0x00, 0x00, 0x98, 0x00, 0x00, 0x00, 0x80, 0x00, 0x00, 0x00, 0xC0, 0x00, 0x00, 0x00],
                vec![0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
            ]]
        }),
        Some(|| {
            vec![vec![
                vec![0x01, 0x00, 0x00, 0x00, 0x04, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x03, 0x00, 0x00, 0x00, 0x04, 0x00, 0x00, 0x00, 0x04, 0x00, 0x00, 0x00, 0x04, 0x00, 0x00, 0x00, 0x05, 0x00, 0x00, 0x00],
            ]]
        }),
    )
}

#[cfg(test)]

mod tests {
    use super::*;

    #[test]
    fn program_uses_block_sized_workgroup() {
        let program = utf8_validate("source", "classes", 513);
        assert_eq!(program.workgroup_size(), UTF8_VALIDATE_WORKGROUP_SIZE);
    }

    #[test]
    fn dispatch_grid_packs_byte_lanes_into_blocks() {
        assert_eq!(utf8_validate_dispatch_grid(0), [1, 1, 1]);
        assert_eq!(utf8_validate_dispatch_grid(1), [1, 1, 1]);
        assert_eq!(utf8_validate_dispatch_grid(256), [1, 1, 1]);
        assert_eq!(utf8_validate_dispatch_grid(257), [2, 1, 1]);
        assert_eq!(utf8_validate_dispatch_grid(513), [3, 1, 1]);
    }

    #[test]
    fn reference_ascii() {
        assert_eq!(reference_utf8_validate(b"Hello"), vec![UTF8_ASCII; 5]);
    }

    #[test]
    fn reference_2_byte_sequence() {
        // U+00E9 (é) = 0xC3 0xA9  -  LEAD_2 + CONT
        assert_eq!(
            reference_utf8_validate(&[0xC3, 0xA9]),
            vec![UTF8_LEAD_2, UTF8_CONT]
        );
    }

    #[test]
    fn reference_3_byte_sequence() {
        // U+20AC (€) = 0xE2 0x82 0xAC  -  LEAD_3 + CONT + CONT
        assert_eq!(
            reference_utf8_validate(&[0xE2, 0x82, 0xAC]),
            vec![UTF8_LEAD_3, UTF8_CONT, UTF8_CONT]
        );
    }

    #[test]
    fn reference_4_byte_sequence() {
        // U+1F600 (😀) = 0xF0 0x9F 0x98 0x80  -  LEAD_4 + CONT × 3
        assert_eq!(
            reference_utf8_validate(&[0xF0, 0x9F, 0x98, 0x80]),
            vec![UTF8_LEAD_4, UTF8_CONT, UTF8_CONT, UTF8_CONT]
        );
    }

    #[test]
    fn reference_overlong_lead_invalid() {
        // 0xC0/0xC1 are forbidden lead bytes (overlong 2-byte
        // encodings of ASCII).
        assert_eq!(
            reference_utf8_validate(&[0xC0, 0xC1]),
            vec![UTF8_INVALID, UTF8_INVALID]
        );
    }

    #[test]
    fn reference_out_of_range_lead_invalid() {
        // 0xF8..0xFF would imply 5+ byte sequences  -  banned since RFC 3629.
        assert_eq!(
            reference_utf8_validate(&[0xF8, 0xFC, 0xFF]),
            vec![UTF8_INVALID, UTF8_INVALID, UTF8_INVALID]
        );
    }

    #[test]
    fn reference_rejects_stray_continuation() {
        assert_eq!(reference_utf8_validate(&[0x80]), vec![UTF8_INVALID]);
        assert_eq!(
            reference_utf8_validate(&[b'a', 0xBF]),
            vec![UTF8_ASCII, UTF8_INVALID]
        );
    }

    #[test]
    fn reference_rejects_truncated_sequences() {
        assert_eq!(reference_utf8_validate(&[0xC3]), vec![UTF8_INVALID]);
        assert_eq!(
            reference_utf8_validate(&[0xE2, 0x82]),
            vec![UTF8_INVALID, UTF8_INVALID]
        );
        assert_eq!(
            reference_utf8_validate(&[0xF0, 0x9F, 0x98]),
            vec![UTF8_INVALID, UTF8_INVALID, UTF8_INVALID]
        );
    }

    #[test]
    fn reference_rejects_surrogate_and_overlong_sequences() {
        assert_eq!(
            reference_utf8_validate(&[0xED, 0xA0, 0x80]),
            vec![UTF8_INVALID, UTF8_INVALID, UTF8_INVALID]
        );
        assert_eq!(
            reference_utf8_validate(&[0xE0, 0x80, 0x80]),
            vec![UTF8_INVALID; 3]
        );
        assert_eq!(
            reference_utf8_validate(&[0xF0, 0x80, 0x80, 0x80]),
            vec![UTF8_INVALID; 4]
        );
    }
}
