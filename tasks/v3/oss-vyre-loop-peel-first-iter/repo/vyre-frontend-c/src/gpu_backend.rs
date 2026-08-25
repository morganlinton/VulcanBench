/// Require a concrete GPU backend for production C frontend work.
///
/// CPU/reference backends are valid only for feature-gated conformance oracles;
/// the release parser and preprocessor paths must fail loudly instead of
/// silently degrading away from GPU execution.
pub(crate) fn require_gpu_dispatch_backend(id: &str) -> Result<(), String> {
    let normalized = id.to_ascii_lowercase();
    if normalized.contains("cpu")
        || normalized.contains("reference")
        || normalized == "ref"
        || normalized.ends_with("-ref")
        || normalized.contains("oracle")
    {
        return Err(format!(
            "vyre-frontend-c requires a concrete GPU dispatch backend; backend `{id}` is not allowed. Fix: select `cuda` or `wgpu` and repair GPU device visibility; CPU/reference execution is reserved for explicit conformance oracles."
        ));
    }
    Ok(())
}
