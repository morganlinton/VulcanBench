//! Backend-neutral fixpoint-iteration resolution.

use crate::{BackendError, DispatchConfig};

/// Resolve the effective fixpoint iteration count for a dispatch.
///
/// `None` means one launch. `Some(0)` is invalid: a caller that explicitly
/// asks for zero fixpoint iterations is expressing a different computation, so
/// backends must not silently rewrite it to one iteration.
///
/// # Errors
///
/// Returns [`BackendError::InvalidProgram`] when `fixpoint_iterations` is
/// explicitly zero.
pub fn resolve_fixpoint_iterations(
    config: &DispatchConfig,
    backend: &str,
) -> Result<u32, BackendError> {
    match config.fixpoint_iterations {
        Some(0) => Err(BackendError::InvalidProgram {
            fix: format!(
                "Fix: DispatchConfig::fixpoint_iterations must be at least 1 when set; {backend} must not silently rewrite an explicit zero-iteration dispatch."
            ),
        }),
        Some(iterations) => Ok(iterations),
        None => Ok(1),
    }
}

/// Resolve fixpoint iterations as a host `usize`.
///
/// Backends with host loops use this helper after the shared zero-iteration
/// validation so conversion behavior is also centralized.
///
/// # Errors
///
/// Returns [`BackendError::InvalidProgram`] when the resolved count is zero or
/// cannot fit host index space.
pub fn resolve_fixpoint_iterations_usize(
    config: &DispatchConfig,
    backend: &str,
) -> Result<usize, BackendError> {
    let iterations = resolve_fixpoint_iterations(config, backend)?;
    usize::try_from(iterations).map_err(|source| BackendError::InvalidProgram {
        fix: format!(
            "Fix: {backend} fixpoint iteration count {iterations} cannot fit usize: {source}. Lower fixpoint_iterations or split the dispatch into bounded phases."
        ),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_fixpoint_iterations_resolve_to_one() {
        assert_eq!(
            resolve_fixpoint_iterations(&DispatchConfig::default(), "test")
                .expect("Fix: default fixpoint iterations must resolve"),
            1
        );
    }

    #[test]
    fn explicit_zero_fixpoint_iterations_fail_loudly() {
        let mut config = DispatchConfig::default();
        config.fixpoint_iterations = Some(0);
        let error = resolve_fixpoint_iterations(&config, "CUDA").unwrap_err();
        let rendered = error.to_string();
        assert!(
            rendered.contains("fixpoint_iterations must be at least 1")
                && rendered.contains("CUDA")
                && rendered.contains("zero-iteration dispatch"),
            "Fix: explicit zero fixpoint iterations must produce an actionable backend-specific error."
        );
    }

    #[test]
    fn generated_nonzero_fixpoint_iterations_roundtrip() {
        for iterations in 1..4096u32 {
            let mut config = DispatchConfig::default();
            config.fixpoint_iterations = Some(iterations);
            assert_eq!(
                resolve_fixpoint_iterations(&config, "generated")
                    .expect("Fix: nonzero fixpoint iteration count must resolve"),
                iterations
            );
            assert_eq!(
                resolve_fixpoint_iterations_usize(&config, "generated")
                    .expect("Fix: nonzero fixpoint iteration count must fit usize"),
                iterations as usize
            );
        }
    }
}
