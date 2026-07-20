//! Unified device-resident token/fact graph layout.

use std::collections::HashMap;

/// Node class stored in the unified compiler/dataflow graph.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum TokenFactNodeKind {
    /// Source or macro-expanded token.
    Token,
    /// Macro expansion boundary.
    MacroExpansion,
    /// Semantic declaration, scope, or type node.
    Semantic,
    /// Dataflow fact node.
    Fact,
    /// Diagnostic/provenance node.
    Diagnostic,
}

/// Dependency edge class stored in the unified compiler/dataflow graph.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum TokenFactEdgeKind {
    /// Token stream order or token-to-token provenance.
    TokenFlow,
    /// Macro expansion/provenance relation.
    MacroExpansion,
    /// Token or semantic node emits a fact.
    SemanticFact,
    /// Fact-to-fact dataflow dependency.
    FactDependency,
    /// Diagnostic depends on source token, semantic node, or fact.
    DiagnosticProvenance,
}

/// One logical node before resident CSR packing.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TokenFactNode {
    /// Stable producer-defined id.
    pub id: u32,
    /// Node class.
    pub kind: TokenFactNodeKind,
    /// Offset into the shared resident payload slab.
    pub payload_offset: u64,
    /// Byte length inside the shared resident payload slab.
    pub payload_bytes: u64,
}

/// One logical edge before resident CSR packing.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TokenFactEdge {
    /// Source node id.
    pub from: u32,
    /// Destination node id.
    pub to: u32,
    /// Edge class.
    pub kind: TokenFactEdgeKind,
}

/// CSR layout shared by parser, semantic, diagnostic, and dataflow execution.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeviceResidentTokenFactGraph {
    /// Stable node ids in resident index order.
    pub node_ids: Vec<u32>,
    /// Node classes in resident index order.
    pub node_kinds: Vec<TokenFactNodeKind>,
    /// Payload offsets in resident index order.
    pub payload_offsets: Vec<u64>,
    /// Payload byte lengths in resident index order.
    pub payload_lengths: Vec<u64>,
    /// CSR row offsets, length `node_count + 1`.
    pub row_offsets: Vec<u32>,
    /// CSR destination node indices.
    pub column_indices: Vec<u32>,
    /// Edge classes aligned with `column_indices`.
    pub edge_kinds: Vec<TokenFactEdgeKind>,
    /// Total resident payload bytes required by the shared slab.
    pub payload_bytes: u64,
    /// Number of token-class nodes.
    pub token_nodes: u32,
    /// Number of fact-class nodes.
    pub fact_nodes: u32,
}

/// Reusable host-side staging for resident token/fact graph CSR packing.
#[derive(Debug, Default)]
pub struct DeviceResidentTokenFactGraphScratch {
    index_by_id: HashMap<u32, usize>,
    ordered_nodes: Vec<TokenFactNode>,
    staged_edges: Vec<(usize, u32, TokenFactEdgeKind)>,
}

impl DeviceResidentTokenFactGraphScratch {
    /// Create empty reusable token/fact graph packing scratch.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    fn clear_preserving_capacity(&mut self) {
        self.index_by_id.clear();
        self.ordered_nodes.clear();
        self.staged_edges.clear();
    }
}

/// Unified graph layout errors.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DeviceResidentTokenFactGraphError {
    /// Duplicate logical node id.
    DuplicateNode {
        /// Duplicate id.
        id: u32,
    },
    /// Edge references an unknown node id.
    UnknownEdgeNode {
        /// Unknown id.
        id: u32,
    },
    /// Payload range arithmetic overflowed.
    PayloadOverflow {
        /// Node whose range overflowed.
        id: u32,
    },
    /// Payload range exceeds the declared resident slab.
    PayloadOutOfBounds {
        /// Node whose range is invalid.
        id: u32,
        /// Exclusive end offset.
        end: u64,
        /// Declared slab length.
        payload_bytes: u64,
    },
    /// CSR row offsets cannot fit the release ABI.
    CsrIndexOverflow,
}

