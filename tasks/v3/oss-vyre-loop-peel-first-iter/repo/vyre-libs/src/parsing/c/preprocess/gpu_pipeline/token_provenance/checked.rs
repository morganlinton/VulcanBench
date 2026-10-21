pub(crate) fn checked_output_offset(
    output_base: usize,
    token_start: u32,
    label: &str,
) -> Result<u32, String> {
    let value = output_base.checked_add(token_start as usize).ok_or_else(|| {
        format!("vyre-libs::gpu_pipeline: {label} overflow. Fix: shard preprocessing before provenance export.")
    })?;
    checked_usize_to_u32(value, label)
}

pub(crate) fn checked_usize_to_u32(value: usize, label: &str) -> Result<u32, String> {
    u32::try_from(value).map_err(|_| {
        format!(
            "vyre-libs::gpu_pipeline: {label} {value} exceeds u32. Fix: shard preprocessing before provenance export."
        )
    })
}
