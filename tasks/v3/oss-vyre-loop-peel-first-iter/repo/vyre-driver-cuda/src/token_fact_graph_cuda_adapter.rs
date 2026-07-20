//! CUDA adapter for the unified resident token/fact graph.

use std::{cmp::Reverse, collections::BinaryHeap};

use crate::backend::accounting::{
    checked_add_u64_count as checked_add, checked_mul_u64_count as checked_mul,
    CudaArithmeticOverflow,
};
use crate::megakernel_scheduler::CudaMegakernelGraphShape;
use vyre_self_substrate::device_resident_token_fact_graph::DeviceResidentTokenFactGraph;

/// Number of rank buckets carried for token/fact out-degree skew planning.
pub const CUDA_TOKEN_FACT_DEGREE_PROFILE_BUCKETS: usize = 16;

/// Power-of-two ranks used by the token/fact out-degree profile.
pub const CUDA_TOKEN_FACT_DEGREE_PROFILE_RANKS: [u64; CUDA_TOKEN_FACT_DEGREE_PROFILE_BUCKETS] = [
    1, 2, 4, 8, 16, 32, 64, 128, 256, 512, 1_024, 2_048, 4_096, 8_192, 16_384, 32_768,
];

const CUDA_TOKEN_FACT_DEGREE_PROFILE_MAX_RANK: usize = 32_768;

/// CUDA resident byte envelope for the unified compiler/dataflow graph.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CudaTokenFactGraphLayout {
    /// Scheduler-visible graph shape.
    pub graph_shape: CudaMegakernelGraphShape,
    /// Maximum outgoing CSR row degree in the resident token/fact graph.
    pub max_out_degree: u64,
    /// Prefix sums of top out-degrees at `CUDA_TOKEN_FACT_DEGREE_PROFILE_RANKS`.
    pub top_out_degree_prefix_sums: [u64; CUDA_TOKEN_FACT_DEGREE_PROFILE_BUCKETS],
    /// Fixed bytes per resident node record.
    pub node_record_bytes: u64,
    /// Fixed bytes per resident edge record.
    pub edge_record_bytes: u64,
    /// Bytes for resident node records.
    pub node_bytes: u64,
    /// Bytes for resident edge records.
    pub edge_bytes: u64,
    /// Bytes for the shared token/fact payload slab.
    pub payload_bytes: u64,
    /// Total bytes that must remain device-resident for the layout.
    pub resident_bytes: u64,
}

impl CudaTokenFactGraphLayout {
    /// Build a layout from aggregate byte fields when CSR row offsets are not
    /// available to the caller. This preserves correctness by treating total
    /// edge count as the maximum possible row degree.
    #[must_use]
    pub const fn from_aggregate_fields(
        graph_shape: CudaMegakernelGraphShape,
        node_record_bytes: u64,
        edge_record_bytes: u64,
        node_bytes: u64,
        edge_bytes: u64,
        payload_bytes: u64,
        resident_bytes: u64,
    ) -> Self {
        Self {
            graph_shape,
            max_out_degree: graph_shape.edge_count,
            top_out_degree_prefix_sums: [graph_shape.edge_count;
                CUDA_TOKEN_FACT_DEGREE_PROFILE_BUCKETS],
            node_record_bytes,
            edge_record_bytes,
            node_bytes,
            edge_bytes,
            payload_bytes,
            resident_bytes,
        }
    }
}

/// CUDA token/fact adapter errors.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CudaTokenFactGraphLayoutError {
    /// Record widths must be explicit, non-zero ABI values.
    ZeroRecordWidth {
        /// Field that was zero.
        field: &'static str,
    },
    /// Public CSR fields are inconsistent with each other.
    InvalidCsrShape {
        /// Invalid CSR field or relationship.
        field: &'static str,
    },
    /// Byte arithmetic overflowed.
    ByteCountOverflow {
        /// Field being computed.
        field: &'static str,
    },
}

