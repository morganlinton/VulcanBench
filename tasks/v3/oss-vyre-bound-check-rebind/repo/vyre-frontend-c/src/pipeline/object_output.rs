use super::*;
pub(super) fn write_object_atomic(dest: &Path, bytes: &[u8]) -> Result<(), String> {
    let pid = std::process::id();
    let tmp = dest.with_extension(match dest.extension().and_then(|ext| ext.to_str()) {
        Some(ext) if !ext.is_empty() => format!("{ext}.{pid}.tmp"),
        _ => format!("{pid}.tmp"),
    });
    fs::write(&tmp, bytes).map_err(|error| {
        format!(
            "write temporary object {}: {error}. Fix: repair output directory permissions and free space.",
            tmp.display()
        )
    })?;
    fs::rename(&tmp, dest).map_err(|error| {
        format!(
            "rename temporary object {} to {}: {error}. Fix: keep output on one filesystem and ensure the destination is writable.",
            tmp.display(),
            dest.display()
        )
    })?;
    Ok(())
}
pub(super) fn validate_object_output_path(input: &Path, dest: &Path) -> Result<(), String> {
    if dest.as_os_str().is_empty() {
        return Err(
            "vyre-frontend-c: object output path is empty. Fix: pass a concrete .o path."
                .to_string(),
        );
    }
    if dest.is_dir() {
        return Err(format!(
            "vyre-frontend-c: object output {} is a directory. Fix: pass a concrete .o file path.",
            dest.display()
        ));
    }
    if let Some(parent) = dest.parent() {
        if !parent.as_os_str().is_empty() && !parent.is_dir() {
            return Err(format!(
                "vyre-frontend-c: object output parent {} does not exist or is not a directory. Fix: create it before compilation.",
                parent.display()
            ));
        }
    }
    let input_canon = std::fs::canonicalize(input).map_err(|error| {
        format!(
            "vyre-frontend-c: cannot canonicalize input {} before output validation: {error}",
            input.display()
        )
    })?;
    if dest.exists() {
        let dest_canon = std::fs::canonicalize(dest).map_err(|error| {
            format!(
                "vyre-frontend-c: cannot canonicalize existing output {}: {error}",
                dest.display()
            )
        })?;
        if dest_canon == input_canon {
            return Err(format!(
                "vyre-frontend-c: object output {} would overwrite input {}. Fix: choose a distinct .o path.",
                dest.display(),
                input.display()
            ));
        }
    }
    Ok(())
}
