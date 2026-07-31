//! Backend-neutral AOT emission and launcher registries.

use std::collections::BTreeMap;
use std::path::PathBuf;

use crate::{BackendError, DispatchConfig};
use vyre_foundation::ir::Program;

/// Stable AOT target identifier.
pub type AotTargetId = &'static str;

/// One backend-owned AOT emitter.
pub struct AotEmitter {
    /// Stable target identifier.
    pub target: AotTargetId,
    /// Emit target-native bytes for `program`.
    pub emit: fn(&Program, &DispatchConfig) -> Result<Vec<u8>, String>,
}

inventory::collect!(AotEmitter);

/// One dependency entry required by a generated launcher crate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LauncherDependency {
    /// Dependency name in the emitted `Cargo.toml`.
    pub name: &'static str,
    /// Inline dependency spec, for example `{ version = "1", features = ["derive"] }`.
    pub spec: &'static str,
}

/// Backend-neutral launcher emission request.
#[derive(Debug)]
pub struct AotLauncherRequest<'a> {
    /// Stable target id matching [`AotEmitter::target`].
    pub target: AotTargetId,
    /// Generated launcher crate name.
    pub crate_name: &'a str,
    /// Whether to include target-owned collective/multi-rank support.
    pub include_collectives: bool,
    /// Whether to include a built-in eval-time training loop.
    pub include_ttt_loop: bool,
}

/// Source files and manifest additions produced by a target-owned launcher emitter.
#[derive(Debug, Clone, Default)]
pub struct AotLauncherFiles {
    /// Additional dependencies required by target-specific launcher files.
    pub dependencies: Vec<LauncherDependency>,
    /// Source files keyed by launcher-crate-relative path.
    pub files: BTreeMap<PathBuf, String>,
}

impl AotLauncherFiles {
    /// Build launcher files from a fixed backend emission list.
    ///
    /// Backends should emit files in a deterministic order and delegate the
    /// final path-keyed container construction here instead of open-coding
    /// per-backend map assembly.
    #[must_use]
    pub fn from_entries(
        dependencies: Vec<LauncherDependency>,
        entries: impl IntoIterator<Item = (PathBuf, String)>,
    ) -> Self {
        Self {
            dependencies,
            files: entries.into_iter().collect(),
        }
    }
}

/// One backend-owned launcher source emitter.
pub struct AotLauncherEmitter {
    /// Stable target identifier.
    pub target: AotTargetId,
    /// Emit target-owned launcher files for `request`.
    pub emit: fn(&AotLauncherRequest<'_>) -> Result<AotLauncherFiles, String>,
}

inventory::collect!(AotLauncherEmitter);

/// Return every linked AOT emitter.
#[must_use]
pub fn registered_aot_emitters() -> Vec<&'static AotEmitter> {
    let emitter_count = inventory::iter::<AotEmitter>.into_iter().count();
    let mut emitters = Vec::new();
    let _ = emitters.try_reserve_exact(emitter_count);
    emitters.extend(inventory::iter::<AotEmitter>);
    emitters
}

/// Return every linked launcher emitter.
#[must_use]
pub fn registered_aot_launcher_emitters() -> Vec<&'static AotLauncherEmitter> {
    let emitter_count = inventory::iter::<AotLauncherEmitter>.into_iter().count();
    let mut emitters = Vec::new();
    let _ = emitters.try_reserve_exact(emitter_count);
    emitters.extend(inventory::iter::<AotLauncherEmitter>);
    emitters
}

/// Emit target-native bytes through the linked emitter matching `target`.
///
/// # Errors
///
/// Returns [`BackendError::UnsupportedFeature`] when no linked backend owns
/// `target`, or [`BackendError::KernelCompileFailed`] when the concrete
/// emitter rejects the program.
pub fn emit_aot_target(
    target: &str,
    program: &Program,
    config: &DispatchConfig,
) -> Result<Vec<u8>, BackendError> {
    let Some(emitter) = inventory::iter::<AotEmitter>
        .into_iter()
        .find(|emitter| emitter.target == target)
    else {
        return Err(BackendError::UnsupportedFeature {
            name: format!("aot target `{target}`"),
            backend: "vyre-driver".to_string(),
        });
    };
    (emitter.emit)(program, config).map_err(|compiler_message| BackendError::KernelCompileFailed {
        backend: target.to_string(),
        compiler_message,
    })
}

/// Emit target-owned launcher files through the linked emitter matching `target`.
///
/// # Errors
///
/// Returns [`BackendError::UnsupportedFeature`] when no linked backend owns
/// launcher generation for `target`, or [`BackendError::KernelCompileFailed`]
/// when the concrete launcher emitter rejects the request.
pub fn emit_aot_launcher_target(
    target: &str,
    request: &AotLauncherRequest<'_>,
) -> Result<AotLauncherFiles, BackendError> {
    let Some(emitter) = inventory::iter::<AotLauncherEmitter>
        .into_iter()
        .find(|emitter| emitter.target == target)
    else {
        return Err(BackendError::UnsupportedFeature {
            name: format!("aot launcher target `{target}`"),
            backend: "vyre-driver".to_string(),
        });
    };
    (emitter.emit)(request).map_err(|compiler_message| BackendError::KernelCompileFailed {
        backend: target.to_string(),
        compiler_message,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn launcher_files_constructor_centralizes_path_keyed_container_assembly() {
        let files = AotLauncherFiles::from_entries(
            vec![LauncherDependency {
                name: "libc",
                spec: "\"0.2\"",
            }],
            [
                (PathBuf::from("src/main.rs"), String::from("fn main() {}")),
                (PathBuf::from("src/cuda_ffi.rs"), String::from("mod ffi {}")),
            ],
        );

        assert_eq!(files.dependencies.len(), 1);
        assert_eq!(files.files.len(), 2);
        assert_eq!(
            files.files[&PathBuf::from("src/main.rs")],
            "fn main() {}",
            "Fix: launcher file construction must preserve emitted file contents while centralizing the map-shaped public API."
        );
    }
}
