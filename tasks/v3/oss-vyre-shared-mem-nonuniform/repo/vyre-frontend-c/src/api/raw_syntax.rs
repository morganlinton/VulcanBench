use std::cell::RefCell;
use std::sync::Arc;

use vyre::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};
use vyre::DispatchConfig;
use vyre_libs::parsing::c::lex::lexer::{
    c11_lexer_regular_sparse, c11_lexer_regular_sparse_no_directives_no_backscan,
};
use vyre_libs::parsing::core::ast::shunting::ast_shunting_yard_with_capacity;

use super::{CParseSummary, PreparedResidentSyntaxBytes, SyntaxParseSummary};

mod ast_windows;
mod common;
mod lexer_stage;
mod sparse_compact;
mod sparse_programs;
#[cfg(test)]
mod tests;

use ast_windows::dispatch_dense_ast_windows;
use common::read_u32_at;
use lexer_stage::{
    mark_raw_sparse_lexer_outputs, raw_sparse_lexer_readbacks, RAW_SPARSE_LEXER_ABI_BUFFERS,
};
use sparse_compact::compact_sparse_token_types_ordered_gpu;
use sparse_programs::{
    sparse_token_block_compact_program, sparse_token_block_totals_program,
    sparse_token_type_block_compact_program,
};

type SummaryAdapter = fn(String, CParseSummary, u32, u64) -> SyntaxParseSummary;

#[derive(Default)]
struct RawSyntaxScratch;

thread_local! {
    static RAW_SYNTAX_SCRATCH: RefCell<RawSyntaxScratch> =
        RefCell::new(RawSyntaxScratch::default());
}

pub(super) fn parse_regular_sparse_syntax_bytes_gpu(
    prepared: &PreparedResidentSyntaxBytes,
    backend: &dyn vyre::VyreBackend,
    backend_id: String,
    summarize: SummaryAdapter,
) -> Result<SyntaxParseSummary, String> {
    RAW_SYNTAX_SCRATCH.with(|scratch| {
        let mut scratch = scratch.try_borrow_mut().map_err(|_| {
            "raw sparse syntax scratch was re-entered on the same thread. Fix: call raw syntax parsing from a non-nested parser context or add explicit caller-owned scratch.".to_string()
        })?;
        parse_regular_sparse_syntax_bytes_gpu_with_scratch(
            prepared,
            backend,
            backend_id,
            summarize,
            &mut scratch,
        )
    })
}

fn parse_regular_sparse_syntax_bytes_gpu_with_scratch(
    prepared: &PreparedResidentSyntaxBytes,
    backend: &dyn vyre::VyreBackend,
    backend_id: String,
    summarize: SummaryAdapter,
    _scratch: &mut RawSyntaxScratch,
) -> Result<SyntaxParseSummary, String> {
    let trace = std::env::var_os("VYRE_STAGE_TRACE").is_some();
    let stage_start = std::time::Instant::now();
    let mut last_t = stage_start;
    let mut log = |label: &str| {
        if trace {
            let now = std::time::Instant::now();
            let stage = now.duration_since(last_t).as_micros();
            let total = now.duration_since(stage_start).as_micros();
            eprintln!("[stage-trace] +{stage}us (total {total}us): raw-syntax {label}");
            last_t = now;
        }
    };
    let quote_free = prepared.quote_free;
    let lex_prog = if quote_free {
        c11_lexer_regular_sparse_no_directives_no_backscan(
            "haystack",
            "out_tok_types",
            "out_tok_starts",
            "out_tok_lens",
            "out_counts",
            prepared.haystack_len,
        )
    } else {
        c11_lexer_regular_sparse(
            "haystack",
            "out_tok_types",
            "out_tok_starts",
            "out_tok_lens",
            "out_counts",
            prepared.haystack_len,
        )
    };
    let lex_prog = mark_raw_sparse_lexer_outputs(
        lex_prog,
        raw_sparse_lexer_readbacks(quote_free),
        RAW_SPARSE_LEXER_ABI_BUFFERS,
    );
    let mut config = DispatchConfig::default();
    config.label = Some("vyre-frontend-c raw-byte sparse syntax lexer".to_string());
    let lex_input: &[u8] = prepared.haystack.as_ref();
    let mut lex_outputs = crate::pipeline::dispatch_resident_stage_cached(
        backend,
        crate::pipeline::stage_pipeline_cache_key(
            "raw_sparse_syntax_lexer",
            &[u64::from(quote_free), u64::from(prepared.haystack_len)],
        ),
        || Ok(lex_prog),
        &[crate::pipeline::ResidentStageInput::Host(lex_input)],
        &config,
    )
    .map_err(|e| format!("raw-byte sparse syntax lexer dispatch failed: {e}"))?;
    log("dispatch sparse lexer");
    let required_lex_outputs = 1;
    if lex_outputs.len() < required_lex_outputs {
        let actual = lex_outputs.len();
        let _ = crate::pipeline::free_resident_blobs(backend, lex_outputs);
        return Err(format!(
            "raw-byte sparse syntax lexer returned {actual} resident outputs, expected at least {required_lex_outputs} buffer(s) for this source shape. Fix: backend must return the declared GPU lexer ABI resources.",
        ));
    }
    let sparse_types = lex_outputs.remove(0);
    let _ = crate::pipeline::free_resident_blobs(backend, lex_outputs);
    let (dense_types, counts) = compact_sparse_token_types_ordered_gpu(
        backend,
        sparse_types,
        prepared.haystack_len,
        &mut config,
    )?;
    log("compact sparse tokens");
    let token_count = read_u32_at(&counts, 0, "raw-byte dense token count")?;
    let (ast_bytes, ast_node_count) =
        dispatch_dense_ast_windows(backend, &dense_types, token_count, &mut config)?;
    log("dispatch AST windows");
    let summary = CParseSummary::syntax_only(
        prepared.source_bytes,
        token_count,
        ast_bytes,
        ast_node_count,
        0,
        0,
    );
    Ok(summarize(
        backend_id,
        summary,
        prepared.haystack_len,
        prepared.haystack.len() as u64,
    ))
}
