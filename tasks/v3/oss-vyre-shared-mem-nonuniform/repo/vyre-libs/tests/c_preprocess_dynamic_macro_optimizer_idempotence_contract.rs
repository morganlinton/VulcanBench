//! Optimizer idempotence contract for GPU dynamic macro expansion.

#![cfg(feature = "c-parser")]

use vyre::ir::Expr;
use vyre_foundation::optimizer::pre_lowering::optimize;
use vyre_libs::parsing::c::preprocess::expansion::opt_dynamic_macro_expansion;

#[test]
fn dynamic_macro_expansion_pre_lowering_optimizer_is_idempotent() {
    let program = opt_dynamic_macro_expansion(
        "in_tok_types",
        "macro_keys",
        "macro_vals",
        "macro_sizes",
        "out_tok_types",
        "out_tok_counts",
        Expr::u32(4),
        16,
    );

    let optimized_once = optimize(program);
    let optimized_twice = optimize(optimized_once.clone());
    assert_eq!(optimized_once, optimized_twice);
}
