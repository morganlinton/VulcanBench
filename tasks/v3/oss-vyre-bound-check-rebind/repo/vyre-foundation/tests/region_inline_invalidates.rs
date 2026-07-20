//! P1.9 regression: `region_inline` must not preserve a cached
//! `validated=true` flag from the source Program. Downstream dispatch
//! paths that trust the flag would skip re-validation of the inlined
//! shape and could execute a broken IR silently.
//!
//! The current invariant is guaranteed by
//! `Program::with_rewritten_entry` setting `validated: AtomicBool::new(false)`
//!  -  this test locks that invariant in.

use vyre::ir::{BufferDecl, DataType, Expr, Node, Program};

/// Build a tiny Program with a small `Region` wrapper so `region_inline`
/// has something to flatten.
fn program_with_region() -> Program {
    let region = Node::Region {
        generator: "test".into(),
        source_region: None,
        body: std::sync::Arc::new(vec![Node::store("out", Expr::u32(0), Expr::u32(42))]),
    };
    Program::wrapped(
        vec![BufferDecl::read_write("out", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![region],
    )
}

#[test]
fn region_inline_clears_validated_flag() {
    let program = program_with_region();
    // Simulate an upstream pass having cached validated=true.
    let errors = vyre_foundation::validate::validate(&program);
    assert!(errors.is_empty(), "base program must validate");
    program.mark_structurally_validated();
    assert!(program.is_structurally_validated());

    // Run the pass.
    let optimized = vyre_foundation::optimizer::passes::cleanup::region_inline_engine::run(program);

    // The inlined Program MUST NOT carry the pre-inline validated=true.
    assert!(
        !optimized.is_structurally_validated(),
        "region_inline must reset Program.validated so dispatchers re-validate the flattened shape"
    );
}

#[test]
fn region_inline_of_already_flat_program_still_invalidates() {
    // If region_inline short-circuits when there's nothing to do, it
    // could accidentally preserve validated=true. This test asserts it
    // still returns a fresh Program with validated=false.
    let program = Program::wrapped(
        vec![BufferDecl::read_write("out", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![Node::store("out", Expr::u32(0), Expr::u32(1))],
    );
    program.mark_structurally_validated();
    let optimized = vyre_foundation::optimizer::passes::cleanup::region_inline_engine::run(program);
    assert!(
        !optimized.is_structurally_validated(),
        "region_inline must always reset validated; even the no-op path"
    );
}
