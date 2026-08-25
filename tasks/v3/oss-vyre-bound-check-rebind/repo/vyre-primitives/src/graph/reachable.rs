//! Transitive reachability over an edge list  -  CPU reference + Tier-2.5
//! GPU Program builder.
//!
//! Consumed by taint analysis (`flows_to`) and graph analyses
//! that need "is B reachable from A given these edges?"
//!
//! AUDIT_2026-04-24 F-REACH-02 (RESOLVED): `reachable_program` now
//! ships as a Tier-2.5 builder. It runs a synchronized wavefront
//! closure in one dispatch: expand the current wave, absorb only
//! newly-discovered neighbors into `reach_out`, and feed those new bits
//! into the next wave. The CPU reference (`reachable`) is retained for
//! the conform harness cpu↔gpu bytecompare oracle.

use std::collections::HashSet;
use std::sync::Arc;

use vyre_foundation::ir::model::expr::Ident;
use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};
use vyre_foundation::MemoryOrdering;

use crate::bitset::bitset_words;
use crate::bitset::frontier::{
    frontier_absorb_new_bits_body_prefixed_with_flag, frontier_tail_mask,
};
use crate::graph::program_graph::{
    ProgramGraphShape, BINDING_PRIMITIVE_START, NAME_EDGE_KIND_MASK, NAME_EDGE_OFFSETS,
    NAME_EDGE_TARGETS,
};

/// Canonical op id.
pub const OP_ID: &str = "vyre-primitives::graph::reachable_program";

/// Error returned by [`reachable`] when the edge list contains a
/// node index outside `0..node_count`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnknownNode {
    /// Index into `edges` of the offending pair.
    pub index: usize,
    /// The out-of-range node id.
    pub node: u32,
    /// Total node count the graph was constructed with.
    pub node_count: u32,
}

impl std::fmt::Display for UnknownNode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "reachable: edges[{}] references node {} but node_count = {}. \
             Fix: callers must deduplicate and bounds-check edges before \
             calling this primitive.",
            self.index, self.node, self.node_count
        )
    }
}

impl std::error::Error for UnknownNode {}

/// Error returned by [`try_reachable`] for malformed graph input or allocation
/// failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReachableError {
    /// The edge list referenced a node outside `0..node_count`.
    UnknownNode(UnknownNode),
    /// Scratch allocation failed before traversal could complete.
    Allocation(String),
}

impl std::fmt::Display for ReachableError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownNode(error) => error.fmt(f),
            Self::Allocation(message) => f.write_str(message),
        }
    }
}

impl std::error::Error for ReachableError {}

impl From<UnknownNode> for ReachableError {
    fn from(error: UnknownNode) -> Self {
        Self::UnknownNode(error)
    }
}

/// CPU reference: returns the set of nodes reachable from any element
/// of `sources` following the directed edges. `edges` is a slice of
/// `(from, to)` u32 pairs  -  a BFS/DFS walks `from → to`.
///
/// AUDIT_2026-04-24 F-REACH-01: prior version silently dropped
/// edges whose `from` or `to` exceeded `node_count`, masking
/// upstream bugs that produce malformed edge lists. Now returns
/// [`UnknownNode`] so the violation is visible at the call site  -
/// consistent with how `toposort` surfaces the same shape of
/// failure.
pub fn reachable(
    node_count: u32,
    edges: &[(u32, u32)],
    sources: &[u32],
) -> Result<HashSet<u32>, UnknownNode> {
    match try_reachable(node_count, edges, sources) {
        Ok(result) => Ok(result),
        Err(ReachableError::UnknownNode(error)) => Err(error),
        Err(ReachableError::Allocation(_)) => Ok(HashSet::new()),
    }
}

