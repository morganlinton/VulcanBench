use std::sync::Arc;

use vyre_foundation::ir::model::expr::Ident;
use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

use super::body::csr_forward_or_changed_body;
use super::layout::OP_ID;
use super::layout::{
    CSR_FORWARD_OR_CHANGED_CHANGED_BUFFER, CSR_FORWARD_OR_CHANGED_FRONTIER_BUFFER,
    CSR_FORWARD_OR_CHANGED_WORKGROUP_SIZE,
};
use crate::graph::program_graph::ProgramGraphShape;

/// Standalone in-place expansion program for primitive conformance.
#[must_use]
pub fn csr_forward_or_changed(
    shape: ProgramGraphShape,
    frontier_out: &str,
    changed: &str,
    edge_kind_mask: u32,
) -> Program {
    let words = crate::bitset::bitset_words(shape.node_count);
    let mut body = vec![Node::let_bind("local_changed", Expr::u32(0))];
    body.extend(csr_forward_or_changed_body(
        shape,
        frontier_out,
        "local_changed",
        edge_kind_mask,
    ));
    body.push(Node::if_then(
        Expr::eq(Expr::var("local_changed"), Expr::u32(1)),
        vec![Node::let_bind(
            "_changed",
            Expr::atomic_or(changed, Expr::u32(0), Expr::u32(1)),
        )],
    ));
    let mut buffers = shape.read_only_buffers();
    buffers.push(
        BufferDecl::storage(
            frontier_out,
            CSR_FORWARD_OR_CHANGED_FRONTIER_BUFFER,
            BufferAccess::ReadWrite,
            DataType::U32,
        )
        .with_count(words.max(1)),
    );
    buffers.push(
        BufferDecl::storage(
            changed,
            CSR_FORWARD_OR_CHANGED_CHANGED_BUFFER,
            BufferAccess::ReadWrite,
            DataType::U32,
        )
        .with_count(1),
    );
    Program::wrapped(
        buffers,
        CSR_FORWARD_OR_CHANGED_WORKGROUP_SIZE,
        vec![Node::Region {
            generator: Ident::from(OP_ID),
            source_region: None,
            body: Arc::new(body),
        }],
    )
}
