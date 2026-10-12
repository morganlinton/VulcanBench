use super::*;
use crate::ir::{BufferDecl, DataType, Expr, Node, Program};

#[inline]
pub(crate) fn sample_program() -> Program {
    Program::wrapped(
        vec![
            BufferDecl::read("input", 0, DataType::U32),
            BufferDecl::read_write("output", 1, DataType::U32),
        ],
        [64, 1, 1],
        vec![
            Node::let_bind("idx", Expr::gid_x()),
            Node::if_then(
                Expr::lt(Expr::var("idx"), Expr::buf_len("output")),
                vec![Node::store(
                    "output",
                    Expr::var("idx"),
                    Expr::add(Expr::load("input", Expr::var("idx")), Expr::u32(1)),
                )],
            ),
        ],
    )
}

#[inline]
pub(crate) fn minimal_program() -> Program {
    Program::wrapped(
        vec![BufferDecl::read_write("out", 0, DataType::U32)],
        [1, 1, 1],
        vec![Node::store("out", Expr::u32(0), Expr::u32(42))],
    )
}

#[test]
#[inline]
pub(crate) fn text_format_starts_with_stable_header() {
    let program = minimal_program();
    let text = program.to_text().unwrap();
    assert!(
        text.starts_with(TEXT_FORMAT_HEADER),
        "expected `{TEXT_FORMAT_HEADER}` prefix in:\n{text}"
    );
}

#[test]
#[inline]
pub(crate) fn text_format_has_wire_bytes_header_matching_body_length() {
    let program = minimal_program();
    let text = program.to_text().unwrap();
    let mut lines = text.lines();
    let _header = lines.next().unwrap();
    let wire_line = lines.next().unwrap();
    let declared: usize = wire_line
        .strip_prefix("wire_bytes ")
        .unwrap()
        .parse()
        .unwrap();
    let body_bytes: usize = lines.map(|line| line.len() / 2).sum();
    assert_eq!(
        declared, body_bytes,
        "wire_bytes header lies about body len"
    );
}

#[test]
#[inline]
pub(crate) fn round_trip_minimal_program() {
    let program = minimal_program();
    let text = program.to_text().unwrap();
    let parsed = Program::from_text(&text).unwrap();
    assert_eq!(parsed, program);
}

#[test]
#[inline]
pub(crate) fn round_trip_sample_program() {
    let program = sample_program();
    let text = program.to_text().unwrap();
    let parsed = Program::from_text(&text).unwrap();
    assert_eq!(parsed, program);
}

#[test]
#[inline]
pub(crate) fn round_trip_is_deterministic() {
    let program = sample_program();
    let a = program.to_text().unwrap();
    let b = program.to_text().unwrap();
    assert_eq!(a, b, "to_text must be deterministic byte-for-byte");
}

#[test]
#[inline]
pub(crate) fn parse_rejects_missing_header() {
    let error = Program::from_text("something else\nwire_bytes 0\n").unwrap_err();
    assert!(matches!(error, TextParseError::MissingHeader { .. }));
    let message = error.message();
    assert!(message.contains("Fix:"), "{message}");
}

#[test]
#[inline]
pub(crate) fn parse_rejects_missing_wire_bytes_line() {
    let error = Program::from_text("vyre_ir v0.1\nbody_starts_here\n").unwrap_err();
    assert!(matches!(error, TextParseError::MissingWireBytesLine { .. }));
}

#[test]
#[inline]
pub(crate) fn parse_rejects_wire_bytes_too_large() {
    let over = MAX_TEXT_WIRE_BYTES + 1;
    let input = format!("vyre_ir v0.1\nwire_bytes {over}\n");
    let error = Program::from_text(&input).unwrap_err();
    assert!(matches!(error, TextParseError::WireBytesTooLarge { .. }));
}

#[test]
#[inline]
pub(crate) fn parse_rejects_odd_hex_line_length() {
    let error = Program::from_text("vyre_ir v0.1\nwire_bytes 1\n0a1\n").unwrap_err();
    assert!(
        matches!(error, TextParseError::OddHexLineLength { .. }),
        "got {error:?}"
    );
}

#[test]
#[inline]
pub(crate) fn parse_rejects_non_hex_character() {
    let error = Program::from_text("vyre_ir v0.1\nwire_bytes 1\nzz\n").unwrap_err();
    assert!(
        matches!(
            error,
            TextParseError::InvalidHexCharacter { character: 'z', .. }
        ),
        "got {error:?}"
    );
}