/// Fallible CPU reference for transitive reachability.
///
/// Unlike [`reachable`], this surfaces allocation failure as a typed error, so
/// hostile graph dimensions cannot abort the process through infallible vector
/// growth.
pub fn try_reachable(
    node_count: u32,
    edges: &[(u32, u32)],
    sources: &[u32],
) -> Result<HashSet<u32>, ReachableError> {
    const NONE: usize = usize::MAX;

    let n = node_count as usize;
    for (index, &(from, to)) in edges.iter().enumerate() {
        if (from as usize) >= n {
            return Err(ReachableError::UnknownNode(UnknownNode {
                index,
                node: from,
                node_count,
            }));
        }
        if (to as usize) >= n {
            return Err(ReachableError::UnknownNode(UnknownNode {
                index,
                node: to,
                node_count,
            }));
        }
    }
    let mut head: Vec<usize> = Vec::new();
    crate::graph::scratch::reserve_graph_items(
        &mut head,
        n,
        "reachable CPU oracle",
        "adjacency heads",
    )
    .map_err(ReachableError::Allocation)?;
    head.resize(n, NONE);
    let mut to_nodes: Vec<u32> = Vec::new();
    crate::graph::scratch::reserve_graph_items(
        &mut to_nodes,
        edges.len(),
        "reachable CPU oracle",
        "adjacency destinations",
    )
    .map_err(ReachableError::Allocation)?;
    let mut next_edges: Vec<usize> = Vec::new();
    crate::graph::scratch::reserve_graph_items(
        &mut next_edges,
        edges.len(),
        "reachable CPU oracle",
        "adjacency next links",
    )
    .map_err(ReachableError::Allocation)?;
    for &(from, to) in edges {
        let edge_index = to_nodes.len();
        to_nodes.push(to);
        next_edges.push(head[from as usize]);
        head[from as usize] = edge_index;
    }
    let mut visited: Vec<bool> = Vec::new();
    crate::graph::scratch::reserve_graph_items(
        &mut visited,
        n,
        "reachable CPU oracle",
        "visited bitmap",
    )
    .map_err(ReachableError::Allocation)?;
    visited.resize(n, false);
    let mut out_of_range_sources: Vec<u32> = Vec::new();
    crate::graph::scratch::reserve_graph_items(
        &mut out_of_range_sources,
        sources.len(),
        "reachable CPU oracle",
        "out-of-range source list",
    )
    .map_err(ReachableError::Allocation)?;
    let mut stack: Vec<u32> = Vec::new();
    crate::graph::scratch::reserve_graph_items(
        &mut stack,
        sources.len(),
        "reachable CPU oracle",
        "DFS stack",
    )
    .map_err(ReachableError::Allocation)?;
    stack.extend_from_slice(sources);
    while let Some(v) = stack.pop() {
        let idx = v as usize;
        if idx >= n {
            out_of_range_sources.push(v);
            continue;
        }
        if visited[idx] {
            continue;
        }
        visited[idx] = true;
        let mut edge = head[idx];
        while edge != NONE {
            let next = to_nodes[edge];
            if !visited[next as usize] {
                stack.push(next);
            }
            edge = next_edges[edge];
        }
    }
    let result_capacity = visited
        .iter()
        .filter(|&&is_visited| is_visited)
        .count()
        .saturating_add(out_of_range_sources.len());
    let mut result = HashSet::new();
    result.try_reserve(result_capacity).map_err(|error| {
        ReachableError::Allocation(format!(
            "Fix: reachable CPU oracle could not reserve {result_capacity} result nodes: {error}"
        ))
    })?;
    for (idx, is_visited) in visited.into_iter().enumerate() {
        if is_visited {
            result.insert(idx as u32);
        }
    }
    result.extend(out_of_range_sources);
    Ok(result)
}

