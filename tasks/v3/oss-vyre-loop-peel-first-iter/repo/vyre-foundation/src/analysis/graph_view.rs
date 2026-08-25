//! P7.5  -  Graph-IR compatibility bridge.
//!
//! Statement-IR (`Program` + `Node` + `Expr`) is the canonical
//! form in 0.6  -  wire-format stable, every backend speaks it.
//! Graph-IR is a pure VIEW on top: lets whole-program optimization
//! passes (kernel fusion, dead-subregion elimination) walk the IR
//! as a DAG of `DataflowNode`s connected by `DataEdge`s.
//!
//! **Contract freeze:** the graph-view types (`GraphNode`,
//! `DataflowKind`, `DataEdge`, `NodeGraph`) are `#[non_exhaustive]`
//! and live in this module permanently. External crates that
//! want a graph-IR pass (sparse-graph coloring, auto-fusion,
//! parallelism scheduling) pin against this surface without
//! forcing a statement-IR rewrite.
//!
//! **No wire-format impact.** `to_graph(program)` is a
//! lossless analysis; `from_graph(graph)` rebuilds an equivalent
//! statement-IR Program. Round-trip is byte-identity under
//! canonicalization.
//!
//! Today's deliverable: the types + a minimal `to_graph` /
//! `from_graph` walker that treats each top-level Node as a
//! graph node with explicit ordering edges. Richer dataflow
//! analysis (reaching-definition edges, implicit-dependency
//! discovery) land as optimization passes without changing the
//! graph types.

use crate::ir_inner::model::expr::Ident;
use crate::ir_inner::model::node::Node;
use crate::ir_inner::model::program::Program;
use core::fmt;

/// Validation error returned when a `NodeGraph` fails structural
/// checks before lowering to statement-IR.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum GraphValidateError {
    /// A cycle was detected in the graph.
    Cycle {
        /// Node ids forming the cycle.
        path: Vec<u32>,
    },
    /// An edge references a node id that does not exist.
    DanglingEdge {
        /// Source node id.
        from: u32,
        /// Target node id.
        to: u32,
    },
    /// A Phi node has no valid predecessors.
    OrphanPhi {
        /// The Phi node id.
        node_id: u32,
    },
}

impl fmt::Display for GraphValidateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Cycle { path } => {
                write!(
                    f,
                    "graph contains a cycle involving nodes {path:?}. Fix: remove cyclic dependencies so the graph is a valid DAG."
                )
            }
            Self::DanglingEdge { from, to } => {
                write!(
                    f,
                    "edge from {from} to {to} references a non-existent node. Fix: ensure all edge endpoints exist in the graph's node list."
                )
            }
            Self::OrphanPhi { node_id } => {
                write!(
                    f,
                    "Phi node {node_id} has no valid predecessors. Fix: ensure every Phi node references at least one existing predecessor node."
                )
            }
        }
    }
}

impl std::error::Error for GraphValidateError {}

/// One node in the graph-IR view.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct GraphNode {
    /// Stable id inside this graph. Issued sequentially during
    /// `to_graph` for reproducibility across runs.
    pub id: u32,
    /// Discriminant + payload. Currently carries a reference back
    /// to the statement-IR Node; graph-native variants land in
    /// follow-on passes (autodiff-tape, sparse-region, etc.).
    pub kind: DataflowKind,
}

impl GraphNode {
    /// Construct a `GraphNode` from explicit fields (V7-EXT-025).
    #[must_use]
    pub fn new(id: u32, kind: DataflowKind) -> Self {
        Self { id, kind }
    }
}

/// Kind of a graph node.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum DataflowKind {
    /// A statement-IR node executed as-is.
    Statement(Node),
    /// A synthetic "phi" introduced by later dataflow-analysis
    /// passes. Carries the set of graph-node ids that feed it.
    Phi(Vec<u32>),
    /// An explicit no-op barrier for scheduling passes that want
    /// to pin ordering without executing anything.
    Barrier,
}

/// Directed edge between two `GraphNode`s.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct DataEdge {
    /// Source node id.
    pub from: u32,
    /// Destination node id.
    pub to: u32,
    /// Kind of the dependency.
    pub kind: EdgeKind,
}

impl DataEdge {
    /// Construct a `DataEdge` from explicit fields (V7-EXT-026).
    #[must_use]
    pub fn new(from: u32, to: u32, kind: EdgeKind) -> Self {
        Self { from, to, kind }
    }
}

/// Dependency kinds an edge can express.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum EdgeKind {
    /// Pure ordering dependency (statement-IR sequence).
    Ordering,
    /// Reaching-definition: `to` reads a variable defined by `from`.
    /// Producers that run reaching-defs analysis populate this edge.
    Def {
        /// The variable name that flows along this edge.
        name: Ident,
    },
    /// Control-flow dependency (branch → body).
    Control,
}

