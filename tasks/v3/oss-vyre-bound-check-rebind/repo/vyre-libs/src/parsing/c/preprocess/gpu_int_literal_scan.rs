//! GPU integer-literal scanner.
//!
//! Phase 17b.2: per starting byte position in `source`, scan a C
//! integer literal (hex `0x`/`0X`, binary `0b`/`0B`, octal leading-`0`,
//! else decimal) and emit `(value, bytes_consumed)`. Suffix tolerance:
//! any combination of `u`/`U`/`l`/`L` is consumed and ignored.
//!
//! `value` is computed as `u32` with saturating arithmetic. Literals
//! larger than `u32::MAX` consume their full scanned digit run and emit
//! `u32::MAX`, matching the preprocessor's conservative truthiness needs
//! without wrapping back to a small value.
//!
//! ## Wire layout
//!
//! Inputs:
//!   - `source` (U8)
//!   - `start_pos` (U32, single element)  -  byte offset to start scanning.
//!
//! Outputs:
//!   - `value_out` (U32, single element).
//!   - `bytes_consumed_out` (U32, single element). `0` indicates "not
//!     an integer literal at this position"  -  the caller treats this
//!     the same way the CPU `consume_integer` returns `None`.

use super::gpu_source_bytes::{
    literal_scan_common_buffers, literal_scan_program, packed_source_byte_len_expr,
    safe_load_source_byte_expr,
};
use vyre::ir::{Expr, Node, Program};

/// Canonical op id.
pub const OP_ID: &str = "vyre-libs::parsing::c::preprocess::gpu_int_literal_scan";

/// Canonical binding indices.
pub const BINDING_SOURCE: u32 = 0;
/// Canonical binding for the input start position.
pub const BINDING_START_POS: u32 = 1;
/// Canonical binding for the output literal value.
pub const BINDING_VALUE_OUT: u32 = 2;
/// Canonical binding for the output bytes-consumed count.
pub const BINDING_BYTES_CONSUMED_OUT: u32 = 3;

/// Maximum digits scanned per literal. Any longer literal saturates.
/// 32 is enough for the longest meaningful u32 representation in any
/// supported radix (binary u32 is at most 32 digits; hex u32 is 8;
/// decimal u32 is 10; octal u32 is 11).
pub const MAX_DIGITS: u32 = 32;

/// Maximum suffix bytes consumed (`u`/`U`/`l`/`L`/`z`/`Z`/`wb`/`WB`).
pub const MAX_SUFFIX: u32 = 4;