/// Build a Tier-2.5 GPU Program for transitive reachability.
///
/// The returned Program performs up to `max_iters` forward-traversal
/// steps over the CSR graph described by `shape`, starting from the
/// packed bitset `sources_buf`. It writes the visited set into
/// `reach_out`.
///
/// # Composition
///
/// 1. Copy `sources_buf` into `reach_out`.
/// 2. For each iteration `0..max_iters`:
///    - clear `reach_frontier_a`;
///    - expand the current wave into `reach_frontier_a`;
///    - absorb only not-yet-visited neighbors into `reach_out`;
///    - write those newly-added bits to `reach_frontier_b` for the next wave.
///
/// # Caller contract
///
/// * Bind the canonical five-buffer ProgramGraph CSR
///   (`pg_nodes`, `pg_edge_offsets`, `pg_edge_targets`,
///   `pg_edge_kind_mask`, `pg_node_tags`) before dispatch.
/// * `sources_buf` must be a packed bitset with `bitset_words(node_count)`
///   u32 words.
#[must_use]
pub fn reachable_program(
    node_count: u32,
    edge_count: u32,
    sources_buf: &str,
    reach_out: &str,
    max_iters: u32,
) -> Program {
    let shape = ProgramGraphShape::new(node_count, edge_count);
    let words = bitset_words(node_count);
    let frontier_a = "reach_frontier_a";
    let frontier_b = "reach_frontier_b";
    let active_flag_idx = words;
    let Some(frontier_b_storage_words) = words.checked_add(1) else {
        return crate::invalid_output_program(
            OP_ID,
            reach_out,
            DataType::U32,
            "Fix: reachable_program active-flag scratch word overflows u32.".to_string(),
        );
    };

    let Some(iter_nodes) = (max_iters as usize).checked_mul(8) else {
        return crate::invalid_output_program(
            OP_ID,
            reach_out,
            DataType::U32,
            "Fix: reachable_program max_iters*8 overflows usize.".to_string(),
        );
    };
    let Some(node_capacity) = iter_nodes.checked_add(4) else {
        return crate::invalid_output_program(
            OP_ID,
            reach_out,
            DataType::U32,
            "Fix: reachable_program node capacity overflows usize.".to_string(),
        );
    };
    let mut entry: Vec<Node> = Vec::new();
    if let Err(error) = entry.try_reserve(node_capacity) {
        return crate::invalid_output_program(
            OP_ID,
            reach_out,
            DataType::U32,
            format!("Fix: reachable_program could not reserve {node_capacity} IR nodes: {error}"),
        );
    }
    let lane = Expr::gid_x();

    entry.push(Node::if_then(
        Expr::lt(lane.clone(), Expr::u32(words)),
        vec![
            Node::store(
                reach_out,
                lane.clone(),
                Expr::load(sources_buf, lane.clone()),
            ),
            Node::store(frontier_a, lane.clone(), Expr::u32(0)),
            Node::store(frontier_b, lane.clone(), Expr::u32(0)),
        ],
    ));
    entry.push(Node::if_then(
        Expr::eq(lane.clone(), Expr::u32(0)),
        vec![Node::store(
            frontier_b,
            Expr::u32(active_flag_idx),
            Expr::u32(0),
        )],
    ));
    if max_iters > 0 {
        entry.push(reachable_wave_barrier(node_count));
    }

    for i in 0..max_iters {
        let current_wave = if i == 0 { sources_buf } else { frontier_b };
        let active_var = format!("iter_{i}_active");
        let active_expr = if i == 0 {
            Expr::u32(1)
        } else {
            Expr::load(frontier_b, Expr::u32(active_flag_idx))
        };
        let active_cond = Expr::ne(Expr::var(active_var.as_str()), Expr::u32(0));
        entry.push(Node::let_bind(active_var.as_str(), active_expr));
        entry.push(Node::if_then(
            Expr::lt(lane.clone(), Expr::u32(words)),
            vec![Node::store(frontier_a, lane.clone(), Expr::u32(0))],
        ));
        entry.push(reachable_wave_barrier(node_count));
        entry.push(Node::if_then(
            active_cond.clone(),
            vec![reachable_forward_wave_node(
                shape,
                current_wave,
                frontier_a,
                &format!("iter_{i}_expand"),
            )],
        ));
        entry.push(reachable_wave_barrier(node_count));
        entry.push(Node::if_then(
            Expr::eq(lane.clone(), Expr::u32(0)),
            vec![Node::store(
                frontier_b,
                Expr::u32(active_flag_idx),
                Expr::u32(0),
            )],
        ));
        entry.push(reachable_wave_barrier(node_count));
        entry.extend(frontier_absorb_new_bits_body_prefixed_with_flag(
            reach_out,
            frontier_a,
            frontier_b,
            None,
            Some((frontier_b, Expr::u32(active_flag_idx))),
            words,
            frontier_tail_mask(node_count),
            &format!("iter_{i}_absorb"),
        ));
        if i + 1 < max_iters {
            entry.push(reachable_wave_barrier(node_count));
        }
    }

    let storage_words = words.max(1);
    let mut buffers = shape.read_only_buffers();
    buffers.push(
        BufferDecl::storage(
            sources_buf,
            BINDING_PRIMITIVE_START,
            BufferAccess::ReadOnly,
            DataType::U32,
        )
        .with_count(storage_words),
    );
    buffers.push(
        BufferDecl::storage(
            reach_out,
            BINDING_PRIMITIVE_START + 1,
            BufferAccess::ReadWrite,
            DataType::U32,
        )
        .with_count(storage_words),
    );
    buffers.push(
        BufferDecl::storage(
            frontier_a,
            BINDING_PRIMITIVE_START + 2,
            BufferAccess::ReadWrite,
            DataType::U32,
        )
        .with_count(storage_words),
    );
    buffers.push(
        BufferDecl::storage(
            frontier_b,
            BINDING_PRIMITIVE_START + 3,
            BufferAccess::ReadWrite,
            DataType::U32,
        )
        .with_count(frontier_b_storage_words),
    );

    Program::wrapped(
        buffers,
        [256, 1, 1],
        vec![Node::Region {
            generator: Ident::from(OP_ID),
            source_region: None,
            body: Arc::new(entry),
        }],
    )
}