impl std::fmt::Display for CudaTokenFactGraphLayoutError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ZeroRecordWidth { field } => write!(
                f,
                "CUDA token/fact graph adapter received zero {field}. Fix: pass the concrete resident ABI record width."
            ),
            Self::InvalidCsrShape { field } => write!(
                f,
                "CUDA token/fact graph adapter received invalid CSR {field}. Fix: rebuild the token/fact graph through the canonical resident graph planner."
            ),
            Self::ByteCountOverflow { field } => write!(
                f,
                "CUDA token/fact graph adapter overflowed while computing {field}. Fix: shard the token/fact graph before resident upload."
            ),
        }
    }
}

impl std::error::Error for CudaTokenFactGraphLayoutError {}

impl CudaArithmeticOverflow for CudaTokenFactGraphLayoutError {
    fn arithmetic_overflow(field: &'static str) -> Self {
        Self::ByteCountOverflow { field }
    }
}

/// Convert the unified token/fact graph into CUDA scheduler shape and bytes.
pub fn adapt_token_fact_graph_to_cuda_layout(
    graph: &DeviceResidentTokenFactGraph,
    node_record_bytes: u64,
    edge_record_bytes: u64,
) -> Result<CudaTokenFactGraphLayout, CudaTokenFactGraphLayoutError> {
    if node_record_bytes == 0 {
        return Err(CudaTokenFactGraphLayoutError::ZeroRecordWidth {
            field: "node_record_bytes",
        });
    }
    if edge_record_bytes == 0 {
        return Err(CudaTokenFactGraphLayoutError::ZeroRecordWidth {
            field: "edge_record_bytes",
        });
    }
    let node_count = u64::try_from(graph.node_ids.len()).map_err(|_| {
        CudaTokenFactGraphLayoutError::ByteCountOverflow {
            field: "node count",
        }
    })?;
    let edge_count = u64::try_from(graph.column_indices.len()).map_err(|_| {
        CudaTokenFactGraphLayoutError::ByteCountOverflow {
            field: "edge count",
        }
    })?;
    let (max_out_degree, top_out_degree_prefix_sums) = csr_out_degree_profile(graph, edge_count)?;
    let node_bytes = checked_mul(node_count, node_record_bytes, "node bytes")?;
    let edge_bytes = checked_mul(edge_count, edge_record_bytes, "edge bytes")?;
    let resident_without_payload = checked_add(node_bytes, edge_bytes, "node plus edge bytes")?;
    let resident_bytes = checked_add(
        resident_without_payload,
        graph.payload_bytes,
        "resident bytes",
    )?;

    Ok(CudaTokenFactGraphLayout {
        graph_shape: CudaMegakernelGraphShape {
            node_count,
            edge_count,
        },
        max_out_degree,
        top_out_degree_prefix_sums,
        node_record_bytes,
        edge_record_bytes,
        node_bytes,
        edge_bytes,
        payload_bytes: graph.payload_bytes,
        resident_bytes,
    })
}

