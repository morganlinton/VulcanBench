//! Consumer-neutral resident workspace adapter for megakernel programs.
//!
//! Runtime owns the buffer/IR composition seam. Frontends, analyzers, parsers,
//! and dataflow engines own their domain-specific manifests and phase machines.

use vyre_foundation::ir::{BufferDecl, Node};

/// Adapter implemented by any domain that wants a GPU-resident megakernel
/// workspace without baking that domain into `vyre-runtime`.
pub trait MegakernelWorkspaceAdapter {
    /// Buffer declaration inserted after the core megakernel buffers.
    fn buffer_decl(&self) -> BufferDecl;

    /// IR nodes that initialize resident workspace state.
    fn bootstrap_nodes(&self) -> Vec<Node>;

    /// IR nodes that validate resident control-plane state before dispatch.
    fn guard_nodes(&self) -> Vec<Node> {
        Vec::new()
    }

    /// IR nodes that run domain-owned phase/control handlers.
    fn dispatch_nodes(&self) -> Vec<Node> {
        Vec::new()
    }
}
