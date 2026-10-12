use super::*;
pub(crate) fn validate_internal_stage(program: &Program, stage: &str) -> Result<(), String> {
    let errors = vyre::validate(program);
    if errors.is_empty() {
        return Ok(());
    }
    Err(format!(
        "{stage} IR validation failed: {errors:?}. Fix: repair the generated parser Program before dispatch."
    ))
}

#[cfg(test)]
pub(crate) fn require_exact_stage_outputs(
    stage: &str,
    outputs: Vec<Vec<u8>>,
    expected: usize,
) -> Result<Vec<Vec<u8>>, String> {
    if outputs.len() == expected {
        return Ok(outputs);
    }
    Err(format!(
        "{stage} returned {} output buffer(s), expected exactly {expected}. Fix: repair stage output marking or backend readback routing.",
        outputs.len()
    ))
}

pub(crate) fn require_full_semantic_summary(
    summary: CParseSummary,
    source: &str,
) -> Result<(), String> {
    if summary.token_count == 0
        || summary.ast_bytes == 0
        || summary.ast_node_count == 0
        || summary.vast_bytes == 0
        || summary.program_graph_bytes == 0
        || summary.semantic_node_bytes == 0
        || summary.semantic_edge_bytes == 0
        || summary.sema_scope_bytes == 0
    {
        return Err(format!(
            "vyre-frontend-c {source} did not produce a full parser/semantic summary: tokens={} ast_bytes={} ast_nodes={} vast_bytes={} pg_bytes={} sem_nodes={} sem_edges={} scopes={}. Fix: run the full GPU parser/sema path instead of a syntax-only or partial pipeline.",
            summary.token_count,
            summary.ast_bytes,
            summary.ast_node_count,
            summary.vast_bytes,
            summary.program_graph_bytes,
            summary.semantic_node_bytes,
            summary.semantic_edge_bytes,
            summary.sema_scope_bytes
        ));
    }
    Ok(())
}
