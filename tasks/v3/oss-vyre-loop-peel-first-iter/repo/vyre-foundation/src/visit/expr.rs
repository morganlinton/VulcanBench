use crate::ir_inner::model::expr::{Expr, ExprNode, Ident};
use crate::ir_inner::model::types::{AtomicOp, BinOp, DataType, UnOp};
use crate::visit::VisitOrder;
use smallvec::SmallVec;
use std::ops::ControlFlow;

/// Visitor over [`Expr`] trees.
///
/// Implementors must handle every core variant explicitly. This is
/// intentional: `Expr` is `#[non_exhaustive]`, so a new variant must
/// become a compile error in every visitor instead of silently
/// disappearing behind a default body.
///
/// Traversal order is explicit:
/// - [`visit_preorder`] visits the current expression before its children.
/// - [`visit_postorder`] visits children before the current expression.
///
/// Visitors that want pass-through recursion can call
/// [`ExprVisitor::walk_children_default`] from a variant method.
pub trait ExprVisitor {
    /// Break payload returned when traversal short-circuits.
    type Break;

    /// Integer literal (`u32`).
    fn visit_lit_u32(&mut self, _expr: &Expr, _value: u32) -> ControlFlow<Self::Break> {
        ControlFlow::Continue(())
    }
    /// Integer literal (`i32`).
    fn visit_lit_i32(&mut self, _expr: &Expr, _value: i32) -> ControlFlow<Self::Break> {
        ControlFlow::Continue(())
    }
    /// Float literal (`f32`).
    fn visit_lit_f32(&mut self, _expr: &Expr, _value: f32) -> ControlFlow<Self::Break> {
        ControlFlow::Continue(())
    }
    /// Bool literal.
    fn visit_lit_bool(&mut self, _expr: &Expr, _value: bool) -> ControlFlow<Self::Break> {
        ControlFlow::Continue(())
    }
    /// Variable reference.
    fn visit_var(&mut self, _expr: &Expr, _name: &Ident) -> ControlFlow<Self::Break> {
        ControlFlow::Continue(())
    }
    /// Buffer load (`buffer[index]`).
    fn visit_load(
        &mut self,
        _expr: &Expr,
        _buffer: &Ident,
        _index: &Expr,
    ) -> ControlFlow<Self::Break> {
        ControlFlow::Continue(())
    }
    /// Buffer length.
    fn visit_buf_len(&mut self, _expr: &Expr, _buffer: &Ident) -> ControlFlow<Self::Break> {
        ControlFlow::Continue(())
    }
    /// Invocation id axis (`gid.{x,y,z}`).
    fn visit_invocation_id(&mut self, _expr: &Expr, _axis: u32) -> ControlFlow<Self::Break> {
        ControlFlow::Continue(())
    }
    /// Workgroup id axis.
    fn visit_workgroup_id(&mut self, _expr: &Expr, _axis: u32) -> ControlFlow<Self::Break> {
        ControlFlow::Continue(())
    }
    /// Local id axis within the workgroup.
    fn visit_local_id(&mut self, _expr: &Expr, _axis: u32) -> ControlFlow<Self::Break> {
        ControlFlow::Continue(())
    }
    /// Subgroup invocation id (lane index within subgroup).
    fn visit_subgroup_local_id(&mut self, _expr: &Expr) -> ControlFlow<Self::Break> {
        ControlFlow::Continue(())
    }
    /// Subgroup size.
    fn visit_subgroup_size(&mut self, _expr: &Expr) -> ControlFlow<Self::Break> {
        ControlFlow::Continue(())
    }
    /// Binary operation.
    fn visit_bin_op(
        &mut self,
        _expr: &Expr,
        _op: &BinOp,
        _left: &Expr,
        _right: &Expr,
    ) -> ControlFlow<Self::Break> {
        ControlFlow::Continue(())
    }
    /// Unary operation.
    fn visit_un_op(
        &mut self,
        _expr: &Expr,
        _op: &UnOp,
        _operand: &Expr,
    ) -> ControlFlow<Self::Break> {
        ControlFlow::Continue(())
    }
    /// Function call.
    fn visit_call(
        &mut self,
        _expr: &Expr,
        _op_id: &str,
        _args: &[Expr],
    ) -> ControlFlow<Self::Break> {
        ControlFlow::Continue(())
    }
    /// Sequence-valued extension hook.
    ///
    /// Core IR does not currently emit a dedicated `Expr::Sequence`
    /// variant, but downstream visitor implementations must still opt in
    /// explicitly so a sequence node cannot compile behind a silent
    /// default body.
    fn visit_sequence(&mut self, _parts: &[Expr]) -> ControlFlow<Self::Break> {
        ControlFlow::Continue(())
    }
    /// Fused multiply-add (`a * b + c`).
    fn visit_fma(
        &mut self,
        _expr: &Expr,
        _a: &Expr,
        _b: &Expr,
        _c: &Expr,
    ) -> ControlFlow<Self::Break> {
        ControlFlow::Continue(())
    }
    /// Ternary `select(cond, true_val, false_val)`.
    fn visit_select(
        &mut self,
        _expr: &Expr,
        _cond: &Expr,
        _true_val: &Expr,
        _false_val: &Expr,
    ) -> ControlFlow<Self::Break> {
        ControlFlow::Continue(())
    }
    /// Numeric cast.
    fn visit_cast(
        &mut self,
        _expr: &Expr,
        _target: &DataType,
        _value: &Expr,
    ) -> ControlFlow<Self::Break> {
        ControlFlow::Continue(())
    }
    /// Atomic operation on a shared buffer.
    fn visit_atomic(
        &mut self,
        _expr: &Expr,
        _op: &AtomicOp,
        _buffer: &Ident,
        _index: &Expr,
        _expected: Option<&Expr>,
        _value: &Expr,
    ) -> ControlFlow<Self::Break> {
        ControlFlow::Continue(())
    }
    /// Subgroup ballot.
    fn visit_subgroup_ballot(&mut self, _expr: &Expr, _cond: &Expr) -> ControlFlow<Self::Break> {
        ControlFlow::Continue(())
    }
    /// Subgroup shuffle.
    fn visit_subgroup_shuffle(
        &mut self,
        _expr: &Expr,
        _value: &Expr,
        _lane: &Expr,
    ) -> ControlFlow<Self::Break> {
        ControlFlow::Continue(())
    }
    /// Subgroup add.
    fn visit_subgroup_add(&mut self, _expr: &Expr, _value: &Expr) -> ControlFlow<Self::Break> {
        ControlFlow::Continue(())
    }
    /// Downstream opaque expression extension.
    fn visit_opaque_expr(
        &mut self,
        _expr: &Expr,
        _extension: &dyn ExprNode,
    ) -> ControlFlow<Self::Break> {
        ControlFlow::Continue(())
    }

