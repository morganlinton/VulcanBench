//! End-to-end GPU C11 compilation pipeline orchestration.
//!
//! This file wires stage modules and exposes public entry points. Stage duties live in one-purpose files under `pipeline/`.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use vyre::ir::{Expr, Program};
use vyre::{DispatchConfig, VyreBackend};

use vyre_libs::compiler::cfg::c11_build_cfg_and_gotos;
use vyre_libs::compiler::types_layout::c11_compute_alignments_for_abi;
use vyre_libs::parsing::c::lex::keyword::{
    c_keyword, c_keyword_map_words, c_keyword_packed_haystack, C_KEYWORDS,
};
use vyre_libs::parsing::c::lex::lexer::{
    c11_lex_regular_single_pass, c11_lex_single_pass, c11_lexer_regular_ranked,
    c11_lexer_regular_sparse,
};
use vyre_libs::parsing::c::lex::tokens::{
    TOK_ASSIGN, TOK_CASE, TOK_COLON, TOK_COMMA, TOK_DEFAULT, TOK_EOF, TOK_GNU_LABEL, TOK_GOTO,
    TOK_IDENTIFIER, TOK_LBRACE, TOK_LBRACKET, TOK_LPAREN, TOK_QUESTION, TOK_RBRACE, TOK_RBRACKET,
    TOK_RPAREN, TOK_SEMICOLON, TOK_SWITCH, TOK_TYPEDEF,
};
use vyre_libs::parsing::c::pipeline::stages::C11_AST_MAX_TOK_SCAN;
use vyre_libs::parsing::c::preprocess::expansion::opt_conditional_mask;
use vyre_libs::parsing::core::ast::shunting::ast_shunting_yard_with_capacity;

use crate::api::{CParseSummary, CTargetAbi, VyreCompileOptions};
use crate::object_format::SectionTag;

mod abi_stage;
mod backend_select;
mod bracket_pair_stage;
mod buffers;
mod compile_unit;
mod dispatch;
mod full_ast_stage;
mod keyword_dispatch;
mod lexer_dispatch;
mod lexer_fast_path;
mod lexer_outputs;
mod lexer_plan;
mod lexer_plan_select;
mod lexer_program_plan;
mod lexer_sparse_cuda;
mod object_output;
mod parse_cache;
mod parse_entry;
mod parse_memory_cache;
mod prefix_scan_dispatch;
mod sema;
mod semantic_fast_path;
mod semantic_features;
mod semantic_graph_stage;
mod semantic_haystack;
mod semantic_parse;
mod span_repair;
mod sparse_compaction;
mod sparse_lexer_megakernel;
mod sparse_prefix_programs;
mod stage_validation;
mod statement_bounds;
mod structure_records;
mod structure_stage;
mod syntax_ast_stage;
mod syntax_parse;
mod token_materialize;
mod translation_unit;
mod vast_pg;

pub use parse_entry::{
    parse_c11_source, parse_c11_translation_unit, parse_c11_translation_unit_bytes,
};
pub use syntax_parse::parse_c11_syntax_source;

pub(crate) use buffers::drop_suppressed_readbacks;
use buffers::{
    build_ast_owned_inputs_with_capacity_into, c_abi_type_table_bytes_into,
    cfg_ssa_words_from_vast, compiler_bytes_from_sections, cuda_lexer_haystack_view,
    megakernel_section_bytes, pack_haystack, pad_dispatch_input_refs, read_u32_at, read_u32_stream,
    reject_c11_lexer_diagnostics, reject_c11_source_diagnostics, token_types_from_lex,
    vec_u32_le_bytes, vec_u32_le_bytes_min_words, AstOwnedInputBuffers,
};
use dispatch::{dispatch_c11_bracket_pairs, try_dispatch_elf};
use lexer_plan::{cuda_sparse_lexer_strategy, CudaSparseLexerStrategy};
use sema::build_sema_scope;
use span_repair::repair_token_spans_from_source;
use structure_records::build_structure_records;
use vast_pg::build_vast_and_pg;

