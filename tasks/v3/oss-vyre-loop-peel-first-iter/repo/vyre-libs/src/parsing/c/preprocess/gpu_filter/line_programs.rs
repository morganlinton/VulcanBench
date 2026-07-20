use super::program_helpers::{
    byte_eq, clear_comment_mask_and_final_keep, singleton_u32_read_buffer, source_byte_load,
    source_byte_load_or_zero, source_bytes_input_buffer, store_comment_mask,
    store_final_keep_from_comment_mask, u32_read_buffer, u32_rw_buffer, wrap_gpu_filter_program,
};
use vyre::ir::{Expr, Node, Program};

pub(super) fn simple_line_newline_flags_program(n: u32) -> Program {
    let i = Expr::var("i");
    let byte = source_byte_load("bytes_in", i.clone());
    let body = vec![
        Node::let_bind("i", Expr::InvocationId { axis: 0 }),
        Node::if_then(
            Expr::lt(i.clone(), Expr::u32(n)),
            vec![
                Node::let_bind("n_real", Expr::load("line_n_real", Expr::u32(0))),
                Node::store(
                    "newline_flags",
                    i.clone(),
                    Expr::select(
                        Expr::and(
                            Expr::lt(i.clone(), Expr::var("n_real")),
                            byte_eq(byte, b'\n'),
                        ),
                        Expr::u32(1),
                        Expr::u32(0),
                    ),
                ),
            ],
        ),
    ];
    wrap_gpu_filter_program(
        "vyre-libs::parsing::c::preprocess::simple_line_newline_flags",
        vec![
            source_bytes_input_buffer("bytes_in", 0, n),
            u32_rw_buffer("newline_flags", 1, n),
            singleton_u32_read_buffer("line_n_real", 2),
        ],
        body,
    )
}

pub(super) fn simple_line_comment_starts_program(n: u32) -> Program {
    let i = Expr::var("i");
    let b0 = source_byte_load("bytes_in", i.clone());
    let b1_addr = Expr::add(i.clone(), Expr::u32(1));
    let b1 = source_byte_load_or_zero("bytes_in", b1_addr, "line_n_real");
    let row = Expr::saturating_sub(
        Expr::load("newline_scan", i.clone()),
        Expr::load("newline_flags", i.clone()),
    );
    let body = vec![
        Node::let_bind("i", Expr::InvocationId { axis: 0 }),
        Node::if_then(
            Expr::lt(i.clone(), Expr::load("line_n_real", Expr::u32(0))),
            vec![Node::if_then(
                Expr::and(byte_eq(b0, b'/'), byte_eq(b1, b'/')),
                vec![Node::let_bind(
                    "line_comment_start_old",
                    Expr::atomic_min("row_comment_starts", row, i.clone()),
                )],
            )],
        ),
    ];
    wrap_gpu_filter_program(
        "vyre-libs::parsing::c::preprocess::simple_line_comment_starts",
        vec![
            source_bytes_input_buffer("bytes_in", 0, n),
            u32_read_buffer("newline_flags", 1, n),
            u32_read_buffer("newline_scan", 2, n),
            u32_rw_buffer("row_comment_starts", 3, n),
            singleton_u32_read_buffer("line_n_real", 4),
        ],
        body,
    )
}

pub(super) fn simple_line_comment_masks_program(n: u32) -> Program {
    let i = Expr::var("i");
    let b = source_byte_load("bytes_in", i.clone());
    let row = Expr::saturating_sub(
        Expr::load("newline_scan", i.clone()),
        Expr::load("newline_flags", i.clone()),
    );
    let body = vec![
        Node::let_bind("i", Expr::InvocationId { axis: 0 }),
        Node::if_then(
            Expr::lt(i.clone(), Expr::u32(n)),
            vec![
                Node::let_bind("n_real", Expr::load("line_n_real", Expr::u32(0))),
                Node::if_then_else(
                    Expr::lt(i.clone(), Expr::var("n_real")),
                    vec![
                        Node::let_bind("is_newline", byte_eq(b, b'\n')),
                        Node::let_bind("row", row),
                        Node::let_bind("start", Expr::load("row_comment_starts", Expr::var("row"))),
                        Node::let_bind(
                            "comment_mask",
                            Expr::select(
                                Expr::and(
                                    Expr::ne(Expr::var("start"), Expr::u32(u32::MAX)),
                                    Expr::and(
                                        Expr::ge(i.clone(), Expr::var("start")),
                                        Expr::not(Expr::var("is_newline")),
                                    ),
                                ),
                                Expr::select(
                                    Expr::eq(i.clone(), Expr::var("start")),
                                    Expr::u32(2),
                                    Expr::u32(1),
                                ),
                                Expr::u32(0),
                            ),
                        ),
                        store_comment_mask(i.clone(), Expr::var("comment_mask")),
                        store_final_keep_from_comment_mask(i.clone(), Expr::var("comment_mask")),
                    ],
                    clear_comment_mask_and_final_keep(i.clone()),
                ),
            ],
        ),
    ];
    wrap_gpu_filter_program(
        "vyre-libs::parsing::c::preprocess::simple_line_comment_masks",
        vec![
            source_bytes_input_buffer("bytes_in", 0, n),
            u32_read_buffer("newline_flags", 1, n),
            u32_read_buffer("newline_scan", 2, n),
            u32_read_buffer("row_comment_starts", 3, n),
            u32_rw_buffer("final_keep", 4, n),
            u32_rw_buffer("comment_mask_out", 5, n),
            singleton_u32_read_buffer("line_n_real", 6),
        ],
        body,
    )
}
