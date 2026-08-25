use std::cell::RefCell;

use super::*;

pub(super) struct C11SyntaxAstSummary {
    pub(super) ast_bytes: u64,
    pub(super) ast_node_count: u32,
}

#[derive(Default)]
struct C11SyntaxAstScratch {
    token_window_bytes: Vec<u8>,
    stmt_pairs: Vec<u32>,
    stmt_bytes: Vec<u8>,
    ast_inputs: AstOwnedInputBuffers,
    ast_padding: Vec<Vec<u8>>,
    ast_outputs: Vec<Vec<u8>>,
    statement_bounds: StatementBoundsScratch,
}

thread_local! {
    static SYNTAX_AST_SCRATCH: RefCell<C11SyntaxAstScratch> =
        RefCell::new(C11SyntaxAstScratch::default());
}

pub(super) fn build_c11_syntax_ast_stage(
    backend: &dyn VyreBackend,
    tok_types: &[u32],
    n_tokens: u32,
    dcfg: &mut DispatchConfig,
    mut log: impl FnMut(&str),
) -> Result<C11SyntaxAstSummary, String> {
    SYNTAX_AST_SCRATCH.with(|scratch| {
        let mut scratch = scratch.try_borrow_mut().map_err(|_| {
            "syntax-only AST scratch was re-entered on the same thread. Fix: call the AST stage from a non-nested parser context or pass explicit scratch.".to_string()
        })?;
        build_c11_syntax_ast_stage_with_scratch(
            backend,
            tok_types,
            n_tokens,
            dcfg,
            &mut log,
            &mut scratch,
        )
    })
}

fn build_c11_syntax_ast_stage_with_scratch(
    backend: &dyn VyreBackend,
    tok_types: &[u32],
    n_tokens: u32,
    dcfg: &mut DispatchConfig,
    mut log: impl FnMut(&str),
    scratch: &mut C11SyntaxAstScratch,
) -> Result<C11SyntaxAstSummary, String> {
    let mut ast_bytes = 0u64;
    let mut ast_node_count = 0u32;
    let mut ast_covered_tokens = 0u32;
    let ast_total_tokens = n_tokens.max(1);
    while ast_covered_tokens < ast_total_tokens {
        let window_tokens = ast_total_tokens
            .saturating_sub(ast_covered_tokens)
            .clamp(1, MAX_TOK_SCAN);
        let window_start = if n_tokens == 0 {
            0
        } else {
            ast_covered_tokens as usize
        };
        let window_end = if n_tokens == 0 {
            0
        } else {
            ast_covered_tokens
                .saturating_add(window_tokens)
                .min(n_tokens) as usize
        };
        let token_window = tok_types
            .get(window_start..window_end)
            .ok_or_else(|| {
                format!(
                    "syntax-only AST window [{}..{}) exceeds {} logical tokens. Fix: keep AST window slicing aligned with lexer compaction.",
                    window_start,
                    window_end,
                    tok_types.len()
                )
            })?;
        vyre_primitives::wire::pack_u32_slice_into(token_window, &mut scratch.token_window_bytes);
        let num_stmt = dispatch_c11_statement_bounds_bytes_into(
            backend,
            &scratch.token_window_bytes,
            if n_tokens == 0 { 0 } else { window_tokens },
            dcfg,
            "vyre-frontend-c syntax-only statement-bounds",
            &mut scratch.stmt_pairs,
            &mut scratch.statement_bounds,
        )?;
        vyre_primitives::wire::pack_u32_slice_into(&scratch.stmt_pairs, &mut scratch.stmt_bytes);
        log("dispatch c11_statement_bounds");
        let ast_prog = ast_shunting_yard_with_capacity(
            "tok_types",
            "statements",
            Expr::u32(num_stmt),
            "out_ast_nodes",
            "out_ast_count",
            "out_statement_roots",
            "scratch_val_stack",
            "scratch_op_stack",
            window_tokens,
            num_stmt.max(1),
        );
        let ast_prog = buffers::suppress_readwrite_readback(
            ast_prog,
            &["scratch_val_stack", "scratch_op_stack"],
        );
        validate_internal_stage(&ast_prog, "ast_shunting_yard")?;
        build_ast_owned_inputs_with_capacity_into(
            token_window,
            num_stmt,
            window_tokens,
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
        dcfg.label = Some(format!(
            "vyre-frontend-c syntax-only ast tokens {}..{}",
            ast_covered_tokens,
            ast_covered_tokens.saturating_add(window_tokens)
        ));
        dispatch_borrowed_cached_into(
            backend,
            &ast_prog,
            &ast_refs,
            dcfg,
            &mut scratch.ast_outputs,
        )
        .map_err(|e| format!("syntax-only ast_shunting_yard dispatch failed: {e}"))?;
        buffers::drop_suppressed_readbacks(&mut scratch.ast_outputs);
        log("dispatch ast_shunting_yard");
        if scratch.ast_outputs.len() != 3 {
            return Err(format!(
                "syntax-only ast_shunting_yard returned {} output buffer(s), expected exactly 3. Fix: repair stage output marking or backend readback routing.",
                scratch.ast_outputs.len()
            ));
        }
        let window_ast_bytes = scratch.ast_outputs.iter().try_fold(0u64, |acc, output| {
            let len = u64::try_from(output.len()).map_err(|_| {
                "syntax-only AST output length exceeds u64. Fix: shard parser output accounting."
                    .to_string()
            })?;
            acc.checked_add(len).ok_or_else(|| {
                "syntax-only AST output byte accounting overflowed u64. Fix: shard parser output accounting."
                    .to_string()
            })
        })?;
        ast_bytes = ast_bytes.checked_add(window_ast_bytes).ok_or_else(|| {
                "syntax-only AST total byte accounting overflowed u64. Fix: shard parser summary output."
                .to_string()
        })?;
        let window_ast_words = read_u32_at(&scratch.ast_outputs[1], 0).map_err(|error| {
            format!("syntax-only ast_shunting_yard node-count output decode failed: {error}")
        })?;
        let window_ast_nodes = (window_ast_words / 4).max(1);
        ast_node_count = ast_node_count
            .checked_add(window_ast_nodes)
            .ok_or_else(|| {
                "syntax-only AST node count overflowed u32. Fix: shard parser summary output."
                    .to_string()
            })?;
        ast_covered_tokens = ast_covered_tokens.checked_add(window_tokens).ok_or_else(|| {
            "syntax-only parser window cursor overflowed token count. Fix: shard parser windows before AST dispatch."
                .to_string()
        })?;
    }

    Ok(C11SyntaxAstSummary {
        ast_bytes,
        ast_node_count,
    })
}