/// Graph-IR view over a statement-IR Program.
#[derive(Debug, Clone, Default)]
#[non_exhaustive]
pub struct NodeGraph {
    /// Nodes indexed by their `id`.
    pub nodes: Vec<GraphNode>,
    /// Edges between nodes.
    pub edges: Vec<DataEdge>,
    /// Original workgroup size. Preserved through view conversion.
    pub workgroup_size: [u32; 3],
    /// Original buffer declarations. Preserved through view conversion.
    pub buffers: Vec<crate::ir_inner::model::program::BufferDecl>,
}

impl NodeGraph {
    /// Construct a `NodeGraph` from explicit node / edge vectors.
    /// Used by external tooling that synthesizes graphs without
    /// going through `from_program` (V7-EXT-027).
    ///
    /// `workgroup_size` defaults to [1, 1, 1] and buffers defaults to
    /// empty. For full control use struct-literal syntax inside the
    /// defining crate.
    #[must_use]
    pub fn new(nodes: Vec<GraphNode>, edges: Vec<DataEdge>) -> Self {
        Self {
            nodes,
            edges,
            workgroup_size: [1, 1, 1],
            buffers: Vec::new(),
        }
    }

    /// Lift a statement-IR `Program` into its graph-IR view.
    ///
    /// The 0.6 lifting emits one `GraphNode::Statement` per top-
    /// level `Node::entry()` node, connected by `Ordering` edges in
    /// document order. Later passes refine edges with reaching-
    /// definition / control-flow / dataflow analyses.
    ///
    /// `VYRE_IR_HOTSPOTS` HIGH (`graph_view.rs:205`): the previous
    /// implementation cloned every top-level node via `n.clone()`.
    /// This helper now delegates to `from_program_owned` after
    /// cloning the inner structure cheaply via `Arc` refcount bumps,
    /// so the hot path (when the caller owns the Program) can move
    /// directly into the graph without the per-node clone.
    #[must_use]
    pub fn from_program(program: &Program) -> Self {
        Self::from_program_owned(program.clone())
    }

    /// Build the graph by consuming the Program  -  moves the entry
    /// `Vec<Node>` out of its `Arc` when uniquely owned and avoids
    /// cloning each node. Use this whenever the caller holds the
    /// only `Program` reference.
    #[must_use]
    pub fn from_program_owned(program: Program) -> Self {
        let workgroup_size = program.workgroup_size();
        let buffers = program.buffers().to_vec();
        let entry_vec = program.into_entry_vec();
        let mut nodes = Vec::with_capacity(entry_vec.len());
        let mut edges = Vec::with_capacity(entry_vec.len().saturating_sub(1));
        for (i, n) in entry_vec.into_iter().enumerate() {
            #[allow(clippy::cast_possible_truncation)]
            let id = i as u32;
            nodes.push(GraphNode {
                id,
                kind: DataflowKind::Statement(n),
            });
            if id > 0 {
                edges.push(DataEdge {
                    from: id - 1,
                    to: id,
                    kind: EdgeKind::Ordering,
                });
            }
        }
        Self {
            nodes,
            edges,
            workgroup_size,
            buffers,
        }
    }

