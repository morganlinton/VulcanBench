use super::*;

pub(super) type ObjectStructure = C11StructureStage;

pub(super) fn build_object_structure(
    backend: &dyn VyreBackend,
    path: &Path,
    decoded: &token_decode::DecodedObjectTokens,
    dcfg: &mut DispatchConfig,
    trace: &mut trace::CompileTrace,
) -> Result<ObjectStructure, String> {
    build_c11_structure_stage(
        backend,
        &decoded.tok_types,
        &decoded.types_logical,
        decoded.n_tokens,
        dcfg,
        &format!("vyre-frontend-c c11-brackets {}", path.display()),
        &format!("vyre-frontend-c structure {}", path.display()),
        |stage| trace.log(stage),
    )
}
