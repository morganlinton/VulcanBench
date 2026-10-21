//! Walk a prepared `Program` collecting unique [`TrapTag`] entries  -
//! one per distinct `Node::Trap` tag in source order. The substrate
//! handles every other Node variant trivially.

use std::ops::ControlFlow::{self, Continue};
use std::sync::Arc;

use rustc_hash::FxHashSet;

use vyre_foundation::ir::model::expr::GeneratorRef;
use vyre_foundation::ir::{Expr, Ident, Node};
use vyre_foundation::visit::NodeVisitor;

use super::types::TrapTag;

#[derive(Default)]
pub(super) struct TrapTagCollector {
    tags: Vec<TrapTag>,
    seen: FxHashSet<Ident>,
}

impl TrapTagCollector {
    pub(super) fn into_tags(self) -> Vec<TrapTag> {
        self.tags
    }
}

impl NodeVisitor for TrapTagCollector {
    type Break = ();

    fn visit_let(&mut self, _: &Node, _: &vyre_foundation::ir::Ident, _: &Expr) -> ControlFlow<()> {
        Continue(())
    }

    fn visit_assign(
        &mut self,
        _: &Node,
        _: &vyre_foundation::ir::Ident,
        _: &Expr,
    ) -> ControlFlow<()> {
        Continue(())
    }

    fn visit_store(
        &mut self,
        _: &Node,
        _: &vyre_foundation::ir::Ident,
        _: &Expr,
        _: &Expr,
    ) -> ControlFlow<()> {
        Continue(())
    }

    fn visit_if(&mut self, _: &Node, _: &Expr, _: &[Node], _: &[Node]) -> ControlFlow<()> {
        Continue(())
    }

    fn visit_loop(
        &mut self,
        _: &Node,
        _: &vyre_foundation::ir::Ident,
        _: &Expr,
        _: &Expr,
        _: &[Node],
    ) -> ControlFlow<()> {
        Continue(())
    }

    fn visit_indirect_dispatch(
        &mut self,
        _: &Node,
        _: &vyre_foundation::ir::Ident,
        _: u64,
    ) -> ControlFlow<()> {
        Continue(())
    }

    fn visit_async_load(
        &mut self,
        _: &Node,
        _: &vyre_foundation::ir::Ident,
        _: &vyre_foundation::ir::Ident,
        _: &Expr,
        _: &Expr,
        _: &vyre_foundation::ir::Ident,
    ) -> ControlFlow<()> {
        Continue(())
    }

    fn visit_async_store(
        &mut self,
        _: &Node,
        _: &vyre_foundation::ir::Ident,
        _: &vyre_foundation::ir::Ident,
        _: &Expr,
        _: &Expr,
        _: &vyre_foundation::ir::Ident,
    ) -> ControlFlow<()> {
        Continue(())
    }

    fn visit_async_wait(&mut self, _: &Node, _: &vyre_foundation::ir::Ident) -> ControlFlow<()> {
        Continue(())
    }

    fn visit_trap(
        &mut self,
        _: &Node,
        _: &Expr,
        tag: &vyre_foundation::ir::Ident,
    ) -> ControlFlow<()> {
        if self.seen.insert(Ident::from(tag)) {
            let code = u32::try_from(self.tags.len())
                .unwrap_or(u32::MAX)
                .saturating_add(1);
            self.tags.push(TrapTag {
                code,
                tag: Arc::from(tag.as_str()),
            });
        }
        Continue(())
    }

    fn visit_resume(&mut self, _: &Node, _: &vyre_foundation::ir::Ident) -> ControlFlow<()> {
        Continue(())
    }

    fn visit_return(&mut self, _: &Node) -> ControlFlow<()> {
        Continue(())
    }

    fn visit_barrier(&mut self, _: &Node) -> ControlFlow<()> {
        Continue(())
    }

    fn visit_block(&mut self, _: &Node, _: &[Node]) -> ControlFlow<()> {
        Continue(())
    }

    fn visit_region(
        &mut self,
        _: &Node,
        _: &vyre_foundation::ir::Ident,
        _: &Option<GeneratorRef>,
        _: &[Node],
    ) -> ControlFlow<()> {
        Continue(())
    }

    fn visit_opaque_node(
        &mut self,
        _: &Node,
        _: &dyn vyre_foundation::ir::NodeExtension,
    ) -> ControlFlow<()> {
        Continue(())
    }
}