    /// Recursively walk this expression's children using the requested order.
    fn walk_children_default(&mut self, expr: &Expr, order: VisitOrder) -> ControlFlow<Self::Break>
    where
        Self: Sized,
    {
        walk_expr_children_default(self, expr, order)
    }
}

/// Visit an expression tree in pre-order.
///
/// This is the historical default entry point for expression traversal.
pub fn visit_expr<V: ExprVisitor>(visitor: &mut V, expr: &Expr) -> ControlFlow<V::Break> {
    visit_preorder(visitor, expr)
}

/// Visit an expression tree in pre-order.
pub fn visit_preorder<V: ExprVisitor>(visitor: &mut V, expr: &Expr) -> ControlFlow<V::Break> {
    let mut stack = SmallVec::<[&Expr; 32]>::new();
    stack.push(expr);
    while let Some(current) = stack.pop() {
        dispatch_expr(visitor, current)?;
        push_expr_children_reverse(&mut stack, current);
    }
    ControlFlow::Continue(())
}

/// Visit an expression tree in post-order.
pub fn visit_postorder<V: ExprVisitor>(visitor: &mut V, expr: &Expr) -> ControlFlow<V::Break> {
    let mut stack = SmallVec::<[ExprVisitTask<'_>; 32]>::new();
    stack.push(ExprVisitTask::Visit(expr));
    while let Some(task) = stack.pop() {
        match task {
            ExprVisitTask::Visit(current) => {
                stack.push(ExprVisitTask::Dispatch(current));
                push_expr_child_tasks_reverse(&mut stack, current);
            }
            ExprVisitTask::Dispatch(current) => dispatch_expr(visitor, current)?,
        }
    }
    ControlFlow::Continue(())
}

/// Walk only the children of `expr`, leaving the current node to the caller.
pub fn walk_expr_children_default<V: ExprVisitor>(
    visitor: &mut V,
    expr: &Expr,
    order: VisitOrder,
) -> ControlFlow<V::Break> {
    match expr {
        Expr::LitU32(_)
        | Expr::LitI32(_)
        | Expr::LitF32(_)
        | Expr::LitBool(_)
        | Expr::Var(_)
        | Expr::BufLen { .. }
        | Expr::InvocationId { .. }
        | Expr::WorkgroupId { .. }
        | Expr::LocalId { .. }
        | Expr::SubgroupLocalId
        | Expr::SubgroupSize
        | Expr::Opaque(_) => ControlFlow::Continue(()),
        Expr::Load { index, .. } | Expr::UnOp { operand: index, .. } => {
            visit_with_order(visitor, index, order)
        }
        Expr::BinOp { left, right, .. } => {
            visit_with_order(visitor, left, order)?;
            visit_with_order(visitor, right, order)
        }
        Expr::Call { args, .. } => {
            for arg in args {
                visit_with_order(visitor, arg, order)?;
            }
            ControlFlow::Continue(())
        }
        Expr::Select {
            cond,
            true_val,
            false_val,
        } => {
            visit_with_order(visitor, cond, order)?;
            visit_with_order(visitor, true_val, order)?;
            visit_with_order(visitor, false_val, order)
        }
        Expr::Cast { value, .. }
        | Expr::SubgroupBallot { cond: value }
        | Expr::SubgroupReduce { value, .. } => visit_with_order(visitor, value, order),
        Expr::Fma { a, b, c } => {
            visit_with_order(visitor, a, order)?;
            visit_with_order(visitor, b, order)?;
            visit_with_order(visitor, c, order)
        }
        Expr::Atomic {
            index,
            expected,
            value,
            ..
        } => {
            visit_with_order(visitor, index, order)?;
            if let Some(expected) = expected.as_deref() {
                visit_with_order(visitor, expected, order)?;
            }
            visit_with_order(visitor, value, order)
        }
        Expr::SubgroupShuffle { value, lane } => {
            visit_with_order(visitor, value, order)?;
            visit_with_order(visitor, lane, order)
        }
    }
}

fn visit_with_order<V: ExprVisitor>(
    visitor: &mut V,
    expr: &Expr,
    order: VisitOrder,
) -> ControlFlow<V::Break> {
    match order {
        VisitOrder::Preorder => visit_preorder(visitor, expr),
        VisitOrder::Postorder => visit_postorder(visitor, expr),
    }
}

fn push_expr_children_reverse<'a>(stack: &mut SmallVec<[&'a Expr; 32]>, expr: &'a Expr) {
    match expr {
        Expr::LitU32(_)
        | Expr::LitI32(_)
        | Expr::LitF32(_)
        | Expr::LitBool(_)
        | Expr::Var(_)
        | Expr::BufLen { .. }
        | Expr::InvocationId { .. }
        | Expr::WorkgroupId { .. }
        | Expr::LocalId { .. }
        | Expr::SubgroupLocalId
        | Expr::SubgroupSize
        | Expr::Opaque(_) => {}
        Expr::Load { index, .. }
        | Expr::UnOp { operand: index, .. }
        | Expr::Cast { value: index, .. }
        | Expr::SubgroupBallot { cond: index }
        | Expr::SubgroupReduce { value: index, .. } => stack.push(index),
        Expr::BinOp { left, right, .. } => {
            stack.push(right);
            stack.push(left);
        }
        Expr::Call { args, .. } => {
            for arg in args.iter().rev() {
                stack.push(arg);
            }
        }
        Expr::Fma { a, b, c } => {
            stack.push(c);
            stack.push(b);
            stack.push(a);
        }
        Expr::Select {
            cond,
            true_val,
            false_val,
        } => {
            stack.push(false_val);
            stack.push(true_val);
            stack.push(cond);
        }
        Expr::Atomic {
            index,
            expected,
            value,
            ..
        } => {
            stack.push(value);
            if let Some(expected) = expected.as_deref() {
                stack.push(expected);
            }
            stack.push(index);
        }
        Expr::SubgroupShuffle { value, lane } => {
            stack.push(lane);
            stack.push(value);
        }
    }
}

