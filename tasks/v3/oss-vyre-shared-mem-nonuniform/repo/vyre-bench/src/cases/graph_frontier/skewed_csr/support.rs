use crate::api::case::BenchError;
use crate::api::suite::SuiteKind;
use vyre_primitives::bitset::frontier::materialize_frontier_queue_exact_count_into;

pub(super) const CSR_NODE_COUNT: u32 = 1_048_576;
pub(super) const CSR_ALLOW_MASK: u32 = 0b0111;
pub(super) const HIGH_DEGREE_THRESHOLD: u32 = 24;
pub(super) const UGLY_HUB_DEGREE: u32 = 2_048;
pub(super) const SUITES: &[SuiteKind] = &[
    SuiteKind::Smoke,
    SuiteKind::Release,
    SuiteKind::Gpu,
    SuiteKind::Deep,
    SuiteKind::Honest,
];

#[derive(Clone, Copy, Debug, Default)]
pub(super) struct SkewedCsrStats {
    pub(super) node_count: u32,
    pub(super) edge_count: u32,
    pub(super) frontier_words: u32,
    pub(super) active_sources: u64,
    pub(super) allowed_edges_from_active: u64,
    pub(super) output_words_set: u64,
    pub(super) max_degree: u32,
    pub(super) high_degree_sources: u64,
}

pub(super) struct SkewedCsrFixture {
    pub(super) nodes: Vec<u32>,
    pub(super) edge_offsets: Vec<u32>,
    pub(super) edge_targets: Vec<u32>,
    pub(super) edge_kind_mask: Vec<u32>,
    pub(super) node_tags: Vec<u32>,
    pub(super) frontier_in: Vec<u32>,
    pub(super) frontier_out_seed: Vec<u32>,
    pub(super) stats: SkewedCsrStats,
}

pub(super) struct SkewedCsrOracle {
    pub(super) output: Vec<u32>,
    pub(super) allowed_edges_from_active: u64,
    pub(super) output_words_set: u64,
}

pub(super) struct SkewedCsrQueueClosureOracle {
    pub(super) output: Vec<u32>,
    pub(super) changed: u32,
    pub(super) iterations: u32,
    pub(super) total_queue_pops: u64,
    pub(super) max_wave_queue_len: u32,
    pub(super) wave_queue_lengths: Vec<u32>,
}

pub(super) fn build_skewed_csr_fixture(node_count: u32) -> Result<SkewedCsrFixture, BenchError> {
    if !node_count.is_power_of_two() || node_count < 32 {
        return Err(BenchError::EnvironmentInvalid(format!(
            "skewed CSR fixture requires a power-of-two node count >= 32, received {node_count}. Fix: choose a power-of-two graph size so target generation stays branch-free."
        )));
    }

    let frontier_words = node_count.div_ceil(32);
    let mut nodes = Vec::with_capacity(node_count as usize);
    let mut edge_offsets = Vec::with_capacity(node_count as usize + 1);
    let mut edge_targets = Vec::with_capacity((node_count as usize).saturating_mul(2));
    let mut edge_kind_mask = Vec::with_capacity((node_count as usize).saturating_mul(2));
    let mut node_tags = Vec::with_capacity(node_count as usize);
    let mut frontier_in = vec![0_u32; frontier_words as usize];

    let mut stats = SkewedCsrStats {
        node_count,
        frontier_words,
        ..Default::default()
    };

    edge_offsets.push(0);
    for src in 0..node_count {
        let degree = skewed_degree(src);
        stats.max_degree = stats.max_degree.max(degree);
        if degree >= HIGH_DEGREE_THRESHOLD {
            stats.high_degree_sources += 1;
        }
        if source_is_active(src) {
            stats.active_sources += 1;
            frontier_in[(src / 32) as usize] |= 1_u32 << (src % 32);
        }
        nodes.push(mix32(src) & 0x1F);
        node_tags.push(skewed_node_tag(src));
        for edge in 0..degree {
            edge_targets.push(skewed_target(node_count, src, edge));
            edge_kind_mask.push(skewed_edge_kind(src, edge));
        }
        let offset = u32::try_from(edge_targets.len()).map_err(|_| {
            BenchError::EnvironmentInvalid(
                "skewed CSR fixture exceeded u32 edge offsets. Fix: split the benchmark graph."
                    .to_string(),
            )
        })?;
        edge_offsets.push(offset);
    }

    stats.edge_count = u32::try_from(edge_targets.len()).map_err(|_| {
        BenchError::EnvironmentInvalid(
            "skewed CSR fixture exceeded u32 edge count. Fix: split the benchmark graph."
                .to_string(),
        )
    })?;

    Ok(SkewedCsrFixture {
        nodes,
        edge_offsets,
        edge_targets,
        edge_kind_mask,
        node_tags,
        frontier_in,
        frontier_out_seed: vec![0_u32; frontier_words as usize],
        stats,
    })
}