/// Build the 17b.2 integer-literal scanner `Program`.
///
/// `source_len` is retained for source compatibility. The source byte
/// bound is read from the runtime-sized `source` buffer so one resident
/// scanner program serves every translation unit size.
#[must_use]
pub fn gpu_int_literal_scan(source_len: u32) -> Program {
    let _ = source_len;
    let source_byte_len = packed_source_byte_len_expr();
    let safe_load =
        |addr: Expr| -> Expr { safe_load_source_byte_expr(addr, source_byte_len.clone()) };

    let body: Vec<Node> = vec![Node::if_then(
        Expr::eq(Expr::InvocationId { axis: 0 }, Expr::u32(0)),
        vec![
            Node::let_bind("start", Expr::load("start_pos", Expr::u32(0))),
            // Determine radix and digits-start.
            Node::let_bind("b0", safe_load(Expr::var("start"))),
            Node::let_bind("b1", safe_load(Expr::add(Expr::var("start"), Expr::u32(1)))),
            Node::let_bind(
                "is_hex",
                Expr::select(
                    Expr::and(
                        Expr::eq(Expr::var("b0"), Expr::u32(b'0' as u32)),
                        Expr::or(
                            Expr::eq(Expr::var("b1"), Expr::u32(b'x' as u32)),
                            Expr::eq(Expr::var("b1"), Expr::u32(b'X' as u32)),
                        ),
                    ),
                    Expr::u32(1),
                    Expr::u32(0),
                ),
            ),
            Node::let_bind(
                "is_bin",
                Expr::select(
                    Expr::and(
                        Expr::eq(Expr::var("b0"), Expr::u32(b'0' as u32)),
                        Expr::or(
                            Expr::eq(Expr::var("b1"), Expr::u32(b'b' as u32)),
                            Expr::eq(Expr::var("b1"), Expr::u32(b'B' as u32)),
                        ),
                    ),
                    Expr::u32(1),
                    Expr::u32(0),
                ),
            ),
            Node::let_bind(
                "is_oct",
                Expr::select(
                    Expr::and(
                        Expr::eq(Expr::var("b0"), Expr::u32(b'0' as u32)),
                        Expr::and(
                            Expr::eq(Expr::var("is_hex"), Expr::u32(0)),
                            Expr::eq(Expr::var("is_bin"), Expr::u32(0)),
                        ),
                    ),
                    Expr::u32(1),
                    Expr::u32(0),
                ),
            ),
            Node::let_bind(
                "radix",
                Expr::select(
                    Expr::eq(Expr::var("is_hex"), Expr::u32(1)),
                    Expr::u32(16),
                    Expr::select(
                        Expr::eq(Expr::var("is_bin"), Expr::u32(1)),
                        Expr::u32(2),
                        Expr::select(
                            Expr::eq(Expr::var("is_oct"), Expr::u32(1)),
                            Expr::u32(8),
                            Expr::u32(10),
                        ),
                    ),
                ),
            ),
            // Hex/bin advance start by 2; oct/dec advance by 0.
            Node::let_bind(
                "digit_start_offset",
                Expr::select(
                    Expr::or(
                        Expr::eq(Expr::var("is_hex"), Expr::u32(1)),
                        Expr::eq(Expr::var("is_bin"), Expr::u32(1)),
                    ),
                    Expr::u32(2),
                    Expr::u32(0),
                ),
            ),
            Node::let_bind(
                "digits_start",
                Expr::add(Expr::var("start"), Expr::var("digit_start_offset")),
            ),
            Node::let_bind("idx", Expr::var("digits_start")),
            Node::let_bind("value", Expr::u32(0)),
            Node::let_bind("done_digits", Expr::u32(0)),
            Node::loop_for(
                "k",
                Expr::u32(0),
                Expr::u32(MAX_DIGITS),
                vec![Node::if_then(
                    Expr::eq(Expr::var("done_digits"), Expr::u32(0)),
                    vec![
                        Node::let_bind("byte", safe_load(Expr::var("idx"))),
                        Node::let_bind(
                            "next_byte",
                            safe_load(Expr::add(Expr::var("idx"), Expr::u32(1))),
                        ),
                        // Compute digit value.
                        Node::let_bind(
                            "is_dec_digit",
                            Expr::select(
                                Expr::and(
                                    Expr::ge(Expr::var("byte"), Expr::u32(b'0' as u32)),
                                    Expr::le(Expr::var("byte"), Expr::u32(b'9' as u32)),
                                ),
                                Expr::u32(1),
                                Expr::u32(0),
                            ),
                        ),
                        Node::let_bind(
                            "is_hex_lower",
                            Expr::select(
                                Expr::and(
                                    Expr::ge(Expr::var("byte"), Expr::u32(b'a' as u32)),
                                    Expr::le(Expr::var("byte"), Expr::u32(b'f' as u32)),
                                ),
                                Expr::u32(1),
                                Expr::u32(0),
                            ),
                        ),
                        Node::let_bind(
                            "is_hex_upper",
                            Expr::select(
                                Expr::and(
                                    Expr::ge(Expr::var("byte"), Expr::u32(b'A' as u32)),
                                    Expr::le(Expr::var("byte"), Expr::u32(b'F' as u32)),
                                ),
                                Expr::u32(1),
                                Expr::u32(0),
                            ),
                        ),
                        Node::let_bind(
                            "raw_digit",
                            Expr::select(
                                Expr::eq(Expr::var("is_dec_digit"), Expr::u32(1)),
                                Expr::sub(Expr::var("byte"), Expr::u32(b'0' as u32)),
                                Expr::select(
                                    Expr::eq(Expr::var("is_hex_lower"), Expr::u32(1)),
                                    Expr::add(
                                        Expr::sub(Expr::var("byte"), Expr::u32(b'a' as u32)),
                                        Expr::u32(10),
                                    ),
                                    Expr::select(
                                        Expr::eq(Expr::var("is_hex_upper"), Expr::u32(1)),
                                        Expr::add(
                                            Expr::sub(Expr::var("byte"), Expr::u32(b'A' as u32)),
                                            Expr::u32(10),
                                        ),
                                        // Sentinel: 99 is greater than
                                        // any valid digit in any radix
                                        // we support, so the radix-bounds
                                        // check below rejects this.
                                        Expr::u32(99),
                                    ),
                                ),
                            ),
                        ),
                        Node::let_bind(
                            "digit_in_range",
                            Expr::select(
                                Expr::lt(Expr::var("raw_digit"), Expr::var("radix")),
                                Expr::u32(1),
                                Expr::u32(0),
                            ),
                        ),
                        Node::let_bind(
                            "next_is_dec_digit",
                            Expr::select(
                                Expr::and(
                                    Expr::ge(Expr::var("next_byte"), Expr::u32(b'0' as u32)),
                                    Expr::le(Expr::var("next_byte"), Expr::u32(b'9' as u32)),
                                ),
                                Expr::u32(1),
                                Expr::u32(0),
                            ),
                        ),
                        Node::let_bind(
                            "next_is_hex_lower",
                            Expr::select(
                                Expr::and(
                                    Expr::ge(Expr::var("next_byte"), Expr::u32(b'a' as u32)),
                                    Expr::le(Expr::var("next_byte"), Expr::u32(b'f' as u32)),
                                ),
                                Expr::u32(1),
                                Expr::u32(0),
                            ),
                        ),
                        Node::let_bind(
                            "next_is_hex_upper",
                            Expr::select(
                                Expr::and(
                                    Expr::ge(Expr::var("next_byte"), Expr::u32(b'A' as u32)),
                                    Expr::le(Expr::var("next_byte"), Expr::u32(b'F' as u32)),
                                ),
                                Expr::u32(1),
                                Expr::u32(0),
                            ),
                        ),
                        Node::let_bind(
                            "next_raw_digit",
                            Expr::select(
                                Expr::eq(Expr::var("next_is_dec_digit"), Expr::u32(1)),
                                Expr::sub(Expr::var("next_byte"), Expr::u32(b'0' as u32)),
                                Expr::select(
                                    Expr::eq(Expr::var("next_is_hex_lower"), Expr::u32(1)),
                                    Expr::add(
                                        Expr::sub(Expr::var("next_byte"), Expr::u32(b'a' as u32)),
                                        Expr::u32(10),
                                    ),
                                    Expr::select(
                                        Expr::eq(Expr::var("next_is_hex_upper"), Expr::u32(1)),
                                        Expr::add(
                                            Expr::sub(
                                                Expr::var("next_byte"),
                                                Expr::u32(b'A' as u32),
                                            ),
                                            Expr::u32(10),
                                        ),
                                        Expr::u32(99),
                                    ),
                                ),
                            ),
                        ),
                        Node::let_bind(
                            "separator_in_range",
                            Expr::select(
                                Expr::and(
                                    Expr::eq(Expr::var("byte"), Expr::u32(b'\'' as u32)),
                                    Expr::lt(Expr::var("next_raw_digit"), Expr::var("radix")),
                                ),
                                Expr::u32(1),
                                Expr::u32(0),
                            ),
                        ),
                        Node::if_then_else(
                            Expr::or(
                                Expr::eq(Expr::var("digit_in_range"), Expr::u32(1)),
                                Expr::eq(Expr::var("separator_in_range"), Expr::u32(1)),
                            ),
                            vec![
                                // value = value * radix + raw_digit (saturating).
                                Node::let_bind(
                                    "sat_limit",
                                    Expr::div(
                                        Expr::sub(Expr::u32(u32::MAX), Expr::var("raw_digit")),
                                        Expr::var("radix"),
                                    ),
                                ),
                                Node::let_bind(
                                    "would_overflow",
                                    Expr::select(
                                        Expr::gt(Expr::var("value"), Expr::var("sat_limit")),
                                        Expr::u32(1),
                                        Expr::u32(0),
                                    ),
                                ),
                                Node::assign(
                                    "value",
                                    Expr::select(
                                        Expr::eq(Expr::var("separator_in_range"), Expr::u32(1)),
                                        Expr::var("value"),
                                        Expr::select(
                                            Expr::eq(Expr::var("would_overflow"), Expr::u32(1)),
                                            Expr::u32(u32::MAX),
                                            Expr::add(
                                                Expr::mul(Expr::var("value"), Expr::var("radix")),
                                                Expr::var("raw_digit"),
                                            ),
                                        ),
                                    ),
                                ),
                                Node::assign("idx", Expr::add(Expr::var("idx"), Expr::u32(1))),
                            ],
                            vec![Node::assign("done_digits", Expr::u32(1))],
                        ),
                    ],
                )],
            ),
            Node::let_bind(
                "saw_digits",
                Expr::select(
                    Expr::gt(Expr::var("idx"), Expr::var("digits_start")),
                    Expr::u32(1),
                    Expr::u32(0),
                ),
            ),
            // If no digits at all → not a literal. Bytes consumed = 0.
            // Otherwise consume up to 4 trailing u/U/l/L/z/Z/wb/WB suffix bytes.
            Node::let_bind("done_suffix", Expr::u32(0)),
            Node::if_then(
                Expr::eq(Expr::var("saw_digits"), Expr::u32(1)),
                vec![Node::loop_for(
                    "s",
                    Expr::u32(0),
                    Expr::u32(MAX_SUFFIX),
                    vec![Node::if_then(
                        Expr::eq(Expr::var("done_suffix"), Expr::u32(0)),
                        vec![
                            Node::let_bind("sb", safe_load(Expr::var("idx"))),
                            Node::let_bind(
                                "sb1",
                                safe_load(Expr::add(Expr::var("idx"), Expr::u32(1))),
                            ),
                            Node::let_bind(
                                "is_single_suffix",
                                Expr::select(
                                    Expr::or(
                                        Expr::or(
                                            Expr::eq(Expr::var("sb"), Expr::u32(b'u' as u32)),
                                            Expr::eq(Expr::var("sb"), Expr::u32(b'U' as u32)),
                                        ),
                                        Expr::or(
                                            Expr::eq(Expr::var("sb"), Expr::u32(b'l' as u32)),
                                            Expr::eq(Expr::var("sb"), Expr::u32(b'L' as u32)),
                                        ),
                                    ),
                                    Expr::u32(1),
                                    Expr::u32(0),
                                ),
                            ),
                            Node::let_bind(
                                "is_z_suffix",
                                Expr::select(
                                    Expr::or(
                                        Expr::eq(Expr::var("sb"), Expr::u32(b'z' as u32)),
                                        Expr::eq(Expr::var("sb"), Expr::u32(b'Z' as u32)),
                                    ),
                                    Expr::u32(1),
                                    Expr::u32(0),
                                ),
                            ),
                            Node::let_bind(
                                "is_wb_suffix",
                                Expr::select(
                                    Expr::and(
                                        Expr::or(
                                            Expr::eq(Expr::var("sb"), Expr::u32(b'w' as u32)),
                                            Expr::eq(Expr::var("sb"), Expr::u32(b'W' as u32)),
                                        ),
                                        Expr::or(
                                            Expr::eq(Expr::var("sb1"), Expr::u32(b'b' as u32)),
                                            Expr::eq(Expr::var("sb1"), Expr::u32(b'B' as u32)),
                                        ),
                                    ),
                                    Expr::u32(1),
                                    Expr::u32(0),
                                ),
                            ),
                            Node::if_then_else(
                                Expr::or(
                                    Expr::or(
                                        Expr::eq(Expr::var("is_single_suffix"), Expr::u32(1)),
                                        Expr::eq(Expr::var("is_z_suffix"), Expr::u32(1)),
                                    ),
                                    Expr::eq(Expr::var("is_wb_suffix"), Expr::u32(1)),
                                ),
                                vec![Node::assign(
                                    "idx",
                                    Expr::add(
                                        Expr::var("idx"),
                                        Expr::select(
                                            Expr::eq(Expr::var("is_wb_suffix"), Expr::u32(1)),
                                            Expr::u32(2),
                                            Expr::u32(1),
                                        ),
                                    ),
                                )],
                                vec![Node::assign("done_suffix", Expr::u32(1))],
                            ),
                        ],
                    )],
                )],
            ),
            // Compute consumed bytes. If saw_digits is false, emit 0.
            Node::let_bind(
                "consumed",
                Expr::select(
                    Expr::eq(Expr::var("saw_digits"), Expr::u32(1)),
                    Expr::sub(Expr::var("idx"), Expr::var("start")),
                    Expr::u32(0),
                ),
            ),
            Node::let_bind(
                "value_final",
                Expr::select(
                    Expr::eq(Expr::var("saw_digits"), Expr::u32(1)),
                    Expr::var("value"),
                    Expr::u32(0),
                ),
            ),
            Node::store("value_out", Expr::u32(0), Expr::var("value_final")),
            Node::store("bytes_consumed_out", Expr::u32(0), Expr::var("consumed")),
        ],
    )];

    literal_scan_program(
        literal_scan_common_buffers(
            BINDING_SOURCE,
            BINDING_START_POS,
            BINDING_VALUE_OUT,
            BINDING_BYTES_CONSUMED_OUT,
        ),
        body,
        OP_ID,
    )
}

