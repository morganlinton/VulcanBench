//! GPU IR builders: lower a C VAST node table into the property-graph
//! buffers that the dataflow analyzer consumes. Two entry points:
//! `c_lower_ast_to_pg_nodes` (node decode only) and
//! `c_lower_ast_to_pg_semantic_graph` (node + semantic edges).

use crate::parsing::c::lower::semantic_edges::*;
use crate::parsing::c::parse::vast::*;
use crate::parsing::composition::child_phase;
use vyre::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

use super::*;

mod classification;
mod context;
mod edge_store;
mod semantic_graph;
mod sizing;
mod structural_nodes;

pub use semantic_graph::{
    c_lower_ast_to_pg_semantic_graph, c_lower_ast_to_pg_semantic_graph_with_pg,
    c_lower_ast_to_pg_semantic_graph_with_pg_no_control_resolution,
};
pub use structural_nodes::c_lower_ast_to_pg_nodes;

use classification::*;
use context::*;
use edge_store::*;
use sizing::infer_node_count_words;

/// Malformed byte input for CPU oracle decoding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PgReferenceDecodeError {
    /// Input byte length is not a whole number of `u32` words.
    MisalignedBytes {
        /// Actual byte length.
        len: usize,
    },
    /// Input word count is not a whole number of VAST rows.
    PartialVastRow {
        /// Actual decoded word count.
        words: usize,
        /// Required row stride.
        stride: usize,
    },
}

/// Semantic PG witness rows computed by the CPU oracle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SemanticPgReference {
    /// Semantic node rows.
    pub nodes: Vec<u8>,
    /// Semantic edge rows.
    pub edges: Vec<u8>,
}

impl std::fmt::Display for PgReferenceDecodeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MisalignedBytes { len } => write!(
                formatter,
                "VAST byte input has {len} bytes, which is not 4-byte aligned. Fix: pass complete u32 rows to the AST-to-PG reference oracle."
            ),
            Self::PartialVastRow { words, stride } => write!(
                formatter,
                "VAST word input has {words} words, which is not a multiple of row stride {stride}. Fix: pass complete VAST rows to the AST-to-PG reference oracle."
            ),
        }
    }
}

impl std::error::Error for PgReferenceDecodeError {}
