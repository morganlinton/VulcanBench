use std::cell::RefCell;

use super::*;

#[derive(Default)]
struct AstWindowScratch {
    stmt_bytes: Vec<u8>,
    ast_inputs: AstWindowOwnedInputs,
    ast_outputs: Vec<Vec<u8>>,
}

thread_local! {
    static AST_WINDOW_SCRATCH: RefCell<AstWindowScratch> =
        RefCell::new(AstWindowScratch::default());
}

pub(super) fn dispatch_dense_ast_windows(
    backend: &dyn vyre::VyreBackend,
    dense_types: &[u8],
    token_count: u32,
    config: &mut DispatchConfig,
) -> Result<(u64, u32), String> {
    AST_WINDOW_SCRATCH.with(|scratch| {
        let mut scratch = scratch.try_borrow_mut().map_err(|_| {
            "raw-byte AST window scratch was re-entered on the same thread. Fix: call raw syntax AST windowing from a non-nested parser context or add explicit caller-owned scratch.".to_string()
        })?;
        dispatch_dense_ast_windows_with_scratch(backend, dense_types, token_count, config, &mut scratch)
    })
}

fn dispatch_dense_ast_windows_with_scratch(
    backend: &dyn vyre::VyreBackend,
    dense_types: &[u8],
    token_count: u32,
    config: &mut DispatchConfig,
    scratch: &mut AstWindowScratch,
) -> Result<(u64, u32), String> {
    let tokens_per_window = vyre_libs::parsing::c::pipeline::stages::C11_AST_MAX_TOK_SCAN.max(1);
    let mut covered = 0u32;
    let mut ast_bytes = 0u64;
    let mut ast_nodes = 0u32;
    while covered < token_count.max(1) {
        let remaining_tokens = token_count.max(1).checked_sub(covered).ok_or_else(|| {
            "raw-byte AST window cursor exceeded token count. Fix: repair parser window accounting."
                .to_string()
        })?;
        let window_tokens = remaining_tokens.clamp(1, tokens_per_window);
        let byte_start = if token_count == 0 {
            0
        } else {
            (covered as usize).checked_mul(4).ok_or_else(|| {
                "raw-byte dense AST byte start overflows usize. Fix: shard parser input."
                    .to_string()
            })?
        };
        let byte_end = if token_count == 0 {
            0
        } else {
            byte_start
                .checked_add((window_tokens as usize).checked_mul(4).ok_or_else(|| {
                    "raw-byte dense AST window byte length overflows usize. Fix: shard parser input."
                        .to_string()
                })?)
                .ok_or_else(|| {
                    "raw-byte dense AST byte end overflows usize. Fix: shard parser input."
                        .to_string()
                })?
        };
        let token_window = dense_types.get(byte_start..byte_end).ok_or_else(|| {
            format!(
                "raw-byte dense AST window [{}..{}) exceeds dense token buffer length {}",
                byte_start,
                byte_end,
                dense_types.len()
            )
        })?;
        let (stmt_pairs, num_stmt) = crate::pipeline::dispatch_statement_bounds_bytes_for_api(
            backend,
            token_window,
            if token_count == 0 { 0 } else { window_tokens },
            config,
            "vyre-frontend-c raw-byte statement bounds",
        )?;
        vyre_primitives::wire::pack_u32_slice_into(&stmt_pairs, &mut scratch.stmt_bytes);
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
        let ast_prog = crate::pipeline::suppress_readwrite_readback(
            ast_prog,
            &["scratch_val_stack", "scratch_op_stack"],
        );
        build_ast_owned_inputs_for_window_into(
            token_window,
            num_stmt,
            window_tokens,
            &mut scratch.ast_inputs,
        )?;
        let ast_refs = [
            scratch.ast_inputs.tok_b.as_slice(),
            scratch.stmt_bytes.as_slice(),
            scratch.ast_inputs.out_ast.as_slice(),
            scratch.ast_inputs.out_cnt.as_slice(),
            scratch.ast_inputs.out_roots.as_slice(),
            scratch.ast_inputs.scratch_v.as_slice(),
            scratch.ast_inputs.scratch_o.as_slice(),
        ];
        config.label = Some(format!(
            "vyre-frontend-c raw-byte ast tokens {}..{}",
            covered,
            covered.checked_add(window_tokens).ok_or_else(|| {
                "raw-byte AST label token end overflowed u32. Fix: shard parser input.".to_string()
            })?
        ));
        crate::pipeline::dispatch_borrowed_cached_into(
            backend,
            &ast_prog,
            &ast_refs,
            config,
            &mut scratch.ast_outputs,
        )
        .map_err(|e| format!("raw-byte ast_shunting_yard dispatch failed: {e}"))?;
        crate::pipeline::drop_suppressed_readbacks(&mut scratch.ast_outputs);
        if scratch.ast_outputs.len() != 3 {
            return Err(format!(
                "raw-byte ast_shunting_yard returned {} outputs, expected exactly AST/count/roots. Fix: backend must return the declared GPU stage ABI outputs and no extras.",
                scratch.ast_outputs.len()
            ));
        }
        let ast_word_count = read_u32_at(&scratch.ast_outputs[1], 0, "raw-byte AST word count")?;
        let root_bytes = (num_stmt as u64).checked_mul(4).ok_or_else(|| {
            "raw-byte AST root byte count overflowed u64. Fix: shard parser summary output."
                .to_string()
        })?;
        let ast_payload_bytes = (ast_word_count as u64)
            .checked_mul(4)
            .and_then(|bytes| bytes.checked_add(4))
            .and_then(|bytes| bytes.checked_add(root_bytes))
            .ok_or_else(|| {
                "raw-byte AST byte count overflowed u64. Fix: shard parser summary output."
                    .to_string()
            })?;
        ast_bytes = ast_bytes.checked_add(ast_payload_bytes).ok_or_else(|| {
            "raw-byte AST byte total overflowed u64. Fix: shard parser summary output.".to_string()
        })?;
        ast_nodes = ast_nodes.checked_add(ast_word_count / 4).ok_or_else(|| {
            "raw-byte AST node count overflowed u32. Fix: shard parser summary output.".to_string()
        })?;
        covered = covered.checked_add(window_tokens).ok_or_else(|| {
            "raw-byte AST covered token count overflowed u32. Fix: shard parser input.".to_string()
        })?;
    }
    Ok((
        ast_bytes,
        ast_nodes.max(token_count.min(tokens_per_window).max(1)),
    ))
}

