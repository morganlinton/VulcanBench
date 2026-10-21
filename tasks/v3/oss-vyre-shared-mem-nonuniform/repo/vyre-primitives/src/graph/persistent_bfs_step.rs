//! One persistent-BFS workgroup step with coalesced change detection.

use std::sync::Arc;

use vyre_foundation::ir::model::expr::{GeneratorRef, Ident};
use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

use crate::graph::csr_forward_or_changed::csr_forward_or_changed_child_prefixed;
use crate::graph::program_graph::{ProgramGraphShape, BINDING_PRIMITIVE_START};
use crate::reduce::workgroup_any::workgroup_any_u32_child_prefixed;

/// Canonical op id for one persistent-BFS workgroup-coalesced step.
pub const PERSISTENT_BFS_STEP_OP_ID: &str = "vyre-primitives::graph::persistent_bfs_step";

/// Build one reusable persistent-BFS step body.
#[must_use]
pub fn persistent_bfs_step_body(
    shape: ProgramGraphShape,
    frontier_out: &str,
    changed: &str,
    scratch: &str,
    edge_kind_mask: u32,
) -> Vec<Node> {
    persistent_bfs_step_body_prefixed(
        shape,
        frontier_out,
        changed,
        scratch,
        edge_kind_mask,
        "step",
    )
}

/// Build one persistent-BFS step body with local names prefixed for repeated
/// inlining inside larger kernels.
#[must_use]
pub fn persistent_bfs_step_body_prefixed(
    shape: ProgramGraphShape,
    frontier_out: &str,
    changed: &str,
    scratch: &str,
    edge_kind_mask: u32,
    local_prefix: &str,
) -> Vec<Node> {
    let local_changed = format!("{local_prefix}_local_changed");
    let any_changed = format!("{local_prefix}_any_changed");
    let t = Expr::gid_x();
    vec![
        Node::let_bind(local_changed.as_str(), Expr::u32(0)),
        Node::store(scratch, Expr::local_x(), Expr::u32(0)),
        Node::barrier(),
        Node::if_then(
            Expr::lt(t, Expr::u32(shape.node_count)),
            vec![csr_forward_or_changed_child_prefixed(
                PERSISTENT_BFS_STEP_OP_ID,
                shape,
                frontier_out,
                local_changed.as_str(),
                edge_kind_mask,
                &format!("{local_prefix}_csr"),
            )],
        ),
        Node::store(scratch, Expr::local_x(), Expr::var(local_changed.as_str())),
        Node::barrier(),
        Node::if_then(
            Expr::eq(Expr::local_x(), Expr::u32(0)),
            vec![
                Node::let_bind(any_changed.as_str(), Expr::u32(0)),
                workgroup_any_u32_child_prefixed(
                    PERSISTENT_BFS_STEP_OP_ID,
                    scratch,
                    any_changed.as_str(),
                    256,
                    &format!("{local_prefix}_any_i"),
                ),
                Node::if_then(
                    Expr::ne(Expr::var(any_changed.as_str()), Expr::u32(0)),
                    vec![Node::let_bind(
                        format!("{local_prefix}_atomic_old"),
                        Expr::atomic_or(changed, Expr::u32(0), Expr::u32(1)),
                    )],
                ),
            ],
        ),
        Node::barrier(),
    ]
}

/// Wrap the persistent-BFS step as a child of `parent_op_id`.
#[must_use]
pub fn persistent_bfs_step_child(
    parent_op_id: &str,
    shape: ProgramGraphShape,
    frontier_out: &str,
    changed: &str,
    scratch: &str,
    edge_kind_mask: u32,
) -> Node {
    persistent_bfs_step_child_prefixed(
        parent_op_id,
        shape,
        frontier_out,
        changed,
        scratch,
        edge_kind_mask,
        "step",
    )
}

/// Wrap one persistent-BFS step as a child with prefixed locals for repeated
/// inlining under a no-shadowing validator.
#[must_use]
pub fn persistent_bfs_step_child_prefixed(
    parent_op_id: &str,
    shape: ProgramGraphShape,
    frontier_out: &str,
    changed: &str,
    scratch: &str,
    edge_kind_mask: u32,
    local_prefix: &str,
) -> Node {
    Node::Region {
        generator: Ident::from(PERSISTENT_BFS_STEP_OP_ID),
        source_region: Some(GeneratorRef {
            name: parent_op_id.to_string(),
        }),
        body: Arc::new(persistent_bfs_step_body_prefixed(
            shape,
            frontier_out,
            changed,
            scratch,
            edge_kind_mask,
            local_prefix,
        )),
    }
}