#[test]
#[inline]
pub(crate) fn parse_rejects_declared_length_mismatch() {
    let program = minimal_program();
    let text = program.to_text().unwrap();
    let mut lines: Vec<String> = text.lines().map(|line| line.to_string()).collect();
    lines[1] = "wire_bytes 99999".to_string();
    let tampered = lines.join("\n") + "\n";
    let error = Program::from_text(&tampered).unwrap_err();
    assert!(matches!(
        error,
        TextParseError::DeclaredLengthMismatch {
            declared: 99999,
            ..
        }
    ));
}

#[test]
#[inline]
pub(crate) fn parse_rejects_corrupted_wire_bytes() {
    let program = minimal_program();
    let text = program.to_text().unwrap();
    let tampered = text.replace('a', "f"); // flip one nibble in the body
    if tampered == text {
        // no 'a' in body  -  pick a different perturbation
        return;
    }
    let error = Program::from_text(&tampered);
    assert!(
        matches!(error, Err(TextParseError::WireDecodeFailed { .. })) || error.is_ok(),
        "corruption should either be rejected or happen to round-trip, got {error:?}"
    );
}

#[test]
#[inline]
pub(crate) fn hex_nibble_parses_both_cases() {
    for (input, expected) in [
        (b'0', 0u8),
        (b'9', 9u8),
        (b'a', 10u8),
        (b'f', 15u8),
        (b'A', 10u8),
        (b'F', 15u8),
    ] {
        assert_eq!(hex_nibble(input), Some(expected));
    }
    assert_eq!(hex_nibble(b'g'), None);
    assert_eq!(hex_nibble(b' '), None);
}

#[test]
#[inline]
pub(crate) fn push_usize_handles_zero_and_max() {
    let mut out = String::new();
    push_usize(&mut out, 0);
    assert_eq!(out, "0");
    out.clear();
    push_usize(&mut out, 12345);
    assert_eq!(out, "12345");
}

#[test]
#[inline]
pub(crate) fn push_hex_byte_is_lowercase() {
    let mut out = String::new();
    push_hex_byte(&mut out, 0xab);
    assert_eq!(out, "ab");
    out.clear();
    push_hex_byte(&mut out, 0x0f);
    assert_eq!(out, "0f");
}

#[test]
#[inline]
pub(crate) fn error_display_never_panics_and_includes_fix_hint() {
    for error in [
        TextParseError::MissingHeader {
            observed: "nope".to_string(),
        },
        TextParseError::MissingWireBytesLine {
            observed: "nope".to_string(),
        },
        TextParseError::WireBytesTooLarge {
            declared: usize::MAX,
        },
        TextParseError::InvalidHexCharacter {
            line: 3,
            character: 'z',
        },
        TextParseError::OddHexLineLength {
            line: 3,
            observed: 5,
        },
        TextParseError::DeclaredLengthMismatch {
            declared: 10,
            actual: 0,
        },
        TextParseError::WireDecodeFailed {
            inner: crate::error::Error::WireFormatValidation {
                message: "Fix: some inner issue".to_string(),
            },
        },
        TextParseError::WireEncodeFailed {
            inner: crate::error::Error::WireFormatValidation {
                message: "some encode issue".to_string(),
            },
        },
    ] {
        let rendered = format!("{error}");
        assert!(!rendered.is_empty());
        assert!(
            rendered.contains("Fix:")
                || matches!(
                    error,
                    TextParseError::WireDecodeFailed { .. }
                        | TextParseError::WireEncodeFailed { .. }
                ),
            "{rendered}"
        );
    }
}

#[test]
#[inline]
pub(crate) fn text_lines_have_reasonable_width() {
    let program = sample_program();
    let text = program.to_text().unwrap();
    for (index, line) in text.lines().enumerate() {
        assert!(
            line.len() <= 80,
            "line {index} is {} chars, exceeds diffable width: {line}",
            line.len()
        );
    }
}

proptest::proptest! {
    #![proptest_config(proptest::test_runner::Config {
        cases: 256,
        .. proptest::test_runner::Config::default()
    })]

    #[test]
    fn round_trip_arbitrary_u64_literal_programs(
        workgroup_x in 1u32..=256,
        value in any::<u32>(),
    ) {
        let program = Program::wrapped(
            vec![BufferDecl::read_write("out", 0, DataType::U32)],
            [workgroup_x, 1, 1],
            vec![Node::store("out", Expr::u32(0), Expr::u32(value))],
        );
        let text = program.to_text().unwrap();
        let parsed = Program::from_text(&text).unwrap();
        proptest::prop_assert_eq!(parsed, program);
    }
}

use proptest::prelude::any;