    /// Lower the graph view back into a statement-IR Program.
    /// Preserves document-order of `GraphNode::Statement` variants;
    /// `Phi` and synthetic `Barrier` variants are dropped (they
    /// don't round-trip to statement-IR by design).
    ///
    /// # Errors
    ///
    /// Returns `GraphValidateError::DanglingEdge` if an edge references
    /// a non-existent node id, `GraphValidateError::Cycle` if the graph
    /// contains a directed cycle, or `GraphValidateError::OrphanPhi` if
    /// a Phi node has no predecessors.
    pub fn try_into_program(self) -> Result<Program, GraphValidateError> {
        let node_count = u32::try_from(self.nodes.len()).unwrap_or(u32::MAX);

        // 1. Check for dangling edges.
        for edge in &self.edges {
            if edge.from >= node_count || edge.to >= node_count {
                return Err(GraphValidateError::DanglingEdge {
                    from: edge.from,
                    to: edge.to,
                });
            }
        }

        // 2. Check for orphan Phi nodes.
        for node in &self.nodes {
            if let DataflowKind::Phi(predecessors) = &node.kind {
                if predecessors.is_empty() {
                    return Err(GraphValidateError::OrphanPhi { node_id: node.id });
                }
                for &pred in predecessors {
                    if pred >= node_count {
                        return Err(GraphValidateError::OrphanPhi { node_id: node.id });
                    }
                }
            }
        }

        // 3. Check for cycles via an explicit-stack (iterative) DFS. A recursive
        //    DFS recurses once per node along a path, so a deep or adversarial
        //    linear graph (e.g. a 200k-statement chain round-tripped through
        //    to_graph) would overflow the call stack and panic. Validation must
        //    reject malformed topology *without panicking* at any scale
        //    (FINDING-FOUNDATION-1), so the traversal state lives on the heap.
        let mut adj: Vec<Vec<u32>> = vec![Vec::new(); self.nodes.len()];
        for edge in &self.edges {
            adj[edge.from as usize].push(edge.to);
        }

        // 0 = unvisited, 1 = on the current DFS path (gray), 2 = fully explored (black).
        let mut state = vec![0u8; self.nodes.len()];
        // Gray nodes on the current root->frontier path; used to reconstruct the
        // cycle when a back edge is found.
        let mut path: Vec<u32> = Vec::new();
        // Explicit DFS frames: (node, index of the next child to visit).
        let mut stack: Vec<(u32, usize)> = Vec::new();

        for root in 0..node_count {
            if state[root as usize] != 0 {
                continue;
            }
            state[root as usize] = 1;
            path.push(root);
            stack.push((root, 0));

            while let Some(&(node, child_idx)) = stack.last() {
                let children = &adj[node as usize];
                if child_idx < children.len() {
                    // Advance this frame's cursor before descending.
                    stack
                        .last_mut()
                        .expect("stack is non-empty inside `while let Some(..) = stack.last()`")
                        .1 += 1;
                    let next = children[child_idx];
                    match state[next as usize] {
                        0 => {
                            state[next as usize] = 1;
                            path.push(next);
                            stack.push((next, 0));
                        }
                        1 => {
                            // Back edge to a gray node: a directed cycle.
                            let cycle_start = path.iter().position(|&n| n == next).unwrap_or(0);
                            return Err(GraphValidateError::Cycle {
                                path: path[cycle_start..].to_vec(),
                            });
                        }
                        _ => {} // black: fully explored, no cycle through it.
                    }
                } else {
                    state[node as usize] = 2;
                    path.pop();
                    stack.pop();
                }
            }
        }

        let entry: Vec<Node> = self
            .nodes
            .into_iter()
            .filter_map(|gn| match gn.kind {
                DataflowKind::Statement(n) => Some(n),
                DataflowKind::Phi(_) => None,
                DataflowKind::Barrier => Some(Node::barrier()),
            })
            .collect();
        Ok(Program::wrapped(self.buffers, self.workgroup_size, entry))
    }
}

/// Convenience  -  `to_graph(program)` style call.
#[must_use]
pub fn to_graph(program: &Program) -> NodeGraph {
    NodeGraph::from_program(program)
}

/// Convenience  -  `from_graph(graph)` style call.
///
/// # Errors
///
/// Returns `GraphValidateError` if the graph contains dangling edges,
/// cycles, or orphan Phi nodes.
pub fn from_graph(graph: NodeGraph) -> Result<Program, GraphValidateError> {
    graph.try_into_program()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{BufferDecl, DataType, Expr, Node, Program};

    fn trivial() -> Program {
        Program::wrapped(
            vec![BufferDecl::read_write("out", 0, DataType::U32).with_count(1)],
            [1, 1, 1],
            vec![
                Node::let_bind("x", Expr::u32(42)),
                Node::store("out", Expr::u32(0), Expr::var("x")),
            ],
        )
    }

    #[test]
    fn graph_view_mirrors_top_level_nodes() {
        let p = trivial();
        let g = to_graph(&p);
        assert_eq!(g.nodes.len(), p.entry().len());
        assert_eq!(g.workgroup_size, p.workgroup_size());
    }

    #[test]
    fn graph_edges_are_ordering_in_sequence() {
        let p = trivial();
        let g = to_graph(&p);
        assert_eq!(g.edges.len(), g.nodes.len() - 1);
        for (i, e) in g.edges.iter().enumerate() {
            assert_eq!(e.from, i as u32);
            assert_eq!(e.to, (i + 1) as u32);
            assert!(matches!(e.kind, EdgeKind::Ordering));
        }
    }

    #[test]
    fn round_trip_is_byte_identical_under_canonicalize() {
        let p = trivial();
        let g = to_graph(&p);
        let p2 = from_graph(g).unwrap();
        // canonicalize both (to normalize operand ordering etc.)
        let p_c = crate::optimizer::passes::algebraic::canonicalize_engine::run(p);
        let p2_c = crate::optimizer::passes::algebraic::canonicalize_engine::run(p2);
        assert_eq!(p_c.to_wire().unwrap(), p2_c.to_wire().unwrap());
    }

    #[test]
    fn phi_node_dropped_on_lowering() {
        let mut g = NodeGraph {
            workgroup_size: [1, 1, 1],
            ..Default::default()
        };
        g.buffers
            .push(BufferDecl::read_write("out", 0, DataType::U32).with_count(1));
        g.nodes.push(GraphNode {
            id: 0,
            kind: DataflowKind::Statement(Node::store("out", Expr::u32(0), Expr::u32(1))),
        });
        g.nodes.push(GraphNode {
            id: 1,
            kind: DataflowKind::Phi(vec![0]),
        });
        let p = from_graph(g).unwrap();
        assert_eq!(
            p.entry().len(),
            1,
            "Phi must not round-trip to statement-IR"
        );
    }
}