use abi_stage::build_c11_abi_stage;
pub(crate) use backend_select::{
    dispatch_borrowed_cached_into, dispatch_borrowed_stage_cached_into,
    dispatch_resident_stage_cached, dispatch_resident_stage_readback_cached_into,
    free_resident_blobs, shared_dispatch_backend, stage_pipeline_cache_key, ResidentBlob,
    ResidentStageInput,
};
use bracket_pair_stage::c11_dual_bracket_pairs_cost_model;
pub(crate) use buffers::{mark_program_outputs, suppress_readwrite_readback};
use compile_unit::compile_translation_unit;
use full_ast_stage::{build_c11_full_ast_stage, C11AstReadback};
use keyword_dispatch::promote_c11_keywords;
use lexer_dispatch::{lex_c11_tokens, C11LexTokens};
use lexer_outputs::{
    bucketed_dense_lex_haystack, expanded_haystack, keyword_map_bytes_cached,
    truncate_lexer_outputs_to_logical_tokens,
};
use lexer_plan_select::c11_lex_program_for_source;
use lexer_sparse_cuda::reject_sparse_dense_lexer_mismatch;
use object_output::{validate_object_output_path, write_object_atomic};
use semantic_fast_path::{
    c_global_typedef_fast_hashes, conditional_expression_shapes_required,
    semantic_control_edges_required,
};
use semantic_features::build_semantic_feature_inputs;
use semantic_graph_stage::build_c11_semantic_graphs;
use semantic_haystack::select_semantic_haystack;
use semantic_parse::parse_c11_source_with_backend;
use stage_validation::{require_full_semantic_summary, validate_internal_stage};
pub(crate) use statement_bounds::dispatch_c11_statement_bounds_bytes as dispatch_statement_bounds_bytes_for_api;
pub(crate) use statement_bounds::{
    dispatch_c11_statement_bounds_bytes_into, StatementBoundsScratch,
};
use structure_stage::{build_c11_structure_stage, C11StructureStage};
use syntax_ast_stage::build_c11_syntax_ast_stage;
use token_materialize::{decode_c11_tokens, DecodedC11Tokens};
use translation_unit::{
    prepare_translation_unit, prepare_translation_unit_from_bytes, read_translation_unit_bounded,
    PreparedTranslationUnit,
};

const MAX_TOK_SCAN: u32 = C11_AST_MAX_TOK_SCAN;
/// `ast_shunting_yard` workgroup uses one lane per statement (see `vyre-libs`).
const MAX_STMT_THREADS: u32 = 256;
const MAX_TRANSLATION_UNIT_BYTES: u64 = 256 * 1024 * 1024;
/// `opt_lower_elf` writes into a 4096-word object buffer with 64 words reserved
/// for ELF headers and 5 words for `.shstrtab` payload.
const ELF_LOWERING_MAX_INPUT_WORDS: usize = 4096 - 64 - 5;

/// Return the selected GPU dispatch backend identifier.
pub fn preferred_backend_id() -> Result<String, String> {
    shared_dispatch_backend().map(|backend| backend.id().to_string())
}

/// Compile C11 source files into object artifacts through the GPU frontend.
pub fn compile_c11_sources(options: &VyreCompileOptions) -> Result<(), String> {
    if options.output_file.is_some() && options.input_files.len() > 1 {
        return Err(
            "vyre-frontend-c: -o with multiple compile-only inputs has no single-output contract; compile one TU at a time or omit -o for per-input objects."
                .to_string(),
        );
    }

    let mut prepared = Vec::with_capacity(options.input_files.len());
    for path in &options.input_files {
        let dest: PathBuf = if options.input_files.len() == 1 {
            options
                .output_file
                .clone()
                .unwrap_or_else(|| path.with_extension("o"))
        } else {
            path.with_extension("o")
        };
        validate_object_output_path(path, &dest)?;
        prepared.push(prepare_translation_unit(path, dest, options)?);
    }

    let backend = shared_dispatch_backend()?;

    for unit in &prepared {
        compile_translation_unit(backend.as_ref(), unit, options.target.abi)?;
    }

    Ok(())
}

/// Link C11 object artifacts into an executable.
pub fn link_c11_executable(options: &VyreCompileOptions) -> Result<(), String> {
    if options.input_files.is_empty() {
        return Err(
            "vyre-frontend-c link mode received no input files. Fix: pass translation units explicitly, or run `vyrec -c` for the CUDA-first pre-lowering object-evidence path."
                .to_string(),
        );
    }
    Err(
        "vyre-frontend-c link mode is not part of the CUDA-first release path and does not spawn a host C linker. Fix: run `vyrec -c` to emit GPU-compiled VYRECOB2 object evidence, then use an explicit external linker step outside vyre."
            .to_string(),
    )
}
