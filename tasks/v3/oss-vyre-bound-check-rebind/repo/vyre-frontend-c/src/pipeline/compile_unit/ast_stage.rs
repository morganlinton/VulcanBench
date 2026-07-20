use super::*;

pub(super) fn build_object_ast(
    backend: &dyn VyreBackend,
    path: &Path,
    decoded: &token_decode::DecodedObjectTokens,
    dcfg: &mut DispatchConfig,
    trace: &mut trace::CompileTrace,
) -> Result<Vec<u8>, String> {
    let ast_stage = build_c11_full_ast_stage(
        backend,
        &decoded.tok_types,
        &decoded.types_logical,
        decoded.n_tokens,
        decoded.nt,
        C11AstReadback::Full,
        dcfg,
        &format!("vyre-frontend-c statement-bounds {}", path.display()),
        &format!("vyre-frontend-c ast {}", path.display()),
        |stage| trace.log(stage),
    )?;
    let mut ast_blob = Vec::new();
    for chunk in ast_stage.outputs {
        ast_blob.extend_from_slice(&chunk);
    }
    Ok(ast_blob)
}
