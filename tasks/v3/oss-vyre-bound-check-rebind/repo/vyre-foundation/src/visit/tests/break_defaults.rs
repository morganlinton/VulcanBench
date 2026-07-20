//! Integration test crate for the containing Vyre package.

use super::*;

#[test]
fn preorder_breaks_at_first_literal_before_second() {
    // Adversarial: visit_preorder on a flat `LitU32 + LitU32` tree
    // must visit the root BinOp, then the left LitU32, then BREAK.
    // The right LitU32 must never be touched.
    let expr = Expr::add(Expr::u32(7), Expr::u32(9));
    let mut visitor = BreakOnFirstLitU32 { seen: Vec::new() };
    let result = visit_preorder(&mut visitor, &expr);
    assert_eq!(result, Break(()), "must short-circuit on first LitU32");
    assert_eq!(visitor.seen, vec![7], "must see ONLY the left literal");
}

#[test]
fn preorder_break_in_left_subtree_never_reaches_right() {
    // Adversarial: in a deeper tree `(1 + 2) + (3 + 4)`, preorder
    // visits the outer BinOp, then the left inner BinOp, then the
    // first LitU32 (1), then BREAK. The right subtree (3+4) must
    // never be visited.
    let expr = Expr::add(
        Expr::add(Expr::u32(1), Expr::u32(2)),
        Expr::add(Expr::u32(3), Expr::u32(4)),
    );
    let mut visitor = BreakOnFirstLitU32 { seen: Vec::new() };
    let result = visit_preorder(&mut visitor, &expr);
    assert_eq!(result, Break(()));
    assert_eq!(
        visitor.seen,
        vec![1],
        "must break in left subtree; right subtree unseen"
    );
}

struct DefaultVisitorTest {
    seen_select: bool,
}

impl ExprVisitor for DefaultVisitorTest {
    type Break = Infallible;
    fn visit_select(&mut self, _: &Expr, _: &Expr, _: &Expr, _: &Expr) -> ControlFlow<Self::Break> {
        self.seen_select = true;
        Continue(())
    }
}

#[test]
fn expr_visitor_default_trait_methods_are_noops() {
    // Adversarial: A tree with multiple types of nodes. Only `visit_select` is overridden.
    // Ensure that all other nodes (fma, add, u32) default to Continue(()) and visit_select fires.
    let expr = Expr::select(
        Expr::bool(true),
        Expr::add(Expr::u32(1), Expr::u32(2)),
        Expr::u32(3),
    );
    let mut visitor = DefaultVisitorTest { seen_select: false };
    visit_preorder(&mut visitor, &expr);
    assert!(
        visitor.seen_select,
        "the overridden method should be visited and other methods should default properly"
    );
}