pub(super) fn skewed_csr_inputs(fixture: &SkewedCsrFixture) -> Vec<Vec<u8>> {
    vec![
        vyre_primitives::wire::pack_u32_slice(&fixture.nodes),
        vyre_primitives::wire::pack_u32_slice(&fixture.edge_offsets),
        vyre_primitives::wire::pack_u32_slice(&fixture.edge_targets),
        vyre_primitives::wire::pack_u32_slice(&fixture.edge_kind_mask),
        vyre_primitives::wire::pack_u32_slice(&fixture.node_tags),
        vyre_primitives::wire::pack_u32_slice(&fixture.frontier_in),
        vyre_primitives::wire::pack_u32_slice(&fixture.frontier_out_seed),
    ]
}

pub(super) fn skewed_csr_queue_capacity(active_sources: u64) -> Result<u32, BenchError> {
    if active_sources == 0 {
        return Err(BenchError::EnvironmentInvalid(
            "skewed CSR queue benchmark requires at least one active source. Fix: seed the frontier before queue sizing."
                .to_string(),
        ));
    }
    u32::try_from(active_sources).map_err(|_| {
        BenchError::EnvironmentInvalid(format!(
            "skewed CSR active source count {active_sources} exceeds u32 indexing. Fix: split the frontier."
        ))
    })
}

pub(super) fn skewed_csr_queue_inputs(
    fixture: &SkewedCsrFixture,
    queue_capacity: u32,
    high_degree_queue_capacity: u32,
) -> Result<Vec<Vec<u8>>, BenchError> {
    if u64::from(queue_capacity) < fixture.stats.active_sources {
        return Err(BenchError::EnvironmentInvalid(format!(
            "skewed CSR queue inputs require queue_capacity >= active_sources, got queue_capacity={queue_capacity} active_sources={}. Fix: size the queue from the packed frontier popcount.",
            fixture.stats.active_sources
        )));
    }
    if high_degree_queue_capacity > queue_capacity {
        return Err(BenchError::EnvironmentInvalid(format!(
            "skewed CSR split queue inputs require high_degree_queue_capacity <= queue_capacity, got high_degree_queue_capacity={high_degree_queue_capacity} queue_capacity={queue_capacity}. Fix: derive high-degree capacity from active sources."
        )));
    }
    let queue_bytes = (queue_capacity as usize)
        .checked_mul(std::mem::size_of::<u32>())
        .ok_or_else(|| {
            BenchError::EnvironmentInvalid(format!(
                "skewed CSR queue_capacity={queue_capacity} overflows host buffer sizing. Fix: split the frontier queue."
            ))
        })?;
    let high_queue_bytes = (high_degree_queue_capacity as usize)
        .checked_mul(std::mem::size_of::<u32>())
        .ok_or_else(|| {
            BenchError::EnvironmentInvalid(format!(
                "skewed CSR high_degree_queue_capacity={high_degree_queue_capacity} overflows host buffer sizing. Fix: split the high-degree queue."
            ))
        })?;
    Ok(vec![
        vyre_primitives::wire::pack_u32_slice(&fixture.frontier_in),
        vec![0_u8; queue_bytes],
        vyre_primitives::wire::pack_u32_slice(&[0]),
        vyre_primitives::wire::pack_u32_slice(&fixture.edge_offsets),
        vyre_primitives::wire::pack_u32_slice(&fixture.edge_targets),
        vyre_primitives::wire::pack_u32_slice(&fixture.edge_kind_mask),
        vyre_primitives::wire::pack_u32_slice(&fixture.frontier_out_seed),
        vec![0_u8; high_queue_bytes],
        vyre_primitives::wire::pack_u32_slice(&[0]),
    ])
}

