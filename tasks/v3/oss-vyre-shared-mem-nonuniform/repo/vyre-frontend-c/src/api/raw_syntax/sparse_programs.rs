use super::*;
pub(super) fn sparse_token_block_totals_program(
    sparse_types: &str,
    block_totals: &str,
    count: u32,
    num_blocks: u32,
) -> Program {
    use vyre_primitives::reduce::multi_block_prefix_scan::BLOCK_LANES;

    let lane = Expr::var("lane");
    let block = Expr::var("block");
    let global = Expr::var("global");
    let active_lane = Expr::lt(global.clone(), Expr::u32(count));
    let active_block = Expr::lt(block.clone(), Expr::u32(num_blocks));
    let scratch_a = "__raw_sparse_block_total_a";
    let scratch_b = "__raw_sparse_block_total_b";
    let mut body = vec![
        Node::let_bind("lane", Expr::LocalId { axis: 0 }),
        Node::let_bind("block", Expr::WorkgroupId { axis: 0 }),
        Node::let_bind(
            "global",
            Expr::add(
                Expr::mul(block.clone(), Expr::u32(BLOCK_LANES)),
                lane.clone(),
            ),
        ),
        Node::store(scratch_a, lane.clone(), Expr::u32(0)),
        Node::if_then(
            active_lane,
            vec![Node::store(
                scratch_a,
                lane.clone(),
                Expr::select(
                    Expr::ne(Expr::load(sparse_types, global.clone()), Expr::u32(0)),
                    Expr::u32(1),
                    Expr::u32(0),
                ),
            )],
        ),
        Node::Barrier {
            ordering: vyre_foundation::MemoryOrdering::SeqCst,
        },
    ];
    let mut stride = 1_u32;
    while stride < BLOCK_LANES {
        body.push(Node::store(
            scratch_b,
            lane.clone(),
            Expr::load(scratch_a, lane.clone()),
        ));
        body.push(Node::if_then(
            Expr::lt(Expr::u32(stride - 1), lane.clone()),
            vec![Node::store(
                scratch_b,
                lane.clone(),
                Expr::add(
                    Expr::load(scratch_a, lane.clone()),
                    Expr::load(
                        scratch_a,
                        Expr::add(lane.clone(), Expr::u32(0u32.wrapping_sub(stride))),
                    ),
                ),
            )],
        ));
        body.push(Node::Barrier {
            ordering: vyre_foundation::MemoryOrdering::SeqCst,
        });
        body.push(Node::store(
            scratch_a,
            lane.clone(),
            Expr::load(scratch_b, lane.clone()),
        ));
        body.push(Node::Barrier {
            ordering: vyre_foundation::MemoryOrdering::SeqCst,
        });
        stride = stride.saturating_mul(2);
    }
    body.push(Node::if_then(
        Expr::and(
            Expr::eq(lane.clone(), Expr::u32(BLOCK_LANES - 1)),
            active_block,
        ),
        vec![Node::store(
            block_totals,
            block,
            Expr::load(scratch_a, lane),
        )],
    ));
    Program::wrapped(
        vec![
            BufferDecl::storage(sparse_types, 0, BufferAccess::ReadOnly, DataType::U32),
            BufferDecl::output(block_totals, 1, DataType::U32).with_count(num_blocks),
            BufferDecl::workgroup(scratch_a, BLOCK_LANES, DataType::U32),
            BufferDecl::workgroup(scratch_b, BLOCK_LANES, DataType::U32),
        ],
        [BLOCK_LANES, 1, 1],
        vec![Node::Region {
            generator: "vyre-frontend-c::raw_sparse_token_block_totals".into(),
            source_region: None,
            body: Arc::new(body),
        }],
    )
    .with_entry_op_id("vyre-frontend-c::raw_sparse_token_block_totals")
    .with_non_composable_with_self(true)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn sparse_token_block_compact_program(
    block_totals_scanned: &str,
    sparse_types: &str,
    sparse_starts: &str,
    sparse_lens: &str,
    out_tok_triplets_and_count: &str,
    count: u32,
    num_blocks: u32,
) -> Program {
    use vyre_primitives::reduce::multi_block_prefix_scan::BLOCK_LANES;

    let lane = Expr::var("lane");
    let block = Expr::var("block");
    let global = Expr::var("global");
    let scratch_a = "__raw_sparse_block_compact_a";
    let scratch_b = "__raw_sparse_block_compact_b";
    let active_lane = Expr::lt(global.clone(), Expr::u32(count));
    let active_block = Expr::lt(block.clone(), Expr::u32(num_blocks));
    let mut body = vec![
        Node::let_bind("lane", Expr::LocalId { axis: 0 }),
        Node::let_bind("block", Expr::WorkgroupId { axis: 0 }),
        Node::let_bind(
            "global",
            Expr::add(
                Expr::mul(block.clone(), Expr::u32(BLOCK_LANES)),
                lane.clone(),
            ),
        ),
        Node::store(scratch_a, lane.clone(), Expr::u32(0)),
        Node::if_then(
            active_lane.clone(),
            vec![Node::store(
                scratch_a,
                lane.clone(),
                Expr::select(
                    Expr::ne(Expr::load(sparse_types, global.clone()), Expr::u32(0)),
                    Expr::u32(1),
                    Expr::u32(0),
                ),
            )],
        ),
        Node::Barrier {
            ordering: vyre_foundation::MemoryOrdering::SeqCst,
        },
    ];
    let mut stride = 1_u32;
    while stride < BLOCK_LANES {
        body.push(Node::store(
            scratch_b,
            lane.clone(),
            Expr::load(scratch_a, lane.clone()),
        ));
        body.push(Node::if_then(
            Expr::lt(Expr::u32(stride - 1), lane.clone()),
            vec![Node::store(
                scratch_b,
                lane.clone(),
                Expr::add(
                    Expr::load(scratch_a, lane.clone()),
                    Expr::load(
                        scratch_a,
                        Expr::add(lane.clone(), Expr::u32(0u32.wrapping_sub(stride))),
                    ),
                ),
            )],
        ));
        body.push(Node::Barrier {
            ordering: vyre_foundation::MemoryOrdering::SeqCst,
        });
        body.push(Node::store(
            scratch_a,
            lane.clone(),
            Expr::load(scratch_b, lane.clone()),
        ));
        body.push(Node::Barrier {
            ordering: vyre_foundation::MemoryOrdering::SeqCst,
        });
        stride = stride.saturating_mul(2);
    }
    body.extend([
        Node::let_bind("offset", Expr::u32(0)),
        Node::if_then(
            Expr::and(Expr::lt(Expr::u32(0), block.clone()), active_block),
            vec![Node::assign(
                "offset",
                Expr::load(
                    block_totals_scanned,
                    Expr::add(block.clone(), Expr::u32(0u32.wrapping_sub(1))),
                ),
            )],
        ),
        Node::if_then(
            active_lane,
            vec![
                Node::let_bind("token_type", Expr::load(sparse_types, global.clone())),
                Node::let_bind(
                    "global_triplet_base",
                    Expr::add(Expr::mul(global.clone(), Expr::u32(3)), Expr::u32(1)),
                ),
                Node::store(
                    out_tok_triplets_and_count,
                    Expr::var("global_triplet_base"),
                    Expr::u32(0),
                ),
                Node::store(
                    out_tok_triplets_and_count,
                    Expr::add(Expr::var("global_triplet_base"), Expr::u32(1)),
                    Expr::u32(0),
                ),
                Node::store(
                    out_tok_triplets_and_count,
                    Expr::add(Expr::var("global_triplet_base"), Expr::u32(2)),
                    Expr::u32(0),
                ),
                Node::let_bind(
                    "rank",
                    Expr::add(Expr::load(scratch_a, lane.clone()), Expr::var("offset")),
                ),
                Node::if_then(
                    Expr::ne(Expr::var("token_type"), Expr::u32(0)),
                    vec![
                        Node::let_bind(
                            "slot",
                            Expr::saturating_sub(Expr::var("rank"), Expr::u32(1)),
                        ),
                        Node::let_bind(
                            "slot_triplet_base",
                            Expr::add(Expr::mul(Expr::var("slot"), Expr::u32(3)), Expr::u32(1)),
                        ),
                        Node::store(
                            out_tok_triplets_and_count,
                            Expr::var("slot_triplet_base"),
                            Expr::var("token_type"),
                        ),
                        Node::store(
                            out_tok_triplets_and_count,
                            Expr::add(Expr::var("slot_triplet_base"), Expr::u32(1)),
                            Expr::load(sparse_starts, global.clone()),
                        ),
                        Node::store(
                            out_tok_triplets_and_count,
                            Expr::add(Expr::var("slot_triplet_base"), Expr::u32(2)),
                            Expr::load(sparse_lens, global.clone()),
                        ),
                    ],
                ),
                Node::if_then(
                    Expr::eq(Expr::add(global, Expr::u32(1)), Expr::u32(count)),
                    vec![Node::store(
                        out_tok_triplets_and_count,
                        Expr::u32(0),
                        Expr::var("rank"),
                    )],
                ),
            ],
        ),
    ]);
    Program::wrapped(
        vec![
            BufferDecl::storage(
                block_totals_scanned,
                0,
                BufferAccess::ReadOnly,
                DataType::U32,
            ),
            BufferDecl::storage(sparse_types, 1, BufferAccess::ReadOnly, DataType::U32),
            BufferDecl::storage(sparse_starts, 2, BufferAccess::ReadOnly, DataType::U32),
            BufferDecl::storage(sparse_lens, 3, BufferAccess::ReadOnly, DataType::U32),
            BufferDecl::output(out_tok_triplets_and_count, 4, DataType::U32)
                .with_count(count.saturating_mul(3).saturating_add(1)),
            BufferDecl::workgroup(scratch_a, BLOCK_LANES, DataType::U32),
            BufferDecl::workgroup(scratch_b, BLOCK_LANES, DataType::U32),
        ],
        [BLOCK_LANES, 1, 1],
        vec![Node::Region {
            generator: "vyre-frontend-c::raw_sparse_token_block_compact".into(),
            source_region: None,
            body: Arc::new(body),
        }],
    )
    .with_entry_op_id("vyre-frontend-c::raw_sparse_token_block_compact")
    .with_non_composable_with_self(true)
}

pub(super) fn sparse_token_type_block_compact_program(
    block_totals_scanned: &str,
    sparse_types: &str,
    out_tok_types_and_count: &str,
    count: u32,
    num_blocks: u32,
) -> Program {
    use vyre_primitives::reduce::multi_block_prefix_scan::BLOCK_LANES;

    let lane = Expr::var("lane");
    let block = Expr::var("block");
    let global = Expr::var("global");
    let scratch_a = "__raw_sparse_type_compact_a";
    let scratch_b = "__raw_sparse_type_compact_b";
    let active_lane = Expr::lt(global.clone(), Expr::u32(count));
    let active_block = Expr::lt(block.clone(), Expr::u32(num_blocks));
    let mut body = vec![
        Node::let_bind("lane", Expr::LocalId { axis: 0 }),
        Node::let_bind("block", Expr::WorkgroupId { axis: 0 }),
        Node::let_bind(
            "global",
            Expr::add(
                Expr::mul(block.clone(), Expr::u32(BLOCK_LANES)),
                lane.clone(),
            ),
        ),
        Node::store(scratch_a, lane.clone(), Expr::u32(0)),
        Node::if_then(
            active_lane.clone(),
            vec![Node::store(
                scratch_a,
                lane.clone(),
                Expr::select(
                    Expr::ne(Expr::load(sparse_types, global.clone()), Expr::u32(0)),
                    Expr::u32(1),
                    Expr::u32(0),
                ),
            )],
        ),
        Node::Barrier {
            ordering: vyre_foundation::MemoryOrdering::SeqCst,
        },
    ];
    let mut stride = 1_u32;
    while stride < BLOCK_LANES {
        body.push(Node::store(
            scratch_b,
            lane.clone(),
            Expr::load(scratch_a, lane.clone()),
        ));
        body.push(Node::if_then(
            Expr::lt(Expr::u32(stride - 1), lane.clone()),
            vec![Node::store(
                scratch_b,
                lane.clone(),
                Expr::add(
                    Expr::load(scratch_a, lane.clone()),
                    Expr::load(
                        scratch_a,
                        Expr::add(lane.clone(), Expr::u32(0u32.wrapping_sub(stride))),
                    ),
                ),
            )],
        ));
        body.push(Node::Barrier {
            ordering: vyre_foundation::MemoryOrdering::SeqCst,
        });
        body.push(Node::store(
            scratch_a,
            lane.clone(),
            Expr::load(scratch_b, lane.clone()),
        ));
        body.push(Node::Barrier {
            ordering: vyre_foundation::MemoryOrdering::SeqCst,
        });
        stride = stride.saturating_mul(2);
    }
    body.extend([
        Node::let_bind("offset", Expr::u32(0)),
        Node::if_then(
            Expr::and(Expr::lt(Expr::u32(0), block.clone()), active_block),
            vec![Node::assign(
                "offset",
                Expr::load(
                    block_totals_scanned,
                    Expr::add(block.clone(), Expr::u32(0u32.wrapping_sub(1))),
                ),
            )],
        ),
        Node::if_then(
            active_lane,
            vec![
                Node::let_bind("token_type", Expr::load(sparse_types, global.clone())),
                Node::let_bind(
                    "rank",
                    Expr::add(Expr::load(scratch_a, lane.clone()), Expr::var("offset")),
                ),
                Node::if_then(
                    Expr::ne(Expr::var("token_type"), Expr::u32(0)),
                    vec![Node::store(
                        out_tok_types_and_count,
                        Expr::add(
                            Expr::saturating_sub(Expr::var("rank"), Expr::u32(1)),
                            Expr::u32(1),
                        ),
                        Expr::var("token_type"),
                    )],
                ),
                Node::if_then(
                    Expr::eq(Expr::add(global, Expr::u32(1)), Expr::u32(count)),
                    vec![Node::store(
                        out_tok_types_and_count,
                        Expr::u32(0),
                        Expr::var("rank"),
                    )],
                ),
            ],
        ),
    ]);
    Program::wrapped(
        vec![
            BufferDecl::storage(
                block_totals_scanned,
                0,
                BufferAccess::ReadOnly,
                DataType::U32,
            ),
            BufferDecl::storage(sparse_types, 1, BufferAccess::ReadOnly, DataType::U32),
            BufferDecl::output(out_tok_types_and_count, 2, DataType::U32)
                .with_count(count.saturating_add(1)),
            BufferDecl::workgroup(scratch_a, BLOCK_LANES, DataType::U32),
            BufferDecl::workgroup(scratch_b, BLOCK_LANES, DataType::U32),
        ],
        [BLOCK_LANES, 1, 1],
        vec![Node::Region {
            generator: "vyre-frontend-c::raw_sparse_token_type_block_compact".into(),
            source_region: None,
            body: Arc::new(body),
        }],
    )
    .with_entry_op_id("vyre-frontend-c::raw_sparse_token_type_block_compact")
    .with_non_composable_with_self(true)
}
