//! Oracle-differential regression: region_inline must not flatten a Region
//! whose top-level `let x` collides with a binding of `x` nested inside a
//! sibling (an If/Loop/Block arm).
//!
//! A `Node::Region` scopes its `Let` bindings. region_inline flattens small
//! Regions into the parent sequence, dropping that scope. Its collision
//! guard re-wraps a flattened Region in a `Node::Block` when one of its
//! top-level `Let` names also occurs as a top-level `Let` among the
//! siblings -- but it only counts TOP-LEVEL sibling lets. When a sibling
//! binds the same name inside a nested scope (e.g. an If arm), the flattened
//! `let x` leaks into the parent scope and stays live across that sibling,
//! so the nested `let x` rebinds an occupied slot: "duplicate local binding".

use std::sync::Arc;

use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr, Ident, Node, Program};
use vyre_foundation::optimizer::passes::cleanup::region_inline_engine;
use vyre_reference::value::Value;

/// `Region { let x = 1; out[0] = x }` followed by a sibling
/// `if (true) { let x = 2; out[1] = x }`. The Region's `x` is scoped, so the
/// nested `x` does not collide in the original -- but flattening the Region
/// leaks `x` into the parent scope where it overlaps the nested binding.
fn program_with_region_then_nested_binder() -> Program {
    let region = Node::Region {
        generator: Ident::from("stage"),
        source_region: None,
        body: Arc::new(vec![
            Node::let_bind("x", Expr::u32(1)),
            Node::store("out", Expr::u32(0), Expr::var("x")),
        ]),
    };
    let guarded = Node::If {
        cond: Expr::eq(Expr::u32(0), Expr::u32(0)),
        then: vec![
            Node::let_bind("x", Expr::u32(2)),
            Node::store("out", Expr::u32(0), Expr::var("x")),
        ],
        otherwise: vec![],
    };
    Program::wrapped(
        vec![BufferDecl::storage("out", 0, BufferAccess::ReadWrite, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![region, guarded],
    )
}

#[test]
fn region_inline_preserves_nested_binder_scope() {
    let program = program_with_region_then_nested_binder();
    let inputs = [Value::U32(0)];

    // The original program is well-scoped: the Region scopes its `x`, so the
    // nested `let x = 2` rebinds a freed slot. It must run cleanly.
    let original = vyre_reference::reference_eval(&program, &inputs)
        .expect("original program is well-scoped and must run");

    let inlined = region_inline_engine::run(program);

    // The inlined program must STILL run and produce the same result.
    // Pre-fix this FAILS: region_inline flattened the Region, leaking
    // `let x = 1` into the parent scope where it collides with the nested
    // `let x = 2` (duplicate local binding), so reference_eval errors.
    let transformed = vyre_reference::reference_eval(&inlined, &inputs).expect(
        "region_inline must not leak a Region-scoped `let x` into the parent \
         scope where it collides with a nested sibling binding of `x`",
    );

    assert_eq!(
        transformed, original,
        "region_inline must preserve observable semantics and scoping"
    );
}
