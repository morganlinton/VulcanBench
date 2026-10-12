#![allow(
    missing_docs,
    dead_code,
    unused_imports,
    unused_variables,
    unreachable_patterns,
    clippy::all
)]
use crate::paths::workspace_root;
use std::path::PathBuf;

/// Return the quick-check cache directory.
pub(crate) fn quick_cache_dir() -> PathBuf {
    workspace_root()
        .join("target")
        .join("vyre-quickcheck-cache")
}
