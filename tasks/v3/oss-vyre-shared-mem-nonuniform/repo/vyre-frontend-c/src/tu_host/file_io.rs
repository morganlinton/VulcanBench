use super::*;
pub(super) fn read_include_bounded(path: &Path) -> Result<Vec<u8>, String> {
    use std::io::Read as _;

    let metadata = fs::metadata(path)
        .map_err(|error| format!("vyre-frontend-c: stat include {}: {error}", path.display()))?;
    if metadata.len() > MAX_INCLUDE_BYTES as u64 {
        return Err(format!(
            "vyre-frontend-c: include {} is {} bytes; maximum accepted include is {MAX_INCLUDE_BYTES} bytes",
            path.display(),
            metadata.len()
        ));
    }
    let capacity = usize::try_from(metadata.len()).map_err(|_| {
        format!(
            "vyre-frontend-c: include {} is {} bytes and exceeds host addressable memory. Fix: shard or reject this include before resident preparation.",
            path.display(),
            metadata.len()
        )
    })?;
    let mut file = fs::File::open(path)
        .map_err(|error| format!("vyre-frontend-c: open include {}: {error}", path.display()))?;
    let mut bytes = Vec::with_capacity(capacity);
    file.by_ref()
        .take(MAX_INCLUDE_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("vyre-frontend-c: read include {}: {error}", path.display()))?;
    if bytes.len() > MAX_INCLUDE_BYTES {
        return Err(format!(
            "vyre-frontend-c: include {} exceeded {MAX_INCLUDE_BYTES} bytes while reading",
            path.display()
        ));
    }
    Ok(bytes)
}
