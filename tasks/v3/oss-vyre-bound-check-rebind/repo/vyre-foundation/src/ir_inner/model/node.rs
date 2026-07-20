// Statement nodes  -  execute effects.
//
// Statements modify state: bind variables, write to buffers, branch, loop.
// A program's entry point is a sequence of statements.

use std::fmt;

pub use crate::ir_inner::model::generated::Node;

/// Public contract for downstream statement extension nodes.
///
/// Implementors own their payload and semantics. Core uses the stable
/// metadata here to validate, compare, and diagnose opaque nodes without
/// pretending it can execute or serialize them.
pub trait NodeExtension: fmt::Debug + Send + Sync + 'static {
    /// Stable extension namespace, for example `my_backend.speculate`.
    fn extension_kind(&self) -> &'static str;

    /// Human-readable identity used in diagnostics and debug logs.
    fn debug_identity(&self) -> &str;

    /// Stable, content-addressed identity for equality and optimizer keys.
    fn stable_fingerprint(&self) -> [u8; 32];

    /// Validate extension-local invariants.
    ///
    /// The returned error must explain the bad invariant and include `Fix:`.
    ///
    /// # Errors
    ///
    /// Returns an extension-defined diagnostic when the payload violates its
    /// local invariants.
    fn validate_extension(&self) -> Result<(), String>;

    /// Downcast to Any to allow backend-specific dispatch from opaque payloads.
    fn as_any(&self) -> &dyn std::any::Any;

    /// Serialize the extension payload into stable bytes used by the wire
    /// encoder's `Node::Opaque` path (tag `0x80`). Default: empty payload.
    ///
    /// The payload contract is endian-fixed: any numeric field wider than
    /// one byte MUST be written with `to_le_bytes` (or the
    /// [`crate::opaque_payload`] helpers) and the matching decoder MUST
    /// reconstruct it with `from_le_bytes`. Host-endian encodings such as
    /// `to_ne_bytes` are forbidden because the wire format must stay
    /// byte-identical across architectures: a Program encoded on a
    /// little-endian host and decoded on a big-endian host must produce
    /// the same `crate::ir::Program::hash` and the same IR.
    ///
    /// Extension authors are recommended (but not required, for API
    /// compatibility) to use [`crate::opaque_payload::LeBytesWriter`] when
    /// building payloads  -  it makes the right endianness the only choice at
    /// the type level.
    fn wire_payload(&self) -> Vec<u8> {
        Vec::new()
    }
}

mod impl_node;

/// Canonical string op id for every statement-node variant.
///
/// Wire-format encode/decode keys on these names to route an encoded node
/// back to its variant. Adding a new `Node` variant REQUIRES extending this
/// function with a matching arm  -  the wire decoder depends on round-tripping
/// the exact name this function returns.
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
        Node::Region { .. } => "vyre.node.region",
        Node::IndirectDispatch { .. } => "vyre.node.indirect_dispatch",
        Node::AsyncLoad { .. } => "vyre.node.async_load",
        Node::AsyncStore { .. } => "vyre.node.async_store",
        Node::AsyncWait { .. } => "vyre.node.async_wait",
        Node::Trap { .. } => "vyre.node.trap",
        Node::Resume { .. } => "vyre.node.resume",
        Node::AllReduce { .. } => "vyre.node.all_reduce",
        Node::AllGather { .. } => "vyre.node.all_gather",
        Node::ReduceScatter { .. } => "vyre.node.reduce_scatter",
        Node::Broadcast { .. } => "vyre.node.broadcast",
        Node::Opaque(extension) => extension.extension_kind(),
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use crate::ir::{BufferAccess, BufferDecl, DataType, Node, Program};

    #[test]
    fn indirect_dispatch_round_trip() {
        let program = Program::wrapped(
            vec![BufferDecl::storage(
                "counts",
                0,
                BufferAccess::ReadOnly,
                DataType::U32,
            )],
            [64, 1, 1],
            vec![Node::indirect_dispatch("counts", 16)],
        );

        let wire = program
            .to_wire()
            .expect("Fix: indirect dispatch must serialize into VIR0");
        let decoded =
            Program::from_wire(&wire).expect("Fix: indirect dispatch must decode from VIR0");

        assert_eq!(decoded, program);
    }

    #[test]
    fn async_load_async_wait_round_trip() {
        let program = Program::wrapped(
            vec![BufferDecl::storage(
                "out",
                0,
                BufferAccess::ReadWrite,
                DataType::U32,
            )],
            [1, 1, 1],
            vec![
                Node::async_load("chunk-0"),
                Node::store("out", crate::ir::Expr::u32(0), crate::ir::Expr::u32(1)),
                Node::async_wait("chunk-0"),
            ],
        );

        let wire = program
            .to_wire()
            .expect("Fix: async stream nodes must serialize into VIR0");
        let decoded =
            Program::from_wire(&wire).expect("Fix: async stream nodes must decode from VIR0");

        assert_eq!(decoded, program);
    }
}
