use crate::error::Result;
use crate::ir_inner::model::expr::{Expr, GeneratorRef, Ident};
use crate::ir_inner::model::generated::Node;
use crate::ir_inner::model::node::NodeExtension;
use crate::visit::VisitOrder;
use smallvec::SmallVec;
use std::ops::ControlFlow;

/// Anything that can be lowered to a target representation.
///
/// Backends implement this trait for their target. The IR does not know
/// what targets exist  -  it only knows that calling `.lower(&mut ctx)`
/// walks the structure through the visitor contract.
///
/// # Errors
///
/// Backends report structured errors through their own context type.
pub trait Lowerable<Ctx: ?Sized> {
    /// Visit this IR structure and emit into the backend-specific context.
    ///
    /// # Errors
    ///
    /// Returns the backend context's structured error when lowering cannot
    /// represent this IR structure.
    fn lower(&self, ctx: &mut Ctx) -> Result<()>;
}

/// Anything that can be executed against a runtime environment.
///
/// The reference interpreter and each backend implement this trait. Two
/// `Evaluatable` implementations that produce the same output for the
/// same input + environment are certifiably equivalent under the
/// conform contract.
pub trait Evaluatable<Env: ?Sized> {
    /// The value type the evaluator produces (typically `Value` for the
    /// reference interpreter, a typed handle for GPU backends).
    type Value;

    /// Evaluate this IR structure against the environment.
    ///
    /// # Errors
    ///
    /// Returns the evaluator's structured error when the environment cannot
    /// execute this IR structure.
    fn evaluate(&self, env: &mut Env) -> Result<Self::Value>;
}

/// Visitor over [`Node`] trees.
///
/// Implementors must handle every core node variant explicitly. Like
/// [`crate::visit::ExprVisitor`], this trait is abstract-by-default so
/// adding a new node variant forces downstream code to make a conscious
/// decision.
///
/// Traversal order is explicit:
/// - [`visit_node_preorder`] visits the current node before nested nodes.
/// - [`visit_node_postorder`] visits nested nodes before the current node.
///
/// `NodeVisitor` traverses node structure only. If a visitor also needs
/// to recurse into node-owned expressions, it should pair this trait
/// with [`crate::visit::ExprVisitor`] and call the expression entry
/// points from the relevant node hooks.
pub trait NodeVisitor {
    /// Break payload returned when traversal short-circuits.
    type Break;

    /// Variable declaration.
    fn visit_let(&mut self, node: &Node, name: &Ident, value: &Expr) -> ControlFlow<Self::Break>;
    /// Variable assignment.
    fn visit_assign(&mut self, node: &Node, name: &Ident, value: &Expr)
        -> ControlFlow<Self::Break>;
    /// Buffer store.
    fn visit_store(
        &mut self,
        node: &Node,
        buffer: &Ident,
        index: &Expr,
        value: &Expr,
    ) -> ControlFlow<Self::Break>;
    /// Conditional branch.
    fn visit_if(
        &mut self,
        node: &Node,
        cond: &Expr,
        then_nodes: &[Node],
        otherwise: &[Node],
    ) -> ControlFlow<Self::Break>;
    /// Counted loop.
    fn visit_loop(
        &mut self,
        node: &Node,
        var: &Ident,
        from: &Expr,
        to: &Expr,
        body: &[Node],
    ) -> ControlFlow<Self::Break>;
    /// Indirect dispatch source.
    fn visit_indirect_dispatch(
        &mut self,
        node: &Node,
        count_buffer: &Ident,
        count_offset: u64,
    ) -> ControlFlow<Self::Break>;
    /// Async load node.
    fn visit_async_load(
        &mut self,
        node: &Node,
        source: &Ident,
        destination: &Ident,
        offset: &Expr,
        size: &Expr,
        tag: &Ident,
    ) -> ControlFlow<Self::Break>;
    /// Async store node.
    fn visit_async_store(
        &mut self,
        node: &Node,
        source: &Ident,
        destination: &Ident,
        offset: &Expr,
        size: &Expr,
        tag: &Ident,
    ) -> ControlFlow<Self::Break>;
    /// Async wait node.
    fn visit_async_wait(&mut self, node: &Node, tag: &Ident) -> ControlFlow<Self::Break>;
    /// Trap node.
    fn visit_trap(&mut self, node: &Node, address: &Expr, tag: &Ident) -> ControlFlow<Self::Break>;
    /// Resume node.
    fn visit_resume(&mut self, node: &Node, tag: &Ident) -> ControlFlow<Self::Break>;
    /// Return node.
    fn visit_return(&mut self, node: &Node) -> ControlFlow<Self::Break>;
    /// Barrier node.
    fn visit_barrier(&mut self, node: &Node) -> ControlFlow<Self::Break>;
    /// Distributed collective node.
    fn visit_collective(&mut self, node: &Node) -> ControlFlow<Self::Break> {
        let _ = node;
        ControlFlow::Continue(())
    }
    /// Block node.
    fn visit_block(&mut self, node: &Node, body: &[Node]) -> ControlFlow<Self::Break>;
    /// Region wrapper node.
    fn visit_region(
        &mut self,
        node: &Node,
        generator: &Ident,
        source_region: &Option<GeneratorRef>,
        body: &[Node],
    ) -> ControlFlow<Self::Break>;
    /// Downstream opaque node extension.
    fn visit_opaque_node(
        &mut self,
        node: &Node,
        extension: &dyn NodeExtension,
    ) -> ControlFlow<Self::Break>;

    /// Recursively walk this node's nested node children using the requested order.
    fn walk_children_default(&mut self, node: &Node, order: VisitOrder) -> ControlFlow<Self::Break>
    where
        Self: Sized,
    {
        walk_node_children_default(self, node, order)
    }
}