/// Wrap one persistent-BFS step and write the per-step convergence flag into
/// `scratch[active_scratch_index]`.
#[must_use]
pub fn persistent_bfs_step_child_prefixed_with_active(
    parent_op_id: &str,
    shape: ProgramGraphShape,
    frontier_out: &str,
    changed: &str,
    scratch: &str,
    active_scratch: &str,
    edge_kind_mask: u32,
    local_prefix: &str,
) -> Node {
    Node::Region {
        generator: Ident::from(PERSISTENT_BFS_STEP_OP_ID),
        source_region: Some(GeneratorRef {
            name: parent_op_id.to_string(),
        }),
        body: Arc::new(persistent_bfs_step_body_prefixed_with_active(
            shape,
            frontier_out,
            changed,
            scratch,
            active_scratch,
            edge_kind_mask,
            local_prefix,
        )),
    }
}

#[must_use]
fn persistent_bfs_step_body_prefixed_with_active(
    shape: ProgramGraphShape,
    frontier_out: &str,
    changed: &str,
    scratch: &str,
    active_scratch: &str,
    edge_kind_mask: u32,
    local_prefix: &str,
) -> Vec<Node> {
    let local_changed = format!("{local_prefix}_local_changed");
    let any_changed = format!("{local_prefix}_any_changed");
    let t = Expr::gid_x();
    vec![
        Node::let_bind(local_changed.as_str(), Expr::u32(0)),
        Node::store(scratch, Expr::local_x(), Expr::u32(0)),
        Node::barrier(),
        Node::if_then(
            Expr::and(
                Expr::ne(Expr::load(active_scratch, Expr::u32(0)), Expr::u32(0)),
                Expr::lt(t, Expr::u32(shape.node_count)),
            ),
            vec![csr_forward_or_changed_child_prefixed(
                PERSISTENT_BFS_STEP_OP_ID,
                shape,
                frontier_out,
                local_changed.as_str(),
                edge_kind_mask,
                &format!("{local_prefix}_csr"),
            )],
        ),
        Node::store(scratch, Expr::local_x(), Expr::var(local_changed.as_str())),
        Node::barrier(),
        Node::if_then(
            Expr::eq(Expr::local_x(), Expr::u32(0)),
            vec![
                Node::let_bind(any_changed.as_str(), Expr::u32(0)),
                workgroup_any_u32_child_prefixed(
                    PERSISTENT_BFS_STEP_OP_ID,
                    scratch,
                    any_changed.as_str(),
                    256,
                    &format!("{local_prefix}_any_i"),
                ),
                Node::store(
                    active_scratch,
                    Expr::u32(0),
                    Expr::var(any_changed.as_str()),
                ),
                Node::if_then(
                    Expr::ne(Expr::var(any_changed.as_str()), Expr::u32(0)),
                    vec![Node::let_bind(
                        format!("{local_prefix}_atomic_old"),
                        Expr::atomic_or(changed, Expr::u32(0), Expr::u32(1)),
                    )],
                ),
            ],
        ),
        Node::barrier(),
    ]
}

/// Standalone one-step program for primitive-level conformance.
#[must_use]
pub fn persistent_bfs_step(
    shape: ProgramGraphShape,
    frontier_out: &str,
    changed: &str,
    edge_kind_mask: u32,
) -> Program {
    let words = crate::bitset::bitset_words(shape.node_count).max(1);
    let mut buffers = shape.read_only_buffers();
    buffers.push(
        BufferDecl::storage(
            frontier_out,
            BINDING_PRIMITIVE_START,
            BufferAccess::ReadWrite,
            DataType::U32,
        )
        .with_count(words),
    );
    buffers.push(
        BufferDecl::storage(
            changed,
            BINDING_PRIMITIVE_START + 1,
            BufferAccess::ReadWrite,
            DataType::U32,
        )
        .with_count(1),
    );
    buffers.push(BufferDecl::workgroup("wg_scratch", 256, DataType::U32));

    Program::wrapped(
        buffers,
        [256, 1, 1],
        vec![Node::Region {
            generator: Ident::from(PERSISTENT_BFS_STEP_OP_ID),
            source_region: None,
            body: Arc::new(persistent_bfs_step_body(
                shape,
                frontier_out,
                changed,
                "wg_scratch",
                edge_kind_mask,
            )),
        }],
    )
}

#[cfg(feature = "inventory-registry")]
inventory::submit! {
    crate::harness::OpEntry::new(
        PERSISTENT_BFS_STEP_OP_ID,
        || persistent_bfs_step(ProgramGraphShape::new(4, 4), "frontier_out", "changed", 0xFFFF_FFFF),
        Some(|| {
            let to_bytes = |w: &[u32]| crate::wire::pack_u32_slice(w);
            vec![vec![
                to_bytes(&[0, 0, 0, 0]),
                to_bytes(&[0, 2, 3, 4, 4]),
                to_bytes(&[1, 2, 3, 3]),
                to_bytes(&[1, 1, 1, 1]),
                to_bytes(&[0, 0, 0, 0]),
                to_bytes(&[0b0001]),
                to_bytes(&[0]),
            ]]
        }),
        Some(|| {
            let to_bytes = |w: &[u32]| crate::wire::pack_u32_slice(w);
            vec![vec![to_bytes(&[0b1111]), to_bytes(&[1])]]
        }),
    )
}
