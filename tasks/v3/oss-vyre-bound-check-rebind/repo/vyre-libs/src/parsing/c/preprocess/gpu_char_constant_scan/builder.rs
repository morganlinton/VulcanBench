use super::super::gpu_source_bytes::{
    literal_scan_common_buffers, literal_scan_program, literal_scan_status_output,
    packed_source_byte_len_expr, safe_load_source_byte_expr,
};
use super::*;

/// Build the 17b.3a char-constant scanner `Program`.
#[must_use]
pub fn gpu_char_constant_scan(source_len: u32) -> Program {
    let _ = source_len;
    let source_byte_len = packed_source_byte_len_expr();
    let safe_load =
        |addr: Expr| -> Expr { safe_load_source_byte_expr(addr, source_byte_len.clone()) };

    let body: Vec<Node> = vec![Node::if_then(
        Expr::eq(Expr::InvocationId { axis: 0 }, Expr::u32(0)),
        vec![
            Node::let_bind("start", Expr::load("start_pos", Expr::u32(0))),
            Node::let_bind("idx", Expr::var("start")),
            // Detect prefix: u8 (2 bytes), L/u/U (1 byte).
            Node::let_bind("p0", safe_load(Expr::var("idx"))),
            Node::let_bind("p1", safe_load(Expr::add(Expr::var("idx"), Expr::u32(1)))),
            Node::let_bind(
                "is_u8_prefix",
                Expr::select(
                    Expr::and(
                        Expr::eq(Expr::var("p0"), Expr::u32(b'u' as u32)),
                        Expr::eq(Expr::var("p1"), Expr::u32(b'8' as u32)),
                    ),
                    Expr::u32(1),
                    Expr::u32(0),
                ),
            ),
            Node::let_bind(
                "is_single_prefix",
                Expr::select(
                    Expr::and(
                        Expr::eq(Expr::var("is_u8_prefix"), Expr::u32(0)),
                        Expr::or(
                            Expr::eq(Expr::var("p0"), Expr::u32(b'L' as u32)),
                            Expr::or(
                                Expr::eq(Expr::var("p0"), Expr::u32(b'u' as u32)),
                                Expr::eq(Expr::var("p0"), Expr::u32(b'U' as u32)),
                            ),
                        ),
                    ),
                    Expr::u32(1),
                    Expr::u32(0),
                ),
            ),
            // Tentatively advance past the prefix.
            Node::if_then(
                Expr::eq(Expr::var("is_u8_prefix"), Expr::u32(1)),
                vec![Node::assign(
                    "idx",
                    Expr::add(Expr::var("idx"), Expr::u32(2)),
                )],
            ),
            Node::if_then(
                Expr::eq(Expr::var("is_single_prefix"), Expr::u32(1)),
                vec![Node::assign(
                    "idx",
                    Expr::add(Expr::var("idx"), Expr::u32(1)),
                )],
            ),
            // Expect opening `'`.
            Node::let_bind("opener", safe_load(Expr::var("idx"))),
            Node::let_bind(
                "opened",
                Expr::select(
                    Expr::eq(Expr::var("opener"), Expr::u32(b'\'' as u32)),
                    Expr::u32(1),
                    Expr::u32(0),
                ),
            ),
            // If we didn't find an opener: this isn't a char
            // constant; return ok=0, consumed=0. Reset idx so the
            // post-loop math doesn't double-count the prefix skip.
            Node::let_bind("ok_so_far", Expr::var("opened")),
            Node::let_bind("value", Expr::u32(0)),
            Node::let_bind("saw_char", Expr::u32(0)),
            Node::if_then(
                Expr::eq(Expr::var("opened"), Expr::u32(1)),
                vec![
                    // Step past `'`.
                    Node::assign("idx", Expr::add(Expr::var("idx"), Expr::u32(1))),
                    Node::let_bind("done_content", Expr::u32(0)),
                    Node::loop_for(
                        "k",
                        Expr::u32(0),
                        Expr::u32(MAX_CONTENT_BYTES),
                        vec![Node::if_then(
                            Expr::eq(Expr::var("done_content"), Expr::u32(0)),
                            vec![
                                Node::let_bind("ch", safe_load(Expr::var("idx"))),
                                // Closing quote → break.
                                Node::if_then(
                                    Expr::eq(Expr::var("ch"), Expr::u32(b'\'' as u32)),
                                    vec![Node::assign("done_content", Expr::u32(1))],
                                ),
                                // Embedded newline → error.
                                Node::if_then(
                                    Expr::and(
                                        Expr::eq(Expr::var("done_content"), Expr::u32(0)),
                                        Expr::or(
                                            Expr::eq(Expr::var("ch"), Expr::u32(b'\n' as u32)),
                                            Expr::eq(Expr::var("ch"), Expr::u32(b'\r' as u32)),
                                        ),
                                    ),
                                    vec![
                                        Node::assign("ok_so_far", Expr::u32(0)),
                                        Node::assign("done_content", Expr::u32(1)),
                                    ],
                                ),
                                // Truncated buffer → error.
                                Node::if_then(
                                    Expr::and(
                                        Expr::eq(Expr::var("done_content"), Expr::u32(0)),
                                        Expr::ge(Expr::var("idx"), source_byte_len.clone()),
                                    ),
                                    vec![
                                        Node::assign("ok_so_far", Expr::u32(0)),
                                        Node::assign("done_content", Expr::u32(1)),
                                    ],
                                ),
                                // Otherwise: regular char or escape.
                                Node::if_then(
                                    Expr::eq(Expr::var("done_content"), Expr::u32(0)),
                                    vec![
                                        Node::let_bind(
                                            "is_escape",
                                            Expr::select(
                                                Expr::eq(Expr::var("ch"), Expr::u32(b'\\' as u32)),
                                                Expr::u32(1),
                                                Expr::u32(0),
                                            ),
                                        ),
                                        Node::if_then_else(
                                            Expr::eq(Expr::var("is_escape"), Expr::u32(1)),
                                            vec![
                                                // Read the byte after `\`.
                                                Node::let_bind(
                                                    "esc",
                                                    safe_load(Expr::add(
                                                        Expr::var("idx"),
                                                        Expr::u32(1),
                                                    )),
                                                ),
                                                // Categorize the escape: numeric kinds need
                                                // dedicated greedy scanners; everything else
                                                // decodes via the simple-escape lookup.
                                                Node::let_bind(
                                                    "is_octal_start",
                                                    Expr::select(
                                                        Expr::and(
                                                            Expr::ge(
                                                                Expr::var("esc"),
                                                                Expr::u32(b'0' as u32),
                                                            ),
                                                            Expr::le(
                                                                Expr::var("esc"),
                                                                Expr::u32(b'7' as u32),
                                                            ),
                                                        ),
                                                        Expr::u32(1),
                                                        Expr::u32(0),
                                                    ),
                                                ),
                                                Node::let_bind(
                                                    "is_hex_start",
                                                    Expr::select(
                                                        Expr::eq(
                                                            Expr::var("esc"),
                                                            Expr::u32(b'x' as u32),
                                                        ),
                                                        Expr::u32(1),
                                                        Expr::u32(0),
                                                    ),
                                                ),
                                                Node::let_bind(
                                                    "is_ucn4_start",
                                                    Expr::select(
                                                        Expr::eq(
                                                            Expr::var("esc"),
                                                            Expr::u32(b'u' as u32),
                                                        ),
                                                        Expr::u32(1),
                                                        Expr::u32(0),
                                                    ),
                                                ),
                                                Node::let_bind(
                                                    "is_ucn8_start",
                                                    Expr::select(
                                                        Expr::eq(
                                                            Expr::var("esc"),
                                                            Expr::u32(b'U' as u32),
                                                        ),
                                                        Expr::u32(1),
                                                        Expr::u32(0),
                                                    ),
                                                ),
                                                // Simple-escape lookup. Numeric-start
                                                // categories take precedence below; this
                                                // value covers named escapes and identity
                                                // escapes.
                                                Node::let_bind(
                                                    "simple_val",
                                                    Expr::select(
                                                        Expr::eq(
                                                            Expr::var("esc"),
                                                            Expr::u32(b'n' as u32),
                                                        ),
                                                        Expr::u32(b'\n' as u32),
                                                        Expr::select(
                                                            Expr::eq(
                                                                Expr::var("esc"),
                                                                Expr::u32(b't' as u32),
                                                            ),
                                                            Expr::u32(b'\t' as u32),
                                                            Expr::select(
                                                                Expr::eq(
                                                                    Expr::var("esc"),
                                                                    Expr::u32(b'r' as u32),
                                                                ),
                                                                Expr::u32(b'\r' as u32),
                                                                Expr::select(
                                                                    Expr::eq(
                                                                        Expr::var("esc"),
                                                                        Expr::u32(b'a' as u32),
                                                                    ),
                                                                    Expr::u32(7),
                                                                    Expr::select(
                                                                        Expr::eq(
                                                                            Expr::var("esc"),
                                                                            Expr::u32(b'b' as u32),
                                                                        ),
                                                                        Expr::u32(8),
                                                                        Expr::select(
                                                                            Expr::eq(
                                                                                Expr::var("esc"),
                                                                                Expr::u32(
                                                                                    b'f' as u32,
                                                                                ),
                                                                            ),
                                                                            Expr::u32(12),
                                                                            Expr::select(
                                                                                Expr::eq(
                                                                                    Expr::var(
                                                                                        "esc",
                                                                                    ),
                                                                                    Expr::u32(
                                                                                        b'v' as u32,
                                                                                    ),
                                                                                ),
                                                                                Expr::u32(11),
                                                                                // Default: the literal byte after `\`
                                                                                // (covers ' " ? \\ and `\<other>`).
                                                                                Expr::var("esc"),
                                                                            ),
                                                                        ),
                                                                    ),
                                                                ),
                                                            ),
                                                        ),
                                                    ),
                                                ),
                                                // ---- Octal: \0..\7, up to 3 digits ----
                                                // Read up to 3 octal digits starting at
                                                // idx+1. octal_value accumulates; octal_len
                                                // counts digits actually consumed.
                                                Node::let_bind("octal_value", Expr::u32(0)),
                                                Node::let_bind("octal_len", Expr::u32(0)),
                                                Node::let_bind("octal_done", Expr::u32(0)),
                                                Node::loop_for(
                                                    "od",
                                                    Expr::u32(0),
                                                    Expr::u32(3),
                                                    vec![Node::if_then(
                                                        Expr::and(
                                                            Expr::eq(
                                                                Expr::var("is_octal_start"),
                                                                Expr::u32(1),
                                                            ),
                                                            Expr::eq(
                                                                Expr::var("octal_done"),
                                                                Expr::u32(0),
                                                            ),
                                                        ),
                                                        vec![
                                                            Node::let_bind(
                                                                "ob",
                                                                safe_load(Expr::add(
                                                                    Expr::add(
                                                                        Expr::var("idx"),
                                                                        Expr::u32(1),
                                                                    ),
                                                                    Expr::var("od"),
                                                                )),
                                                            ),
                                                            Node::let_bind(
                                                                "is_oct",
                                                                Expr::select(
                                                                    Expr::and(
                                                                        Expr::ge(
                                                                            Expr::var("ob"),
                                                                            Expr::u32(b'0' as u32),
                                                                        ),
                                                                        Expr::le(
                                                                            Expr::var("ob"),
                                                                            Expr::u32(b'7' as u32),
                                                                        ),
                                                                    ),
                                                                    Expr::u32(1),
                                                                    Expr::u32(0),
                                                                ),
                                                            ),
                                                            Node::if_then_else(
                                                                Expr::eq(
                                                                    Expr::var("is_oct"),
                                                                    Expr::u32(1),
                                                                ),
                                                                vec![
                                                                    Node::assign(
                                                                        "octal_value",
                                                                        Expr::add(
                                                                            Expr::mul(
                                                                                Expr::var(
                                                                                    "octal_value",
                                                                                ),
                                                                                Expr::u32(8),
                                                                            ),
                                                                            Expr::sub(
                                                                                Expr::var("ob"),
                                                                                Expr::u32(
                                                                                    b'0' as u32,
                                                                                ),
                                                                            ),
                                                                        ),
                                                                    ),
                                                                    Node::assign(
                                                                        "octal_len",
                                                                        Expr::add(
                                                                            Expr::var("octal_len"),
                                                                            Expr::u32(1),
                                                                        ),
                                                                    ),
                                                                ],
                                                                vec![Node::assign(
                                                                    "octal_done",
                                                                    Expr::u32(1),
                                                                )],
                                                            ),
                                                        ],
                                                    )],
                                                ),
                                                // ---- Hex: \xH+, greedy ----
                                                Node::let_bind("hex_value", Expr::u32(0)),
                                                Node::let_bind("hex_len", Expr::u32(0)),
                                                Node::let_bind("hex_done", Expr::u32(0)),
                                                Node::loop_for(
                                                    "hd",
                                                    Expr::u32(0),
                                                    Expr::u32(8),
                                                    vec![Node::if_then(
                                                        Expr::and(
                                                            Expr::eq(
                                                                Expr::var("is_hex_start"),
                                                                Expr::u32(1),
                                                            ),
                                                            Expr::eq(
                                                                Expr::var("hex_done"),
                                                                Expr::u32(0),
                                                            ),
                                                        ),
                                                        vec![
                                                            Node::let_bind(
                                                                "hb",
                                                                safe_load(Expr::add(
                                                                    Expr::add(
                                                                        Expr::var("idx"),
                                                                        Expr::u32(2),
                                                                    ),
                                                                    Expr::var("hd"),
                                                                )),
                                                            ),
                                                            Node::let_bind(
                                                                "hb_dec",
                                                                Expr::select(
                                                                    Expr::and(
                                                                        Expr::ge(
                                                                            Expr::var("hb"),
                                                                            Expr::u32(b'0' as u32),
                                                                        ),
                                                                        Expr::le(
                                                                            Expr::var("hb"),
                                                                            Expr::u32(b'9' as u32),
                                                                        ),
                                                                    ),
                                                                    Expr::u32(1),
                                                                    Expr::u32(0),
                                                                ),
                                                            ),
                                                            Node::let_bind(
                                                                "hb_lc",
                                                                Expr::select(
                                                                    Expr::and(
                                                                        Expr::ge(
                                                                            Expr::var("hb"),
                                                                            Expr::u32(b'a' as u32),
                                                                        ),
                                                                        Expr::le(
                                                                            Expr::var("hb"),
                                                                            Expr::u32(b'f' as u32),
                                                                        ),
                                                                    ),
                                                                    Expr::u32(1),
                                                                    Expr::u32(0),
                                                                ),
                                                            ),
                                                            Node::let_bind(
                                                                "hb_uc",
                                                                Expr::select(
                                                                    Expr::and(
                                                                        Expr::ge(
                                                                            Expr::var("hb"),
                                                                            Expr::u32(b'A' as u32),
                                                                        ),
                                                                        Expr::le(
                                                                            Expr::var("hb"),
                                                                            Expr::u32(b'F' as u32),
                                                                        ),
                                                                    ),
                                                                    Expr::u32(1),
                                                                    Expr::u32(0),
                                                                ),
                                                            ),
                                                            Node::let_bind(
                                                                "hb_val",
                                                                Expr::select(
                                                                    Expr::eq(
                                                                        Expr::var("hb_dec"),
                                                                        Expr::u32(1),
                                                                    ),
                                                                    Expr::sub(
                                                                        Expr::var("hb"),
                                                                        Expr::u32(b'0' as u32),
                                                                    ),
                                                                    Expr::select(
                                                                        Expr::eq(
                                                                            Expr::var("hb_lc"),
                                                                            Expr::u32(1),
                                                                        ),
                                                                        Expr::add(
                                                                            Expr::sub(
                                                                                Expr::var("hb"),
                                                                                Expr::u32(
                                                                                    b'a' as u32,
                                                                                ),
                                                                            ),
                                                                            Expr::u32(10),
                                                                        ),
                                                                        Expr::select(
                                                                            Expr::eq(
                                                                                Expr::var("hb_uc"),
                                                                                Expr::u32(1),
                                                                            ),
                                                                            Expr::add(
                                                                                Expr::sub(
                                                                                    Expr::var("hb"),
                                                                                    Expr::u32(
                                                                                        b'A' as u32,
                                                                                    ),
                                                                                ),
                                                                                Expr::u32(10),
                                                                            ),
                                                                            Expr::u32(99),
                                                                        ),
                                                                    ),
                                                                ),
                                                            ),
                                                            Node::let_bind(
                                                                "is_hexd",
                                                                Expr::select(
                                                                    Expr::lt(
                                                                        Expr::var("hb_val"),
                                                                        Expr::u32(16),
                                                                    ),
                                                                    Expr::u32(1),
                                                                    Expr::u32(0),
                                                                ),
                                                            ),
                                                            Node::if_then_else(
                                                                Expr::eq(
                                                                    Expr::var("is_hexd"),
                                                                    Expr::u32(1),
                                                                ),
                                                                vec![
                                                                    Node::assign(
                                                                        "hex_value",
                                                                        Expr::add(
                                                                            Expr::mul(
                                                                                Expr::var(
                                                                                    "hex_value",
                                                                                ),
                                                                                Expr::u32(16),
                                                                            ),
                                                                            Expr::var("hb_val"),
                                                                        ),
                                                                    ),
                                                                    Node::assign(
                                                                        "hex_len",
                                                                        Expr::add(
                                                                            Expr::var("hex_len"),
                                                                            Expr::u32(1),
                                                                        ),
                                                                    ),
                                                                ],
                                                                vec![Node::assign(
                                                                    "hex_done",
                                                                    Expr::u32(1),
                                                                )],
                                                            ),
                                                        ],
                                                    )],
                                                ),
                                                // ---- UCN: \uHHHH or \UHHHHHHHH, fixed length ----
                                                Node::let_bind(
                                                    "ucn_digits",
                                                    Expr::select(
                                                        Expr::eq(
                                                            Expr::var("is_ucn4_start"),
                                                            Expr::u32(1),
                                                        ),
                                                        Expr::u32(4),
                                                        Expr::select(
                                                            Expr::eq(
                                                                Expr::var("is_ucn8_start"),
                                                                Expr::u32(1),
                                                            ),
                                                            Expr::u32(8),
                                                            Expr::u32(0),
                                                        ),
                                                    ),
                                                ),
                                                Node::let_bind("ucn_value", Expr::u32(0)),
                                                Node::let_bind("ucn_ok", Expr::u32(1)),
                                                Node::loop_for(
                                                    "ud",
                                                    Expr::u32(0),
                                                    Expr::u32(8),
                                                    vec![Node::if_then(
                                                        Expr::lt(
                                                            Expr::var("ud"),
                                                            Expr::var("ucn_digits"),
                                                        ),
                                                        vec![
                                                            Node::let_bind(
                                                                "ub",
                                                                safe_load(Expr::add(
                                                                    Expr::add(
                                                                        Expr::var("idx"),
                                                                        Expr::u32(2),
                                                                    ),
                                                                    Expr::var("ud"),
                                                                )),
                                                            ),
                                                            Node::let_bind(
                                                                "ub_dec",
                                                                Expr::select(
                                                                    Expr::and(
                                                                        Expr::ge(
                                                                            Expr::var("ub"),
                                                                            Expr::u32(b'0' as u32),
                                                                        ),
                                                                        Expr::le(
                                                                            Expr::var("ub"),
                                                                            Expr::u32(b'9' as u32),
                                                                        ),
                                                                    ),
                                                                    Expr::u32(1),
                                                                    Expr::u32(0),
                                                                ),
                                                            ),
                                                            Node::let_bind(
                                                                "ub_lc",
                                                                Expr::select(
                                                                    Expr::and(
                                                                        Expr::ge(
                                                                            Expr::var("ub"),
                                                                            Expr::u32(b'a' as u32),
                                                                        ),
                                                                        Expr::le(
                                                                            Expr::var("ub"),
                                                                            Expr::u32(b'f' as u32),
                                                                        ),
                                                                    ),
                                                                    Expr::u32(1),
                                                                    Expr::u32(0),
                                                                ),
                                                            ),
                                                            Node::let_bind(
                                                                "ub_uc",
                                                                Expr::select(
                                                                    Expr::and(
                                                                        Expr::ge(
                                                                            Expr::var("ub"),
                                                                            Expr::u32(b'A' as u32),
                                                                        ),
                                                                        Expr::le(
                                                                            Expr::var("ub"),
                                                                            Expr::u32(b'F' as u32),
                                                                        ),
                                                                    ),
                                                                    Expr::u32(1),
                                                                    Expr::u32(0),
                                                                ),
                                                            ),
                                                            Node::let_bind(
                                                                "ub_val",
                                                                Expr::select(
                                                                    Expr::eq(
                                                                        Expr::var("ub_dec"),
                                                                        Expr::u32(1),
                                                                    ),
                                                                    Expr::sub(
                                                                        Expr::var("ub"),
                                                                        Expr::u32(b'0' as u32),
                                                                    ),
                                                                    Expr::select(
                                                                        Expr::eq(
                                                                            Expr::var("ub_lc"),
                                                                            Expr::u32(1),
                                                                        ),
                                                                        Expr::add(
                                                                            Expr::sub(
                                                                                Expr::var("ub"),
                                                                                Expr::u32(
                                                                                    b'a' as u32,
                                                                                ),
                                                                            ),
                                                                            Expr::u32(10),
                                                                        ),
                                                                        Expr::select(
                                                                            Expr::eq(
                                                                                Expr::var("ub_uc"),
                                                                                Expr::u32(1),
                                                                            ),
                                                                            Expr::add(
                                                                                Expr::sub(
                                                                                    Expr::var("ub"),
                                                                                    Expr::u32(
                                                                                        b'A' as u32,
                                                                                    ),
                                                                                ),
                                                                                Expr::u32(10),
                                                                            ),
                                                                            Expr::u32(99),
                                                                        ),
                                                                    ),
                                                                ),
                                                            ),
                                                            Node::if_then_else(
                                                                Expr::lt(
                                                                    Expr::var("ub_val"),
                                                                    Expr::u32(16),
                                                                ),
                                                                vec![Node::assign(
                                                                    "ucn_value",
                                                                    Expr::add(
                                                                        Expr::mul(
                                                                            Expr::var("ucn_value"),
                                                                            Expr::u32(16),
                                                                        ),
                                                                        Expr::var("ub_val"),
                                                                    ),
                                                                )],
                                                                vec![Node::assign(
                                                                    "ucn_ok",
                                                                    Expr::u32(0),
                                                                )],
                                                            ),
                                                        ],
                                                    )],
                                                ),
                                                // ---- Compose final esc_val + extra_advance ----
                                                Node::let_bind(
                                                    "esc_val",
                                                    Expr::select(
                                                        Expr::eq(
                                                            Expr::var("is_octal_start"),
                                                            Expr::u32(1),
                                                        ),
                                                        Expr::var("octal_value"),
                                                        Expr::select(
                                                            Expr::eq(
                                                                Expr::var("is_hex_start"),
                                                                Expr::u32(1),
                                                            ),
                                                            Expr::var("hex_value"),
                                                            Expr::select(
                                                                Expr::or(
                                                                    Expr::eq(
                                                                        Expr::var("is_ucn4_start"),
                                                                        Expr::u32(1),
                                                                    ),
                                                                    Expr::eq(
                                                                        Expr::var("is_ucn8_start"),
                                                                        Expr::u32(1),
                                                                    ),
                                                                ),
                                                                Expr::var("ucn_value"),
                                                                Expr::var("simple_val"),
                                                            ),
                                                        ),
                                                    ),
                                                ),
                                                // Bytes to advance from the `\`. Octal: 1
                                                // (for `\`) + octal_len. Hex: 2 (for `\x`) +
                                                // hex_len. UCN: 2 (for `\u`/`\U`) + ucn_digits.
                                                // Simple: 2 (for `\<one>`).
                                                Node::let_bind(
                                                    "extra_advance",
                                                    Expr::select(
                                                        Expr::eq(
                                                            Expr::var("is_octal_start"),
                                                            Expr::u32(1),
                                                        ),
                                                        Expr::add(
                                                            Expr::u32(1),
                                                            Expr::var("octal_len"),
                                                        ),
                                                        Expr::select(
                                                            Expr::eq(
                                                                Expr::var("is_hex_start"),
                                                                Expr::u32(1),
                                                            ),
                                                            Expr::add(
                                                                Expr::u32(2),
                                                                Expr::var("hex_len"),
                                                            ),
                                                            Expr::select(
                                                                Expr::or(
                                                                    Expr::eq(
                                                                        Expr::var("is_ucn4_start"),
                                                                        Expr::u32(1),
                                                                    ),
                                                                    Expr::eq(
                                                                        Expr::var("is_ucn8_start"),
                                                                        Expr::u32(1),
                                                                    ),
                                                                ),
                                                                Expr::add(
                                                                    Expr::u32(2),
                                                                    Expr::var("ucn_digits"),
                                                                ),
                                                                Expr::u32(2),
                                                            ),
                                                        ),
                                                    ),
                                                ),
                                                // Hex with no digits is an error per CPU
                                                // ref. Same for UCN with bad digits.
                                                Node::if_then(
                                                    Expr::and(
                                                        Expr::eq(
                                                            Expr::var("is_hex_start"),
                                                            Expr::u32(1),
                                                        ),
                                                        Expr::eq(
                                                            Expr::var("hex_len"),
                                                            Expr::u32(0),
                                                        ),
                                                    ),
                                                    vec![Node::assign("ok_so_far", Expr::u32(0))],
                                                ),
                                                Node::if_then(
                                                    Expr::and(
                                                        Expr::or(
                                                            Expr::eq(
                                                                Expr::var("is_ucn4_start"),
                                                                Expr::u32(1),
                                                            ),
                                                            Expr::eq(
                                                                Expr::var("is_ucn8_start"),
                                                                Expr::u32(1),
                                                            ),
                                                        ),
                                                        Expr::eq(Expr::var("ucn_ok"), Expr::u32(0)),
                                                    ),
                                                    vec![Node::assign("ok_so_far", Expr::u32(0))],
                                                ),
                                                // Append to value.
                                                Node::assign(
                                                    "value",
                                                    Expr::bitor(
                                                        Expr::shl(Expr::var("value"), Expr::u32(8)),
                                                        Expr::bitand(
                                                            Expr::var("esc_val"),
                                                            Expr::u32(0xff),
                                                        ),
                                                    ),
                                                ),
                                                Node::assign("saw_char", Expr::u32(1)),
                                                Node::assign(
                                                    "idx",
                                                    Expr::add(
                                                        Expr::var("idx"),
                                                        Expr::var("extra_advance"),
                                                    ),
                                                ),
                                            ],
                                            vec![
                                                // Plain byte.
                                                Node::assign(
                                                    "value",
                                                    Expr::bitor(
                                                        Expr::shl(Expr::var("value"), Expr::u32(8)),
                                                        Expr::bitand(
                                                            Expr::var("ch"),
                                                            Expr::u32(0xff),
                                                        ),
                                                    ),
                                                ),
                                                Node::assign("saw_char", Expr::u32(1)),
                                                Node::assign(
                                                    "idx",
                                                    Expr::add(Expr::var("idx"), Expr::u32(1)),
                                                ),
                                            ],
                                        ),
                                    ],
                                ),
                            ],
                        )],
                    ),
                    // After the loop, idx must be at the closing `'`.
                    Node::let_bind("closer", safe_load(Expr::var("idx"))),
                    Node::if_then(
                        Expr::ne(Expr::var("closer"), Expr::u32(b'\'' as u32)),
                        vec![Node::assign("ok_so_far", Expr::u32(0))],
                    ),
                    // Empty `''` is an error.
                    Node::if_then(
                        Expr::eq(Expr::var("saw_char"), Expr::u32(0)),
                        vec![Node::assign("ok_so_far", Expr::u32(0))],
                    ),
                    // On success, step past closing `'`.
                    Node::if_then(
                        Expr::eq(Expr::var("ok_so_far"), Expr::u32(1)),
                        vec![Node::assign(
                            "idx",
                            Expr::add(Expr::var("idx"), Expr::u32(1)),
                        )],
                    ),
                ],
            ),
            Node::let_bind(
                "consumed",
                Expr::select(
                    Expr::eq(Expr::var("ok_so_far"), Expr::u32(1)),
                    Expr::sub(Expr::var("idx"), Expr::var("start")),
                    Expr::u32(0),
                ),
            ),
            Node::let_bind(
                "value_final",
                Expr::select(
                    Expr::eq(Expr::var("ok_so_far"), Expr::u32(1)),
                    Expr::var("value"),
                    Expr::u32(0),
                ),
            ),
            Node::store("value_out", Expr::u32(0), Expr::var("value_final")),
            Node::store("bytes_consumed_out", Expr::u32(0), Expr::var("consumed")),
            Node::store("ok_out", Expr::u32(0), Expr::var("ok_so_far")),
        ],
    )];

    let mut buffers = literal_scan_common_buffers(
        BINDING_SOURCE,
        BINDING_START_POS,
        BINDING_VALUE_OUT,
        BINDING_BYTES_CONSUMED_OUT,
    );
    buffers.push(literal_scan_status_output("ok_out", BINDING_OK_OUT));
    literal_scan_program(buffers, body, OP_ID)
}
