//! Integration test crate for the containing Vyre package.

use super::*;

#[test]
fn expr_postorder_handles_deep_trees_without_host_recursion() {
    let mut expr = Expr::u32(0);
    for _ in 0..4096 {
        expr = Expr::add(expr, Expr::u32(1));
    }

    let mut visitor = CountingExprVisitor { count: 0 };
    visit_postorder(&mut visitor, &expr);
    assert_eq!(visitor.count, 8193);
}

struct OrderVisitor {
    seen: Vec<&'static str>,
}

impl OrderVisitor {
    fn unexpected(&mut self) -> ControlFlow<Infallible> {
        self.seen.push("unexpected");
        Continue(())
    }
}

impl ExprVisitor for OrderVisitor {
    type Break = Infallible;

    fn visit_lit_u32(&mut self, _: &Expr, _: u32) -> ControlFlow<Self::Break> {
        self.seen.push("lit");
        Continue(())
    }
    fn visit_lit_i32(&mut self, _: &Expr, _: i32) -> ControlFlow<Self::Break> {
        self.unexpected()
    }
    fn visit_lit_f32(&mut self, _: &Expr, _: f32) -> ControlFlow<Self::Break> {
        self.unexpected()
    }
    fn visit_lit_bool(&mut self, _: &Expr, _: bool) -> ControlFlow<Self::Break> {
        self.seen.push("bool");
        Continue(())
    }
    fn visit_var(&mut self, _: &Expr, _: &Ident) -> ControlFlow<Self::Break> {
        self.unexpected()
    }
    fn visit_load(&mut self, _: &Expr, _: &Ident, _: &Expr) -> ControlFlow<Self::Break> {
        self.unexpected()
    }
    fn visit_buf_len(&mut self, _: &Expr, _: &Ident) -> ControlFlow<Self::Break> {
        self.unexpected()
    }
    fn visit_invocation_id(&mut self, _: &Expr, _: u32) -> ControlFlow<Self::Break> {
        self.unexpected()
    }
    fn visit_workgroup_id(&mut self, _: &Expr, _: u32) -> ControlFlow<Self::Break> {
        self.unexpected()
    }
    fn visit_local_id(&mut self, _: &Expr, _: u32) -> ControlFlow<Self::Break> {
        self.unexpected()
    }
    fn visit_bin_op(
        &mut self,
        _: &Expr,
        _: &BinOp,
        _: &Expr,
        _: &Expr,
    ) -> ControlFlow<Self::Break> {
        self.seen.push("bin");
        Continue(())
    }
    fn visit_un_op(&mut self, _: &Expr, _: &UnOp, _: &Expr) -> ControlFlow<Self::Break> {
        self.unexpected()
    }
    fn visit_call(&mut self, _: &Expr, _: &str, _: &[Expr]) -> ControlFlow<Self::Break> {
        self.unexpected()
    }
    fn visit_sequence(&mut self, _: &[Expr]) -> ControlFlow<Self::Break> {
        self.unexpected()
    }
    fn visit_fma(&mut self, _: &Expr, _: &Expr, _: &Expr, _: &Expr) -> ControlFlow<Self::Break> {
        self.unexpected()
    }
    fn visit_select(&mut self, _: &Expr, _: &Expr, _: &Expr, _: &Expr) -> ControlFlow<Self::Break> {
        self.seen.push("select");
        Continue(())
    }
    fn visit_cast(&mut self, _: &Expr, _: &DataType, _: &Expr) -> ControlFlow<Self::Break> {
        self.unexpected()
    }
    fn visit_atomic(
        &mut self,
        _: &Expr,
        _: &AtomicOp,
        _: &Ident,
        _: &Expr,
        _: Option<&Expr>,
        _: &Expr,
    ) -> ControlFlow<Self::Break> {
        self.unexpected()
    }
    fn visit_subgroup_ballot(&mut self, _: &Expr, _: &Expr) -> ControlFlow<Self::Break> {
        self.unexpected()
    }
    fn visit_subgroup_shuffle(&mut self, _: &Expr, _: &Expr, _: &Expr) -> ControlFlow<Self::Break> {
        self.unexpected()
    }
    fn visit_subgroup_add(&mut self, _: &Expr, _: &Expr) -> ControlFlow<Self::Break> {
        self.unexpected()
    }
    fn visit_subgroup_local_id(&mut self, _: &Expr) -> ControlFlow<Self::Break> {
        self.unexpected()
    }
    fn visit_subgroup_size(&mut self, _: &Expr) -> ControlFlow<Self::Break> {
        self.unexpected()
    }
    fn visit_opaque_expr(&mut self, _: &Expr, _: &dyn ExprNode) -> ControlFlow<Self::Break> {
        self.unexpected()
    }
}

#[test]
fn expr_postorder_visits_children_before_parent() {
    let expr = Expr::select(
        Expr::bool(true),
        Expr::u32(1),
        Expr::add(Expr::u32(2), Expr::u32(3)),
    );
    let mut visitor = OrderVisitor { seen: Vec::new() };
    visit_postorder(&mut visitor, &expr);
    assert_eq!(
        visitor.seen,
        vec!["bool", "lit", "lit", "lit", "bin", "select"]
    );
}

struct FirstAtomicVisitor;

impl ExprVisitor for FirstAtomicVisitor {
    type Break = &'static str;

    fn visit_atomic(
        &mut self,
        _: &Expr,
        _: &AtomicOp,
        _: &Ident,
        _: &Expr,
        _: Option<&Expr>,
        _: &Expr,
    ) -> ControlFlow<Self::Break> {
        Break("atomic")
    }
}

#[test]
fn expr_visitor_can_short_circuit() {
    let expr = Expr::select(
        Expr::bool(true),
        Expr::Atomic {
            op: AtomicOp::Add,
            buffer: "out".into(),
            index: Box::new(Expr::u32(0)),
            expected: None,
            value: Box::new(Expr::u32(1)),
            ordering: MemoryOrdering::SeqCst,
        },
        Expr::u32(0),
    );
    assert_eq!(
        visit_preorder(&mut FirstAtomicVisitor, &expr),
        Break("atomic")
    );
}
