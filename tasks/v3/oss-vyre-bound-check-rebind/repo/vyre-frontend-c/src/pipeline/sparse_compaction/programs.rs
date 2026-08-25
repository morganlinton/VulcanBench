use std::sync::Arc;

use vyre::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

pub(super) fn block_totals_nonzero_scan(
    sparse_types: &str,
    block_totals: &str,
    n: u32,
    num_blocks: u32,
) -> Program {
    use vyre_primitives::reduce::multi_block_prefix_scan::BLOCK_LANES;

    let lane = Expr::var("lane");
    let block = Expr::var("block");
    let global = Expr::var("global");
    let scratch_a = "__sparse_type_block_totals_scratch_a";
    let scratch_b = "__sparse_type_block_totals_scratch_b";

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
            Expr::lt(global.clone(), Expr::u32(n)),
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
            ordering: vyre_foundation::memory_model::MemoryOrdering::SeqCst,
        },
    ];
    let mut stride = 1_u32;
    while stride < BLOCK_LANES {
        body.push(Node::store(
            scratch_b,
            lane.clone(),
            Expr::load(scratch_a, lane.clone()),
        ));
        let previous_lane = Expr::add(lane.clone(), Expr::u32(0u32.wrapping_sub(stride)));
        body.push(Node::if_then(
            Expr::lt(Expr::u32(stride - 1), lane.clone()),
            vec![Node::store(
                scratch_b,
                lane.clone(),
                Expr::add(
                    Expr::load(scratch_a, lane.clone()),
                    Expr::load(scratch_a, previous_lane),
                ),
            )],
        ));
        body.push(Node::Barrier {
            ordering: vyre_foundation::memory_model::MemoryOrdering::SeqCst,
        });
        body.push(Node::store(
            scratch_a,
            lane.clone(),
            Expr::load(scratch_b, lane.clone()),
        ));
        body.push(Node::Barrier {
            ordering: vyre_foundation::memory_model::MemoryOrdering::SeqCst,
        });
        stride *= 2;
    }
    body.push(Node::if_then(
        Expr::eq(lane, Expr::u32(BLOCK_LANES - 1)),
        vec![Node::store(
            block_totals,
            block,
            Expr::load(scratch_a, Expr::u32(BLOCK_LANES - 1)),
        )],
    ));

    Program::wrapped(
        vec![
            BufferDecl::storage(sparse_types, 0, BufferAccess::ReadOnly, DataType::U32)
                .with_count(n.max(1)),
            BufferDecl::output(block_totals, 1, DataType::U32).with_count(num_blocks),
            BufferDecl::workgroup(scratch_a, BLOCK_LANES, DataType::U32),
            BufferDecl::workgroup(scratch_b, BLOCK_LANES, DataType::U32),
        ],
        [BLOCK_LANES, 1, 1],
        vec![Node::Region {
            generator: "vyre-frontend-c::block_totals_nonzero_scan".into(),
            source_region: None,
            body: Arc::new(body),
        }],
    )
    .with_entry_op_id("vyre-frontend-c::block_totals_nonzero_scan")
    .with_non_composable_with_self(true)
}

pub(in crate::pipeline) fn pass_c_rescan_compact_sparse_tokens(
    block_totals_scanned: &str,
    sparse_types: &str,
    sparse_starts: &str,
    sparse_lens: &str,
    out_tok_types: &str,
    out_tok_starts: &str,
    out_tok_lens: &str,
    out_counts: &str,
    n: u32,
    num_blocks: u32,
) -> Program {
    pass_c_rescan_compact_sparse_tokens_with_capacity(
        block_totals_scanned,
        sparse_types,
        sparse_starts,
        sparse_lens,
        out_tok_types,
        out_tok_starts,
        out_tok_lens,
        out_counts,
        n,
        num_blocks,
        n,
    )
}

