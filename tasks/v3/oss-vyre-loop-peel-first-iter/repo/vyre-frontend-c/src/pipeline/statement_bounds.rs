use vyre::ir::Expr;
use vyre::{DispatchConfig, VyreBackend};
use vyre_libs::parsing::c::parse::structure_statement::c11_statement_bounds;

use super::backend_select::dispatch_borrowed_stage_cached_into;
use super::buffers::{mark_program_outputs, read_u32_at};
use super::{stage_pipeline_cache_key, validate_internal_stage, MAX_STMT_THREADS, MAX_TOK_SCAN};

#[derive(Default)]
pub(crate) struct StatementBoundsScratch {
    tok_bytes: Vec<u8>,
    outputs: Vec<Vec<u8>>,
}

pub(crate) fn dispatch_c11_statement_bounds_bytes(
    backend: &dyn VyreBackend,
    tok_type_bytes: &[u8],
    n_tokens: u32,
    config: &mut DispatchConfig,
    label: &str,
) -> Result<(Vec<u32>, u32), String> {
    let mut scratch = StatementBoundsScratch::default();
    let mut stmt_pairs = Vec::new();
    let num_stmt = dispatch_c11_statement_bounds_bytes_into(
        backend,
        tok_type_bytes,
        n_tokens,
        config,
        label,
        &mut stmt_pairs,
        &mut scratch,
    )?;
    Ok((stmt_pairs, num_stmt))
}

pub(crate) fn dispatch_c11_statement_bounds_bytes_into(
    backend: &dyn VyreBackend,
    tok_type_bytes: &[u8],
    n_tokens: u32,
    config: &mut DispatchConfig,
    label: &str,
    stmt_pairs: &mut Vec<u32>,
    scratch: &mut StatementBoundsScratch,
) -> Result<u32, String> {
    let scan_tokens = statement_scan_tokens(n_tokens, label)?;
    let required_bytes = n_tokens as usize * 4;
    if tok_type_bytes.len() < required_bytes {
        return Err(format!(
            "{label}: token byte window truncated: need {required_bytes} bytes for {n_tokens} tokens, have {}",
            tok_type_bytes.len()
        ));
    }
    scratch.tok_bytes.clear();
    scratch.tok_bytes.resize(scan_tokens as usize * 4, 0);
    if n_tokens != 0 {
        let live_bytes = scan_tokens as usize * 4;
        scratch.tok_bytes[..live_bytes].copy_from_slice(&tok_type_bytes[..live_bytes]);
    }
    config.label = Some(label.to_string());
    let stmt_key = stage_pipeline_cache_key("c11_statement_bounds", &[scan_tokens as u64]);
    dispatch_borrowed_stage_cached_into(
        backend,
        stmt_key,
        || {
            let stmt_prog = c11_statement_bounds(
                "tok_types",
                Expr::u32(scan_tokens),
                "out_statements",
                "out_counts",
            );
            let stmt_prog = mark_program_outputs(stmt_prog, &["out_statements", "out_counts"]);
            validate_internal_stage(&stmt_prog, "c11_statement_bounds")?;
            Ok(stmt_prog)
        },
        &[&scratch.tok_bytes],
        config,
        &mut scratch.outputs,
    )
    .map_err(|e| format!("{label} dispatch failed: {e}"))?;
    if scratch.outputs.len() != 2 {
        return Err(format!(
            "{label}: expected exactly statement and count outputs, got {}. Fix: backend must return the declared c11_statement_bounds ABI outputs and no extras.",
            scratch.outputs.len()
        ));
    }
    let num_stmt = read_u32_at(&scratch.outputs[1], 0)?
        .clamp(1, MAX_STMT_THREADS)
        .min(scan_tokens);
    read_u32_stream_into(
        &scratch.outputs[0],
        num_stmt as usize * 2,
        "c11 statement bounds",
        stmt_pairs,
    )?;
    Ok(num_stmt)
}

fn statement_scan_tokens(n_tokens: u32, label: &str) -> Result<u32, String> {
    if n_tokens > MAX_TOK_SCAN {
        return Err(format!(
            "{label}: statement-bounds received {n_tokens} tokens, exceeding MAX_TOK_SCAN {MAX_TOK_SCAN}. Fix: dispatch explicit token windows and merge statement ranges instead of clamping the scan."
        ));
    }
    Ok(n_tokens.max(1))
}

fn read_u32_stream_into(
    bytes: &[u8],
    count: usize,
    label: &str,
    out: &mut Vec<u32>,
) -> Result<(), String> {
    vyre_primitives::wire::unpack_u32_slice_into(bytes, count, label, out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn statement_scan_tokens_rejects_silent_max_tok_scan_clamp() {
        let err = statement_scan_tokens(MAX_TOK_SCAN + 1, "contract")
            .expect_err("oversized token stream must fail");
        assert!(err.contains("exceeding MAX_TOK_SCAN"), "{err}");
        assert!(err.contains("instead of clamping"), "{err}");
    }
}
