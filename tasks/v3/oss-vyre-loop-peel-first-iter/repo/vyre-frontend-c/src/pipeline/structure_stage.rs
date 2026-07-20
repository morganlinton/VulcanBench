use super::*;

pub(in crate::pipeline) struct C11StructureStage {
    pub(in crate::pipeline) paren_bytes: Vec<u8>,
    pub(in crate::pipeline) brace_bytes: Vec<u8>,
    pub(in crate::pipeline) fn_records: Vec<u8>,
    pub(in crate::pipeline) call_records: Vec<u8>,
    pub(in crate::pipeline) n_fn: u32,
}

pub(in crate::pipeline) fn build_c11_structure_stage(
    backend: &dyn VyreBackend,
    tok_types: &[u32],
    types_logical: &[u8],
    n_tokens: u32,
    dcfg: &mut DispatchConfig,
    bracket_label: &str,
    structure_label: &str,
    mut log: impl FnMut(&str),
) -> Result<C11StructureStage, String> {
    let (paren_pairs, brace_pairs) =
        c11_dual_bracket_pairs_cost_model(backend, tok_types, bracket_label)?;
    log("dispatch c11 dual bracket pairs");
    let paren_bytes = vec_u32_le_bytes_min_words(&paren_pairs, n_tokens.max(1))?;
    let brace_bytes = vec_u32_le_bytes_min_words(&brace_pairs, n_tokens.max(1))?;
    let structure_records = build_structure_records(
        backend,
        types_logical,
        &paren_bytes,
        &brace_bytes,
        n_tokens,
        dcfg,
        structure_label,
    )?;
    log("dispatch c11 structure records");
    Ok(C11StructureStage {
        paren_bytes,
        brace_bytes,
        fn_records: structure_records.functions,
        call_records: structure_records.calls,
        n_fn: structure_records.function_count.max(1),
    })
}
