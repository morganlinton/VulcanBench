use std::sync::Arc;

use vyre::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};
use vyre_primitives::reduce::multi_block_prefix_scan::BLOCK_LANES;

pub(super) fn prefix_scan_nonzero_workgroup(in_buf: &str, out_buf: &str, n: u32) -> Program {
    let lanes = n.max(1).next_power_of_two().min(BLOCK_LANES);
    let lane = Expr::InvocationId { axis: 0 };
    let scratch_a = format!("__{out_buf}_nonzero_scan_a");
    let scratch_b = format!("__{out_buf}_nonzero_scan_b");

    let mut body = Vec::new();
    body.push(Node::store(&scratch_a, lane.clone(), Expr::u32(0)));
    body.push(Node::if_then(
        Expr::lt(lane.clone(), Expr::u32(n)),
        vec![Node::store(
            &scratch_a,
            lane.clone(),
            Expr::select(
                Expr::ne(Expr::load(in_buf, lane.clone()), Expr::u32(0)),
                Expr::u32(1),
                Expr::u32(0),
            ),
        )],
    ));
    body.push(Node::Barrier {
        ordering: vyre_foundation::MemoryOrdering::SeqCst,
    });

    let mut stride = 1_u32;
    while stride < lanes {
        let previous_lane = Expr::add(lane.clone(), Expr::u32(0u32.wrapping_sub(stride)));
        body.push(Node::store(
            &scratch_b,
            lane.clone(),
            Expr::load(&scratch_a, lane.clone()),
        ));
        body.push(Node::if_then(
            Expr::lt(Expr::u32(stride - 1), lane.clone()),
            vec![Node::store(
                &scratch_b,
                lane.clone(),
                Expr::add(
                    Expr::load(&scratch_a, lane.clone()),
                    Expr::load(&scratch_a, previous_lane),
                ),
            )],
        ));
        body.push(Node::Barrier {
            ordering: vyre_foundation::MemoryOrdering::SeqCst,
        });
        body.push(Node::store(
            &scratch_a,
            lane.clone(),
            Expr::load(&scratch_b, lane.clone()),
        ));
        body.push(Node::Barrier {
            ordering: vyre_foundation::MemoryOrdering::SeqCst,
        });
        stride *= 2;
    }

    body.push(Node::if_then(
        Expr::lt(lane.clone(), Expr::u32(n)),
        vec![Node::store(
            out_buf,
            lane.clone(),
            Expr::load(&scratch_a, lane),
        )],
    ));
    let output_bytes = usize::try_from(n)
        .unwrap_or(usize::MAX)
        .saturating_mul(4);

    Program::wrapped(
        vec![
            BufferDecl::storage(in_buf, 0, BufferAccess::ReadOnly, DataType::U32)
                .with_count(n.max(1)),
            BufferDecl::output(out_buf, 1, DataType::U32)
                .with_count(n.max(1))
                .with_output_byte_range(0..output_bytes),
            BufferDecl::workgroup(&scratch_a, lanes, DataType::U32),
            BufferDecl::workgroup(&scratch_b, lanes, DataType::U32),
        ],
        [lanes, 1, 1],
        vec![Node::Region {
            generator: "vyre-frontend-c::prefix_scan_nonzero_workgroup".into(),
            source_region: None,
            body: Arc::new(body),
        }],
    )
    .with_entry_op_id("vyre-frontend-c::prefix_scan_nonzero_workgroup")
}