fn push_expr_child_tasks_reverse<'a>(
    stack: &mut SmallVec<[ExprVisitTask<'a>; 32]>,
    expr: &'a Expr,
) {
    match expr {
        Expr::LitU32(_)
        | Expr::LitI32(_)
        | Expr::LitF32(_)
        | Expr::LitBool(_)
        | Expr::Var(_)
        | Expr::BufLen { .. }
        | Expr::InvocationId { .. }
        | Expr::WorkgroupId { .. }
        | Expr::LocalId { .. }
        | Expr::SubgroupLocalId
        | Expr::SubgroupSize
        | Expr::Opaque(_) => {}
        Expr::Load { index, .. }
        | Expr::UnOp { operand: index, .. }
        | Expr::Cast { value: index, .. }
        | Expr::SubgroupBallot { cond: index }
        | Expr::SubgroupReduce { value: index, .. } => stack.push(ExprVisitTask::Visit(index)),
        Expr::BinOp { left, right, .. } => {
            stack.push(ExprVisitTask::Visit(right));
            stack.push(ExprVisitTask::Visit(left));
        }
        Expr::Call { args, .. } => {
            for arg in args.iter().rev() {
                stack.push(ExprVisitTask::Visit(arg));
            }
        }
        Expr::Fma { a, b, c } => {
            stack.push(ExprVisitTask::Visit(c));
            stack.push(ExprVisitTask::Visit(b));
            stack.push(ExprVisitTask::Visit(a));
        }
        Expr::Select {
            cond,
            true_val,
            false_val,
        } => {
            stack.push(ExprVisitTask::Visit(false_val));
            stack.push(ExprVisitTask::Visit(true_val));
            stack.push(ExprVisitTask::Visit(cond));
        }
        Expr::Atomic {
            index,
            expected,
            value,
            ..
        } => {
            stack.push(ExprVisitTask::Visit(value));
            if let Some(expected) = expected.as_deref() {
                stack.push(ExprVisitTask::Visit(expected));
            }
            stack.push(ExprVisitTask::Visit(index));
        }
        Expr::SubgroupShuffle { value, lane } => {
            stack.push(ExprVisitTask::Visit(lane));
            stack.push(ExprVisitTask::Visit(value));
        }
    }
}

