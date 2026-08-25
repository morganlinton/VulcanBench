//! GPU/CPU parity tests for difficult C declarator edge cases.
//! Implementation lives in `__split/` chunks.
#![cfg(feature = "c-parser")]
#![allow(deprecated)]
mod c_ast_declarator_edge_cases_suite {
    include!("__split/c_ast_declarator_edge_cases_support.rs");
    mod c_ast_declarator_edge_cases_part1 {
        include!("__split/c_ast_declarator_edge_cases_part1.rs");
    }
    mod c_ast_declarator_edge_cases_part2 {
        include!("__split/c_ast_declarator_edge_cases_part2.rs");
    }
}
