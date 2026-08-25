#![allow(clippy::too_many_arguments, clippy::type_complexity)]

use std::cell::RefCell;
use std::mem;
use std::path::Path;

use vyre::ir::Expr;
use vyre::{DispatchConfig, VyreBackend};

use vyre_libs::parsing::c::lex::tokens::TOK_TYPEDEF;
use vyre_libs::parsing::c::lower::ast_to_pg_nodes::{
    c_lower_ast_to_pg_semantic_graph_with_pg,
    c_lower_ast_to_pg_semantic_graph_with_pg_no_control_resolution,
};
use vyre_libs::parsing::c::parse::vast::{
    c11_annotate_global_typedef_names_fast, c11_annotate_typedef_names_precomputed_context,
    c11_annotate_typedef_names_precomputed_context_packed_haystack,
    c11_build_expression_shape_nodes, c11_build_expression_shape_nodes_no_conditional,
    c11_build_vast_nodes, c11_build_vast_nodes_uses_global_last_child,
    c11_classify_annotated_vast_node_kinds_precomputed_context, c11_precompute_vast_decl_contexts,
    c11_precompute_vast_decl_prefix_starts, c11_precompute_vast_scopes,
    c11_precompute_vast_scopes_uses_global_stack, c11_prehash_vast_identifiers,
    c11_prehash_vast_identifiers_packed_haystack, C_EXPR_SHAPE_STRIDE_U32,
};

use super::{
    buffers, dispatch_borrowed_cached_into, dispatch_borrowed_stage_cached_into,
    stage_pipeline_cache_key, validate_internal_stage,
};

mod contracts;
mod decl_context;
mod dump;
mod fusion;
mod prehash;
mod raw_vast;
mod result;
mod scopes;
mod typedef_classify;
mod typedef_hashes;

use decl_context::precompute_decl_contexts;
use dump::dump_typed_vast_as_json;
use fusion::light_runtime_fusion_enabled;
use prehash::prehash_vast_identifiers;
use raw_vast::build_raw_vast;
use result::{finish_vast_pg_result, TerminalSemanticBlobs, VastPgResult};
use scopes::precompute_vast_scopes;
use typedef_classify::classify_typedef_vast;
use typedef_hashes::global_typedef_hash_count;

#[derive(Default)]
struct VastTerminalScratch {
    expr_outputs: Vec<Vec<u8>>,
    semantic_outputs: Vec<Vec<u8>>,
}

thread_local! {
    static VAST_TERMINAL_SCRATCH: RefCell<VastTerminalScratch> =
        RefCell::new(VastTerminalScratch::default());
}

