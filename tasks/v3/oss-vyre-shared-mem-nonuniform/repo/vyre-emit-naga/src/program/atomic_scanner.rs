//! Pre-emission scan that walks a `Program` to identify which buffers
//! are the targets of `Expr::Atomic*` (so element types must wrap in
//! `atomic<...>`) and which buffers receive any write at all (so
//! `BufferAccess` can be auto-downgraded to `ReadOnly` when nobody
//! writes them).

use std::ops::ControlFlow::{self, Continue};

use rustc_hash::FxHashSet;

use vyre_foundation::ir::model::expr::GeneratorRef;
use vyre_foundation::ir::{Expr, Ident, Node};
use vyre_foundation::visit::{visit_node_preorder, visit_preorder, ExprVisitor, NodeVisitor};

use super::extension_ops::{scan_registered_atomic_expr, scan_registered_atomic_node};
use super::LoweringError;

/// Walk every node + sub-expression collecting buffer names that
/// appear as the target of `Expr::Atomic`. The result drives
/// `add_buffer`'s decision to wrap an element type in `atomic<...>`.
pub(super) fn scan_atomic_targets(
    node: &Node,
    out: &mut FxHashSet<Ident>,
) -> Result<(), LoweringError> {
    let mut scanner = AtomicTargetScanner { out };
    match visit_node_preorder(&mut scanner, node) {
        Continue(()) => Ok(()),
        std::ops::ControlFlow::Break(error) => Err(error),
    }
}

/// Mirror of [`scan_atomic_targets`] that ALSO collects buffers
/// that receive a write via `Node::Store` / `Node::AsyncStore` /
/// `Node::AsyncLoad` / `Node::IndirectDispatch` / `Expr::Atomic*`.
/// Both the atomic-target set and the write-target set come out of
/// one walk.
pub(super) fn scan_atomic_targets_into(
    node: &Node,
    atomic_out: &mut FxHashSet<Ident>,
    write_out: &mut FxHashSet<Ident>,
) -> Result<(), LoweringError> {
    // Atomic-target scan reuses the existing scanner for naga's
    // atomic-element-type decision.
    scan_atomic_targets(node, atomic_out)?;
    // Write-target scan: traverse Node::Store / AsyncStore /
    // AsyncLoad / IndirectDispatch buffer names. We use the atomic
    // set seed (every atomic target is also a write target) and add
    // the direct-store destinations.
    write_out.extend(atomic_out.iter().cloned());
    collect_node_store_buffers(node, write_out);
    Ok(())
}

/// Recursively walk a Node tree (without going through the visitor
/// trait) and collect every buffer name that appears as the
/// destination of `Node::Store`, `Node::AsyncStore` (`dest_buffer`),
/// `Node::AsyncLoad` (`destination`  -  DMA target buffer that the
/// shader writes into via the synthesized in-shader copy loop in
/// `emit_synchronous_async_load`), or `Node::IndirectDispatch`
/// (the dispatch buffer is written by
/// the host but ALSO tagged as a write target so the storage class
/// reflects WG↔HOST shared writability).
fn collect_node_store_buffers(node: &Node, out: &mut FxHashSet<Ident>) {
    use vyre_foundation::ir::Node as N;
    match node {
        N::Store { buffer, .. } => {
            out.insert(Ident::from(buffer));
        }
        N::AsyncStore { destination, .. } => {
            out.insert(Ident::from(destination));
        }
        N::AsyncLoad { destination, .. } => {
            // The destination of AsyncLoad is the buffer that receives
            // the remote bytes  -  it IS a write target. Without this,
            // BufferAccess auto-inference downgrades it to ReadOnly,
            // naga emits `var<storage, read>`, and the in-shader Store
            // synthesized by emit_synchronous_async_load is rejected
            // with `InvalidStorePointer`. The entry.rs docstring at the
            // call site already said "AsyncLoad" should be tracked;
            // this matches that intent.
            out.insert(Ident::from(destination));
        }
        N::IndirectDispatch { count_buffer, .. } => {
            out.insert(Ident::from(count_buffer));
        }
        N::If {
            then, otherwise, ..
        } => {
            for c in then.iter().chain(otherwise.iter()) {
                collect_node_store_buffers(c, out);
            }
        }
        N::Loop { body, .. } => {
            for c in body {
                collect_node_store_buffers(c, out);
            }
        }
        N::Block(body) => {
            for c in body {
                collect_node_store_buffers(c, out);
            }
        }
        N::Region { body, .. } => {
            for c in body.as_ref() {
                collect_node_store_buffers(c, out);
            }
        }
        // Other variants (Let, Assign, Return, Barrier, AsyncLoad,
        // AsyncWait, Trap, Resume, Opaque) either don't write to a
        // buffer or write to one already covered by the atomic scan
        // (atomic-result is captured via scan_atomic_targets).
        _ => {}
    }
}

