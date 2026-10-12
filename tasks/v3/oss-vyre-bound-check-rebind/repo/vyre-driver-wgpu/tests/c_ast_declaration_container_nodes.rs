//! Contract tests for C AST declaration container node classification.

#![cfg(feature = "c-parser")]
#![allow(deprecated)]
mod cpu {
    include!("c_ast_declaration_container_nodes/cpu.rs");
}
mod fixtures {
    include!("c_ast_declaration_container_nodes/fixtures.rs");
}
mod gpu {
    include!("c_ast_declaration_container_nodes/gpu.rs");
}
mod support {
    include!("c_ast_declaration_container_nodes/support.rs");
}
