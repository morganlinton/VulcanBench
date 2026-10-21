//! Indirect-dispatch discovery over backend-neutral IR.

use std::ops::ControlFlow::{self, Continue};

use vyre_foundation::ir::model::expr::GeneratorRef;
use vyre_foundation::ir::{Expr, Ident, Node, Program};
use vyre_foundation::visit::{visit_node_preorder, NodeVisitor};

use crate::backend::BackendError;

/// Command-buffer indirect dispatch source.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IndirectDispatch {
    /// Buffer containing the indirect x/y/z workgroup tuple.
    pub count_buffer: Ident,
    /// Byte offset of the tuple in the buffer.
    pub count_offset: u64,
}

/// Locates the single [`Node::IndirectDispatch`] in a program, if any.
///
/// # Errors
///
/// Returns when the program is inconsistent (e.g. multiple indirect
/// sources, or a misaligned offset).
pub fn find_indirect_dispatch(program: &Program) -> Result<Option<IndirectDispatch>, BackendError> {
    if !program.has_indirect_dispatch() {
        return Ok(None);
    }
    let mut found = None;
    let mut collector = IndirectDispatchCollector { found: &mut found };
    for node in program.entry() {
        if let ControlFlow::Break(err) = visit_node_preorder(&mut collector, node) {
            return Err(err);
        }
    }
    Ok(found)
}

struct IndirectDispatchCollector<'a> {
    found: &'a mut Option<IndirectDispatch>,
}

impl NodeVisitor for IndirectDispatchCollector<'_> {
    type Break = BackendError;

    fn visit_let(&mut self, _: &Node, _: &Ident, _: &Expr) -> ControlFlow<Self::Break> {
        Continue(())
    }

    fn visit_assign(&mut self, _: &Node, _: &Ident, _: &Expr) -> ControlFlow<Self::Break> {
        Continue(())
    }

    fn visit_store(&mut self, _: &Node, _: &Ident, _: &Expr, _: &Expr) -> ControlFlow<Self::Break> {
        Continue(())
    }

    fn visit_if(&mut self, _: &Node, _: &Expr, _: &[Node], _: &[Node]) -> ControlFlow<Self::Break> {
        Continue(())
    }

    fn visit_loop(
        &mut self,
        _: &Node,
        _: &Ident,
        _: &Expr,
        _: &Expr,
        _: &[Node],
    ) -> ControlFlow<Self::Break> {
        Continue(())
    }

    fn visit_indirect_dispatch(
        &mut self,
        _: &Node,
        count_buffer: &Ident,
        count_offset: u64,
    ) -> ControlFlow<Self::Break> {
        if count_offset % 4 != 0 {
            return ControlFlow::Break(BackendError::new(format!(
                "indirect dispatch offset {count_offset} is not 4-byte aligned. Fix: use a u32-aligned dispatch tuple."
            )));
        }
        let next = IndirectDispatch {
            count_buffer: count_buffer.clone(),
            count_offset,
        };
        if self.found.replace(next).is_some() {
            return ControlFlow::Break(BackendError::new(
                "program declares more than one indirect dispatch source. Fix: keep exactly one Node::IndirectDispatch per Program.",
            ));
        }
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
    ) -> ControlFlow<Self::Break> {
        Continue(())
    }

    fn visit_async_store(
        &mut self,
        _: &Node,
        _: &Ident,
        _: &Ident,
        _: &Expr,
        _: &Expr,
        _: &Ident,
    ) -> ControlFlow<Self::Break> {
        Continue(())
    }

    fn visit_async_wait(&mut self, _: &Node, _: &Ident) -> ControlFlow<Self::Break> {
        Continue(())
    }

    fn visit_trap(&mut self, _: &Node, _: &Expr, _: &Ident) -> ControlFlow<Self::Break> {
        Continue(())
    }

    fn visit_resume(&mut self, _: &Node, _: &Ident) -> ControlFlow<Self::Break> {
        Continue(())
    }

    fn visit_return(&mut self, _: &Node) -> ControlFlow<Self::Break> {
        Continue(())
    }

    fn visit_barrier(&mut self, _: &Node) -> ControlFlow<Self::Break> {
        Continue(())
    }

    fn visit_block(&mut self, _: &Node, _: &[Node]) -> ControlFlow<Self::Break> {
        Continue(())
    }

    fn visit_region(
        &mut self,
        _: &Node,
        _: &Ident,
        _: &Option<GeneratorRef>,
        _: &[Node],
    ) -> ControlFlow<Self::Break> {
        Continue(())
    }

    fn visit_opaque_node(
        &mut self,
        _: &Node,
        _: &dyn vyre_foundation::ir::NodeExtension,
    ) -> ControlFlow<Self::Break> {
        Continue(())
    }
}
