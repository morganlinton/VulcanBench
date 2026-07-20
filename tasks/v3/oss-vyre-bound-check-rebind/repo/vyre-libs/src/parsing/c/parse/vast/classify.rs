//! GPU VAST node-kind classification builders.

#![allow(missing_docs)] // Internal VAST-builder helpers are documented at the owning module boundary.
use crate::parsing::c::lex::tokens::*;
use crate::parsing::composition::child_phase;
use crate::region::wrap_anonymous;
use vyre::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

use super::helpers::*;
use super::*;

mod nodes_00;
mod nodes_01;
mod nodes_02;
mod nodes_03;
mod nodes_04;
mod nodes_05;
mod nodes_06;
mod nodes_07;
mod nodes_08;
mod nodes_09;

pub fn c11_classify_vast_node_kinds(
    vast_nodes: &str,
    num_nodes: Expr,
    out_typed_vast_nodes: &str,
) -> Program {
    c11_classify_vast_node_kinds_impl(vast_nodes, None, num_nodes, out_typed_vast_nodes, false)
}

pub fn c11_classify_vast_node_kinds_precomputed_context(
    vast_nodes: &str,
    decl_contexts: &str,
    num_nodes: Expr,
    out_typed_vast_nodes: &str,
) -> Program {
    c11_classify_vast_node_kinds_impl(
        vast_nodes,
        Some(decl_contexts),
        num_nodes,
        out_typed_vast_nodes,
        false,
    )
}

pub fn c11_classify_annotated_vast_node_kinds_precomputed_context(
    vast_nodes: &str,
    decl_contexts: &str,
    num_nodes: Expr,
    out_typed_vast_nodes: &str,
) -> Program {
    c11_classify_vast_node_kinds_impl(
        vast_nodes,
        Some(decl_contexts),
        num_nodes,
        out_typed_vast_nodes,
        true,
    )
}

fn c11_classify_vast_node_kinds_impl(
    vast_nodes: &str,
    decl_contexts: Option<&str>,
    num_nodes: Expr,
    out_typed_vast_nodes: &str,
    typedef_annotations_available: bool,
) -> Program {
    let t = Expr::InvocationId { axis: 0 };
    let base = Expr::mul(t.clone(), Expr::u32(VAST_NODE_STRIDE_U32));

    let mut loop_body = Vec::new();
    nodes_00::extend(
        &mut loop_body,
        vast_nodes,
        out_typed_vast_nodes,
        num_nodes.clone(),
        t.clone(),
        base.clone(),
    );
    nodes_01::extend(
        &mut loop_body,
        vast_nodes,
        out_typed_vast_nodes,
        num_nodes.clone(),
        t.clone(),
        base.clone(),
    );
    nodes_02::extend(
        &mut loop_body,
        vast_nodes,
        out_typed_vast_nodes,
        num_nodes.clone(),
        t.clone(),
        base.clone(),
        decl_contexts,
    );
    nodes_03::extend(
        &mut loop_body,
        vast_nodes,
        out_typed_vast_nodes,
        num_nodes.clone(),
        t.clone(),
        base.clone(),
        decl_contexts,
    );
    nodes_04::extend(
        &mut loop_body,
        vast_nodes,
        out_typed_vast_nodes,
        num_nodes.clone(),
        t.clone(),
        base.clone(),
    );
    nodes_05::extend(
        &mut loop_body,
        vast_nodes,
        out_typed_vast_nodes,
        num_nodes.clone(),
        t.clone(),
        base.clone(),
        typedef_annotations_available,
    );
    nodes_06::extend(
        &mut loop_body,
        vast_nodes,
        out_typed_vast_nodes,
        num_nodes.clone(),
        t.clone(),
        base.clone(),
    );
    nodes_07::extend(
        &mut loop_body,
        vast_nodes,
        out_typed_vast_nodes,
        num_nodes.clone(),
        t.clone(),
        base.clone(),
    );
    nodes_08::extend(
        &mut loop_body,
        vast_nodes,
        out_typed_vast_nodes,
        num_nodes.clone(),
        t.clone(),
        base.clone(),
    );
    nodes_09::extend(
        &mut loop_body,
        vast_nodes,
        out_typed_vast_nodes,
        num_nodes.clone(),
        t.clone(),
        base.clone(),
    );

    for field in 1..VAST_NODE_STRIDE_U32 {
        let value = if field == 1 {
            Expr::var("declarator_parent_override")
        } else {
            Expr::load(vast_nodes, Expr::add(base.clone(), Expr::u32(field)))
        };
        loop_body.push(Node::store(
            out_typed_vast_nodes,
            Expr::add(base.clone(), Expr::u32(field)),
            value,
        ));
    }

    let n = node_count(&num_nodes).max(1);
    let mut buffers =
        vec![
            BufferDecl::storage(vast_nodes, 0, BufferAccess::ReadOnly, DataType::U32)
                .with_count(n.saturating_mul(VAST_NODE_STRIDE_U32)),
        ];
    let out_binding = if let Some(decl_contexts) = decl_contexts {
        buffers.push(
            BufferDecl::storage(decl_contexts, 1, BufferAccess::ReadOnly, DataType::U32)
                .with_count(n.saturating_mul(VAST_DECL_CONTEXT_STRIDE_U32)),
        );
        2
    } else {
        1
    };
    buffers.push(
        BufferDecl::output(out_typed_vast_nodes, out_binding, DataType::U32)
            .with_count(n.saturating_mul(VAST_NODE_STRIDE_U32)),
    );

    Program::wrapped(
        buffers,
        [256, 1, 1],
        vec![wrap_anonymous(
            CLASSIFY_VAST_OP_ID,
            vec![Node::if_then(
                Expr::lt(t.clone(), num_nodes),
                vec![child_phase(
                    CLASSIFY_VAST_OP_ID,
                    "vyre-libs::parsing::c11_classify_vast_node_kinds::node_classification_pass",
                    loop_body,
                )],
            )],
        )],
    )
    .with_entry_op_id(CLASSIFY_VAST_OP_ID)
}