pub(super) fn skewed_csr_active_high_degree_sources(
    fixture: &SkewedCsrFixture,
    min_degree: u32,
) -> Result<u32, BenchError> {
    let mut high_sources = 0_u32;
    for src in 0..fixture.stats.node_count {
        let word = (src / 32) as usize;
        let bit = 1_u32 << (src % 32);
        if fixture.frontier_in[word] & bit == 0 {
            continue;
        }
        let start = fixture.edge_offsets[src as usize];
        let end = fixture.edge_offsets[src as usize + 1];
        if end.saturating_sub(start) >= min_degree {
            high_sources = high_sources.checked_add(1).ok_or_else(|| {
                BenchError::EnvironmentInvalid(
                    "skewed CSR split queue high-degree active source count exceeded u32. Fix: split the frontier queue."
                        .to_string(),
                )
            })?;
        }
    }
    Ok(high_sources)
}

pub(super) fn materialize_skewed_csr_active_queue(
    fixture: &SkewedCsrFixture,
    queue_capacity: usize,
    context: &str,
) -> Result<Vec<u32>, BenchError> {
    let expected = u32::try_from(fixture.stats.active_sources).map_err(|_| {
        BenchError::EnvironmentInvalid(format!(
            "{context} active source count {} exceeds u32 indexing. Fix: split the sparse graph frontier.",
            fixture.stats.active_sources
        ))
    })?;
    let mut active_queue = Vec::new();
    let seen = materialize_frontier_queue_exact_count_into(
        fixture.stats.node_count,
        &fixture.frontier_in,
        expected,
        queue_capacity,
        &mut active_queue,
    )
    .map_err(|error| {
        BenchError::EnvironmentInvalid(format!(
            "{context} could not materialize sparse graph frontier queue: {error} Fix: rebuild queue capacity from active source stats."
        ))
    })?;
    if u64::from(seen) != fixture.stats.active_sources {
        return Err(BenchError::EnvironmentInvalid(format!(
            "{context} counted {seen} active sources but fixture stats recorded {}. Fix: rebuild frontier stats from the same bitset.",
            fixture.stats.active_sources
        )));
    }
    Ok(active_queue)
}

pub(super) fn skewed_csr_queue_closure_inputs(
    fixture: &SkewedCsrFixture,
    queue_capacity: u32,
) -> Result<Vec<Vec<u8>>, BenchError> {
    if u64::from(queue_capacity) < fixture.stats.active_sources {
        return Err(BenchError::EnvironmentInvalid(format!(
            "skewed CSR queue closure requires queue_capacity >= active_sources, got capacity={queue_capacity} active_sources={}. Fix: size ping-pong queues for the seed frontier.",
            fixture.stats.active_sources
        )));
    }
    let seed_queue_len = u32::try_from(fixture.stats.active_sources).map_err(|_| {
        BenchError::EnvironmentInvalid(format!(
            "skewed CSR queue closure active source count {} exceeds u32 indexing. Fix: split the seed queue.",
            fixture.stats.active_sources
        ))
    })?;
    let queue_bytes = (queue_capacity as usize)
        .checked_mul(std::mem::size_of::<u32>())
        .ok_or_else(|| {
            BenchError::EnvironmentInvalid(format!(
                "skewed CSR queue closure queue_capacity={queue_capacity} overflows host buffer sizing. Fix: split the frontier queue."
            ))
        })?;
    let seed_frontier = vyre_primitives::wire::pack_u32_slice(&fixture.frontier_in);
    let seed_queue = materialize_skewed_csr_active_queue(
        fixture,
        seed_queue_len as usize,
        "skewed CSR queue closure seed",
    )?;

    Ok(vec![
        seed_frontier.clone(),
        vyre_primitives::wire::pack_u32_slice(&seed_queue),
        vyre_primitives::wire::pack_u32_slice(&[seed_queue_len]),
        vec![0_u8; queue_bytes],
        vyre_primitives::wire::pack_u32_slice(&[0]),
        vec![0_u8; queue_bytes],
        vyre_primitives::wire::pack_u32_slice(&[0]),
        vyre_primitives::wire::pack_u32_slice(&fixture.edge_offsets),
        vyre_primitives::wire::pack_u32_slice(&fixture.edge_targets),
        vyre_primitives::wire::pack_u32_slice(&fixture.edge_kind_mask),
        seed_frontier,
    ])
}