#[derive(Default)]
struct AstWindowOwnedInputs {
    tok_b: Vec<u8>,
    out_ast: Vec<u8>,
    out_cnt: Vec<u8>,
    out_roots: Vec<u8>,
    scratch_v: Vec<u8>,
    scratch_o: Vec<u8>,
}

fn build_ast_owned_inputs_for_window_into(
    token_window: &[u8],
    num_stmt: u32,
    token_capacity: u32,
    inputs: &mut AstWindowOwnedInputs,
) -> Result<(), String> {
    let token_bytes = (token_capacity as usize).checked_mul(4).ok_or_else(|| {
        "raw-byte AST token input byte length overflows usize. Fix: shard parser input.".to_string()
    })?;
    if inputs.tok_b.len() != token_bytes {
        inputs.tok_b.resize(token_bytes, 0);
    }
    let live = token_window.len().min(inputs.tok_b.len());
    if live < inputs.tok_b.len() {
        inputs.tok_b[live..].fill(0);
    }
    inputs.tok_b[..live].copy_from_slice(&token_window[..live]);
    let out_ast_bytes = token_bytes.checked_mul(4).ok_or_else(|| {
        "raw-byte AST output byte length overflows usize. Fix: shard parser input.".to_string()
    })?;
    inputs.out_ast.clear();
    inputs.out_ast.resize(out_ast_bytes, 0);
    inputs.out_cnt.clear();
    inputs.out_cnt.resize(4, 0);
    let root_bytes = (num_stmt.max(1) as usize).checked_mul(4).ok_or_else(|| {
        "raw-byte AST root buffer byte length overflows usize. Fix: shard parser input.".to_string()
    })?;
    inputs.out_roots.clear();
    inputs.out_roots.resize(root_bytes, 0);
    let scratch_words = num_stmt
        .checked_mul(64)
        .ok_or_else(|| {
            "raw-byte AST scratch word count overflowed u32. Fix: shard parser input.".to_string()
        })?
        .max(64);
    let scratch_bytes = (scratch_words as usize).checked_mul(4).ok_or_else(|| {
        "raw-byte AST scratch byte length overflows usize. Fix: shard parser input.".to_string()
    })?;
    inputs.scratch_v.clear();
    inputs.scratch_v.resize(scratch_bytes, 0);
    inputs.scratch_o.clear();
    inputs.scratch_o.resize(scratch_bytes, 0);
    Ok(())
}