fn reachable_wave_barrier(node_count: u32) -> Node {
    if node_count <= 256 {
        Node::barrier()
    } else {
        Node::barrier_with_ordering(MemoryOrdering::GridSync)
    }
}

fn reachable_forward_wave_node(
    shape: ProgramGraphShape,
    frontier_in: &str,
    frontier_out: &str,
    local_prefix: &str,
) -> Node {
    let local = |name: &str| -> String { format!("{local_prefix}_{name}") };
    let lane = Expr::gid_x();
    let word_idx = local("word_idx");
    let bit_mask = local("bit_mask");
    let src_word = local("src_word");
    let edge_start = local("edge_start");
    let edge_end = local("edge_end");
    let edge_iter = local("edge");
    let kind_mask = local("kind_mask");
    let dst = local("dst");
    let dst_word_idx = local("dst_word_idx");
    let dst_bit = local("dst_bit");
    let previous = local("_prev");

    Node::if_then(
        Expr::lt(lane.clone(), Expr::u32(shape.node_count)),
        vec![
            Node::let_bind(word_idx.as_str(), Expr::shr(lane.clone(), Expr::u32(5))),
            Node::let_bind(
                bit_mask.as_str(),
                Expr::shl(Expr::u32(1), Expr::bitand(lane.clone(), Expr::u32(31))),
            ),
            Node::let_bind(
                src_word.as_str(),
                Expr::load(frontier_in, Expr::var(word_idx.as_str())),
            ),
            Node::if_then(
                Expr::ne(
                    Expr::bitand(Expr::var(src_word.as_str()), Expr::var(bit_mask.as_str())),
                    Expr::u32(0),
                ),
                vec![
                    Node::let_bind(
                        edge_start.as_str(),
                        Expr::load(NAME_EDGE_OFFSETS, lane.clone()),
                    ),
                    Node::let_bind(
                        edge_end.as_str(),
                        Expr::load(NAME_EDGE_OFFSETS, Expr::add(lane.clone(), Expr::u32(1))),
                    ),
                    Node::loop_for(
                        edge_iter.as_str(),
                        Expr::var(edge_start.as_str()),
                        Expr::var(edge_end.as_str()),
                        vec![
                            Node::let_bind(
                                kind_mask.as_str(),
                                Expr::load(NAME_EDGE_KIND_MASK, Expr::var(edge_iter.as_str())),
                            ),
                            Node::if_then(
                                Expr::ne(Expr::var(kind_mask.as_str()), Expr::u32(0)),
                                vec![
                                    Node::let_bind(
                                        dst.as_str(),
                                        Expr::load(
                                            NAME_EDGE_TARGETS,
                                            Expr::var(edge_iter.as_str()),
                                        ),
                                    ),
                                    Node::if_then(
                                        Expr::lt(
                                            Expr::var(dst.as_str()),
                                            Expr::u32(shape.node_count),
                                        ),
                                        vec![
                                            Node::let_bind(
                                                dst_word_idx.as_str(),
                                                Expr::shr(Expr::var(dst.as_str()), Expr::u32(5)),
                                            ),
                                            Node::let_bind(
                                                dst_bit.as_str(),
                                                Expr::shl(
                                                    Expr::u32(1),
                                                    Expr::bitand(
                                                        Expr::var(dst.as_str()),
                                                        Expr::u32(31),
                                                    ),
                                                ),
                                            ),
                                            Node::let_bind(
                                                previous.as_str(),
                                                Expr::atomic_or(
                                                    frontier_out,
                                                    Expr::var(dst_word_idx.as_str()),
                                                    Expr::var(dst_bit.as_str()),
                                                ),
                                            ),
                                        ],
                                    ),
                                ],
                            ),
                        ],
                    ),
                ],
            ),
        ],
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hs(items: &[u32]) -> HashSet<u32> {
        items.iter().copied().collect()
    }

    #[test]
    fn generated_try_reachable_matches_legacy_reachable() {
        for node_count in 1u32..=64 {
            for seed in 0u32..64 {
                let edges: Vec<(u32, u32)> = (0..node_count)
                    .filter_map(|node| {
                        let step = (seed % 7) + 1;
                        let dst = node.saturating_add(step);
                        (dst < node_count).then_some((node, dst))
                    })
                    .collect();
                let sources = [seed % node_count, node_count + seed];
                let fallible = try_reachable(node_count, &edges, &sources).unwrap();
                let legacy = reachable(node_count, &edges, &sources).unwrap();
                assert_eq!(fallible, legacy);
                assert!(fallible.contains(&(node_count + seed)));
            }
        }
    }

    #[test]
    fn empty_sources_reach_nothing() {
        let got = reachable(3, &[(0, 1), (1, 2)], &[]).unwrap();
        assert!(got.is_empty());
    }

    #[test]
    fn single_source_reaches_chain() {
        let got = reachable(3, &[(0, 1), (1, 2)], &[0]).unwrap();
        assert_eq!(got, hs(&[0, 1, 2]));
    }

    #[test]
    fn cycle_terminates() {
        // 0 → 1 → 0 (cycle). Starting from 0 should still terminate.
        let got = reachable(2, &[(0, 1), (1, 0)], &[0]).unwrap();
        assert_eq!(got, hs(&[0, 1]));
    }

    #[test]
    fn disconnected_source_not_included() {
        let got = reachable(4, &[(0, 1), (2, 3)], &[0]).unwrap();
        assert_eq!(got, hs(&[0, 1]));
        assert!(!got.contains(&2));
        assert!(!got.contains(&3));
    }

    #[test]
    fn unknown_source_is_noop() {
        // Source node 7 doesn't exist in a 2-node graph; reachable
        // should return just {7} (source is trivially reachable from
        // itself) without panicking.
        let got = reachable(2, &[(0, 1)], &[7]).unwrap();
        assert_eq!(got, hs(&[7]));
    }

    #[test]
    fn out_of_range_edge_is_reported_not_silently_dropped() {
        // AUDIT_2026-04-24 F-REACH-01: prior code silently dropped
        // the (5, 1) edge. Now it surfaces UnknownNode.
        let err = reachable(3, &[(0, 1), (5, 1)], &[0]).unwrap_err();
        assert_eq!(err.index, 1);
        assert_eq!(err.node, 5);
        assert_eq!(err.node_count, 3);
    }

    #[test]
    fn reachable_program_smoke() {
        // AUDIT_2026-04-24 F-REACH-02: smoke test that the Tier-2.5
        // builder produces a valid, non-empty fused Program.
        let program = reachable_program(4, 4, "sources", "reach", 2);
        assert!(!program.is_explicit_noop());
        assert!(!program.buffers().is_empty());
        assert!(!program.entry().is_empty());
        assert_eq!(program.workgroup_size(), [256, 1, 1]);

        // The program should declare the canonical CSR buffers, the
        // caller-provided bitsets, and the two wavefront scratch buffers.
        let names: Vec<&str> = program.buffers().iter().map(|b| b.name()).collect();
        assert!(names.contains(&"pg_edge_offsets"));
        assert!(names.contains(&"pg_edge_targets"));
        assert!(names.contains(&"sources"));
        assert!(names.contains(&"reach"));
        assert!(names.contains(&"reach_frontier_a"));
        assert!(names.contains(&"reach_frontier_b"));
        let frontier_b = program
            .buffers()
            .iter()
            .find(|buffer| buffer.name() == "reach_frontier_b")
            .expect("Fix: reachable wavefront scratch must be declared.");
        assert_eq!(
            frontier_b.count(),
            bitset_words(4) + 1,
            "Fix: reach_frontier_b must reserve one extra word for the converged-wave flag."
        );
    }

    #[test]
    fn reachable_program_zero_iters_seeds_only() {
        // With max_iters = 0 the program should still contain the
        // preliminary seed copy into reach_out.
        let program = reachable_program(4, 4, "sources", "reach", 0);
        assert!(!program.is_explicit_noop());
        assert!(!program.buffers().is_empty());
    }

    #[test]
    fn generated_wavefront_depth_limited_reachability_matches_scalar_reference() {
        for seed in 0..10_000_u32 {
            let mut state = mix32(seed ^ 0xA11C_E5E7);
            let node_count = 1 + (state % 96);
            state = mix32(state);
            let edge_budget = state % (node_count * 3);
            let mut edges = Vec::new();
            for edge_idx in 0..edge_budget {
                state = mix32(state ^ edge_idx.wrapping_mul(0x9E37_79B9));
                let from = state % node_count;
                state = mix32(state.rotate_left(7));
                let to = match edge_idx % 11 {
                    0 => from,
                    1 => (from + 1) % node_count,
                    2 => node_count - 1,
                    _ => state % node_count,
                };
                edges.push((from, to));
            }
            let source_count = 1 + (mix32(state ^ 0x5150_ACE5) % 4);
            let mut sources = Vec::new();
            for idx in 0..source_count {
                state = mix32(state ^ idx.wrapping_mul(0x85EB_CA6B));
                sources.push(state % node_count);
            }
            let max_iters = mix32(state ^ 0xD47A_F10D) % (node_count.min(16) + 1);

            let wave = depth_limited_wavefront(node_count, &edges, &sources, max_iters);
            let scalar = depth_limited_scalar(node_count, &edges, &sources, max_iters);

            assert_eq!(
                wave, scalar,
                "seed={seed} node_count={node_count} max_iters={max_iters}"
            );
        }
    }

    fn depth_limited_wavefront(
        node_count: u32,
        edges: &[(u32, u32)],
        sources: &[u32],
        max_iters: u32,
    ) -> HashSet<u32> {
        let mut visited = HashSet::new();
        let mut current = HashSet::new();
        for &source in sources {
            if source < node_count && visited.insert(source) {
                current.insert(source);
            }
        }

        for _ in 0..max_iters {
            let mut next = HashSet::new();
            for &(from, to) in edges {
                if from < node_count
                    && to < node_count
                    && current.contains(&from)
                    && visited.insert(to)
                {
                    next.insert(to);
                }
            }
            current = next;
        }
        visited
    }

    fn depth_limited_scalar(
        node_count: u32,
        edges: &[(u32, u32)],
        sources: &[u32],
        max_iters: u32,
    ) -> HashSet<u32> {
        let mut min_depth = vec![u32::MAX; node_count as usize];
        let mut queue = std::collections::VecDeque::new();
        for &source in sources {
            if source < node_count && min_depth[source as usize] > 0 {
                min_depth[source as usize] = 0;
                queue.push_back(source);
            }
        }
        while let Some(node) = queue.pop_front() {
            let depth = min_depth[node as usize];
            if depth >= max_iters {
                continue;
            }
            let next_depth = depth + 1;
            for &(from, to) in edges {
                if from == node && to < node_count && next_depth < min_depth[to as usize] {
                    min_depth[to as usize] = next_depth;
                    queue.push_back(to);
                }
            }
        }
        min_depth
            .into_iter()
            .enumerate()
            .filter_map(|(node, depth)| (depth <= max_iters).then_some(node as u32))
            .collect()
    }

    fn mix32(mut value: u32) -> u32 {
        value ^= value >> 16;
        value = value.wrapping_mul(0x7FEB_352D);
        value ^= value >> 15;
        value = value.wrapping_mul(0x846C_A68B);
        value ^ (value >> 16)
    }
}
