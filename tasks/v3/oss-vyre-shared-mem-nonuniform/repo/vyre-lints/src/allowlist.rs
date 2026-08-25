//! Allowlist for the `raw_ir_in_libs` lint during migration.
//!
//! Each entry exempts one file path (relative to the workspace root)
//! from the lint. Removed when the file's lego-migration ticket lands.

use anyhow::{Context, Result};
use serde::Deserialize;
use std::collections::HashSet;
use std::path::Path;

#[derive(Debug, Deserialize)]
struct AllowlistFile {
    /// Workspace-root-relative paths to exempt.
    #[serde(default)]
    exempt_files: Vec<String>,
}

#[derive(Debug, Default)]
pub struct Allowlist {
    paths: HashSet<String>,
}

impl Allowlist {
    pub fn empty() -> Self {
        Self::default()
    }

    pub fn contains(&self, workspace_relative_path: &str) -> bool {
        self.paths.contains(workspace_relative_path)
    }

    pub fn len(&self) -> usize {
        self.paths.len()
    }

    pub fn is_empty(&self) -> bool {
        self.paths.is_empty()
    }
}

pub fn load(path: &Path) -> Result<Allowlist> {
    let bytes = crate::read_source_bounded(path)
        .with_context(|| format!("read allowlist {}", path.display()))?;
    let parsed: AllowlistFile =
        toml::from_str(&bytes).with_context(|| format!("parse allowlist {}", path.display()))?;
    Ok(Allowlist {
        paths: parsed.exempt_files.into_iter().collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_allowlist_contains_nothing() {
        let a = Allowlist::empty();
        assert!(!a.contains("vyre-libs/src/nn/attention/gqa_attention.rs"));
        assert_eq!(a.len(), 0);
        assert!(a.is_empty());
    }

    #[test]
    fn loaded_allowlist_contains_listed_paths() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("allowlist.toml");
        std::fs::write(
            &path,
            "exempt_files = [\n  \"vyre-libs/src/nn/attention/gqa_attention.rs\",\n  \"vyre-libs/src/visual/shadow/mod.rs\",\n]\n",
        )
        .unwrap();
        let a = load(&path).unwrap();
        assert!(a.contains("vyre-libs/src/nn/attention/gqa_attention.rs"));
        assert!(a.contains("vyre-libs/src/visual/shadow/mod.rs"));
        assert!(!a.contains("vyre-libs/src/nn/other.rs"));
        assert_eq!(a.len(), 2);
    }

    #[test]
    fn missing_allowlist_file_errors() {
        let r = load(Path::new("/nonexistent/path/allowlist.toml"));
        assert!(
            matches!(r, Err(_)),
            "load of nonexistent path must return Err, got Ok"
        );
    }

    #[test]
    fn malformed_allowlist_errors() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("allowlist.toml");
        std::fs::write(&path, "not valid toml at all = = =").unwrap();
        let r = load(&path);
        assert!(matches!(r, Err(_)));
    }
}
