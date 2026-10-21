use crate::api::case::BenchError;
use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

use super::QUEUE_CLOSURE_WORKGROUP_SIZE;
use crate::cases::dataflow_irregular::fixture::{
    materialize_ifds_active_queue, IfdsSkewedFixture, IFDS_REACH_MASK,
};

pub(in crate::cases::dataflow_irregular) struct QueueClosureOracle {
    pub(in crate::cases::dataflow_irregular) output: Vec<u32>,
    pub(in crate::cases::dataflow_irregular) iterations: u32,
    pub(in crate::cases::dataflow_irregular) changed: u32,
    pub(in crate::cases::dataflow_irregular) total_queue_pops: u64,
    pub(in crate::cases::dataflow_irregular) max_wave_queue_len: u32,
    pub(in crate::cases::dataflow_irregular) wave_queue_lengths: Vec<u32>,
}

pub(in crate::cases::dataflow_irregular) fn ifds_queue_closure_inputs(
    fixture: &IfdsSkewedFixture,
    queue_capacity: u32,
) -> Result<Vec<Vec<u8>>, BenchError> {
    if u64::from(queue_capacity) < fixture.stats.active_sources {
        return Err(BenchError::EnvironmentInvalid(format!(
            "IFDS queue closure requires queue_capacity >= active_sources, got capacity={queue_capacity} active_sources={}. Fix: size ping-pong queues for the seed frontier.",
            fixture.stats.active_sources
        )));
    }
    let seed_queue_len = u32::try_from(fixture.stats.active_sources).map_err(|_| {
        BenchError::EnvironmentInvalid(format!(
            "IFDS queue closure active source count {} exceeds u32 indexing. Fix: split the seed queue.",
            fixture.stats.active_sources
        ))
    })?;
    let queue_bytes = (queue_capacity as usize)
        .checked_mul(std::mem::size_of::<u32>())
        .ok_or_else(|| {
            BenchError::EnvironmentInvalid(format!(
                "IFDS queue closure queue_capacity={queue_capacity} overflows host buffer sizing. Fix: split the frontier queue."
            ))
        })?;
    let seed = vyre_primitives::wire::pack_u32_slice(&fixture.frontier_in);
    let seed_queue =
        materialize_ifds_active_queue(fixture, seed_queue_len as usize, "IFDS queue closure seed")?;

    Ok(vec![
        seed.clone(),
        vyre_primitives::wire::pack_u32_slice(&seed_queue),
        vyre_primitives::wire::pack_u32_slice(&[seed_queue_len]),
        vec![0_u8; queue_bytes],
        vyre_primitives::wire::pack_u32_slice(&[0]),
        vec![0_u8; queue_bytes],
        vyre_primitives::wire::pack_u32_slice(&[0]),
        vyre_primitives::wire::pack_u32_slice(&fixture.edge_offsets),
        vyre_primitives::wire::pack_u32_slice(&fixture.edge_targets),
        vyre_primitives::wire::pack_u32_slice(&fixture.edge_kind_mask),
        seed,
    ])
}

