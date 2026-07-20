use super::program_helpers::{
    byte_eq, clear_comment_mask_and_final_keep, singleton_u32_read_buffer, source_byte_load,
    source_byte_load_or_zero, source_bytes_input_buffer, store_comment_mask,
    store_final_keep_from_comment_mask, u32_read_buffer, u32_rw_buffer, wrap_gpu_filter_program,
};
use vyre::ir::{Expr, Node, Program};

pub(super) fn simple_block_comment_marks_program(n: u32) -> Program {
    let i = Expr::var("i");
    let b0 = source_byte_load("bytes_in", i.clone());
    let b1_addr = Expr::add(i.clone(), Expr::u32(1));
    let b1 = source_byte_load_or_zero("bytes_in", b1_addr, "block_n_real");
    let after_close = Expr::add(i.clone(), Expr::u32(2));
    let body = vec![
        Node::let_bind("i", Expr::InvocationId { axis: 0 }),
        Node::if_then(
            Expr::lt(i.clone(), Expr::u32(n)),
            vec![
                Node::let_bind("n_real", Expr::load("block_n_real", Expr::u32(0))),
                Node::let_bind(
                    "opens_block",
                    Expr::and(
                        Expr::lt(i.clone(), Expr::var("n_real")),
                        Expr::and(byte_eq(b0.clone(), b'/'), byte_eq(b1.clone(), b'*')),
                    ),
                ),
                Node::store(
                    "block_open_flags",
                    i.clone(),
                    Expr::select(Expr::var("opens_block"), Expr::u32(1), Expr::u32(0)),
                ),
                Node::if_then(
                    Expr::and(
                        Expr::lt(after_close.clone(), Expr::u32(n)),
                        Expr::and(
                            Expr::lt(i.clone(), Expr::var("n_real")),
                            Expr::and(byte_eq(b0, b'*'), byte_eq(b1, b'/')),
                        ),
                    ),
                    vec![Node::store(
                        "block_close_after_flags",
                        after_close,
                        Expr::u32(1),
                    )],
                ),
            ],
        ),
    ];
    wrap_gpu_filter_program(
        "vyre-libs::parsing::c::preprocess::simple_block_comment_marks",
        vec![
            source_bytes_input_buffer("bytes_in", 0, n),
            u32_rw_buffer("block_open_flags", 1, n),
            u32_rw_buffer("block_close_after_flags", 2, n),
            singleton_u32_read_buffer("block_n_real", 3),
        ],
        body,
    )
}

pub(super) fn simple_block_comment_topology_program(n: u32) -> Program {
    let i = Expr::var("i");
    let prev = Expr::saturating_sub(i.clone(), Expr::u32(1));
    let body = vec![
        Node::let_bind("i", Expr::InvocationId { axis: 0 }),
        Node::if_then(
            Expr::lt(i.clone(), Expr::load("block_n_real", Expr::u32(0))),
            vec![
                Node::let_bind("open_here", Expr::load("block_open_flags", i.clone())),
                Node::let_bind(
                    "open_before",
                    Expr::saturating_sub(
                        Expr::load("block_open_scan", i.clone()),
                        Expr::var("open_here"),
                    ),
                ),
                Node::let_bind(
                    "closed_before",
                    Expr::load("block_close_after_scan", i.clone()),
                ),
                Node::if_then(
                    Expr::and(
                        Expr::eq(Expr::var("open_here"), Expr::u32(1)),
                        Expr::gt(Expr::var("open_before"), Expr::var("closed_before")),
                    ),
                    vec![Node::let_bind(
                        "nested_block_open_old",
                        Expr::atomic_or("block_topology_invalid", Expr::u32(0), Expr::u32(1)),
                    )],
                ),
                Node::let_bind(
                    "close_after_here",
                    Expr::load("block_close_after_flags", i.clone()),
                ),
                Node::if_then(
                    Expr::eq(Expr::var("close_after_here"), Expr::u32(1)),
                    vec![
                        Node::let_bind(
                            "open_at_close",
                            Expr::load("block_open_scan", prev.clone()),
                        ),
                        Node::let_bind(
                            "closed_before_close",
                            Expr::load("block_close_after_scan", prev),
                        ),
                        Node::if_then(
                            Expr::le(Expr::var("open_at_close"), Expr::var("closed_before_close")),
                            vec![Node::let_bind(
                                "stray_block_close_old",
                                Expr::atomic_or(
                                    "block_topology_invalid",
                                    Expr::u32(0),
                                    Expr::u32(1),
                                ),
                            )],
                        ),
                    ],
                ),
            ],
        ),
    ];
    wrap_gpu_filter_program(
        "vyre-libs::parsing::c::preprocess::simple_block_comment_topology",
        vec![
            u32_read_buffer("block_open_flags", 0, n),
            u32_read_buffer("block_close_after_flags", 1, n),
            u32_read_buffer("block_open_scan", 2, n),
            u32_read_buffer("block_close_after_scan", 3, n),
            u32_rw_buffer("block_topology_invalid", 4, 1),
            singleton_u32_read_buffer("block_n_real", 5),
        ],
        body,
    )
}

pub(super) fn simple_block_comment_masks_program(n: u32) -> Program {
    let i = Expr::var("i");
    let body = vec![
        Node::let_bind("i", Expr::InvocationId { axis: 0 }),
        Node::if_then(
            Expr::lt(i.clone(), Expr::u32(n)),
            vec![
                Node::let_bind("n_real", Expr::load("block_n_real", Expr::u32(0))),
                Node::if_then_else(
                    Expr::lt(i.clone(), Expr::var("n_real")),
                    vec![
                        Node::let_bind("open_count", Expr::load("block_open_scan", i.clone())),
                        Node::let_bind(
                            "closed_before_count",
                            Expr::load("block_close_after_scan", i.clone()),
                        ),
                        Node::let_bind(
                            "inside_block",
                            Expr::gt(Expr::var("open_count"), Expr::var("closed_before_count")),
                        ),
                        Node::let_bind(
                            "comment_mask",
                            Expr::select(
                                Expr::var("inside_block"),
                                Expr::select(
                                    Expr::eq(
                                        Expr::load("block_open_flags", i.clone()),
                                        Expr::u32(1),
                                    ),
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
        "vyre-libs::parsing::c::preprocess::simple_block_comment_masks",
        vec![
            u32_read_buffer("block_open_flags", 0, n),
            u32_read_buffer("block_open_scan", 1, n),
            u32_read_buffer("block_close_after_scan", 2, n),
            u32_rw_buffer("final_keep", 3, n),
            u32_rw_buffer("comment_mask_out", 4, n),
            singleton_u32_read_buffer("block_n_real", 5),
        ],
        body,
    )
}
