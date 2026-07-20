use super::ast_to_pg_nodes::{
    C_AST_PG_EDGE_CASE_VALUE, C_AST_PG_EDGE_GOTO_TARGET, C_AST_PG_EDGE_NONE,
    C_AST_PG_EDGE_SWITCH_CASE, C_AST_PG_EDGE_SWITCH_DEFAULT, C_AST_PG_EDGE_SWITCH_SELECTOR,
};
use crate::parsing::c::parse::vast::{
    C_AST_KIND_CASE_STMT, C_AST_KIND_DEFAULT_STMT, C_AST_KIND_GOTO_STMT, C_AST_KIND_LABEL_STMT,
    C_AST_KIND_SWITCH_STMT,
};
use vyre::ir::{Expr, Node};

mod gpu_helpers;
mod gpu_resolution;
mod model;
#[cfg(any(test, feature = "cpu-parity"))]
mod reference;
#[cfg(test)]
mod tests;

pub(super) use gpu_resolution::semantic_resolution_nodes;
#[cfg(any(test, feature = "cpu-parity"))]
pub(super) use model::SemanticEdge;
#[cfg(any(test, feature = "cpu-parity"))]
pub(super) use reference::resolved_semantic_edges;

use gpu_helpers::*;
use model::*;
