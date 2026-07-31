//! CPU-only reference tests for Linux-grade C constructs not covered
//! by the existing GNU extension test suite.
#![cfg(feature = "c-parser")]
#![allow(deprecated)]
include!("__split/c_ast_linux_grade_gnu_and_c11_construct_coverage_chunk1.rs");
include!("__split/c_ast_linux_grade_gnu_and_c11_construct_coverage_chunk2.rs");
