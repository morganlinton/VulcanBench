//! Backend support validation before dispatch.

use super::capability::Backend;
use std::sync::Arc;
use vyre_foundation::ir::model::node::Node;
use vyre_foundation::ir::{OpId, Program, ValidationError};

const CORE_SUPPORTED_OP_IDS: &[&str] = &[
    "vyre.node.let",
    "vyre.node.assign",
    "vyre.node.store",
    "vyre.node.if",
    "vyre.node.loop",
    "vyre.node.return",
    "vyre.node.block",
    "vyre.node.barrier",
    "vyre.node.indirect_dispatch",
    "vyre.node.async_load",
    "vyre.node.async_wait",
    "vyre.node.region",
    "vyre.lit_u32",
    "vyre.lit_i32",
    "vyre.lit_f32",
    "vyre.lit_bool",
    "vyre.var",
    "vyre.bin_op",
    "vyre.un_op",
    "vyre.load",
    "vyre.store",
];

/// Validate that `backend` supports every operation in `program`.
pub fn validate_program(program: &Program, backend: &dyn Backend) -> Result<(), ValidationError> {
    for (index, node) in program.entry().iter().enumerate() {
        validate_node(node, index, backend.id(), backend.supported_ops())?;
    }
    Ok(())
}

/// Default core operation support set for legacy backends.
pub fn default_supported_ops() -> &'static std::collections::HashSet<OpId> {
    static OPS: std::sync::OnceLock<std::collections::HashSet<OpId>> = std::sync::OnceLock::new();
    OPS.get_or_init(|| {
        let mut ops = std::collections::HashSet::new();
        let _ = ops.try_reserve(CORE_SUPPORTED_OP_IDS.len());
        ops.extend(CORE_SUPPORTED_OP_IDS.iter().copied().map(Arc::<str>::from));
        ops
    })
}

/// Default core operation set plus `Node::Trap`.
///
/// `Trap` is a structural control-flow node, not a concrete-driver extension:
/// backends that lower it as lane termination should use this shared set
/// instead of carrying a backend-local `OnceLock` and literal allocation.
pub fn default_supported_ops_with_trap() -> &'static std::collections::HashSet<OpId> {
    static OPS: std::sync::OnceLock<std::collections::HashSet<OpId>> = std::sync::OnceLock::new();
    OPS.get_or_init(|| {
        let base = default_supported_ops();
        let reserve = base.len().saturating_add(1);
        let mut ops = std::collections::HashSet::new();
        let _ = ops.try_reserve(reserve);
        ops.extend(base.iter().cloned());
        ops.insert(Arc::<str>::from("vyre.node.trap"));
        ops
    })
}

fn validate_node(
    node: &Node,
    index: usize,
    backend: &'static str,
    supported: &std::collections::HashSet<OpId>,
) -> Result<(), ValidationError> {
    let op = node_op_id(node);
    if !supported.contains(op) {
        let op_id = Arc::<str>::from(op);
        return Err(ValidationError::unsupported_op(backend, &op_id, index));
    }
    match node {
        Node::If {
            then, otherwise, ..
        } => {
            for (offset, nested) in then.iter().enumerate() {
                validate_node(nested, offset, backend, supported)?;
            }
            for (offset, nested) in otherwise.iter().enumerate() {
                validate_node(nested, offset, backend, supported)?;
            }
        }
        Node::Loop { body, .. } | Node::Block(body) => {
            for (offset, nested) in body.iter().enumerate() {
                validate_node(nested, offset, backend, supported)?;
            }
        }
        Node::Region { body, .. } => {
            for (offset, nested) in body.iter().enumerate() {
                validate_node(nested, offset, backend, supported)?;
            }
        }
        // Leaf nodes and backend-transparent nodes (opaque extensions
        // validate themselves via `NodeExtension::validate_extension`).
        Node::Let { .. }
        | Node::Assign { .. }
        | Node::Store { .. }
        | Node::Return
        | Node::Barrier { .. }
        | Node::IndirectDispatch { .. }
        | Node::AsyncLoad { .. }
        | Node::AsyncWait { .. }
        | Node::Opaque(_) => {}
        // `Node` is `#[non_exhaustive]` in vyre-foundation. Future variants
        // land here as transparent leaves until a dedicated arm is added.
        _ => {}
    }
    Ok(())
}

/// Return the stable operation id for legacy statement nodes.
#[must_use]
pub fn node_op_id(node: &Node) -> &'static str {
    match node {
        Node::Let { .. } => "vyre.node.let",
        Node::Assign { .. } => "vyre.node.assign",
        Node::Store { .. } => "vyre.node.store",
        Node::If { .. } => "vyre.node.if",
        Node::Loop { .. } => "vyre.node.loop",
        Node::Return => "vyre.node.return",
        Node::Block(_) => "vyre.node.block",
        Node::Barrier { .. } => "vyre.node.barrier",
        Node::IndirectDispatch { .. } => "vyre.node.indirect_dispatch",
        Node::AsyncLoad { .. } => "vyre.node.async_load",
        Node::AsyncWait { .. } => "vyre.node.async_wait",
        Node::Trap { .. } => "vyre.node.trap",
        Node::Resume { .. } => "vyre.node.resume",
        Node::AllReduce { .. } => "vyre.node.all_reduce",
        Node::AllGather { .. } => "vyre.node.all_gather",
        Node::ReduceScatter { .. } => "vyre.node.reduce_scatter",
        Node::Broadcast { .. } => "vyre.node.broadcast",
        // Region is a debug wrapper produced by vyre-libs Cat-A
        // compositions. Every backend must accept it  -  either by
        // lowering its body transparently or via the region_inline
        // optimizer pass. Treat it as a structural node
        // with no capability requirement.
        Node::Region { .. } => "vyre.node.region",
        Node::Opaque(extension) => extension.extension_kind(),
        // Non-exhaustive safety net: future Node variants added in
        // vyre-foundation must receive a dedicated op id before release.
        _ => "vyre.node.unknown",
    }
}
