//! Reject `Async*` / `Resume` nodes at the high-level `Program` emit entry.
//!
//! `emit_module` is the Program-compatibility path; it does NOT run the async
//! resolution pass (`vfs::resolve`) that rewrites `Node::AsyncLoad`/`AsyncStore`
//! streaming intents into the bounded copy loops the descriptor emitter lowers,
//! and `Node::Resume` is a trap-protocol continuation that only has meaning once
//! the descriptor-level trap machinery is in place. A raw async/resume node
//! reaching this entry therefore means a required earlier pass was skipped.
//!
//! Failing closed here (Law 10) is correct: silently lowering — or, worse,
//! treating `Resume` as a no-op — would drop the node's semantics invisibly.
//! The descriptor-level emitter (`crate::emit`) still lowers `AsyncLoad` /
//! `AsyncStore` directly for callers that built a `KernelDescriptor` after
//! resolving their program (see `descriptor_control` tests), so this guard
//! narrows the rejection to exactly the unresolved Program-entry path.

use std::ops::ControlFlow::{self, Break, Continue};

use vyre_foundation::ir::model::expr::GeneratorRef;
use vyre_foundation::ir::{Expr, Ident, NodeExtension, Program};
use vyre_foundation::ir::Node;
use vyre_foundation::visit::{visit_node_preorder, NodeVisitor};

/// Breaks the preorder walk with the offending node's kind token the moment an
/// `Async*` / `Resume` node is reached. Every other variant continues; in
/// particular `Trap` is NOT rejected — it lowers to a backend sidecar.
struct AsyncResumeRejector;

impl NodeVisitor for AsyncResumeRejector {
    type Break = &'static str;

    fn visit_let(&mut self, _: &Node, _: &Ident, _: &Expr) -> ControlFlow<&'static str> {
        Continue(())
    }

    fn visit_assign(&mut self, _: &Node, _: &Ident, _: &Expr) -> ControlFlow<&'static str> {
        Continue(())
    }

    fn visit_store(
        &mut self,
        _: &Node,
        _: &Ident,
        _: &Expr,
        _: &Expr,
    ) -> ControlFlow<&'static str> {
        Continue(())
    }

    fn visit_if(&mut self, _: &Node, _: &Expr, _: &[Node], _: &[Node]) -> ControlFlow<&'static str> {
        Continue(())
    }

    fn visit_loop(
        &mut self,
        _: &Node,
        _: &Ident,
        _: &Expr,
        _: &Expr,
        _: &[Node],
    ) -> ControlFlow<&'static str> {
        Continue(())
    }

    fn visit_indirect_dispatch(&mut self, _: &Node, _: &Ident, _: u64) -> ControlFlow<&'static str> {
        Continue(())
    }

    fn visit_async_load(
        &mut self,
        _: &Node,
        _: &Ident,
        _: &Ident,
        _: &Expr,
        _: &Expr,
        _: &Ident,
    ) -> ControlFlow<&'static str> {
        Break("AsyncLoad")
    }

    fn visit_async_store(
        &mut self,
        _: &Node,
        _: &Ident,
        _: &Ident,
        _: &Expr,
        _: &Expr,
        _: &Ident,
    ) -> ControlFlow<&'static str> {
        Break("AsyncStore")
    }

    fn visit_async_wait(&mut self, _: &Node, _: &Ident) -> ControlFlow<&'static str> {
        Break("AsyncWait")
    }

    fn visit_trap(&mut self, _: &Node, _: &Expr, _: &Ident) -> ControlFlow<&'static str> {
        Continue(())
    }

    fn visit_resume(&mut self, _: &Node, _: &Ident) -> ControlFlow<&'static str> {
        Break("Resume")
    }

    fn visit_return(&mut self, _: &Node) -> ControlFlow<&'static str> {
        Continue(())
    }

    fn visit_barrier(&mut self, _: &Node) -> ControlFlow<&'static str> {
        Continue(())
    }

    fn visit_block(&mut self, _: &Node, _: &[Node]) -> ControlFlow<&'static str> {
        Continue(())
    }

    fn visit_region(
        &mut self,
        _: &Node,
        _: &Ident,
        _: &Option<GeneratorRef>,
        _: &[Node],
    ) -> ControlFlow<&'static str> {
        Continue(())
    }

    fn visit_opaque_node(
        &mut self,
        _: &Node,
        _: &dyn NodeExtension,
    ) -> ControlFlow<&'static str> {
        Continue(())
    }
}

/// Returns `Err(kind)` naming the first `Async*` / `Resume` node anywhere in
/// `program` (including nested `If`/`Loop`/`Block`/`Region` bodies — the
/// preorder walk recurses), or `Ok(())` when the program contains none.
pub(super) fn reject_async_resume(program: &Program) -> Result<(), &'static str> {
    let mut rejector = AsyncResumeRejector;
    for node in program.entry() {
        if let Break(kind) = visit_node_preorder(&mut rejector, node) {
            return Err(kind);
        }
    }
    Ok(())
}