pub(in crate::cases::dataflow_irregular) fn ifds_queue_closure_reset_program(
    frontier_words: u32,
    seed_queue_len: u32,
    queue_capacity: u32,
) -> Program {
    let idx = Expr::InvocationId { axis: 0 };
    Program::wrapped(
        vec![
            BufferDecl::storage("frontier_seed", 0, BufferAccess::ReadOnly, DataType::U32)
                .with_count(frontier_words.max(1)),
            BufferDecl::storage("seed_queue", 1, BufferAccess::ReadOnly, DataType::U32)
                .with_count(seed_queue_len.max(1)),
            BufferDecl::storage("seed_len", 2, BufferAccess::ReadOnly, DataType::U32).with_count(1),
            BufferDecl::storage("active_queue", 3, BufferAccess::ReadWrite, DataType::U32)
                .with_count(queue_capacity.max(1)),
            BufferDecl::storage("accumulator", 4, BufferAccess::ReadWrite, DataType::U32)
                .with_count(frontier_words.max(1)),
            BufferDecl::storage("queue_a_len", 5, BufferAccess::ReadWrite, DataType::U32)
                .with_count(1),
            BufferDecl::storage("queue_b_len", 6, BufferAccess::ReadWrite, DataType::U32)
                .with_count(1),
        ],
        QUEUE_CLOSURE_WORKGROUP_SIZE,
        vec![
            Node::if_then(
                Expr::lt(idx.clone(), Expr::u32(frontier_words)),
                vec![Node::store(
                    "accumulator",
                    idx.clone(),
                    Expr::load("frontier_seed", idx.clone()),
                )],
            ),
            Node::if_then(
                Expr::and(
                    Expr::lt(idx.clone(), Expr::u32(queue_capacity)),
                    Expr::and(
                        Expr::lt(idx.clone(), Expr::u32(seed_queue_len)),
                        Expr::lt(idx.clone(), Expr::load("seed_len", Expr::u32(0))),
                    ),
                ),
                vec![Node::store(
                    "active_queue",
                    idx.clone(),
                    Expr::load("seed_queue", idx.clone()),
                )],
            ),
            Node::if_then(
                Expr::eq(idx, Expr::u32(0)),
                vec![
                    Node::store(
                        "queue_a_len",
                        Expr::u32(0),
                        Expr::load("seed_len", Expr::u32(0)),
                    ),
                    Node::store("queue_b_len", Expr::u32(0), Expr::u32(0)),
                ],
            ),
        ],
    )
}

pub(in crate::cases::dataflow_irregular) fn ifds_skewed_queue_closure_oracle(
    fixture: &IfdsSkewedFixture,
    max_iters: u32,
    queue_capacity: u32,
) -> Result<QueueClosureOracle, BenchError> {
    let capacity = queue_capacity as usize;
    let mut accumulator = fixture.frontier_in.clone();
    let mut current =
        materialize_ifds_active_queue(fixture, capacity, "IFDS queue closure oracle seed")?;
    let mut next = Vec::with_capacity(capacity.min(fixture.stats.nodes as usize));
    let mut iterations = 0_u32;
    let mut total_queue_pops = 0_u64;
    let mut max_wave_queue_len = current.len() as u32;
    let mut wave_queue_lengths = Vec::new();

    while !current.is_empty() && iterations < max_iters {
        wave_queue_lengths.push(current.len() as u32);
        max_wave_queue_len = max_wave_queue_len.max(current.len() as u32);
        total_queue_pops = total_queue_pops.saturating_add(current.len() as u64);
        next.clear();
        for &src in &current {
            if src >= fixture.stats.nodes {
                continue;
            }
            let start = fixture.edge_offsets[src as usize] as usize;
            let end = fixture.edge_offsets[src as usize + 1] as usize;
            for edge in start..end {
                if fixture.edge_kind_mask[edge] & IFDS_REACH_MASK == 0 {
                    continue;
                }
                let dst = fixture.edge_targets[edge];
                if dst >= fixture.stats.nodes {
                    continue;
                }
                let dst_word = dst as usize / 32;
                let dst_bit = 1_u32 << (dst % 32);
                if accumulator[dst_word] & dst_bit != 0 {
                    continue;
                }
                accumulator[dst_word] |= dst_bit;
                if next.len() >= capacity {
                    return Err(BenchError::EnvironmentInvalid(format!(
                        "IFDS queue closure next wave exceeded queue_capacity={queue_capacity}. Fix: increase queue capacity or shard closure waves."
                    )));
                }
                next.push(dst);
            }
        }
        iterations = iterations.saturating_add(1);
        std::mem::swap(&mut current, &mut next);
    }

    if !current.is_empty() {
        return Err(BenchError::EnvironmentInvalid(format!(
            "IFDS queue closure did not converge within {max_iters} queue waves. Fix: raise CLOSURE_MAX_ITERS or use a smaller fixture diameter."
        )));
    }

    Ok(QueueClosureOracle {
        changed: u32::from(accumulator != fixture.frontier_in),
        output: accumulator,
        iterations,
        total_queue_pops,
        max_wave_queue_len,
        wave_queue_lengths,
    })
}
