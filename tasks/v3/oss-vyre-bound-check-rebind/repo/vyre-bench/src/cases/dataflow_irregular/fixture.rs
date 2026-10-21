use crate::api::case::BenchError;
use vyre_primitives::bitset::frontier::materialize_frontier_queue_exact_count_into;
use vyre_primitives::predicate::edge_kind;

pub(super) const NODE_COUNT: u32 = 1_048_576;
pub(super) const FRONTIER_WORDS: usize = NODE_COUNT.div_ceil(32) as usize;
pub(super) const IFDS_REACH_MASK: u32 = edge_kind::ASSIGNMENT
    | edge_kind::CALL_ARG
    | edge_kind::RETURN
    | edge_kind::PHI
    | edge_kind::ALIAS
    | edge_kind::MEM_STORE
    | edge_kind::MEM_LOAD
    | edge_kind::MUT_REF;

const HIGH_DEGREE_THRESHOLD: u32 = 24;
pub(super) const UGLY_HUB_DEGREE: u32 = 2_048;

#[derive(Clone, Copy, Debug, Default)]
pub(super) struct IfdsSkewedStats {
    pub(super) nodes: u32,
    pub(super) edges: u32,
    pub(super) frontier_words: u32,
    pub(super) active_sources: u64,
    pub(super) allowed_edges_from_active: u64,
    pub(super) filtered_edges_from_active: u64,
    pub(super) output_words_set: u64,
    pub(super) max_degree: u32,
    pub(super) high_degree_sources: u64,
}

pub(super) struct IfdsSkewedFixture {
    pub(super) nodes: Vec<u32>,
    pub(super) edge_offsets: Vec<u32>,
    pub(super) edge_targets: Vec<u32>,
    pub(super) edge_kind_mask: Vec<u32>,
    pub(super) node_tags: Vec<u32>,
    pub(super) frontier_in: Vec<u32>,
    pub(super) frontier_out_seed: Vec<u32>,
    pub(super) stats: IfdsSkewedStats,
}

pub(super) struct IfdsSkewedOracle {
    pub(super) output: Vec<u32>,
    pub(super) allowed_edges_from_active: u64,
    pub(super) filtered_edges_from_active: u64,
    pub(super) output_words_set: u64,
}

pub(super) struct IfdsSkewedClosureOracle {
    pub(super) output: Vec<u32>,
    pub(super) changed: u32,
    pub(super) iterations: u32,
    pub(super) output_words_set: u64,
}

pub(super) fn build_ifds_skewed_fixture(node_count: u32) -> Result<IfdsSkewedFixture, BenchError> {
    if !node_count.is_power_of_two() || node_count < 32 {
        return Err(BenchError::EnvironmentInvalid(format!(
            "IFDS skewed fixture requires a power-of-two node count >= 32, received {node_count}. Fix: choose a power-of-two exploded-supergraph size."
        )));
    }

    let words = node_count.div_ceil(32);
    let mut nodes = Vec::with_capacity(node_count as usize);
    let mut edge_offsets = Vec::with_capacity(node_count as usize + 1);
    let mut edge_targets = Vec::with_capacity((node_count as usize).saturating_mul(2));
    let mut edge_kind_mask = Vec::with_capacity((node_count as usize).saturating_mul(2));
    let mut node_tags = Vec::with_capacity(node_count as usize);
    let mut frontier_in = vec![0_u32; words as usize];

    let mut stats = IfdsSkewedStats {
        nodes: node_count,
        frontier_words: words,
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
        nodes.push(ifds_node_kind(src));
        node_tags.push(ifds_node_tag(src));
        for edge in 0..degree {
            edge_targets.push(skewed_target(node_count, src, edge));
            edge_kind_mask.push(ifds_edge_kind(src, edge));
        }
        edge_offsets.push(u32::try_from(edge_targets.len()).map_err(|_| {
            BenchError::EnvironmentInvalid(
                "IFDS skewed fixture exceeded u32 edge offsets. Fix: split the benchmark graph."
                    .to_string(),
            )
        })?);
    }

    stats.edges = u32::try_from(edge_targets.len()).map_err(|_| {
        BenchError::EnvironmentInvalid(
            "IFDS skewed fixture exceeded u32 edge count. Fix: split the benchmark graph."
                .to_string(),
        )
    })?;

    Ok(IfdsSkewedFixture {
        nodes,
        edge_offsets,
        edge_targets,
        edge_kind_mask,
        node_tags,
        frontier_in,
        frontier_out_seed: vec![0_u32; words as usize],
        stats,
    })
}

