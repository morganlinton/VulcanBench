use std::path::PathBuf;

/// Return the repository workspace root.
pub(crate) fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|path| path.parent())
        .map(PathBuf::from)
        .expect("Fix: vyre/xtask must live two levels below the workspace root.")
}