struct AtomicTargetScanner<'a> {
    out: &'a mut FxHashSet<Ident>,
}

impl ExprVisitor for AtomicTargetScanner<'_> {
    type Break = LoweringError;

    fn visit_atomic(
        &mut self,
        _expr: &Expr,
        _: &vyre_foundation::ir::AtomicOp,
        buffer: &vyre_foundation::ir::Ident,
        _: &Expr,
        _: Option<&Expr>,
        _: &Expr,
    ) -> ControlFlow<Self::Break> {
        self.out.insert(Ident::from(buffer));
        Continue(())
    }

    fn visit_opaque_expr(
        &mut self,
        _: &Expr,
        ext: &dyn vyre_foundation::ir::ExprNode,
    ) -> ControlFlow<Self::Break> {
        match scan_registered_atomic_expr(ext, self.out) {
            Ok(true) => Continue(()),
            Ok(false) => ControlFlow::Break(LoweringError::invalid(format!(
                "unsupported opaque expression `{}` in atomic scan. Fix: register NagaProgramScanAtomicExpr for this extension or lower it before Naga atomic-target analysis.",
                ext.debug_identity()
            ))),
            Err(error) => ControlFlow::Break(error),
        }
    }
}

impl NodeVisitor for AtomicTargetScanner<'_> {
    type Break = LoweringError;

    fn visit_let(
        &mut self,
        _: &Node,
        _: &vyre_foundation::ir::Ident,
        value: &Expr,
    ) -> ControlFlow<Self::Break> {
        visit_preorder(self, value)
    }

    fn visit_assign(
        &mut self,
        _: &Node,
        _: &vyre_foundation::ir::Ident,
        value: &Expr,
    ) -> ControlFlow<Self::Break> {
        visit_preorder(self, value)
    }

    fn visit_store(
        &mut self,
        _: &Node,
        _: &vyre_foundation::ir::Ident,
        index: &Expr,
        value: &Expr,
    ) -> ControlFlow<Self::Break> {
        visit_preorder(self, index)?;
        visit_preorder(self, value)
    }

    fn visit_if(
        &mut self,
        _node: &Node,
        cond: &Expr,
        _: &[Node],
        _: &[Node],
    ) -> ControlFlow<Self::Break> {
        visit_preorder(self, cond)?;
        Continue(())
    }

    fn visit_loop(
        &mut self,
        _node: &Node,
        _: &vyre_foundation::ir::Ident,
        from: &Expr,
        to: &Expr,
        _: &[Node],
    ) -> ControlFlow<Self::Break> {
        visit_preorder(self, from)?;
        visit_preorder(self, to)?;
        Continue(())
    }

    fn visit_indirect_dispatch(
        &mut self,
        _: &Node,
        _: &vyre_foundation::ir::Ident,
        _: u64,
    ) -> ControlFlow<Self::Break> {
        Continue(())
    }

    fn visit_async_load(
        &mut self,
        _: &Node,
        _: &vyre_foundation::ir::Ident,
        _: &vyre_foundation::ir::Ident,
        offset: &Expr,
        size: &Expr,
        _: &vyre_foundation::ir::Ident,
    ) -> ControlFlow<Self::Break> {
        visit_preorder(self, offset)?;
        visit_preorder(self, size)
    }

    fn visit_async_store(
        &mut self,
        _: &Node,
        _: &vyre_foundation::ir::Ident,
        _: &vyre_foundation::ir::Ident,
        offset: &Expr,
        size: &Expr,
        _: &vyre_foundation::ir::Ident,
    ) -> ControlFlow<Self::Break> {
        visit_preorder(self, offset)?;
        visit_preorder(self, size)
    }

    fn visit_async_wait(
        &mut self,
        _: &Node,
        _: &vyre_foundation::ir::Ident,
    ) -> ControlFlow<Self::Break> {
        Continue(())
    }

    fn visit_trap(
        &mut self,
        _: &Node,
        address: &Expr,
        _: &vyre_foundation::ir::Ident,
    ) -> ControlFlow<Self::Break> {
        visit_preorder(self, address)
    }

    fn visit_resume(
        &mut self,
        _: &Node,
        _: &vyre_foundation::ir::Ident,
    ) -> ControlFlow<Self::Break> {
        Continue(())
    }

    fn visit_return(&mut self, _: &Node) -> ControlFlow<Self::Break> {
        Continue(())
    }

    fn visit_barrier(&mut self, _: &Node) -> ControlFlow<Self::Break> {
        Continue(())
    }

    fn visit_block(&mut self, _node: &Node, _: &[Node]) -> ControlFlow<Self::Break> {
        Continue(())
    }

    fn visit_region(
        &mut self,
        _node: &Node,
        _: &vyre_foundation::ir::Ident,
        _: &Option<GeneratorRef>,
        _: &[Node],
    ) -> ControlFlow<Self::Break> {
        Continue(())
    }

    fn visit_opaque_node(
        &mut self,
        _: &Node,
        ext: &dyn vyre_foundation::ir::NodeExtension,
    ) -> ControlFlow<Self::Break> {
        match scan_registered_atomic_node(ext, self.out) {
            Ok(true) => return Continue(()),
            Ok(false) => {}
            Err(error) => return ControlFlow::Break(error),
        }
        ControlFlow::Break(LoweringError::invalid(format!(
            "unsupported opaque node `{}` in atomic scan. Fix: register NagaProgramScanAtomicNode for this extension before lowering to Naga.",
            ext.extension_kind()
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vyre_foundation::ir::{Expr, Ident, Node};

    fn write_targets(node: &Node) -> FxHashSet<Ident> {
        let mut atomic = FxHashSet::default();
        let mut writes = FxHashSet::default();
        scan_atomic_targets_into(node, &mut atomic, &mut writes)
            .expect("Fix: scan_atomic_targets_into must succeed for these IR shapes");
        writes
    }

    #[test]
    fn async_load_destination_is_a_write_target() {
        // Reproducer for the cat_a_gpu_differential vfs::resolve panic
        // (2026-05-02): without this, BufferAccess auto-inference
        // downgrades the AsyncLoad destination to ReadOnly, naga emits
        // `var<storage, read>`, and the in-shader Store synthesized by
        // emit_synchronous_async_load fails validation with
        // `InvalidStorePointer`.
        let node = Node::AsyncLoad {
            source: Ident::from("src_buf"),
            destination: Ident::from("dst_buf"),
            offset: Box::new(Expr::u32(0)),
            size: Box::new(Expr::u32(4096)),
            tag: Ident::from("dma_tag"),
        };
        let writes = write_targets(&node);
        assert!(
            writes.contains(&Ident::from("dst_buf")),
            "AsyncLoad.destination must be tracked as a write target so wgpu's BufferAccess auto-inference keeps it ReadWrite"
        );
        assert!(
            !writes.contains(&Ident::from("src_buf")),
            "AsyncLoad.source is read, not written; must not be tracked as a write target"
        );
    }

    #[test]
    fn async_store_destination_is_a_write_target() {
        let node = Node::AsyncStore {
            source: Ident::from("src_buf"),
            destination: Ident::from("dst_buf"),
            offset: Box::new(Expr::u32(0)),
            size: Box::new(Expr::u32(4096)),
            tag: Ident::from("dma_tag"),
        };
        let writes = write_targets(&node);
        assert!(writes.contains(&Ident::from("dst_buf")));
        assert!(!writes.contains(&Ident::from("src_buf")));
    }

    #[test]
    fn async_load_destination_inside_loop_is_tracked() {
        // The synthesized Node::Loop body in vfs::resolve nests AsyncLoad
        // through Region/If/Loop layers; the recursive walk must reach it.
        let node = Node::Region {
            generator: Ident::from("region"),
            source_region: None,
            body: std::sync::Arc::new(vec![Node::if_then(
                Expr::lt(Expr::InvocationId { axis: 0 }, Expr::u32(1)),
                vec![Node::AsyncLoad {
                    source: Ident::from("src_buf"),
                    destination: Ident::from("dst_buf"),
                    offset: Box::new(Expr::var("file_hash")),
                    size: Box::new(Expr::u32(4096)),
                    tag: Ident::from("vfs_req"),
                }],
            )]),
        };
        let writes = write_targets(&node);
        assert!(writes.contains(&Ident::from("dst_buf")));
    }
}