/// Visit a node tree in pre-order.
pub fn visit_node<V: NodeVisitor>(visitor: &mut V, node: &Node) -> ControlFlow<V::Break> {
    visit_node_preorder(visitor, node)
}

/// Visit a node tree in pre-order without recursive stack growth.
pub fn visit_node_preorder<V: NodeVisitor>(visitor: &mut V, node: &Node) -> ControlFlow<V::Break> {
    let mut stack = SmallVec::<[&Node; 32]>::new();
    stack.push(node);
    while let Some(current) = stack.pop() {
        dispatch_node(visitor, current)?;
        match current {
            Node::If {
                then, otherwise, ..
            } => {
                for n in otherwise.iter().rev() {
                    stack.push(n);
                }
                for n in then.iter().rev() {
                    stack.push(n);
                }
            }
            Node::Loop { body, .. } | Node::Block(body) => {
                for n in body.iter().rev() {
                    stack.push(n);
                }
            }
            Node::Region { body, .. } => {
                for n in body.iter().rev() {
                    stack.push(n);
                }
            }
            _ => {}
        }
    }
    ControlFlow::Continue(())
}

/// Visit a node tree in post-order without recursive stack growth.
pub fn visit_node_postorder<V: NodeVisitor>(visitor: &mut V, node: &Node) -> ControlFlow<V::Break> {
    enum Task<'a> {
        Visit(&'a Node),
        Dispatch(&'a Node),
    }
    let mut stack = SmallVec::<[Task<'_>; 32]>::new();
    stack.push(Task::Visit(node));
    while let Some(task) = stack.pop() {
        match task {
            Task::Visit(n) => {
                stack.push(Task::Dispatch(n));
                match n {
                    Node::If {
                        then, otherwise, ..
                    } => {
                        for child in otherwise.iter().rev() {
                            stack.push(Task::Visit(child));
                        }
                        for child in then.iter().rev() {
                            stack.push(Task::Visit(child));
                        }
                    }
                    Node::Loop { body, .. } | Node::Block(body) => {
                        for child in body.iter().rev() {
                            stack.push(Task::Visit(child));
                        }
                    }
                    Node::Region { body, .. } => {
                        for child in body.iter().rev() {
                            stack.push(Task::Visit(child));
                        }
                    }
                    _ => {}
                }
            }
            Task::Dispatch(n) => {
                dispatch_node(visitor, n)?;
            }
        }
    }
    ControlFlow::Continue(())
}

/// Walk only the nested node children of `node`, leaving the current node to the caller.
pub fn walk_node_children_default<V: NodeVisitor>(
    visitor: &mut V,
    node: &Node,
    order: VisitOrder,
) -> ControlFlow<V::Break> {
    match node {
        Node::If {
            then, otherwise, ..
        } => {
            for child in then {
                visit_node_with_order(visitor, child, order)?;
            }
            for child in otherwise {
                visit_node_with_order(visitor, child, order)?;
            }
        }
        Node::Loop { body, .. } | Node::Block(body) => {
            for child in body {
                visit_node_with_order(visitor, child, order)?;
            }
        }
        Node::Region { body, .. } => {
            for child in body.iter() {
                visit_node_with_order(visitor, child, order)?;
            }
        }
        _ => {}
    }
    ControlFlow::Continue(())
}

fn visit_node_with_order<V: NodeVisitor>(
    visitor: &mut V,
    node: &Node,
    order: VisitOrder,
) -> ControlFlow<V::Break> {
    match order {
        VisitOrder::Preorder => visit_node_preorder(visitor, node),
        VisitOrder::Postorder => visit_node_postorder(visitor, node),
    }
}

pub(crate) fn dispatch_node<V: NodeVisitor>(visitor: &mut V, node: &Node) -> ControlFlow<V::Break> {
    match node {
        Node::Let { name, value } => visitor.visit_let(node, name, value),
        Node::Assign { name, value } => visitor.visit_assign(node, name, value),
        Node::Store {
            buffer,
            index,
            value,
        } => visitor.visit_store(node, buffer, index, value),
        Node::If {
            cond,
            then,
            otherwise,
        } => visitor.visit_if(node, cond, then, otherwise),
        Node::Loop {
            var,
            from,
            to,
            body,
        } => visitor.visit_loop(node, var, from, to, body),
        Node::IndirectDispatch {
            count_buffer,
            count_offset,
        } => visitor.visit_indirect_dispatch(node, count_buffer, *count_offset),
        Node::AsyncLoad {
            source,
            destination,
            offset,
            size,
            tag,
        } => visitor.visit_async_load(node, source, destination, offset, size, tag),
        Node::AsyncStore {
            source,
            destination,
            offset,
            size,
            tag,
        } => visitor.visit_async_store(node, source, destination, offset, size, tag),
        Node::AsyncWait { tag } => visitor.visit_async_wait(node, tag),
        Node::Trap { address, tag } => visitor.visit_trap(node, address, tag),
        Node::Resume { tag } => visitor.visit_resume(node, tag),
        Node::AllReduce { .. }
        | Node::AllGather { .. }
        | Node::ReduceScatter { .. }
        | Node::Broadcast { .. } => visitor.visit_collective(node),
        Node::Return => visitor.visit_return(node),
        Node::Barrier { .. } => visitor.visit_barrier(node),
        Node::Block(body) => visitor.visit_block(node, body),
        Node::Region {
            generator,
            source_region,
            body,
        } => visitor.visit_region(node, generator, source_region, body),
        Node::Opaque(extension) => visitor.visit_opaque_node(node, extension.as_ref()),
    }
}
