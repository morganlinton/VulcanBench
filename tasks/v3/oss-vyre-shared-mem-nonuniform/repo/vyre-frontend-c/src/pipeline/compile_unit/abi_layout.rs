use super::*;

pub(super) fn build_object_abi_layout(
    backend: &dyn VyreBackend,
    path: &Path,
    target_abi: CTargetAbi,
    decoded: &token_decode::DecodedObjectTokens,
    dcfg: &mut DispatchConfig,
    trace: &mut trace::CompileTrace,
) -> Result<Vec<u8>, String> {
    let abi_stage = build_c11_abi_stage(
        backend,
        target_abi,
        &decoded.tok_types,
        dcfg,
        &format!("vyre-frontend-c abi {}", path.display()),
        |stage| trace.log(stage),
    )?;
    let mut abi_blob = Vec::new();
    abi_blob.reserve_exact(usize::try_from(abi_stage.byte_len).map_err(|_| {
        "ABI blob length exceeds usize. Fix: shard ABI layout carrier.".to_string()
    })?);
    for output in abi_stage.outputs {
        abi_blob.extend_from_slice(&output);
    }
    Ok(abi_blob)
}