pub(in crate::pipeline) fn pass_c_rescan_compact_sparse_tokens_with_capacity(
    block_totals_scanned: &str,
    sparse_types: &str,
    sparse_starts: &str,
    sparse_lens: &str,
    out_tok_types: &str,
    out_tok_starts: &str,
    out_tok_lens: &str,
    out_counts: &str,
    n: u32,
    num_blocks: u32,
    output_capacity: u32,
) -> Program {
    use vyre_primitives::reduce::multi_block_prefix_scan::BLOCK_LANES;

    let lane = Expr::var("lane");
    let block = Expr::var("block");
    let global = Expr::var("global");
    let offset = Expr::var("offset");
    let scratch_a = "__sparse_token_compact_scratch_a";
    let scratch_b = "__sparse_token_compact_scratch_b";
    let bounded_output_capacity = output_capacity.max(1);

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
            Expr::lt(global.clone(), Expr::u32(n)),
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
            ordering: vyre_foundation::memory_model::MemoryOrdering::SeqCst,
        },
    ];
    let mut stride = 1_u32;
    while stride < BLOCK_LANES {
        body.push(Node::store(
            scratch_b,
            lane.clone(),
            Expr::load(scratch_a, lane.clone()),
        ));
        let previous_lane = Expr::add(lane.clone(), Expr::u32(0u32.wrapping_sub(stride)));
        body.push(Node::if_then(
            Expr::lt(Expr::u32(stride - 1), lane.clone()),
            vec![Node::store(
                scratch_b,
                lane.clone(),
                Expr::add(
                    Expr::load(scratch_a, lane.clone()),
                    Expr::load(scratch_a, previous_lane),
                ),
            )],
        ));
        body.push(Node::Barrier {
            ordering: vyre_foundation::memory_model::MemoryOrdering::SeqCst,
        });
        body.push(Node::store(
            scratch_a,
            lane.clone(),
            Expr::load(scratch_b, lane.clone()),
        ));
        body.push(Node::Barrier {
            ordering: vyre_foundation::memory_model::MemoryOrdering::SeqCst,
        });
        stride *= 2;
    }
    body.extend([
        Node::let_bind("offset", Expr::u32(0)),
        Node::if_then(
            Expr::lt(Expr::u32(0), block.clone()),
            vec![Node::assign(
                "offset",
                Expr::load(
                    block_totals_scanned,
                    Expr::add(block.clone(), Expr::u32(0u32.wrapping_sub(1))),
                ),
            )],
        ),
        Node::if_then(
            Expr::lt(global.clone(), Expr::u32(n)),
            vec![
                Node::let_bind("token_type", Expr::load(sparse_types, global.clone())),
                Node::let_bind("rank", Expr::add(Expr::load(scratch_a, lane), offset)),
                Node::if_then(
                    Expr::and(
                        Expr::ne(Expr::var("token_type"), Expr::u32(0)),
                        Expr::lt(
                            Expr::saturating_sub(Expr::var("rank"), Expr::u32(1)),
                            Expr::u32(bounded_output_capacity),
                        ),
                    ),
                    vec![
                        Node::let_bind(
                            "slot",
                            Expr::saturating_sub(Expr::var("rank"), Expr::u32(1)),
                        ),
                        Node::store(out_tok_types, Expr::var("slot"), Expr::var("token_type")),
                        Node::store(
                            out_tok_starts,
                            Expr::var("slot"),
                            Expr::load(sparse_starts, global.clone()),
                        ),
                        Node::store(
                            out_tok_lens,
                            Expr::var("slot"),
                            Expr::load(sparse_lens, global.clone()),
                        ),
                    ],
                ),
                Node::if_then(
                    Expr::eq(
                        global,
                        Expr::u32(n.saturating_sub(1)),
                    ),
                    vec![Node::store(out_counts, Expr::u32(0), Expr::var("rank"))],
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
            )
            .with_count(num_blocks),
            BufferDecl::storage(sparse_types, 1, BufferAccess::ReadOnly, DataType::U32)
                .with_count(n.max(1)),
            BufferDecl::storage(sparse_starts, 2, BufferAccess::ReadOnly, DataType::U32)
                .with_count(n.max(1)),
            BufferDecl::storage(sparse_lens, 3, BufferAccess::ReadOnly, DataType::U32)
                .with_count(n.max(1)),
            BufferDecl::storage(out_tok_types, 4, BufferAccess::ReadWrite, DataType::U32)
                .with_count(bounded_output_capacity),
            BufferDecl::storage(out_tok_starts, 5, BufferAccess::ReadWrite, DataType::U32)
                .with_count(bounded_output_capacity),
            BufferDecl::storage(out_tok_lens, 6, BufferAccess::ReadWrite, DataType::U32)
                .with_count(bounded_output_capacity),
            BufferDecl::output(out_counts, 7, DataType::U32).with_count(1),
            BufferDecl::workgroup(scratch_a, BLOCK_LANES, DataType::U32),
            BufferDecl::workgroup(scratch_b, BLOCK_LANES, DataType::U32),
        ],
        [BLOCK_LANES, 1, 1],
        vec![Node::Region {
            generator: "vyre-frontend-c::pass_c_rescan_compact_sparse_tokens".into(),
            source_region: None,
            body: Arc::new(body),
        }],
    )
    .with_entry_op_id("vyre-frontend-c::pass_c_rescan_compact_sparse_tokens")
    .with_non_composable_with_self(true)
}

#[cfg(test)]
mod tests {
    use super::{
        pass_c_rescan_compact_sparse_tokens, pass_c_rescan_compact_sparse_tokens_with_capacity,
    };

    #[test]
    fn capacity_aware_compact_keeps_scan_domain_but_shrinks_dense_outputs() {
        let program = pass_c_rescan_compact_sparse_tokens_with_capacity(
            "block_totals_scanned",
            "sparse_types",
            "sparse_starts",
            "sparse_lens",
            "out_tok_types",
            "out_tok_starts",
            "out_tok_lens",
            "out_counts",
            4096,
            4,
            17,
        );
        let buffer_count = |name: &str| {
            program
                .buffers()
                .iter()
                .find(|buffer| buffer.name() == name)
                .expect("Fix: sparse compact buffer should exist")
                .count()
        };

        assert_eq!(buffer_count("sparse_types"), 4096);
        assert_eq!(buffer_count("sparse_starts"), 4096);
        assert_eq!(buffer_count("sparse_lens"), 4096);
        assert_eq!(buffer_count("out_tok_types"), 17);
        assert_eq!(buffer_count("out_tok_starts"), 17);
        assert_eq!(buffer_count("out_tok_lens"), 17);
    }

    #[test]
    fn legacy_compact_keeps_haystack_length_capacity() {
        let program = pass_c_rescan_compact_sparse_tokens(
            "block_totals_scanned",
            "sparse_types",
            "sparse_starts",
            "sparse_lens",
            "out_tok_types",
            "out_tok_starts",
            "out_tok_lens",
            "out_counts",
            4096,
            4,
        );
        let out_types = program
            .buffers()
            .iter()
            .find(|buffer| buffer.name() == "out_tok_types")
            .expect("Fix: sparse compact output should exist");

        assert_eq!(out_types.count(), 4096);
    }

    #[test]
    fn zero_capacity_still_declares_one_physical_slot_for_empty_files() {
        let program = pass_c_rescan_compact_sparse_tokens_with_capacity(
            "block_totals_scanned",
            "sparse_types",
            "sparse_starts",
            "sparse_lens",
            "out_tok_types",
            "out_tok_starts",
            "out_tok_lens",
            "out_counts",
            1024,
            1,
            0,
        );
        let out_types = program
            .buffers()
            .iter()
            .find(|buffer| buffer.name() == "out_tok_types")
            .expect("Fix: sparse compact output should exist");

        assert_eq!(out_types.count(), 1);
    }

    #[test]
    fn capacity_aware_compact_declares_generated_output_counts_without_shrinking_scan_inputs() {
        for case in 0..2_048_u32 {
            let n = (case % 4_096) + 1;
            let num_blocks = n.div_ceil(1_024);
            let output_capacity = case.wrapping_mul(37) % n;
            let expected_output_count = output_capacity.max(1);
            let program = pass_c_rescan_compact_sparse_tokens_with_capacity(
                "block_totals_scanned",
                "sparse_types",
                "sparse_starts",
                "sparse_lens",
                "out_tok_types",
                "out_tok_starts",
                "out_tok_lens",
                "out_counts",
                n,
                num_blocks,
                output_capacity,
            );
            let buffer_count = |name: &str| {
                program
                    .buffers()
                    .iter()
                    .find(|buffer| buffer.name() == name)
                    .expect("Fix: generated sparse compact buffer should exist")
                    .count()
            };

            assert_eq!(buffer_count("block_totals_scanned"), num_blocks);
            assert_eq!(buffer_count("sparse_types"), n);
            assert_eq!(buffer_count("sparse_starts"), n);
            assert_eq!(buffer_count("sparse_lens"), n);
            assert_eq!(buffer_count("out_tok_types"), expected_output_count);
            assert_eq!(buffer_count("out_tok_starts"), expected_output_count);
            assert_eq!(buffer_count("out_tok_lens"), expected_output_count);
            assert_eq!(buffer_count("out_counts"), 1);
        }
    }
}