pub(super) fn build_vast_and_pg(
    backend: &dyn VyreBackend,
    path: &Path,
    tok_types_bytes: &[u8],
    starts: &[u8],
    lens: &[u8],
    _source: &[u8],
    haystack: &[u8],
    haystack_len: u32,
    nt: u32,
    packed_haystack: bool,
    readback_terminal_outputs: bool,
    resolve_control_edges: bool,
    resolve_conditional_shapes: bool,
    global_typedef_hashes: Option<&[u8]>,
) -> Result<VastPgResult, String> {
    let trace = std::env::var_os("VYRE_STAGE_TRACE").is_some();
    let stage_start = std::time::Instant::now();
    let mut last_t = stage_start;
    let mut log = |label: &str| {
        if trace {
            let now = std::time::Instant::now();
            let stage = now.duration_since(last_t).as_micros();
            let total = now.duration_since(stage_start).as_micros();
            eprintln!("[stage-trace] +{stage}us (total {total}us): build_vast_and_pg {label}");
            last_t = now;
        }
    };

    let mut cfg = DispatchConfig::default();
    let (raw_vast_blob, vast_count) = build_raw_vast(
        backend,
        path,
        tok_types_bytes,
        starts,
        lens,
        nt,
        &mut cfg,
        &mut log,
    )?;
    let hashed_vast_blob = prehash_vast_identifiers(
        backend,
        path,
        &raw_vast_blob,
        haystack,
        haystack_len,
        vast_count,
        packed_haystack,
        &mut cfg,
        &mut log,
    )?;
    let global_typedef_fast_path = global_typedef_hashes.is_some();
    let scoped_vast_blob = precompute_vast_scopes(
        backend,
        path,
        hashed_vast_blob,
        vast_count,
        global_typedef_fast_path,
        &mut cfg,
        &mut log,
    )?;
    let decl_context_blob = precompute_decl_contexts(
        backend,
        path,
        &scoped_vast_blob,
        vast_count,
        global_typedef_fast_path,
        &mut cfg,
        &mut log,
    )?;
    let has_typedef_keyword = contains_u32_word(tok_types_bytes, TOK_TYPEDEF);
    let typed_vast_blob = classify_typedef_vast(
        backend,
        path,
        &scoped_vast_blob,
        &decl_context_blob,
        haystack,
        haystack_len,
        vast_count,
        packed_haystack,
        readback_terminal_outputs,
        has_typedef_keyword,
        global_typedef_hashes,
        &mut cfg,
        &mut log,
    )?;

    // Divergence-gate hook: when `VYRE_DUMP_TYPED_VAST` is set, write the
    // post-classify typed VAST as JSON before downstream stages run.
    // Format: `{ "stride": 10, "count": <N>, "nodes": [[k, parent, fc, ns, …], …] }`.
    // The script that compares vyre vs. clang reads this directly.
    if let Ok(dump_dir) = std::env::var("VYRE_DUMP_TYPED_VAST") {
        dump_typed_vast_as_json(&dump_dir, path, &typed_vast_blob, vast_count).map_err(|e| {
            format!(
                "typed VAST dump failed for `{}`: {e}. Fix: set VYRE_DUMP_TYPED_VAST to a writable directory or unset it.",
                path.display()
            )
        })?;
    }

    let (expr_shape_blob, pg_blob, semantic_pg_nodes, semantic_pg_edges) =
        VAST_TERMINAL_SCRATCH.with(|scratch| -> Result<_, String> {
            let mut scratch = scratch.try_borrow_mut().map_err(|_| {
                "VAST terminal dispatch scratch was re-entered on the same thread. Fix: call VAST/PG construction from a non-nested parser context or add explicit caller-owned scratch.".to_string()
            })?;
            cfg.label = Some(format!("vyre-frontend-c expr-shape {}", path.display()));
            let expr_key = super::stage_pipeline_cache_key(
                "c11_build_expression_shape_nodes",
                &[
                    vast_count.max(1) as u64,
                    resolve_conditional_shapes as u64,
                    readback_terminal_outputs as u64,
                ],
            );
            let expr_inputs = [raw_vast_blob.as_slice(), typed_vast_blob.as_slice()];
            scratch.expr_outputs.clear();
            super::dispatch_borrowed_stage_cached_into(
                backend,
                expr_key,
                || {
                    let expr_prog = if resolve_conditional_shapes {
                        c11_build_expression_shape_nodes(
                            "raw_vast_nodes",
                            "typed_vast_nodes",
                            Expr::u32(vast_count.max(1)),
                            "expr_shape_nodes",
                        )
                    } else {
                        c11_build_expression_shape_nodes_no_conditional(
                            "raw_vast_nodes",
                            "typed_vast_nodes",
                            Expr::u32(vast_count.max(1)),
                            "expr_shape_nodes",
                        )
                    };
                    let expr_prog = super::buffers::mark_program_outputs_readback(
                        expr_prog,
                        &["expr_shape_nodes"],
                        readback_terminal_outputs,
                    );
                    super::validate_internal_stage(&expr_prog, "c11_build_expression_shape_nodes")?;
                    Ok(expr_prog)
                },
                &expr_inputs,
                &cfg,
                &mut scratch.expr_outputs,
            )
            .map_err(|e| format!("c11_build_expression_shape_nodes dispatch failed: {e}"))?;
            super::buffers::drop_suppressed_readbacks(&mut scratch.expr_outputs);
            log("dispatch c11_build_expression_shape_nodes");
            require_scratch_output_count(
                "c11_build_expression_shape_nodes",
                &scratch.expr_outputs,
                usize::from(readback_terminal_outputs),
            )?;
            let expr_shape_blob = if readback_terminal_outputs {
                take_scratch_output(&mut scratch.expr_outputs, 0)
            } else {
                Vec::new()
            };
            cfg.label = Some(format!("vyre-frontend-c semantic-pg {}", path.display()));
            let semantic_key = super::stage_pipeline_cache_key(
                "c_lower_ast_to_pg_semantic_graph_with_pg",
                &[
                    vast_count.max(1) as u64,
                    resolve_control_edges as u64,
                    readback_terminal_outputs as u64,
                ],
            );
            let semantic_inputs = [typed_vast_blob.as_slice()];
            scratch.semantic_outputs.clear();
            super::dispatch_borrowed_stage_cached_into(
                backend,
                semantic_key,
                || {
                    let semantic_pg_prog = if resolve_control_edges {
                        c_lower_ast_to_pg_semantic_graph_with_pg(
                            "typed_vast_nodes",
                            Expr::u32(vast_count.max(1)),
                            "pg_nodes",
                            "semantic_pg_nodes",
                            "semantic_pg_edges",
                        )
                    } else {
                        c_lower_ast_to_pg_semantic_graph_with_pg_no_control_resolution(
                            "typed_vast_nodes",
                            Expr::u32(vast_count.max(1)),
                            "pg_nodes",
                            "semantic_pg_nodes",
                            "semantic_pg_edges",
                        )
                    };
                    let semantic_pg_prog = super::buffers::mark_program_outputs_readback(
                        semantic_pg_prog,
                        &["pg_nodes", "semantic_pg_nodes", "semantic_pg_edges"],
                        readback_terminal_outputs,
                    );
                    super::validate_internal_stage(
                        &semantic_pg_prog,
                        "c_lower_ast_to_pg_semantic_graph",
                    )?;
                    Ok(semantic_pg_prog)
                },
                &semantic_inputs,
                &cfg,
                &mut scratch.semantic_outputs,
            )
            .map_err(|e| format!("c_lower_ast_to_pg_semantic_graph dispatch failed: {e}"))?;
            super::buffers::drop_suppressed_readbacks(&mut scratch.semantic_outputs);
            log("dispatch c_lower_ast_to_pg_semantic_graph");
            require_scratch_output_count(
                "c_lower_ast_to_pg_semantic_graph",
                &scratch.semantic_outputs,
                if readback_terminal_outputs { 3 } else { 0 },
            )?;
            let pg_blob = if readback_terminal_outputs {
                take_scratch_output(&mut scratch.semantic_outputs, 0)
            } else {
                Vec::new()
            };
            let semantic_pg_nodes = if readback_terminal_outputs {
                take_scratch_output(&mut scratch.semantic_outputs, 1)
            } else {
                Vec::new()
            };
            let semantic_pg_edges = if readback_terminal_outputs {
                take_scratch_output(&mut scratch.semantic_outputs, 2)
            } else {
                Vec::new()
            };
            Ok((
                expr_shape_blob,
                pg_blob,
                semantic_pg_nodes,
                semantic_pg_edges,
            ))
        })?;

    finish_vast_pg_result(
        typed_vast_blob,
        TerminalSemanticBlobs {
            expr_shape_blob,
            pg_blob,
            semantic_pg_nodes,
            semantic_pg_edges,
        },
        vast_count,
        readback_terminal_outputs,
    )
}

fn require_scratch_output_count(
    stage: &str,
    outputs: &[Vec<u8>],
    expected: usize,
) -> Result<(), String> {
    if outputs.len() == expected {
        return Ok(());
    }
    Err(format!(
        "{stage} returned {} output buffer(s), expected {expected}. Fix: backend output marking must match readback_terminal_outputs.",
        outputs.len()
    ))
}

fn take_scratch_output(outputs: &mut [Vec<u8>], index: usize) -> Vec<u8> {
    let mut output = Vec::new();
    mem::swap(&mut output, &mut outputs[index]);
    output
}

fn contains_u32_word(bytes: &[u8], needle: u32) -> bool {
    bytes
        .chunks_exact(4)
        .any(|chunk| u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]) == needle)
}
