//! Private compatibility shim for the public GPU if-expression ABI.
//!
//! The stable binding, stack, and scan-limit constants live in the
//! sibling `gpu_if_expression_abi` module so tests, host callers, and
//! the kernel builder cannot drift.

pub use super::super::gpu_if_expression_abi::{
    BINDING_DIRECTIVE_KINDS, BINDING_DIRECTIVE_VALUES, BINDING_MACRO_NAMES_PACKED,
    BINDING_MACRO_OFFSETS, BINDING_MACRO_VALUES, BINDING_SOURCE, BINDING_TOK_LENS,
    BINDING_TOK_STARTS, MAX_IDENT_LEN, MAX_PAYLOAD_BYTES, OP_ID, STACK_DEPTH,
};
