use super::super::gpu_source_bytes::safe_load_packed_byte_expr;
use super::*;

/// Build the 17b.5 comment-strip-mask `Program`.
#[must_use]
pub fn gpu_comment_strip_mask(byte_count: u32) -> Program {
    gpu_comment_strip_mask_with_source_type(byte_count, DataType::U32)
}

/// Build the 17b.5 comment-strip-mask `Program` over runtime-sized raw
/// `DataType::U8` source bytes.
#[must_use]
pub fn gpu_comment_strip_mask_u8(byte_count: u32) -> Program {
    gpu_comment_strip_mask_with_source_type(byte_count, DataType::U8)
}

fn gpu_comment_strip_mask_with_source_type(byte_count: u32, source_type: DataType) -> Program {
    let safe_load = |addr: Expr| -> Expr {
        if source_type == DataType::U8 {
            let buf_len = Expr::buf_len("bytes_in");
            let logical_len = Expr::u32(byte_count);
            let bound = Expr::select(
                Expr::lt(buf_len.clone(), logical_len.clone()),
                buf_len,
                logical_len,
            );
            let in_bounds = Expr::lt(addr.clone(), bound.clone());
            let safe_addr = Expr::select(
                in_bounds.clone(),
                addr,
                Expr::saturating_sub(bound, Expr::u32(1)),
            );
            let byte = Expr::bitand(
                Expr::cast(DataType::U32, Expr::load("bytes_in", safe_addr)),
                Expr::u32(0xFF),
            );
            Expr::select(in_bounds, byte, Expr::u32(0))
        } else {
            safe_load_packed_byte_expr("bytes_in", addr, Expr::u32(byte_count))
        }
    };
    let first_comment_fixup = if byte_count >= 2 {
        Node::if_then(
            Expr::and(
                Expr::eq(safe_load(Expr::u32(0)), Expr::u32(b'/' as u32)),
                Expr::or(
                    Expr::eq(safe_load(Expr::u32(1)), Expr::u32(b'*' as u32)),
                    Expr::eq(safe_load(Expr::u32(1)), Expr::u32(b'/' as u32)),
                ),
            ),
            vec![Node::store("comment_mask_out", Expr::u32(0), Expr::u32(2))],
        )
    } else {
        Node::let_bind("comment_mask_short_input", Expr::u32(0))
    };

    // One GPU lane walks the byte stream
    // sequentially, maintaining (in_line, in_block) state. Every byte
    // either gets `0` (code) or `1` (comment) written to
    // `comment_mask_out`.
    let body: Vec<Node> = vec![Node::if_then(
        Expr::eq(Expr::InvocationId { axis: 0 }, Expr::u32(0)),
        vec![
            Node::let_bind("in_line", Expr::u32(0)),
            Node::let_bind("in_block", Expr::u32(0)),
            Node::let_bind("in_string", Expr::u32(0)),
            Node::let_bind("in_char", Expr::u32(0)),
            Node::let_bind("escaped", Expr::u32(0)),
            Node::let_bind("i", Expr::u32(0)),
            Node::loop_for(
                "step",
                Expr::u32(0),
                Expr::u32(byte_count),
                vec![
                    Node::let_bind("b", safe_load(Expr::var("i"))),
                    Node::let_bind("b1", safe_load(Expr::add(Expr::var("i"), Expr::u32(1)))),
                    Node::let_bind("b2", safe_load(Expr::add(Expr::var("i"), Expr::u32(2)))),
                    Node::let_bind("b3", safe_load(Expr::add(Expr::var("i"), Expr::u32(3)))),
                    Node::let_bind("b4", safe_load(Expr::add(Expr::var("i"), Expr::u32(4)))),
                    // Default mask value for this byte = currently in
                    // any comment.
                    Node::let_bind(
                        "mask",
                        Expr::select(
                            Expr::or(
                                Expr::eq(Expr::var("in_line"), Expr::u32(1)),
                                Expr::eq(Expr::var("in_block"), Expr::u32(1)),
                            ),
                            Expr::u32(1),
                            Expr::u32(0),
                        ),
                    ),
                    // Detect comment open when not inside any comment or literal.
                    Node::if_then(
                        Expr::and(
                            Expr::and(
                                Expr::eq(Expr::var("in_line"), Expr::u32(0)),
                                Expr::eq(Expr::var("in_block"), Expr::u32(0)),
                            ),
                            Expr::and(
                                Expr::eq(Expr::var("in_string"), Expr::u32(0)),
                                Expr::eq(Expr::var("in_char"), Expr::u32(0)),
                            ),
                        ),
                        vec![
                            // `//` opens a line comment.
                            Node::if_then(
                                Expr::and(
                                    Expr::eq(Expr::var("b"), Expr::u32(b'/' as u32)),
                                    Expr::or(
                                        Expr::eq(Expr::var("b1"), Expr::u32(b'/' as u32)),
                                        Expr::or(
                                            Expr::and(
                                                Expr::and(
                                                    Expr::eq(
                                                        Expr::var("b1"),
                                                        Expr::u32(b'\\' as u32),
                                                    ),
                                                    Expr::eq(
                                                        Expr::var("b2"),
                                                        Expr::u32(b'\n' as u32),
                                                    ),
                                                ),
                                                Expr::eq(Expr::var("b3"), Expr::u32(b'/' as u32)),
                                            ),
                                            Expr::and(
                                                Expr::and(
                                                    Expr::and(
                                                        Expr::eq(
                                                            Expr::var("b1"),
                                                            Expr::u32(b'\\' as u32),
                                                        ),
                                                        Expr::eq(
                                                            Expr::var("b2"),
                                                            Expr::u32(b'\r' as u32),
                                                        ),
                                                    ),
                                                    Expr::eq(
                                                        Expr::var("b3"),
                                                        Expr::u32(b'\n' as u32),
                                                    ),
                                                ),
                                                Expr::eq(Expr::var("b4"), Expr::u32(b'/' as u32)),
                                            ),
                                        ),
                                    ),
                                ),
                                vec![
                                    Node::assign("in_line", Expr::u32(1)),
                                    // The first comment byte becomes the single
                                    // replacement space required by C translation
                                    // phase 3.
                                    Node::assign("mask", Expr::u32(2)),
                                ],
                            ),
                            // `/*` opens a block comment.
                            Node::if_then(
                                Expr::and(
                                    Expr::eq(Expr::var("b"), Expr::u32(b'/' as u32)),
                                    Expr::or(
                                        Expr::eq(Expr::var("b1"), Expr::u32(b'*' as u32)),
                                        Expr::or(
                                            Expr::and(
                                                Expr::and(
                                                    Expr::eq(
                                                        Expr::var("b1"),
                                                        Expr::u32(b'\\' as u32),
                                                    ),
                                                    Expr::eq(
                                                        Expr::var("b2"),
                                                        Expr::u32(b'\n' as u32),
                                                    ),
                                                ),
                                                Expr::eq(Expr::var("b3"), Expr::u32(b'*' as u32)),
                                            ),
                                            Expr::and(
                                                Expr::and(
                                                    Expr::and(
                                                        Expr::eq(
                                                            Expr::var("b1"),
                                                            Expr::u32(b'\\' as u32),
                                                        ),
                                                        Expr::eq(
                                                            Expr::var("b2"),
                                                            Expr::u32(b'\r' as u32),
                                                        ),
                                                    ),
                                                    Expr::eq(
                                                        Expr::var("b3"),
                                                        Expr::u32(b'\n' as u32),
                                                    ),
                                                ),
                                                Expr::eq(Expr::var("b4"), Expr::u32(b'*' as u32)),
                                            ),
                                        ),
                                    ),
                                ),
                                vec![
                                    Node::assign("in_block", Expr::u32(1)),
                                    Node::assign("mask", Expr::u32(2)),
                                ],
                            ),
                        ],
                    ),
                    // Write the mask before processing closes  -  this
                    // ensures the closing `*/` bytes are themselves
                    // marked as comment.
                    Node::store("comment_mask_out", Expr::var("i"), Expr::var("mask")),
                    // Detect comment closes AFTER writing the mask.
                    // - Line comment closes at `\n` (the newline itself
                    //   is OUTSIDE the comment per typical C semantics;
                    //   tools that mask comments usually keep newlines
                    //   so line counts stay correct, but we already
                    //   wrote 1 before checking  -  fix that next).
                    Node::if_then(
                        Expr::and(
                            Expr::eq(Expr::var("in_line"), Expr::u32(1)),
                            Expr::eq(Expr::var("b"), Expr::u32(b'\n' as u32)),
                        ),
                        vec![
                            // Newline is NOT part of the comment  -  overwrite.
                            Node::store("comment_mask_out", Expr::var("i"), Expr::u32(0)),
                            Node::assign("in_line", Expr::u32(0)),
                        ],
                    ),
                    Node::if_then(
                        Expr::and(
                            Expr::eq(Expr::var("in_line"), Expr::u32(1)),
                            Expr::and(
                                Expr::eq(Expr::var("b"), Expr::u32(b'\r' as u32)),
                                Expr::eq(Expr::var("b1"), Expr::u32(b'\n' as u32)),
                            ),
                        ),
                        vec![
                            Node::store("comment_mask_out", Expr::var("i"), Expr::u32(0)),
                            Node::store(
                                "comment_mask_out",
                                Expr::add(Expr::var("i"), Expr::u32(1)),
                                Expr::u32(0),
                            ),
                            Node::assign("i", Expr::add(Expr::var("i"), Expr::u32(1))),
                            Node::assign("in_line", Expr::u32(0)),
                        ],
                    ),
                    Node::if_then(
                        Expr::and(
                            Expr::eq(Expr::var("in_line"), Expr::u32(1)),
                            Expr::and(
                                Expr::eq(Expr::var("b"), Expr::u32(b'\\' as u32)),
                                Expr::eq(Expr::var("b1"), Expr::u32(b'\n' as u32)),
                            ),
                        ),
                        vec![
                            Node::store("comment_mask_out", Expr::var("i"), Expr::u32(1)),
                            Node::store(
                                "comment_mask_out",
                                Expr::add(Expr::var("i"), Expr::u32(1)),
                                Expr::u32(1),
                            ),
                            Node::assign("i", Expr::add(Expr::var("i"), Expr::u32(1))),
                        ],
                    ),
                    Node::if_then(
                        Expr::and(
                            Expr::eq(Expr::var("in_line"), Expr::u32(1)),
                            Expr::and(
                                Expr::and(
                                    Expr::eq(Expr::var("b"), Expr::u32(b'\\' as u32)),
                                    Expr::eq(Expr::var("b1"), Expr::u32(b'\r' as u32)),
                                ),
                                Expr::eq(Expr::var("b2"), Expr::u32(b'\n' as u32)),
                            ),
                        ),
                        vec![
                            Node::store("comment_mask_out", Expr::var("i"), Expr::u32(1)),
                            Node::store(
                                "comment_mask_out",
                                Expr::add(Expr::var("i"), Expr::u32(1)),
                                Expr::u32(1),
                            ),
                            Node::store(
                                "comment_mask_out",
                                Expr::add(Expr::var("i"), Expr::u32(2)),
                                Expr::u32(1),
                            ),
                            Node::assign("i", Expr::add(Expr::var("i"), Expr::u32(2))),
                        ],
                    ),
                    // Block comment closes when current byte is `*`
                    // and next byte is `/`. The closing `/` (i+1) is
                    // also part of the comment, so we set in_block=0
                    // only after the `/` byte itself is processed.
                    // We do this by detecting (b == '*' && b1 == '/')
                    // and writing the mask for byte i+1 in the next
                    // iteration normally; here we just transition out
                    // AFTER advancing past the `/`. Simplest: set
                    // in_block=0 next iteration when (prev was `*` and
                    // current is `/`). Track via `prev_star` flag.
                    // Implemented below via two-step trailing close.
                    Node::if_then(
                        Expr::and(
                            Expr::eq(Expr::var("in_block"), Expr::u32(1)),
                            Expr::and(
                                Expr::eq(Expr::var("b"), Expr::u32(b'*' as u32)),
                                Expr::eq(Expr::var("b1"), Expr::u32(b'/' as u32)),
                            ),
                        ),
                        vec![
                            // Mark the trailing '/' (i+1) as comment now,
                            // and exit the block AFTER it. We do this by
                            // pre-storing into i+1 here, then on the next
                            // iteration the loop body will run with
                            // in_block=0 but the mask we already stored
                            // wins because we won't re-store unless we
                            // write the same slot.
                            Node::if_then(
                                Expr::lt(
                                    Expr::add(Expr::var("i"), Expr::u32(1)),
                                    Expr::u32(byte_count),
                                ),
                                vec![Node::store(
                                    "comment_mask_out",
                                    Expr::add(Expr::var("i"), Expr::u32(1)),
                                    Expr::u32(1),
                                )],
                            ),
                            // Skip the next byte by bumping i, then
                            // exit the comment.
                            Node::assign("i", Expr::add(Expr::var("i"), Expr::u32(1))),
                            Node::assign("in_block", Expr::u32(0)),
                        ],
                    ),
                    Node::if_then(
                        Expr::and(
                            Expr::eq(Expr::var("in_block"), Expr::u32(1)),
                            Expr::and(
                                Expr::and(
                                    Expr::eq(Expr::var("b"), Expr::u32(b'*' as u32)),
                                    Expr::eq(Expr::var("b1"), Expr::u32(b'\\' as u32)),
                                ),
                                Expr::and(
                                    Expr::eq(Expr::var("b2"), Expr::u32(b'\n' as u32)),
                                    Expr::eq(Expr::var("b3"), Expr::u32(b'/' as u32)),
                                ),
                            ),
                        ),
                        vec![
                            Node::store("comment_mask_out", Expr::var("i"), Expr::u32(1)),
                            Node::store(
                                "comment_mask_out",
                                Expr::add(Expr::var("i"), Expr::u32(1)),
                                Expr::u32(1),
                            ),
                            Node::store(
                                "comment_mask_out",
                                Expr::add(Expr::var("i"), Expr::u32(2)),
                                Expr::u32(1),
                            ),
                            Node::store(
                                "comment_mask_out",
                                Expr::add(Expr::var("i"), Expr::u32(3)),
                                Expr::u32(1),
                            ),
                            Node::assign("i", Expr::add(Expr::var("i"), Expr::u32(3))),
                            Node::assign("in_block", Expr::u32(0)),
                        ],
                    ),
                    Node::if_then(
                        Expr::and(
                            Expr::eq(Expr::var("in_block"), Expr::u32(1)),
                            Expr::and(
                                Expr::and(
                                    Expr::and(
                                        Expr::eq(Expr::var("b"), Expr::u32(b'*' as u32)),
                                        Expr::eq(Expr::var("b1"), Expr::u32(b'\\' as u32)),
                                    ),
                                    Expr::eq(Expr::var("b2"), Expr::u32(b'\r' as u32)),
                                ),
                                Expr::and(
                                    Expr::eq(Expr::var("b3"), Expr::u32(b'\n' as u32)),
                                    Expr::eq(Expr::var("b4"), Expr::u32(b'/' as u32)),
                                ),
                            ),
                        ),
                        vec![
                            Node::store("comment_mask_out", Expr::var("i"), Expr::u32(1)),
                            Node::store(
                                "comment_mask_out",
                                Expr::add(Expr::var("i"), Expr::u32(1)),
                                Expr::u32(1),
                            ),
                            Node::store(
                                "comment_mask_out",
                                Expr::add(Expr::var("i"), Expr::u32(2)),
                                Expr::u32(1),
                            ),
                            Node::store(
                                "comment_mask_out",
                                Expr::add(Expr::var("i"), Expr::u32(3)),
                                Expr::u32(1),
                            ),
                            Node::store(
                                "comment_mask_out",
                                Expr::add(Expr::var("i"), Expr::u32(4)),
                                Expr::u32(1),
                            ),
                            Node::assign("i", Expr::add(Expr::var("i"), Expr::u32(4))),
                            Node::assign("in_block", Expr::u32(0)),
                        ],
                    ),
                    // Track string/char literals after comment handling so comment
                    // openers inside literals never enter comment state. Escape state
                    // is one-byte delayed and only meaningful inside a literal.
                    Node::if_then(
                        Expr::and(
                            Expr::eq(Expr::var("in_line"), Expr::u32(0)),
                            Expr::eq(Expr::var("in_block"), Expr::u32(0)),
                        ),
                        vec![
                            Node::let_bind("literal_was_in_string", Expr::var("in_string")),
                            Node::let_bind("literal_was_in_char", Expr::var("in_char")),
                            Node::if_then(
                                Expr::eq(Expr::var("in_string"), Expr::u32(1)),
                                vec![
                                    Node::if_then(
                                        Expr::eq(Expr::var("escaped"), Expr::u32(1)),
                                        vec![Node::assign("escaped", Expr::u32(0))],
                                    ),
                                    Node::if_then(
                                        Expr::eq(Expr::var("escaped"), Expr::u32(0)),
                                        vec![
                                            Node::if_then(
                                                Expr::eq(Expr::var("b"), Expr::u32(b'\\' as u32)),
                                                vec![Node::assign("escaped", Expr::u32(1))],
                                            ),
                                            Node::if_then(
                                                Expr::eq(Expr::var("b"), Expr::u32(b'"' as u32)),
                                                vec![Node::assign("in_string", Expr::u32(0))],
                                            ),
                                        ],
                                    ),
                                ],
                            ),
                            Node::if_then(
                                Expr::eq(Expr::var("in_char"), Expr::u32(1)),
                                vec![
                                    Node::if_then(
                                        Expr::eq(Expr::var("escaped"), Expr::u32(1)),
                                        vec![Node::assign("escaped", Expr::u32(0))],
                                    ),
                                    Node::if_then(
                                        Expr::eq(Expr::var("escaped"), Expr::u32(0)),
                                        vec![
                                            Node::if_then(
                                                Expr::eq(Expr::var("b"), Expr::u32(b'\\' as u32)),
                                                vec![Node::assign("escaped", Expr::u32(1))],
                                            ),
                                            Node::if_then(
                                                Expr::eq(Expr::var("b"), Expr::u32(b'\'' as u32)),
                                                vec![Node::assign("in_char", Expr::u32(0))],
                                            ),
                                        ],
                                    ),
                                ],
                            ),
                            Node::if_then(
                                Expr::and(
                                    Expr::eq(Expr::var("literal_was_in_string"), Expr::u32(0)),
                                    Expr::eq(Expr::var("literal_was_in_char"), Expr::u32(0)),
                                ),
                                vec![
                                    Node::if_then(
                                        Expr::eq(Expr::var("b"), Expr::u32(b'"' as u32)),
                                        vec![
                                            Node::assign("in_string", Expr::u32(1)),
                                            Node::assign("escaped", Expr::u32(0)),
                                        ],
                                    ),
                                    Node::if_then(
                                        Expr::eq(Expr::var("b"), Expr::u32(b'\'' as u32)),
                                        vec![
                                            Node::assign("in_char", Expr::u32(1)),
                                            Node::assign("escaped", Expr::u32(0)),
                                        ],
                                    ),
                                ],
                            ),
                        ],
                    ),
                    Node::assign("i", Expr::add(Expr::var("i"), Expr::u32(1))),
                ],
            ),
            first_comment_fixup,
        ],
    )];

    Program::wrapped(
        vec![
            BufferDecl::storage(
                "bytes_in",
                BINDING_BYTES_IN,
                BufferAccess::ReadOnly,
                source_type.clone(),
            )
            .with_count(if source_type == DataType::U8 {
                0
            } else {
                byte_count.div_ceil(4).max(1)
            }),
            BufferDecl::storage(
                "comment_mask_out",
                BINDING_COMMENT_MASK_OUT,
                BufferAccess::ReadWrite,
                DataType::U32,
            )
            .with_count(byte_count.max(1)),
        ],
        [1, 1, 1],
        body,
    )
    .with_entry_op_id(OP_ID)
}