fn csr_out_degree_profile(
    graph: &DeviceResidentTokenFactGraph,
    edge_count: u64,
) -> Result<(u64, [u64; CUDA_TOKEN_FACT_DEGREE_PROFILE_BUCKETS]), CudaTokenFactGraphLayoutError> {
    let expected_row_offsets = graph.node_ids.len().checked_add(1).ok_or(
        CudaTokenFactGraphLayoutError::ByteCountOverflow {
            field: "row offset count",
        },
    )?;
    if graph.row_offsets.len() != expected_row_offsets {
        return Err(CudaTokenFactGraphLayoutError::InvalidCsrShape {
            field: "row_offsets length",
        });
    }
    let declared_edges = u64::from(*graph.row_offsets.last().ok_or(
        CudaTokenFactGraphLayoutError::InvalidCsrShape {
            field: "row_offsets terminator",
        },
    )?);
    if declared_edges != edge_count {
        return Err(CudaTokenFactGraphLayoutError::InvalidCsrShape {
            field: "row_offsets edge count",
        });
    }
    let profile_capacity = CUDA_TOKEN_FACT_DEGREE_PROFILE_MAX_RANK.min(graph.node_ids.len());
    let mut top_degrees = BinaryHeap::with_capacity(profile_capacity);
    let mut max_out_degree = 0_u64;
    for row in graph.row_offsets.windows(2) {
        let start = row[0];
        let end = row[1];
        if end < start {
            return Err(CudaTokenFactGraphLayoutError::InvalidCsrShape {
                field: "row_offsets ordering",
            });
        }
        let degree = u64::from(end - start);
        max_out_degree = max_out_degree.max(degree);
        if top_degrees.len() < profile_capacity {
            top_degrees.push(Reverse(degree));
        } else if let Some(mut min_degree) = top_degrees.peek_mut() {
            if degree > min_degree.0 {
                *min_degree = Reverse(degree);
            }
        }
    }
    let mut degrees = top_degrees
        .into_iter()
        .map(|Reverse(degree)| degree)
        .collect::<Vec<_>>();
    degrees.sort_unstable_by(|lhs, rhs| rhs.cmp(lhs));

    let mut prefix_sum = 0_u64;
    let mut prefix_sums = [0_u64; CUDA_TOKEN_FACT_DEGREE_PROFILE_BUCKETS];
    let mut bucket = 0_usize;
    for (index, degree) in degrees.into_iter().enumerate() {
        prefix_sum = checked_add(prefix_sum, degree, "top out-degree prefix sum")?;
        let rank = u64::try_from(index + 1).map_err(|_| {
            CudaTokenFactGraphLayoutError::ByteCountOverflow {
                field: "out-degree profile rank",
            }
        })?;
        while bucket < CUDA_TOKEN_FACT_DEGREE_PROFILE_BUCKETS
            && rank >= CUDA_TOKEN_FACT_DEGREE_PROFILE_RANKS[bucket]
        {
            prefix_sums[bucket] = prefix_sum;
            bucket += 1;
        }
    }
    while bucket < CUDA_TOKEN_FACT_DEGREE_PROFILE_BUCKETS {
        prefix_sums[bucket] = prefix_sum;
        bucket += 1;
    }
    Ok((max_out_degree, prefix_sums))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::megakernel_scheduler::{plan_cuda_megakernel_memory_budget, CudaMegakernelTopology};
    use vyre_self_substrate::device_resident_token_fact_graph::{
        plan_device_resident_token_fact_graph, TokenFactEdge, TokenFactEdgeKind, TokenFactNode,
        TokenFactNodeKind,
    };

    #[test]
    fn token_fact_adapter_uses_shared_typed_cuda_arithmetic() {
        let source = include_str!("token_fact_graph_cuda_adapter.rs");
        let production = source
            .split("#[cfg(test)]")
            .next()
            .expect("Fix: token/fact CUDA adapter source should contain production section");

        assert!(source.contains("checked_add_u64_count as checked_add"));
        assert!(source.contains("checked_mul_u64_count as checked_mul"));
        assert!(source.contains("impl CudaArithmeticOverflow for CudaTokenFactGraphLayoutError"));
        assert!(production.contains("BinaryHeap::with_capacity(profile_capacity)"));
        assert!(!production.contains("Vec::with_capacity(graph.node_ids.len())"));
        assert!(!source.contains(concat!("fn checked_", "mul(")));
        assert!(!source.contains(concat!("fn checked_", "add(")));
    }

    #[test]
    fn adapter_accounts_for_cuda_resident_token_fact_layout() {
        let graph = plan_device_resident_token_fact_graph(
            &[
                node(1, TokenFactNodeKind::Token, 0, 8),
                node(2, TokenFactNodeKind::Semantic, 8, 8),
                node(3, TokenFactNodeKind::Fact, 16, 8),
            ],
            &[
                edge(1, 2, TokenFactEdgeKind::SemanticFact),
                edge(2, 3, TokenFactEdgeKind::FactDependency),
            ],
            24,
        )
        .expect("Fix: token/fact graph should pack");

        let cuda = adapt_token_fact_graph_to_cuda_layout(&graph, 32, 16)
            .expect("Fix: token/fact graph should adapt to CUDA layout");

        assert_eq!(cuda.graph_shape.node_count, 3);
        assert_eq!(cuda.graph_shape.edge_count, 2);
        assert_eq!(cuda.max_out_degree, 1);
        assert_eq!(cuda.top_out_degree_prefix_sums[0], 1);
        assert_eq!(cuda.top_out_degree_prefix_sums[1], 2);
        assert_eq!(cuda.top_out_degree_prefix_sums[15], 2);
        assert_eq!(cuda.node_bytes, 96);
        assert_eq!(cuda.edge_bytes, 32);
        assert_eq!(cuda.resident_bytes, 152);
        let memory = plan_cuda_megakernel_memory_budget(
            CudaMegakernelTopology::SparseFrontier,
            cuda.graph_shape,
            cuda.node_record_bytes,
            cuda.edge_record_bytes,
            64,
            cuda.payload_bytes,
            16,
            512,
        )
        .expect("Fix: adapted token/fact graph should feed CUDA memory planning");
        assert_eq!(memory.graph_bytes, 128);
    }

    #[test]
    fn adapter_exports_max_out_degree_for_hub_heavy_queue_planning() {
        let graph = plan_device_resident_token_fact_graph(
            &[
                node(1, TokenFactNodeKind::Fact, 0, 4),
                node(2, TokenFactNodeKind::Fact, 4, 4),
                node(3, TokenFactNodeKind::Fact, 8, 4),
                node(4, TokenFactNodeKind::Fact, 12, 4),
            ],
            &[
                edge(1, 2, TokenFactEdgeKind::FactDependency),
                edge(1, 3, TokenFactEdgeKind::FactDependency),
                edge(1, 4, TokenFactEdgeKind::FactDependency),
                edge(2, 3, TokenFactEdgeKind::FactDependency),
            ],
            16,
        )
        .expect("Fix: hub-heavy token/fact graph should pack");

        let cuda = adapt_token_fact_graph_to_cuda_layout(&graph, 32, 16)
            .expect("Fix: hub-heavy token/fact graph should adapt to CUDA layout");

        assert_eq!(cuda.graph_shape.edge_count, 4);
        assert_eq!(cuda.max_out_degree, 3);
        assert_eq!(cuda.top_out_degree_prefix_sums[0], 3);
        assert_eq!(cuda.top_out_degree_prefix_sums[1], 4);
        assert_eq!(cuda.top_out_degree_prefix_sums[2], 4);
    }

    #[test]
    fn generated_adapter_profiles_top_out_degree_prefixes() {
        let mut state = 0x5eec_c0de_f00d_7715_u64;
        for case_index in 0..4096_u64 {
            let node_count = 1 + (next_u64(&mut state) % 64) as u32;
            let nodes = (0..node_count)
                .map(|index| node(index + 1, TokenFactNodeKind::Fact, u64::from(index) * 4, 4))
                .collect::<Vec<_>>();
            let mut edges = Vec::new();
            if case_index % 4 == 0 {
                for to in 2..=node_count {
                    edges.push(edge(1, to, TokenFactEdgeKind::FactDependency));
                }
            }
            let attempts = next_u64(&mut state) % (u64::from(node_count) * 5 + 1);
            for _ in 0..attempts {
                let from = 1 + (next_u64(&mut state) % u64::from(node_count)) as u32;
                let to = 1 + (next_u64(&mut state) % u64::from(node_count)) as u32;
                let kind = if next_u64(&mut state) & 1 == 0 {
                    TokenFactEdgeKind::FactDependency
                } else {
                    TokenFactEdgeKind::DiagnosticProvenance
                };
                edges.push(edge(from, to, kind));
            }
            let graph =
                plan_device_resident_token_fact_graph(&nodes, &edges, u64::from(node_count) * 4)
                    .expect("Fix: generated token/fact graph should pack");
            let cuda = adapt_token_fact_graph_to_cuda_layout(&graph, 32, 16)
                .expect("Fix: generated token/fact graph should adapt");
            let mut degrees = graph
                .row_offsets
                .windows(2)
                .map(|row| u64::from(row[1] - row[0]))
                .collect::<Vec<_>>();
            degrees.sort_unstable_by(|lhs, rhs| rhs.cmp(lhs));

            assert_eq!(
                cuda.max_out_degree,
                degrees.first().copied().unwrap_or(0),
                "case {case_index}"
            );
            for (bucket, rank) in CUDA_TOKEN_FACT_DEGREE_PROFILE_RANKS.iter().enumerate() {
                let expected = degrees
                    .iter()
                    .take((*rank as usize).min(degrees.len()))
                    .copied()
                    .sum::<u64>();
                assert_eq!(
                    cuda.top_out_degree_prefix_sums[bucket], expected,
                    "case {case_index} bucket {bucket}"
                );
            }
        }
    }

    #[test]
    fn adapter_profiles_large_graph_with_bounded_top_rank_storage() {
        let node_count = 32_770_u32;
        let nodes = (0..node_count)
            .map(|index| node(index + 1, TokenFactNodeKind::Fact, u64::from(index) * 4, 4))
            .collect::<Vec<_>>();
        let mut edges = Vec::with_capacity(32_858);
        for to in 2..=51 {
            edges.push(edge(1, to, TokenFactEdgeKind::FactDependency));
        }
        for to in 3..=42 {
            edges.push(edge(2, to, TokenFactEdgeKind::FactDependency));
        }
        for from in 3..=node_count {
            edges.push(edge(from, 1, TokenFactEdgeKind::FactDependency));
        }
        let graph =
            plan_device_resident_token_fact_graph(&nodes, &edges, u64::from(node_count) * 4)
                .expect("Fix: large skewed token/fact graph should pack");

        let cuda = adapt_token_fact_graph_to_cuda_layout(&graph, 32, 16)
            .expect("Fix: large skewed token/fact graph should adapt");

        assert_eq!(cuda.graph_shape.node_count, u64::from(node_count));
        assert_eq!(cuda.graph_shape.edge_count, 32_858);
        assert_eq!(cuda.max_out_degree, 50);
        assert_eq!(cuda.top_out_degree_prefix_sums[0], 50);
        assert_eq!(cuda.top_out_degree_prefix_sums[1], 90);
        assert_eq!(cuda.top_out_degree_prefix_sums[2], 92);
        assert_eq!(cuda.top_out_degree_prefix_sums[15], 32_856);
    }

    #[test]
    fn aggregate_layout_constructor_preserves_legacy_safe_edge_bound() {
        let layout = CudaTokenFactGraphLayout::from_aggregate_fields(
            CudaMegakernelGraphShape {
                node_count: 4,
                edge_count: 9,
            },
            32,
            16,
            128,
            144,
            64,
            336,
        );

        assert_eq!(layout.max_out_degree, 9);
        assert_eq!(
            layout.top_out_degree_prefix_sums,
            [9; CUDA_TOKEN_FACT_DEGREE_PROFILE_BUCKETS]
        );
        assert_eq!(layout.resident_bytes, 336);
    }

    #[test]
    fn adapter_rejects_missing_abi_widths() {
        let graph = plan_device_resident_token_fact_graph(&[], &[], 0)
            .expect("Fix: empty graph still has a valid resident layout");

        assert_eq!(
            adapt_token_fact_graph_to_cuda_layout(&graph, 0, 8)
                .expect_err("zero node record width should fail"),
            CudaTokenFactGraphLayoutError::ZeroRecordWidth {
                field: "node_record_bytes",
            }
        );
        assert_eq!(
            adapt_token_fact_graph_to_cuda_layout(&graph, 8, 0)
                .expect_err("zero edge record width should fail"),
            CudaTokenFactGraphLayoutError::ZeroRecordWidth {
                field: "edge_record_bytes",
            }
        );
    }

    #[test]
    fn adapter_rejects_public_graphs_with_invalid_csr_rows() {
        let mut graph = plan_device_resident_token_fact_graph(
            &[node(1, TokenFactNodeKind::Fact, 0, 4)],
            &[],
            4,
        )
        .expect("Fix: token/fact graph should pack before adversarial mutation");
        graph.row_offsets[1] = 1;

        assert_eq!(
            adapt_token_fact_graph_to_cuda_layout(&graph, 32, 16)
                .expect_err("invalid CSR row offsets should fail before CUDA planning"),
            CudaTokenFactGraphLayoutError::InvalidCsrShape {
                field: "row_offsets edge count",
            }
        );
    }

    fn node(
        id: u32,
        kind: TokenFactNodeKind,
        payload_offset: u64,
        payload_bytes: u64,
    ) -> TokenFactNode {
        TokenFactNode {
            id,
            kind,
            payload_offset,
            payload_bytes,
        }
    }

    fn edge(from: u32, to: u32, kind: TokenFactEdgeKind) -> TokenFactEdge {
        TokenFactEdge { from, to, kind }
    }

    fn next_u64(state: &mut u64) -> u64 {
        *state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        *state
    }
}
