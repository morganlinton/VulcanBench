//! Integration test crate for the containing Vyre package.

use super::*;

#[derive(Debug)]
struct TestOpaqueExpr;

impl ExprNode for TestOpaqueExpr {
    fn extension_kind(&self) -> &'static str {
        "test.opaque_expr"
    }

    fn debug_identity(&self) -> &str {
        "test"
    }

    fn result_type(&self) -> Option<DataType> {
        None
    }

    fn cse_safe(&self) -> bool {
        false
    }

    fn stable_fingerprint(&self) -> [u8; 32] {
        [7; 32]
    }

    fn validate_extension(&self) -> std::result::Result<(), String> {
        Ok(())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

struct CountingNodeVisitor {
    count: usize,
}

impl NodeVisitor for CountingNodeVisitor {
    type Break = Infallible;

    fn visit_let(&mut self, _: &Node, _: &Ident, _: &Expr) -> ControlFlow<Self::Break> {
        self.count += 1;
        Continue(())
    }
    fn visit_assign(&mut self, _: &Node, _: &Ident, _: &Expr) -> ControlFlow<Self::Break> {
        self.count += 1;
        Continue(())
    }
    fn visit_store(&mut self, _: &Node, _: &Ident, _: &Expr, _: &Expr) -> ControlFlow<Self::Break> {
        self.count += 1;
        Continue(())
    }
    fn visit_if(&mut self, _: &Node, _: &Expr, _: &[Node], _: &[Node]) -> ControlFlow<Self::Break> {
        self.count += 1;
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
        self.count += 1;
        Continue(())
    }
    fn visit_indirect_dispatch(&mut self, _: &Node, _: &Ident, _: u64) -> ControlFlow<Self::Break> {
        self.count += 1;
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
        self.count += 1;
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
        self.count += 1;
        Continue(())
    }
    fn visit_async_wait(&mut self, _: &Node, _: &Ident) -> ControlFlow<Self::Break> {
        self.count += 1;
        Continue(())
    }
    fn visit_trap(&mut self, _: &Node, _: &Expr, _: &Ident) -> ControlFlow<Self::Break> {
        self.count += 1;
        Continue(())
    }
    fn visit_resume(&mut self, _: &Node, _: &Ident) -> ControlFlow<Self::Break> {
        self.count += 1;
        Continue(())
    }
    fn visit_return(&mut self, _: &Node) -> ControlFlow<Self::Break> {
        self.count += 1;
        Continue(())
    }
    fn visit_barrier(&mut self, _: &Node) -> ControlFlow<Self::Break> {
        self.count += 1;
        Continue(())
    }
    fn visit_block(&mut self, _: &Node, _: &[Node]) -> ControlFlow<Self::Break> {
        self.count += 1;
        Continue(())
    }
    fn visit_region(
        &mut self,
        _: &Node,
        _: &Ident,
        _: &Option<GeneratorRef>,
        _: &[Node],
    ) -> ControlFlow<Self::Break> {
        self.count += 1;
        Continue(())
    }
    fn visit_opaque_node(
        &mut self,
        _: &Node,
        _: &dyn crate::ir_inner::model::node::NodeExtension,
    ) -> ControlFlow<Self::Break> {
        self.count += 1;
        Continue(())
    }
}

#[test]
fn node_preorder_visits_nested_nodes() {
    let node = Node::if_then(
        Expr::bool(true),
        vec![Node::loop_for(
            "i",
            Expr::u32(0),
            Expr::u32(2),
            vec![Node::return_()],
        )],
    );
    let mut visitor = CountingNodeVisitor { count: 0 };
    visit_node_preorder(&mut visitor, &node);
    assert_eq!(visitor.count, 3);
}

#[test]
fn node_visitors_use_inline_stack_for_shallow_trees_and_survive_deep_trees() {
    let mut node = Node::return_();
    for _ in 0..4096 {
        node = Node::Block(vec![node]);
    }
    let mut visitor = CountingNodeVisitor { count: 0 };

    visit_node_postorder(&mut visitor, &node);

    assert_eq!(visitor.count, 4097);
}

#[test]
fn expr_entry_point_handles_opaque_expr_explicitly() {
    let expr = Expr::Opaque(Arc::new(TestOpaqueExpr));
    let mut visitor = CountingExprVisitor { count: 0 };
    visit_expr(&mut visitor, &expr);
    assert_eq!(visitor.count, 1);
}

// ------------------------------------------------------------------
// Adversarial ControlFlow::Break tests for F-IR visitor exhaustiveness.
// ------------------------------------------------------------------
