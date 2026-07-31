//! End-to-end C parser coverage for expression-shape gaps.

#![cfg(feature = "c-parser")]
mod fixtures {
    include!("c_ast_expression_shape_gaps_e2e/fixtures.rs");
}
#[allow(deprecated)]
mod gpu_parity {
    include!("c_ast_expression_shape_gaps_e2e/gpu_parity.rs");
}
mod kind_shape {
    include!("c_ast_expression_shape_gaps_e2e/kind_shape.rs");
}
#[allow(deprecated)]
mod support {
    include!("c_ast_expression_shape_gaps_e2e/support.rs");
}
