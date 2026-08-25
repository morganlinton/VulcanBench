use super::*;
use crate::ir_inner::model::expr::{Expr, ExprNode, GeneratorRef, Ident};
use crate::ir_inner::model::generated::Node;
use crate::ir_inner::model::types::{AtomicOp, BinOp, DataType, UnOp};
use crate::MemoryOrdering;
use std::convert::Infallible;
use std::ops::ControlFlow::{self, Break, Continue};
use std::sync::Arc;

struct CountingExprVisitor {
    count: usize,
}

struct BreakOnFirstLitU32 {
    seen: Vec<u32>,
}

impl ExprVisitor for BreakOnFirstLitU32 {
    type Break = ();

    fn visit_lit_u32(&mut self, _: &Expr, value: u32) -> ControlFlow<Self::Break> {
        self.seen.push(value);
        Break(())
    }
}

impl ExprVisitor for CountingExprVisitor {
    type Break = Infallible;

    fn visit_lit_u32(&mut self, _: &Expr, _: u32) -> ControlFlow<Self::Break> {
        self.count += 1;
        Continue(())
    }
    fn visit_lit_i32(&mut self, _: &Expr, _: i32) -> ControlFlow<Self::Break> {
        self.count += 1;
        Continue(())
    }
    fn visit_lit_f32(&mut self, _: &Expr, _: f32) -> ControlFlow<Self::Break> {
        self.count += 1;
        Continue(())
    }
    fn visit_lit_bool(&mut self, _: &Expr, _: bool) -> ControlFlow<Self::Break> {
        self.count += 1;
        Continue(())
    }
    fn visit_var(&mut self, _: &Expr, _: &Ident) -> ControlFlow<Self::Break> {
        self.count += 1;
        Continue(())
    }
    fn visit_load(&mut self, _: &Expr, _: &Ident, _: &Expr) -> ControlFlow<Self::Break> {
        self.count += 1;
        Continue(())
    }
    fn visit_buf_len(&mut self, _: &Expr, _: &Ident) -> ControlFlow<Self::Break> {
        self.count += 1;
        Continue(())
    }
    fn visit_invocation_id(&mut self, _: &Expr, _: u32) -> ControlFlow<Self::Break> {
        self.count += 1;
        Continue(())
    }
    fn visit_workgroup_id(&mut self, _: &Expr, _: u32) -> ControlFlow<Self::Break> {
        self.count += 1;
        Continue(())
    }
    fn visit_local_id(&mut self, _: &Expr, _: u32) -> ControlFlow<Self::Break> {
        self.count += 1;
        Continue(())
    }
    fn visit_bin_op(
        &mut self,
        _expr: &Expr,
        _: &BinOp,
        _: &Expr,
        _: &Expr,
    ) -> ControlFlow<Self::Break> {
        self.count += 1;
        Continue(())
    }
    fn visit_un_op(&mut self, _expr: &Expr, _: &UnOp, _: &Expr) -> ControlFlow<Self::Break> {
        self.count += 1;
        Continue(())
    }
    fn visit_call(&mut self, _expr: &Expr, _: &str, _: &[Expr]) -> ControlFlow<Self::Break> {
        self.count += 1;
        Continue(())
    }
    fn visit_sequence(&mut self, _parts: &[Expr]) -> ControlFlow<Self::Break> {
        self.count += 1;
        Continue(())
    }
    fn visit_fma(
        &mut self,
        _expr: &Expr,
        _: &Expr,
        _: &Expr,
        _: &Expr,
    ) -> ControlFlow<Self::Break> {
        self.count += 1;
        Continue(())
    }
    fn visit_select(
        &mut self,
        _expr: &Expr,
        _: &Expr,
        _: &Expr,
        _: &Expr,
    ) -> ControlFlow<Self::Break> {
        self.count += 1;
        Continue(())
    }
    fn visit_cast(&mut self, _expr: &Expr, _: &DataType, _: &Expr) -> ControlFlow<Self::Break> {
        self.count += 1;
        Continue(())
    }
    fn visit_atomic(
        &mut self,
        _expr: &Expr,
        _: &AtomicOp,
        _: &Ident,
        _: &Expr,
        _: Option<&Expr>,
        _: &Expr,
    ) -> ControlFlow<Self::Break> {
        self.count += 1;
        Continue(())
    }
    fn visit_subgroup_ballot(&mut self, _expr: &Expr, _: &Expr) -> ControlFlow<Self::Break> {
        self.count += 1;
        Continue(())
    }
    fn visit_subgroup_shuffle(
        &mut self,
        _expr: &Expr,
        _: &Expr,
        _: &Expr,
    ) -> ControlFlow<Self::Break> {
        self.count += 1;
        Continue(())
    }
    fn visit_subgroup_add(&mut self, _expr: &Expr, _: &Expr) -> ControlFlow<Self::Break> {
        self.count += 1;
        Continue(())
    }
    fn visit_subgroup_local_id(&mut self, _: &Expr) -> ControlFlow<Self::Break> {
        self.count += 1;
        Continue(())
    }
    fn visit_subgroup_size(&mut self, _: &Expr) -> ControlFlow<Self::Break> {
        self.count += 1;
        Continue(())
    }
    fn visit_opaque_expr(&mut self, _: &Expr, _: &dyn ExprNode) -> ControlFlow<Self::Break> {
        self.count += 1;
        Continue(())
    }
}

mod break_defaults;
mod node_opaque;
mod postorder_short_circuit;
mod preorder;
