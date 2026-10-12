use std::path::Path;

use vyre_libs::parsing::c::lex::diagnostics::C11LexerDiagnosticKind;
use vyre_libs::parsing::c::parse::vast::{C_AST_KIND_GOTO_STMT, C_AST_KIND_LABEL_STMT};
use vyre_runtime::megakernel::protocol;

use super::MAX_TOK_SCAN;

mod abi;
mod ast_inputs;
mod cfg_lowering;
mod compiler_sections;
mod diagnostics;
mod dispatch_inputs;
mod lexer_diagnostic_report;
mod lexer_diagnostics;
mod megakernel_section;
mod packing;
mod program_outputs;
mod source_diagnostics;

pub(super) use ast_inputs::{build_ast_owned_inputs_with_capacity_into, AstOwnedInputBuffers};
pub(super) use cfg_lowering::{c_abi_type_table_bytes_into, cfg_ssa_words_from_vast};
pub(super) use compiler_sections::compiler_bytes_from_sections;
pub(super) use dispatch_inputs::pad_dispatch_input_refs;
pub(super) use lexer_diagnostics::{reject_c11_lexer_diagnostics, token_types_from_lex};
pub(super) use megakernel_section::megakernel_section_bytes;
pub(super) use packing::{
    cuda_lexer_haystack_view, pack_haystack, read_u32_at, read_u32_stream, vec_u32_le_bytes,
    vec_u32_le_bytes_min_words,
};
pub(crate) use program_outputs::{
    drop_suppressed_readbacks, is_input_buffer, mark_program_outputs,
    mark_program_outputs_readback, suppress_readwrite_readback, take_exact_output,
    take_last_output_into,
};
pub(super) use source_diagnostics::reject_c11_source_diagnostics;