impl std::fmt::Display for DeviceResidentTokenFactGraphError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DuplicateNode { id } => write!(
                f,
                "device-resident token/fact graph has duplicate node id {id}. Fix: assign one stable id before CSR packing."
            ),
            Self::UnknownEdgeNode { id } => write!(
                f,
                "device-resident token/fact graph edge references unknown node {id}. Fix: emit all parser, semantic, and fact nodes before edges."
            ),
            Self::PayloadOverflow { id } => write!(
                f,
                "device-resident token/fact graph payload range overflowed for node {id}. Fix: shard the translation unit or payload slab before CUDA upload."
            ),
            Self::PayloadOutOfBounds {
                id,
                end,
                payload_bytes,
            } => write!(
                f,
                "device-resident token/fact graph node {id} payload ends at {end}, beyond slab length {payload_bytes}. Fix: compute payload offsets from the shared slab allocator."
            ),
            Self::CsrIndexOverflow => write!(
                f,
                "device-resident token/fact graph exceeds u32 CSR limits. Fix: shard before CUDA resident layout packing."
            ),
        }
    }
}

impl std::error::Error for DeviceResidentTokenFactGraphError {}

/// Pack parser, semantic, diagnostic, and dataflow nodes into one resident CSR.
pub fn plan_device_resident_token_fact_graph(
    nodes: &[TokenFactNode],
    edges: &[TokenFactEdge],
    payload_bytes: u64,
) -> Result<DeviceResidentTokenFactGraph, DeviceResidentTokenFactGraphError> {
    let mut scratch = DeviceResidentTokenFactGraphScratch::new();
    plan_device_resident_token_fact_graph_with_scratch(nodes, edges, payload_bytes, &mut scratch)
}