#[cfg(test)]

mod tests {
    use super::*;
    use vyre_reference::value::Value;

    fn run_literal_scan(source: &[u8], start: u32) -> (u32, u32) {
        let program = gpu_int_literal_scan(source.len() as u32);
        let mut packed_source = Vec::with_capacity(source.len().div_ceil(4).max(1) * 4);
        for chunk in source.chunks(4) {
            let mut word = [0u8; 4];
            word[..chunk.len()].copy_from_slice(chunk);
            packed_source.extend_from_slice(&word);
        }
        if packed_source.is_empty() {
            packed_source.extend_from_slice(&0u32.to_le_bytes());
        }
        let inputs = vec![
            Value::Bytes(packed_source.into()),
            Value::Bytes(start.to_le_bytes().to_vec().into()),
            Value::Bytes(vec![0u8; 4].into()),
            Value::Bytes(vec![0u8; 4].into()),
        ];
        let outputs = vyre_reference::reference_eval(&program, &inputs)
            .expect("Fix: GPU integer literal scanner reference evaluation must run.");
        let value = outputs[0].to_bytes();
        let consumed = outputs[1].to_bytes();
        (
            vyre_primitives::wire::read_u32_le_word(&value, 0, "int-literal value")
                .expect("Fix: integer literal value output must contain one u32."),
            vyre_primitives::wire::read_u32_le_word(&consumed, 0, "int-literal consumed")
                .expect("Fix: integer literal consumed output must contain one u32."),
        )
    }

