mod buffer_validation_contracts;
mod call_collection_contracts;
mod equality_fingerprint_contracts;
mod validation_cache_contracts;

use std::sync::Arc;

use super::Program;
use crate::error::Error;
use crate::ir::{Expr, Ident, Node};
use crate::ir_inner::model::program::BufferDecl;
use crate::ir_inner::model::types::DataType;
use crate::transform::visit::collect_call_op_ids;

fn sample_body() -> Vec<Node> {
    vec![
        Node::let_bind("value", Expr::u32(7)),
        Node::store("out", Expr::u32(0), Expr::var("value")),
        Node::Return,
    ]
}