/// Pack a resident token/fact graph while reusing caller-owned staging scratch.
pub fn plan_device_resident_token_fact_graph_with_scratch(
    nodes: &[TokenFactNode],
    edges: &[TokenFactEdge],
    payload_bytes: u64,
    scratch: &mut DeviceResidentTokenFactGraphScratch,
) -> Result<DeviceResidentTokenFactGraph, DeviceResidentTokenFactGraphError> {
    scratch.clear_preserving_capacity();
    scratch.index_by_id.reserve(nodes.len());
    scratch.ordered_nodes.reserve(nodes.len());
    scratch.staged_edges.reserve(edges.len());
    for node in nodes {
        if scratch
            .index_by_id
            .insert(node.id, scratch.ordered_nodes.len())
            .is_some()
        {
            return Err(DeviceResidentTokenFactGraphError::DuplicateNode { id: node.id });
        }
        let end = node
            .payload_offset
            .checked_add(node.payload_bytes)
            .ok_or(DeviceResidentTokenFactGraphError::PayloadOverflow { id: node.id })?;
        if end > payload_bytes {
            return Err(DeviceResidentTokenFactGraphError::PayloadOutOfBounds {
                id: node.id,
                end,
                payload_bytes,
            });
        }
        scratch.ordered_nodes.push(*node);
    }
    scratch.ordered_nodes.sort_unstable_by_key(|node| node.id);

    u32::try_from(scratch.ordered_nodes.len())
        .map_err(|_| DeviceResidentTokenFactGraphError::CsrIndexOverflow)?;
    scratch.index_by_id.clear();
    let mut node_ids = Vec::with_capacity(scratch.ordered_nodes.len());
    let mut node_kinds = Vec::with_capacity(scratch.ordered_nodes.len());
    let mut payload_offsets = Vec::with_capacity(scratch.ordered_nodes.len());
    let mut payload_lengths = Vec::with_capacity(scratch.ordered_nodes.len());
    let mut token_nodes = 0_u32;
    let mut fact_nodes = 0_u32;
    for (index, node) in scratch.ordered_nodes.iter().enumerate() {
        u32::try_from(index).map_err(|_| DeviceResidentTokenFactGraphError::CsrIndexOverflow)?;
        scratch.index_by_id.insert(node.id, index);
        node_ids.push(node.id);
        node_kinds.push(node.kind);
        payload_offsets.push(node.payload_offset);
        payload_lengths.push(node.payload_bytes);
        match node.kind {
            TokenFactNodeKind::Token => token_nodes += 1,
            TokenFactNodeKind::Fact => fact_nodes += 1,
            TokenFactNodeKind::MacroExpansion
            | TokenFactNodeKind::Semantic
            | TokenFactNodeKind::Diagnostic => {}
        }
    }

    for edge in edges {
        let from = *scratch
            .index_by_id
            .get(&edge.from)
            .ok_or(DeviceResidentTokenFactGraphError::UnknownEdgeNode { id: edge.from })?;
        let to = *scratch
            .index_by_id
            .get(&edge.to)
            .ok_or(DeviceResidentTokenFactGraphError::UnknownEdgeNode { id: edge.to })?;
        let to =
            u32::try_from(to).map_err(|_| DeviceResidentTokenFactGraphError::CsrIndexOverflow)?;
        scratch.staged_edges.push((from, to, edge.kind));
    }

    u32::try_from(edges.len()).map_err(|_| DeviceResidentTokenFactGraphError::CsrIndexOverflow)?;
    scratch
        .staged_edges
        .sort_unstable_by_key(|&(from, to, kind)| (from, to, kind));
    let mut row_offsets = Vec::with_capacity(scratch.ordered_nodes.len() + 1);
    let mut column_indices = Vec::with_capacity(scratch.staged_edges.len());
    let mut edge_kinds = Vec::with_capacity(scratch.staged_edges.len());
    row_offsets.push(0);
    let mut edge_index = 0_usize;
    for row in 0..scratch.ordered_nodes.len() {
        let mut last_edge = None;
        while let Some(&(from, to, kind)) = scratch.staged_edges.get(edge_index) {
            if from != row {
                break;
            }
            let edge_key = (to, kind);
            if last_edge != Some(edge_key) {
                column_indices.push(to);
                edge_kinds.push(kind);
                last_edge = Some(edge_key);
            }
            edge_index += 1;
        }
        let next = u32::try_from(column_indices.len())
            .map_err(|_| DeviceResidentTokenFactGraphError::CsrIndexOverflow)?;
        row_offsets.push(next);
    }

    Ok(DeviceResidentTokenFactGraph {
        node_ids,
        node_kinds,
        payload_offsets,
        payload_lengths,
        row_offsets,
        column_indices,
        edge_kinds,
        payload_bytes,
        token_nodes,
        fact_nodes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_fact_graph_packs_stable_shared_csr() {
        let graph = plan_device_resident_token_fact_graph(
            &[
                node(20, TokenFactNodeKind::Fact, 12, 4),
                node(10, TokenFactNodeKind::Token, 0, 4),
                node(30, TokenFactNodeKind::Diagnostic, 20, 8),
            ],
            &[
                edge(20, 30, TokenFactEdgeKind::DiagnosticProvenance),
                edge(10, 20, TokenFactEdgeKind::SemanticFact),
            ],
            32,
        )
        .expect("Fix: valid token/fact graph should pack");

        assert_eq!(graph.node_ids, vec![10, 20, 30]);
        assert_eq!(
            graph.node_kinds,
            vec![
                TokenFactNodeKind::Token,
                TokenFactNodeKind::Fact,
                TokenFactNodeKind::Diagnostic,
            ]
        );
        assert_eq!(graph.row_offsets, vec![0, 1, 2, 2]);
        assert_eq!(graph.column_indices, vec![1, 2]);
        assert_eq!(
            graph.edge_kinds,
            vec![
                TokenFactEdgeKind::SemanticFact,
                TokenFactEdgeKind::DiagnosticProvenance,
            ]
        );
        assert_eq!(graph.token_nodes, 1);
        assert_eq!(graph.fact_nodes, 1);
    }

    #[test]
    fn token_fact_graph_deduplicates_parallel_edges_deterministically() {
        let graph = plan_device_resident_token_fact_graph(
            &[
                node(2, TokenFactNodeKind::Fact, 4, 4),
                node(1, TokenFactNodeKind::Token, 0, 4),
            ],
            &[
                edge(1, 2, TokenFactEdgeKind::SemanticFact),
                edge(1, 2, TokenFactEdgeKind::SemanticFact),
            ],
            8,
        )
        .expect("Fix: duplicate edges should deduplicate inside a resident row");

        assert_eq!(graph.row_offsets, vec![0, 1, 1]);
        assert_eq!(graph.column_indices, vec![1]);
    }

    #[test]
    fn token_fact_graph_rejects_invalid_layouts() {
        assert_eq!(
            plan_device_resident_token_fact_graph(
                &[
                    node(1, TokenFactNodeKind::Token, 0, 1),
                    node(1, TokenFactNodeKind::Fact, 1, 1),
                ],
                &[],
                2,
            )
            .expect_err("duplicate nodes should fail"),
            DeviceResidentTokenFactGraphError::DuplicateNode { id: 1 }
        );
        assert_eq!(
            plan_device_resident_token_fact_graph(
                &[node(1, TokenFactNodeKind::Token, 0, 1)],
                &[edge(1, 2, TokenFactEdgeKind::SemanticFact)],
                1,
            )
            .expect_err("unknown edge nodes should fail"),
            DeviceResidentTokenFactGraphError::UnknownEdgeNode { id: 2 }
        );
        assert_eq!(
            plan_device_resident_token_fact_graph(
                &[node(1, TokenFactNodeKind::Token, 8, 8)],
                &[],
                12,
            )
            .expect_err("payload overflow beyond slab should fail"),
            DeviceResidentTokenFactGraphError::PayloadOutOfBounds {
                id: 1,
                end: 16,
                payload_bytes: 12,
            }
        );
    }

    #[test]
    fn token_fact_graph_packer_avoids_ordered_maps_and_fallible_capacity_defaults() {
        let source = include_str!("device_resident_token_fact_graph.rs");
        assert!(
            !source.contains(concat!("BTree", "Map"))
                && !source.contains(concat!("BTree", "Set")),
            "Fix: token/fact resident graph packing must not use ordered maps or sets in the CUDA release path."
        );
        assert!(
            !source.contains(concat!("unwrap", "_or(0)")),
            "Fix: token/fact resident graph packing must not hide capacity conversion failures behind zero-capacity defaults."
        );
        assert!(
            !source.contains(concat!("vec![", "Vec::<")),
            "Fix: token/fact resident graph packing must not allocate one adjacency vector per resident node."
        );
    }

    #[test]
    fn token_fact_graph_packs_large_unsorted_input_with_stable_indices() {
        let mut nodes = Vec::new();
        let mut edges = Vec::new();
        for id in (0..1024_u32).rev() {
            nodes.push(node(id, TokenFactNodeKind::Token, u64::from(id), 1));
            if id > 0 {
                edges.push(edge(id - 1, id, TokenFactEdgeKind::TokenFlow));
            }
        }

        let graph = plan_device_resident_token_fact_graph(&nodes, &edges, 1024)
            .expect("Fix: large unsorted token/fact graph should pack deterministically");

        assert_eq!(graph.node_ids[0], 0);
        assert_eq!(graph.node_ids[1023], 1023);
        assert_eq!(graph.row_offsets[0], 0);
        assert_eq!(graph.row_offsets[1024], 1023);
        assert_eq!(graph.column_indices[0], 1);
    }

    #[test]
    fn token_fact_graph_scratch_reuses_staging_allocations() {
        let mut scratch = DeviceResidentTokenFactGraphScratch::new();
        let nodes = [
            node(3, TokenFactNodeKind::Fact, 2, 1),
            node(1, TokenFactNodeKind::Token, 0, 1),
            node(2, TokenFactNodeKind::Semantic, 1, 1),
        ];
        let edges = [
            edge(1, 2, TokenFactEdgeKind::SemanticFact),
            edge(2, 3, TokenFactEdgeKind::FactDependency),
        ];
        plan_device_resident_token_fact_graph_with_scratch(&nodes, &edges, 3, &mut scratch)
            .expect("Fix: first scratch-backed token/fact graph should pack");
        let ordered_capacity = scratch.ordered_nodes.capacity();
        let staged_capacity = scratch.staged_edges.capacity();
        let index_capacity = scratch.index_by_id.capacity();

        let graph =
            plan_device_resident_token_fact_graph_with_scratch(&nodes[..2], &[], 3, &mut scratch)
                .expect("Fix: smaller scratch-backed token/fact graph should reuse staging");

        assert_eq!(scratch.ordered_nodes.capacity(), ordered_capacity);
        assert_eq!(scratch.staged_edges.capacity(), staged_capacity);
        assert_eq!(scratch.index_by_id.capacity(), index_capacity);
        assert_eq!(graph.node_ids, vec![1, 3]);
        assert_eq!(
            graph.row_offsets,
            vec![0, 0, 0],
            "Fix: unknown edge rows must not leak from previous scratch contents."
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
}