pub(super) fn ifds_skewed_inputs(fixture: &IfdsSkewedFixture) -> Vec<Vec<u8>> {
    let mut inputs = ifds_graph_inputs(fixture);
    inputs.push(vyre_primitives::wire::pack_u32_slice(&fixture.frontier_in));
    inputs.push(vyre_primitives::wire::pack_u32_slice(
        &fixture.frontier_out_seed,
    ));
    inputs
}

pub(super) fn ifds_queue_inputs(
    fixture: &IfdsSkewedFixture,
    queue_capacity: u32,
    high_degree_queue_capacity: u32,
) -> Result<Vec<Vec<u8>>, BenchError> {
    if u64::from(queue_capacity) < fixture.stats.active_sources {
        return Err(BenchError::EnvironmentInvalid(format!(
            "IFDS queue fixture requires queue_capacity >= active_sources, got capacity={queue_capacity} active_sources={}. Fix: size the sparse frontier queue from fixture stats.",
            fixture.stats.active_sources
        )));
    }
    if high_degree_queue_capacity > queue_capacity {
        return Err(BenchError::EnvironmentInvalid(format!(
            "IFDS split queue fixture requires high_degree_queue_capacity <= queue_capacity, got high_degree_queue_capacity={high_degree_queue_capacity} queue_capacity={queue_capacity}. Fix: derive high-degree capacity from active sources."
        )));
    }
    let queue_bytes = (queue_capacity as usize)
        .checked_mul(std::mem::size_of::<u32>())
        .ok_or_else(|| {
            BenchError::EnvironmentInvalid(format!(
                "IFDS queue fixture queue_capacity={queue_capacity} overflows host buffer sizing. Fix: split the frontier queue."
            ))
        })?;
    let high_queue_bytes = (high_degree_queue_capacity as usize)
        .checked_mul(std::mem::size_of::<u32>())
        .ok_or_else(|| {
            BenchError::EnvironmentInvalid(format!(
                "IFDS high_degree_queue_capacity={high_degree_queue_capacity} overflows host buffer sizing. Fix: split the high-degree queue."
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

pub(super) fn ifds_active_high_degree_sources(
    fixture: &IfdsSkewedFixture,
    min_degree: u32,
) -> Result<u32, BenchError> {
    let mut high_sources = 0_u32;
    for src in 0..fixture.stats.nodes {
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
                    "IFDS split queue high-degree active source count exceeded u32. Fix: split the frontier queue."
                        .to_string(),
                )
            })?;
        }
    }
    Ok(high_sources)
}

pub(super) fn ifds_active_queue_inputs(
    fixture: &IfdsSkewedFixture,
    queue_capacity: u32,
) -> Result<Vec<Vec<u8>>, BenchError> {
    if u64::from(queue_capacity) < fixture.stats.active_sources {
        return Err(BenchError::EnvironmentInvalid(format!(
            "IFDS active-queue fixture requires queue_capacity >= active_sources, got capacity={queue_capacity} active_sources={}. Fix: size the sparse frontier queue from fixture stats.",
            fixture.stats.active_sources
        )));
    }
    let capacity = queue_capacity as usize;
    let mut active_queue =
        materialize_ifds_active_queue(fixture, capacity, "IFDS active-queue fixture")?;
    active_queue.resize(capacity, 0);

    Ok(vec![
        vyre_primitives::wire::pack_u32_slice(&active_queue),
        vyre_primitives::wire::pack_u32_slice(&[fixture.stats.active_sources as u32]),
        vyre_primitives::wire::pack_u32_slice(&fixture.edge_offsets),
        vyre_primitives::wire::pack_u32_slice(&fixture.edge_targets),
        vyre_primitives::wire::pack_u32_slice(&fixture.edge_kind_mask),
        vyre_primitives::wire::pack_u32_slice(&fixture.frontier_out_seed),
    ])
}

pub(in crate::cases::dataflow_irregular) fn materialize_ifds_active_queue(
    fixture: &IfdsSkewedFixture,
    queue_capacity: usize,
    context: &str,
) -> Result<Vec<u32>, BenchError> {
    let mut active_queue = Vec::new();
    let expected = u32::try_from(fixture.stats.active_sources).map_err(|_| {
        BenchError::EnvironmentInvalid(format!(
            "{context} active source count {} exceeds u32 indexing. Fix: split the frontier.",
            fixture.stats.active_sources
        ))
    })?;
    let seen = materialize_frontier_queue_exact_count_into(
        fixture.stats.nodes,
        &fixture.frontier_in,
        expected,
        queue_capacity,
        &mut active_queue,
    )
    .map_err(|error| {
        BenchError::EnvironmentInvalid(format!(
            "{context} could not materialize the sparse frontier queue: {error} Fix: size the queue from the active frontier and rebuild stale fixture stats."
        ))
    })?;
    if u64::from(seen) != fixture.stats.active_sources {
        return Err(BenchError::EnvironmentInvalid(format!(
            "{context} counted {seen} active sources but stats recorded {}. Fix: rebuild the fixture active frontier stats from the same bitset.",
            fixture.stats.active_sources
        )));
    }
    Ok(active_queue)
}

pub(super) fn ifds_closure_inputs(fixture: &IfdsSkewedFixture) -> Vec<Vec<u8>> {
    let mut inputs = ifds_graph_inputs(fixture);
    inputs.push(vyre_primitives::wire::pack_u32_slice(&fixture.frontier_in));
    inputs.push(vyre_primitives::wire::pack_u32_slice(&[0]));
    inputs
}

pub(super) fn ifds_closure_resident_inputs(fixture: &IfdsSkewedFixture) -> Vec<Vec<u8>> {
    let mut inputs = ifds_graph_inputs(fixture);
    let seed = vyre_primitives::wire::pack_u32_slice(&fixture.frontier_in);
    inputs.push(seed.clone());
    inputs.push(seed);
    inputs.push(vyre_primitives::wire::pack_u32_slice(&[0]));
    inputs
}

pub(super) fn ifds_skewed_cpu_oracle(fixture: &IfdsSkewedFixture) -> IfdsSkewedOracle {
    let mut output = fixture.frontier_out_seed.clone();
    let mut allowed_edges_from_active = 0_u64;
    let mut filtered_edges_from_active = 0_u64;

    for src in 0..fixture.stats.nodes {
        let src_word = (src / 32) as usize;
        let src_bit = 1_u32 << (src % 32);
        if (fixture.frontier_in[src_word] & src_bit) == 0 {
            continue;
        }
        let start = fixture.edge_offsets[src as usize] as usize;
        let end = fixture.edge_offsets[src as usize + 1] as usize;
        for edge in start..end {
            if (fixture.edge_kind_mask[edge] & IFDS_REACH_MASK) == 0 {
                filtered_edges_from_active += 1;
                continue;
            }
            allowed_edges_from_active += 1;
            let dst = fixture.edge_targets[edge];
            if dst < fixture.stats.nodes {
                output[(dst / 32) as usize] |= 1_u32 << (dst % 32);
            }
        }
    }

    IfdsSkewedOracle {
        output_words_set: output.iter().filter(|word| **word != 0).count() as u64,
        output,
        allowed_edges_from_active,
        filtered_edges_from_active,
    }
}

pub(super) fn ifds_skewed_closure_oracle(
    fixture: &IfdsSkewedFixture,
    max_iters: u32,
) -> IfdsSkewedClosureOracle {
    let mut current = Vec::new();
    let mut next = Vec::new();
    let mut iterations = 0_u32;
    vyre_primitives::graph::csr_forward_or_changed::cpu_ref_closure_into_with_step_hook(
        fixture.stats.nodes,
        &fixture.edge_offsets,
        &fixture.edge_targets,
        &fixture.edge_kind_mask,
        &fixture.frontier_in,
        IFDS_REACH_MASK,
        max_iters,
        &mut current,
        &mut next,
        |_| {
            iterations = iterations.saturating_add(1);
        },
    );
    let changed = u32::from(current != fixture.frontier_in);
    IfdsSkewedClosureOracle {
        output_words_set: current.iter().filter(|word| **word != 0).count() as u64,
        output: current,
        changed,
        iterations,
    }
}

pub(super) fn ifds_skewed_launch_wave_iterations(
    fixture: &IfdsSkewedFixture,
    max_iters: u32,
) -> u32 {
    let mut current = fixture.frontier_in.clone();
    let mut next = current.clone();
    let mut changed_passes = 0_u32;
    for iteration in 1..=max_iters {
        next.clear();
        next.extend_from_slice(&current);
        if expand_one_launch_wave(fixture, &current, &mut next) == 0 {
            return changed_passes.max(1);
        }
        changed_passes = iteration;
        std::mem::swap(&mut current, &mut next);
    }
    max_iters
}

fn ifds_graph_inputs(fixture: &IfdsSkewedFixture) -> Vec<Vec<u8>> {
    vec![
        vyre_primitives::wire::pack_u32_slice(&fixture.nodes),
        vyre_primitives::wire::pack_u32_slice(&fixture.edge_offsets),
        vyre_primitives::wire::pack_u32_slice(&fixture.edge_targets),
        vyre_primitives::wire::pack_u32_slice(&fixture.edge_kind_mask),
        vyre_primitives::wire::pack_u32_slice(&fixture.node_tags),
    ]
}

fn expand_one_launch_wave(
    fixture: &IfdsSkewedFixture,
    snapshot: &[u32],
    output: &mut [u32],
) -> u32 {
    let mut changed = 0_u32;
    for src in 0..fixture.stats.nodes {
        let src_word = (src / 32) as usize;
        let src_bit = 1_u32 << (src % 32);
        if (snapshot[src_word] & src_bit) == 0 {
            continue;
        }
        let start = fixture.edge_offsets[src as usize] as usize;
        let end = fixture.edge_offsets[src as usize + 1] as usize;
        for edge in start..end {
            if (fixture.edge_kind_mask[edge] & IFDS_REACH_MASK) == 0 {
                continue;
            }
            let dst = fixture.edge_targets[edge];
            if dst >= fixture.stats.nodes {
                continue;
            }
            let dst_word = (dst / 32) as usize;
            let dst_bit = 1_u32 << (dst % 32);
            let old = output[dst_word];
            output[dst_word] = old | dst_bit;
            if output[dst_word] != old {
                changed = 1;
            }
        }
    }
    changed
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

fn ifds_edge_kind(src: u32, edge: u32) -> u32 {
    const ALLOWED: [u32; 8] = [
        edge_kind::ASSIGNMENT,
        edge_kind::CALL_ARG,
        edge_kind::RETURN,
        edge_kind::PHI,
        edge_kind::ALIAS,
        edge_kind::MEM_STORE,
        edge_kind::MEM_LOAD,
        edge_kind::MUT_REF,
    ];
    match mix32(src ^ edge.wrapping_mul(0xA5A5_9651)) & 15 {
        0 => edge_kind::DOMINANCE,
        1 => edge_kind::CONTROL,
        value => ALLOWED[(value as usize - 2) % ALLOWED.len()],
    }
}

fn ifds_node_kind(src: u32) -> u32 {
    mix32(src ^ 0x1F_D5_0001) & 0x1F
}

fn ifds_node_tag(src: u32) -> u32 {
    let base = 1_u32 << (mix32(src ^ 0x51CE_7A6D) & 7);
    if src % 257 == 0 {
        base | 0x100
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
