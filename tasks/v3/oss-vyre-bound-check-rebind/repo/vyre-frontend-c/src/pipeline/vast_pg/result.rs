use super::*;

pub(in crate::pipeline) struct VastPgResult {
    pub(in crate::pipeline) typed_vast_blob: Vec<u8>,
    pub(in crate::pipeline) typed_vast_bytes: u64,
    pub(in crate::pipeline) expression_shape_blob: Vec<u8>,
    pub(in crate::pipeline) program_graph_blob: Vec<u8>,
    pub(in crate::pipeline) semantic_pg_nodes: Vec<u8>,
    pub(in crate::pipeline) semantic_pg_edges: Vec<u8>,
    pub(in crate::pipeline) expression_shape_bytes: u64,
    pub(in crate::pipeline) program_graph_bytes: u64,
    pub(in crate::pipeline) semantic_node_bytes: u64,
    pub(in crate::pipeline) semantic_edge_bytes: u64,
}

pub(super) struct TerminalSemanticBlobs {
    pub(super) expr_shape_blob: Vec<u8>,
    pub(super) pg_blob: Vec<u8>,
    pub(super) semantic_pg_nodes: Vec<u8>,
    pub(super) semantic_pg_edges: Vec<u8>,
}

pub(super) fn finish_vast_pg_result(
    typed_vast_blob: Vec<u8>,
    terminal: TerminalSemanticBlobs,
    vast_count: u32,
    readback_terminal_outputs: bool,
) -> Result<VastPgResult, String> {
    let node_count = u64::from(vast_count.max(1));
    let checked_stage_bytes = |stage: &str, words_per_node: u64| -> Result<u64, String> {
        node_count
            .checked_mul(words_per_node)
            .and_then(|words| words.checked_mul(4))
            .ok_or_else(|| {
                format!(
                    "{stage}: expected readback byte count overflows u64 for node_count={node_count}, words_per_node={words_per_node}. Fix: shard the VAST/semantic graph before GPU lowering."
                )
            })
    };
    let expected_expression_shape_bytes = checked_stage_bytes(
        "c11_build_expression_shape_nodes",
        u64::from(C_EXPR_SHAPE_STRIDE_U32),
    )?;
    let expected_program_graph_bytes = checked_stage_bytes("c_lower_ast_to_pg_nodes", 6)?;
    let expected_semantic_node_bytes =
        checked_stage_bytes("c_lower_ast_to_pg_semantic_graph nodes", 10)?;
    let expected_semantic_edge_bytes =
        checked_stage_bytes("c_lower_ast_to_pg_semantic_graph edges", 30)?;
    contracts::require_exact_readback_bytes(
        "c11_build_expression_shape_nodes",
        "expr_shape_nodes",
        &terminal.expr_shape_blob,
        expected_expression_shape_bytes,
        readback_terminal_outputs,
    )?;
    contracts::require_exact_readback_bytes(
        "c_lower_ast_to_pg_nodes",
        "pg_nodes",
        &terminal.pg_blob,
        expected_program_graph_bytes,
        readback_terminal_outputs,
    )?;
    contracts::require_exact_readback_bytes(
        "c_lower_ast_to_pg_semantic_graph",
        "semantic_pg_nodes",
        &terminal.semantic_pg_nodes,
        expected_semantic_node_bytes,
        readback_terminal_outputs,
    )?;
    contracts::require_exact_readback_bytes(
        "c_lower_ast_to_pg_semantic_graph",
        "semantic_pg_edges",
        &terminal.semantic_pg_edges,
        expected_semantic_edge_bytes,
        readback_terminal_outputs,
    )?;
    Ok(VastPgResult {
        typed_vast_bytes: typed_vast_blob.len() as u64,
        typed_vast_blob,
        expression_shape_bytes: if readback_terminal_outputs {
            terminal.expr_shape_blob.len() as u64
        } else {
            expected_expression_shape_bytes
        },
        program_graph_bytes: if readback_terminal_outputs {
            terminal.pg_blob.len() as u64
        } else {
            expected_program_graph_bytes
        },
        semantic_node_bytes: if readback_terminal_outputs {
            terminal.semantic_pg_nodes.len() as u64
        } else {
            expected_semantic_node_bytes
        },
        semantic_edge_bytes: if readback_terminal_outputs {
            terminal.semantic_pg_edges.len() as u64
        } else {
            expected_semantic_edge_bytes
        },
        expression_shape_blob: terminal.expr_shape_blob,
        program_graph_blob: terminal.pg_blob,
        semantic_pg_nodes: terminal.semantic_pg_nodes,
        semantic_pg_edges: terminal.semantic_pg_edges,
    })
}