pub(super) fn skewed_csr_cpu_oracle(fixture: &SkewedCsrFixture) -> SkewedCsrOracle {
    let node_count = fixture.stats.node_count;
    let mut output = fixture.frontier_out_seed.clone();
    let mut allowed_edges_from_active = 0_u64;

    for src in 0..node_count {
        let src_word = (src / 32) as usize;
        let src_bit = 1_u32 << (src % 32);
        if (fixture.frontier_in[src_word] & src_bit) == 0 {
            continue;
        }
        let edge_start = fixture.edge_offsets[src as usize] as usize;
        let edge_end = fixture.edge_offsets[src as usize + 1] as usize;
        for edge in edge_start..edge_end {
            if (fixture.edge_kind_mask[edge] & CSR_ALLOW_MASK) == 0 {
                continue;
            }
            allowed_edges_from_active += 1;
            let dst = fixture.edge_targets[edge];
            if dst < node_count {
                output[(dst / 32) as usize] |= 1_u32 << (dst % 32);
            }
        }
    }

    SkewedCsrOracle {
        output_words_set: output.iter().filter(|word| **word != 0).count() as u64,
        output,
        allowed_edges_from_active,
    }
}

pub(super) fn skewed_csr_queue_closure_oracle(
    fixture: &SkewedCsrFixture,
    max_iters: u32,
    queue_capacity: u32,
) -> Result<SkewedCsrQueueClosureOracle, BenchError> {
    let capacity = queue_capacity as usize;
    let mut accumulator = fixture.frontier_in.clone();
    let mut current =
        materialize_skewed_csr_active_queue(fixture, capacity, "skewed CSR queue closure oracle")?;
    let mut next = Vec::with_capacity(capacity.min(fixture.stats.node_count as usize));
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
            if src >= fixture.stats.node_count {
                continue;
            }
            let start = fixture.edge_offsets[src as usize] as usize;
            let end = fixture.edge_offsets[src as usize + 1] as usize;
            for edge in start..end {
                if fixture.edge_kind_mask[edge] & CSR_ALLOW_MASK == 0 {
                    continue;
                }
                let dst = fixture.edge_targets[edge];
                if dst >= fixture.stats.node_count {
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
                        "skewed CSR queue closure next wave exceeded queue_capacity={queue_capacity}. Fix: increase queue capacity or shard closure waves."
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
            "skewed CSR queue closure did not converge within {max_iters} queue waves. Fix: raise the closure wave bound or use a smaller fixture diameter."
        )));
    }

    Ok(SkewedCsrQueueClosureOracle {
        changed: u32::from(accumulator != fixture.frontier_in),
        output: accumulator,
        iterations,
        total_queue_pops,
        max_wave_queue_len,
        wave_queue_lengths,
    })
}

fn skewed_degree(src: u32) -> u32 {
    if src % 4096 == 0 {
        UGLY_HUB_DEGREE
    } else if src % 257 == 0 {
        24
    } else if src % 31 == 0 {
        8
    } else if src % 7 == 0 {
        3
    } else {
        1
    }
}

fn skewed_target(node_count: u32, src: u32, edge: u32) -> u32 {
    let mask = node_count - 1;
    match edge & 7 {
        0 => src.wrapping_add((edge + 1).wrapping_mul(17)) & mask,
        1 => src.wrapping_sub((edge + 3).wrapping_mul(11)) & mask,
        _ => {
            let salt = edge.wrapping_mul(0x9E37_79B9).rotate_left((edge & 15) + 1);
            mix32(src ^ salt ^ src.rotate_left(edge & 15)) & mask
        }
    }
}

fn skewed_edge_kind(src: u32, edge: u32) -> u32 {
    1_u32 << (mix32(src ^ edge.wrapping_mul(0xA5A5_9651)) & 3)
}

fn skewed_node_tag(src: u32) -> u32 {
    let base = 1_u32 << (mix32(src ^ 0xC001_D00D) & 7);
    if src % 4096 == 0 {
        base | 0x80
    } else {
        base
    }
}

fn source_is_active(src: u32) -> bool {
    src % 97 == 0 || src % 4096 == 0 || (mix32(src ^ 0xD1B5_4A32) & 0x3FF) == 0
}

fn mix32(mut value: u32) -> u32 {
    value ^= value >> 16;
    value = value.wrapping_mul(0x7FEB_352D);
    value ^= value >> 15;
    value = value.wrapping_mul(0x846C_A68B);
    value ^ (value >> 16)
}
