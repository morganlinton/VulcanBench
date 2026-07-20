//! Integration test crate for the containing Vyre package.

use super::*;

#[test]
fn expr_preorder_visits_every_node_once() {
    let expr = Expr::add(
        Expr::u32(1),
        Expr::select(Expr::bool(true), Expr::u32(2), Expr::u32(3)),
    );
    let mut visitor = CountingExprVisitor { count: 0 };
    visit_preorder(&mut visitor, &expr);
    assert_eq!(visitor.count, 6);
}

#[test]
fn expr_preorder_handles_deep_trees_without_host_recursion() {
    let mut expr = Expr::u32(0);
    for _ in 0..4096 {
        expr = Expr::add(expr, Expr::u32(1));
    }
    let mut visitor = CountingExprVisitor { count: 0 };
    visit_preorder(&mut visitor, &expr);
    assert_eq!(visitor.count, 8193);
}