    #[test]
    fn op_id_is_canonical_and_stable() {
        assert_eq!(
            OP_ID,
            "vyre-libs::parsing::c::preprocess::gpu_int_literal_scan"
        );
    }

    #[test]
    fn binding_indices_are_canonical_and_stable() {
        assert_eq!(BINDING_SOURCE, 0);
        assert_eq!(BINDING_START_POS, 1);
        assert_eq!(BINDING_VALUE_OUT, 2);
        assert_eq!(BINDING_BYTES_CONSUMED_OUT, 3);
    }

    #[test]
    fn build_program_returns_well_formed_program() {
        let p = gpu_int_literal_scan(64);
        assert_eq!(p.buffers().len(), 4);
        assert_eq!(p.workgroup_size(), [256, 1, 1]);
    }

    #[test]
    fn source_buffer_is_runtime_sized_not_source_length_specialized() {
        let p = gpu_int_literal_scan(64);
        let source = p
            .buffers()
            .iter()
            .find(|buffer| buffer.name() == "source")
            .expect("Fix: source buffer must exist");
        assert_eq!(
            source.count(),
            0,
            "source must be runtime-sized so one scanner program serves all source lengths"
        );
    }

    #[test]
    fn reference_eval_consumes_digit_separators_and_modern_suffixes() {
        assert_eq!(run_literal_scan(b"1'024ULL", 0), (1024, 8));
        assert_eq!(run_literal_scan(b"xx0xFF'00z", 2), (65_280, 8));
        assert_eq!(run_literal_scan(b"0b1010'0101WB", 0), (165, 13));
    }
}