enum ExprVisitTask<'a> {
    Visit(&'a Expr),
    Dispatch(&'a Expr),
}

fn dispatch_expr<V: ExprVisitor>(visitor: &mut V, expr: &Expr) -> ControlFlow<V::Break> {
    match expr {
        Expr::LitU32(value) => visitor.visit_lit_u32(expr, *value),
        Expr::LitI32(value) => visitor.visit_lit_i32(expr, *value),
        Expr::LitF32(value) => visitor.visit_lit_f32(expr, *value),
        Expr::LitBool(value) => visitor.visit_lit_bool(expr, *value),
        Expr::Var(name) => visitor.visit_var(expr, name),
        Expr::Load { buffer, index } => visitor.visit_load(expr, buffer, index),
        Expr::BufLen { buffer } => visitor.visit_buf_len(expr, buffer),
        Expr::InvocationId { axis } => visitor.visit_invocation_id(expr, (*axis).into()),
        Expr::WorkgroupId { axis } => visitor.visit_workgroup_id(expr, (*axis).into()),
        Expr::LocalId { axis } => visitor.visit_local_id(expr, (*axis).into()),
        Expr::BinOp { op, left, right } => visitor.visit_bin_op(expr, op, left, right),
        Expr::UnOp { op, operand } => visitor.visit_un_op(expr, op, operand),
        Expr::Call { op_id, args } => visitor.visit_call(expr, op_id, args),
        Expr::Fma { a, b, c } => visitor.visit_fma(expr, a, b, c),
        Expr::Select {
            cond,
            true_val,
            false_val,
        } => visitor.visit_select(expr, cond, true_val, false_val),
        Expr::Cast { target, value } => visitor.visit_cast(expr, target, value),
        Expr::Atomic {
            op,
            buffer,
            index,
            expected,
            value,
            ordering: _,
        } => visitor.visit_atomic(expr, op, buffer, index, expected.as_deref(), value),
        Expr::SubgroupBallot { cond } => visitor.visit_subgroup_ballot(expr, cond),
        Expr::SubgroupShuffle { value, lane } => visitor.visit_subgroup_shuffle(expr, value, lane),
        Expr::SubgroupReduce { value, .. } => visitor.visit_subgroup_add(expr, value),
        Expr::SubgroupLocalId => visitor.visit_subgroup_local_id(expr),
        Expr::SubgroupSize => visitor.visit_subgroup_size(expr),
        Expr::Opaque(extension) => visitor.visit_opaque_expr(expr, extension.as_ref()),
    }
}
