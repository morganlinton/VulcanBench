use std::cell::RefCell;
use std::mem;

use super::*;

pub(in crate::pipeline) enum C11AstReadback {
    CountOnly,
    Full,
}

pub(in crate::pipeline) struct C11AstStage {
    pub(in crate::pipeline) outputs: Vec<Vec<u8>>,
    pub(in crate::pipeline) num_stmt: u32,
    pub(in crate::pipeline) ast_capacity: u32,
}

#[derive(Default)]
struct FullAstStageScratch {
    stmt_pairs: Vec<u32>,
    stmt_bytes: Vec<u8>,
    ast_inputs: AstOwnedInputBuffers,
    ast_padding: Vec<Vec<u8>>,
    ast_outputs: Vec<Vec<u8>>,
    statement_bounds: StatementBoundsScratch,
}

thread_local! {
    static FULL_AST_STAGE_SCRATCH: RefCell<FullAstStageScratch> =
        RefCell::new(FullAstStageScratch::default());
}

pub(in crate::pipeline) fn build_c11_full_ast_stage(
    backend: &dyn VyreBackend,
    tok_types: &[u32],
    types_logical: &[u8],
    n_tokens: u32,
    nt: u32,
    readback: C11AstReadback,
    dcfg: &mut DispatchConfig,
    statement_label: &str,
    ast_label: &str,
    mut log: impl FnMut(&str),
) -> Result<C11AstStage, String> {
    FULL_AST_STAGE_SCRATCH.with(|scratch| {
        let mut scratch = scratch.try_borrow_mut().map_err(|_| {
            "full AST stage scratch was re-entered on the same thread. Fix: call full AST lowering from a non-nested parser context or add explicit caller-owned scratch.".to_string()
        })?;
        build_c11_full_ast_stage_with_scratch(
            backend,
            tok_types,
            types_logical,
            n_tokens,
            nt,
            readback,
            dcfg,
            statement_label,
            ast_label,
            &mut log,
            &mut scratch,
        )
    })
}

fn build_c11_full_ast_stage_with_scratch(
    backend: &dyn VyreBackend,
    tok_types: &[u32],
    types_logical: &[u8],
    n_tokens: u32,
    nt: u32,
    readback: C11AstReadback,
    dcfg: &mut DispatchConfig,
    statement_label: &str,
    ast_label: &str,
    mut log: impl FnMut(&str),
    scratch: &mut FullAstStageScratch,
) -> Result<C11AstStage, String> {
    let num_stmt = dispatch_c11_statement_bounds_bytes_into(
        backend,
        types_logical,
        nt,
        dcfg,
        statement_label,
        &mut scratch.stmt_pairs,
        &mut scratch.statement_bounds,
    )?;
    vyre_primitives::wire::pack_u32_slice_into(&scratch.stmt_pairs, &mut scratch.stmt_bytes);
    log("dispatch c11_statement_bounds");
    if n_tokens > MAX_TOK_SCAN {
        return Err(format!(
            "{ast_label} received {n_tokens} tokens, exceeding MAX_TOK_SCAN {MAX_TOK_SCAN}. Fix: use a real AST window stitching path instead of silently truncating the token stream."
        ));
    }
    let ast_capacity = n_tokens.max(1);
    let ast_prog = ast_shunting_yard_with_capacity(
        "tok_types",
        "statements",
        Expr::u32(num_stmt),
        "out_ast_nodes",
        "out_ast_count",
        "out_statement_roots",
        "scratch_val_stack",
        "scratch_op_stack",
        ast_capacity,
        num_stmt.max(1),
    );
    let suppress_outputs: &[&str] = match readback {
        C11AstReadback::CountOnly => &[
            "out_ast_nodes",
            "out_statement_roots",
            "scratch_val_stack",
            "scratch_op_stack",
        ],
        C11AstReadback::Full => &["scratch_val_stack", "scratch_op_stack"],
    };
    let ast_prog = buffers::suppress_readwrite_readback(ast_prog, suppress_outputs);
    validate_internal_stage(&ast_prog, "ast_shunting_yard")?;
    build_ast_owned_inputs_with_capacity_into(
        tok_types,
        num_stmt,
        ast_capacity,
        &mut scratch.ast_inputs,
    )?;
    let ast_refs = pad_dispatch_input_refs(
        &ast_prog,
        vec![
            scratch.ast_inputs.tok_b.as_slice(),
            scratch.stmt_bytes.as_slice(),
            scratch.ast_inputs.out_ast.as_slice(),
            scratch.ast_inputs.out_cnt.as_slice(),
            scratch.ast_inputs.out_roots.as_slice(),
            scratch.ast_inputs.scratch_v.as_slice(),
            scratch.ast_inputs.scratch_o.as_slice(),
        ],
        &mut scratch.ast_padding,
    );
    dcfg.label = Some(ast_label.to_string());
    dispatch_borrowed_cached_into(
        backend,
        &ast_prog,
        &ast_refs,
        dcfg,
        &mut scratch.ast_outputs,
    )
    .map_err(|error| format!("{ast_label} ast_shunting_yard dispatch failed: {error}"))?;
    buffers::drop_suppressed_readbacks(&mut scratch.ast_outputs);
    log("dispatch ast_shunting_yard");
    let expected_outputs = match readback {
        C11AstReadback::CountOnly => 1,
        C11AstReadback::Full => 3,
    };
    if scratch.ast_outputs.len() != expected_outputs {
        return Err(format!(
            "ast_shunting_yard returned {} output buffer(s), expected {expected_outputs}. Fix: backend output marking must match AST readback mode.",
            scratch.ast_outputs.len()
        ));
    }
    let mut outputs = Vec::new();
    mem::swap(&mut outputs, &mut scratch.ast_outputs);
    Ok(C11AstStage {
        outputs,
        num_stmt,
        ast_capacity,
    })
}
