//! Runtime TOML dialect loader (A-B5).
//!
//! The Rust path registers ops at link time via `inventory::submit!`.
//! Some consumers  -  DSL authors, CVE-rule contributors, community
//! Nuclei-template-style writers  -  want to drop a TOML file in a
//! directory and have the runtime pick it up without recompiling.
//!
//! This module provides that mechanism for the **metadata** part of
//! a dialect: op id, dialect name, category, signature, laws. The
//! behavioral part (`cpu_ref`, `primary_text`, etc.) still comes from
//! Rust because TOML can't declaratively describe a compute kernel.
//! External dialect crates can thus ship the behavioral half as Rust
//! and the declarative half as TOML  -  the TOML supports community
//! contributions of new rule-like ops whose behavior is composed from
//! existing primitives.
//!
//! Load sequence:
//!
//! 1. `VYRE_DIALECT_PATH` colon-separated directories are scanned for
//!    `*.toml` files.
//! 2. Each file is parsed against the [`DialectManifest`] schema.
//! 3. Manifests are registered into an in-memory [`TomlDialectStore`].
//! 4. Consumers query the store via [`TomlDialectStore::dialect`] and
//!    [`TomlDialectStore::ops_in`].
//!
//! Runtime TOML ops are *additive*  -  they don't override an
//! inventory-registered OpDef with the same id. A conflict is
//! surfaced as a Diagnostic so downstream consumers can disambiguate.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::diagnostics::{Diagnostic, DiagnosticCode};

const DIALECT_PATH_ENV: &str = "VYRE_DIALECT_PATH";
const MAX_DIALECT_TOML_BYTES: u64 = 1024 * 1024;

/// Top-level TOML schema for a dialect manifest.
///
/// Every file in `VYRE_DIALECT_PATH` is parsed into one of these.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DialectManifest {
    /// Dialect identifier (e.g. `"community.cve_rules"`).
    pub dialect: String,
    /// Version of this dialect revision (semver string).
    pub version: String,
    /// Optional human-readable note surfaced in diagnostics.
    #[serde(default)]
    pub description: Option<String>,
    /// List of ops.
    #[serde(default)]
    pub ops: Vec<OpManifest>,
}

/// Per-op TOML entry.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OpManifest {
    /// Fully qualified op id (`<dialect>.<name>`).
    pub id: String,
    /// Category  -  `"A"`, `"B"`, or `"C"`.
    pub category: String,
    /// Optional free-form summary (one line) surfaced in catalogs.
    #[serde(default)]
    pub summary: Option<String>,
    /// Declarative input list  -  `(name, type)` pairs.
    #[serde(default)]
    pub inputs: Vec<(String, String)>,
    /// Declarative output list  -  `(name, type)` pairs.
    #[serde(default)]
    pub outputs: Vec<(String, String)>,
    /// Algebraic-law tags the op claims to satisfy.
    #[serde(default)]
    pub laws: Vec<String>,
}

/// In-memory store of every TOML-loaded dialect manifest.
#[derive(Debug, Default, Clone)]
pub struct TomlDialectStore {
    manifests: BTreeMap<String, DialectManifest>,
    diagnostics: Vec<Diagnostic>,
}

impl TomlDialectStore {
    /// Build a store by scanning the directories in
    /// `VYRE_DIALECT_PATH`. The env var itself is optional, but every
    /// configured entry must resolve to a readable directory; missing
    /// rule roots are surfaced as diagnostics instead of being treated
    /// as an empty community knowledge database.
    #[must_use]
    pub fn from_env() -> Self {
        let mut store = Self::default();
        if let Some(path) = dialect_path_value() {
            for entry in path.split(':') {
                if entry.is_empty() {
                    store.diagnostics.push(
                        Diagnostic::error(
                            "E-TOML-EMPTY-DIALECT-PATH",
                            format!("{DIALECT_PATH_ENV} contains an empty path entry"),
                        )
                        .with_fix(
                            "remove empty entries or point them at an explicit dialect directory",
                        ),
                    );
                    continue;
                }
                let dir = Path::new(entry);
                if dir.is_dir() {
                    store.scan_dir(dir);
                } else {
                    store.diagnostics.push(
                        Diagnostic::error(
                            "E-TOML-DIALECT-DIR-MISSING",
                            format!(
                                "{DIALECT_PATH_ENV} entry `{}` is not a directory",
                                dir.display()
                            ),
                        )
                        .with_fix("create the directory, fix the path, or unset VYRE_DIALECT_PATH"),
                    );
                }
            }
        }
        store
    }

    /// Scan one directory for `*.toml` manifests. Invalid manifests
    /// surface as [`Diagnostic`]s attached to the store; the scan
    /// never short-circuits.
    pub fn scan_dir(&mut self, dir: &Path) {
        let Ok(entries) = fs::read_dir(dir) else {
            self.diagnostics.push(
                Diagnostic::error(
                    "E-TOML-DIALECT-DIR-UNREADABLE",
                    format!("TOML dialect directory `{}` is unreadable", dir.display()),
                )
                .with_fix(
                    "fix directory permissions or remove the directory from VYRE_DIALECT_PATH",
                ),
            );
            return;
        };
        for entry in entries {
            let path = match entry {
                Ok(entry) => entry.path(),
                Err(error) => {
                    self.diagnostics.push(
                        Diagnostic::error(
                            "E-TOML-DIALECT-DIR-ENTRY",
                            format!(
                                "failed to read an entry from TOML dialect directory `{}`: {error}",
                                dir.display()
                            ),
                        )
                        .with_fix("fix directory permissions or remove the unreadable entry"),
                    );
                    continue;
                }
            };
            if path.extension().and_then(|s| s.to_str()) != Some("toml") {
                continue;
            }
            self.load_file(&path);
        }
    }

    /// Load a single TOML file. Errors become diagnostics; the
    /// function never panics.
    pub fn load_file(&mut self, path: &Path) {
        let Ok(contents) = read_toml_bounded(path) else {
            self.diagnostics.push(
                Diagnostic::error(
                    "E-TOML-UNREADABLE",
                    format!("TOML dialect file `{}` is unreadable", path.display()),
                )
                .with_fix("confirm file permissions and that VYRE_DIALECT_PATH points at an intended directory"),
            );
            return;
        };
        match toml::from_str::<DialectManifest>(&contents) {
            Ok(mut manifest) => {
                if manifest.dialect.trim().is_empty() {
                    self.diagnostics.push(
                        Diagnostic::error(
                            "E-TOML-EMPTY-DIALECT",
                            format!(
                                "TOML dialect file `{}` has an empty dialect id",
                                path.display()
                            ),
                        )
                        .with_fix("set `dialect` to a stable non-empty identifier"),
                    );
                    return;
                }
                if manifest.version.trim().is_empty() {
                    self.diagnostics.push(
                        Diagnostic::error(
                            "E-TOML-EMPTY-VERSION",
                            format!(
                                "dialect manifest `{}` has an empty version",
                                manifest.dialect
                            ),
                        )
                        .with_fix("set `version` to a stable non-empty version string"),
                    );
                    return;
                }
                // Sanity pass: every op id must begin with the
                // dialect prefix. Reject the whole manifest on any
                // invalid op id so a partial community rule database
                // cannot load with missing operations.
                let mut invalid_manifest = false;
                let mut seen_ops = BTreeSet::new();
                manifest.ops.retain(|op| {
                    let mut keep = true;
                    if !op.id.starts_with(&format!("{}.", manifest.dialect)) {
                        invalid_manifest = true;
                        self.diagnostics.push(
                            Diagnostic::error(
                                "E-TOML-BAD-OP-ID",
                                format!(
                                    "op id `{}` does not start with dialect prefix `{}.`",
                                    op.id, manifest.dialect
                                ),
                            )
                            .with_fix("rename the op to `<dialect>.<name>`"),
                        );
                        keep = false;
                    }
                    if !matches!(op.category.as_str(), "A" | "B" | "C") {
                        invalid_manifest = true;
                        self.diagnostics.push(
                            Diagnostic::error(
                                "E-TOML-BAD-CATEGORY",
                                format!("op `{}` has invalid category `{}`", op.id, op.category),
                            )
                            .with_fix("set category to `A`, `B`, or `C`"),
                        );
                        keep = false;
                    }
                    if keep && !seen_ops.insert(op.id.clone()) {
                        invalid_manifest = true;
                        self.diagnostics.push(
                            Diagnostic::error(
                                "E-TOML-DUPLICATE-OP",
                                format!(
                                    "dialect manifest `{}` declares op `{}` more than once",
                                    manifest.dialect, op.id
                                ),
                            )
                            .with_fix("keep one declaration per stable op id"),
                        );
                        keep = false;
                    }
                    keep
                });
                if invalid_manifest {
                    self.diagnostics.push(
                        Diagnostic::error(
                            "E-TOML-MANIFEST-REJECTED",
                            format!(
                                "dialect manifest `{}` was rejected because one or more op declarations are invalid",
                                manifest.dialect
                            ),
                        )
                        .with_fix("fix every op declaration in the manifest before loading this dialect"),
                    );
                    return;
                }
                // Keep the highest-versioned manifest per dialect, but only
                // after validating this file. Losing version selection must
                // not hide malformed community metadata.
                if let Some(existing) = self.manifests.get(&manifest.dialect) {
                    if existing.version >= manifest.version {
                        self.diagnostics.push(Diagnostic::note(
                            "N-TOML-DIALECT-SHADOWED",
                            format!(
                                "dialect `{}` has multiple manifests; keeping version {} over {}",
                                manifest.dialect, existing.version, manifest.version
                            ),
                        ));
                        return;
                    }
                    self.diagnostics.push(Diagnostic::note(
                        "N-TOML-DIALECT-SHADOWED",
                        format!(
                            "dialect `{}` has multiple manifests; keeping version {} over {}",
                            manifest.dialect, manifest.version, existing.version
                        ),
                    ));
                }
                self.manifests.insert(manifest.dialect.clone(), manifest);
            }
            Err(err) => {
                self.diagnostics.push(
                    Diagnostic::error(
                        "E-TOML-PARSE",
                        format!("TOML dialect file `{}` is malformed: {err}", path.display()),
                    )
                    .with_fix("validate the file against the DialectManifest schema"),
                );
            }
        }
    }

    /// Look up one dialect manifest by name.
    #[must_use]
    pub fn dialect(&self, id: &str) -> Option<&DialectManifest> {
        self.manifests.get(id)
    }

    /// List every op declared in the given dialect.
    #[must_use]
    pub fn ops_in(&self, dialect: &str) -> &[OpManifest] {
        self.manifests
            .get(dialect)
            .map(|m| m.ops.as_slice())
            .unwrap_or(&[])
    }

    /// Return every loaded dialect manifest.
    #[must_use]
    pub fn manifests(&self) -> Vec<&DialectManifest> {
        let mut manifests = Vec::new();
        vyre_foundation::allocation::try_reserve_vec_to_capacity(
            &mut manifests,
            self.manifests.len(),
        )
        .ok();
        manifests.extend(self.manifests.values());
        manifests
    }

    /// Diagnostics accumulated during load.
    #[must_use]
    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }

    /// Convenience: check whether a given op id is declared by any
    /// loaded TOML manifest.
    #[must_use]
    pub fn contains_op(&self, op_id: &str) -> bool {
        self.manifests
            .values()
            .any(|m| m.ops.iter().any(|op| op.id == op_id))
    }
}

fn dialect_path_value() -> Option<String> {
    #[cfg(test)]
    {
        if let Some(path) = dialect_path_override()
            .lock()
            .ok()
            .and_then(|guard| guard.clone())
        {
            return Some(path);
        }
    }
    std::env::var(DIALECT_PATH_ENV).ok()
}

#[cfg(test)]
fn dialect_path_override() -> &'static std::sync::Mutex<Option<String>> {
    static OVERRIDE: std::sync::OnceLock<std::sync::Mutex<Option<String>>> =
        std::sync::OnceLock::new();
    OVERRIDE.get_or_init(|| std::sync::Mutex::new(None))
}

fn read_toml_bounded(path: &Path) -> std::io::Result<String> {
    use std::io::Read as _;

    let mut file = fs::File::open(path)?;
    let metadata = file.metadata()?;
    if metadata.len() > MAX_DIALECT_TOML_BYTES {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("dialect TOML exceeds {MAX_DIALECT_TOML_BYTES} byte limit"),
        ));
    }
    let mut text = String::with_capacity(metadata.len() as usize);
    file.by_ref()
        .take(MAX_DIALECT_TOML_BYTES + 1)
        .read_to_string(&mut text)?;
    if text.len() as u64 > MAX_DIALECT_TOML_BYTES {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "dialect TOML exceeded bounded read limit",
        ));
    }
    Ok(text)
}

/// Stable diagnostic code family for TOML loader issues. Tooling
/// hangs rules off these codes; do not rename.
pub const CODE_PARSE: DiagnosticCode = DiagnosticCode(std::borrow::Cow::Borrowed("E-TOML-PARSE"));

/// Compute an absolute path relative to the workspace root. Handy
/// for tests that stage TOML fixtures under `tests/fixtures`.
#[must_use]
pub fn workspace_dialect_fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("dialect")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn write_tmp(contents: &str) -> tempfile::NamedTempFile {
        // Many tests need a .toml file on disk; NamedTempFile
        // exposes its path and cleans up on drop.
        let mut file = tempfile::Builder::new()
            .suffix(".toml")
            .tempfile()
            .expect("Fix: tmp file");
        file.write_all(contents.as_bytes()).expect("Fix: write");
        file.flush().expect("Fix: flush");
        file
    }

    #[test]
    fn parses_minimal_dialect() {
        let file = write_tmp(
            r#"
dialect = "community.test"
version = "1.0.0"
ops = [
  { id = "community.test.pass", category = "A", summary = "no-op" },
]
"#,
        );
        let mut store = TomlDialectStore::default();
        store.load_file(file.path());
        assert!(store.dialect("community.test").is_some());
        assert_eq!(store.ops_in("community.test").len(), 1);
        assert!(store.contains_op("community.test.pass"));
        assert_eq!(store.diagnostics().len(), 0);
    }

    #[test]
    fn rejects_mismatched_op_prefix_with_diagnostic() {
        let file = write_tmp(
            r#"
dialect = "community.test"
version = "1.0.0"
ops = [
  { id = "other.not_my_dialect", category = "A" },
]
"#,
        );
        let mut store = TomlDialectStore::default();
        store.load_file(file.path());
        assert_eq!(store.ops_in("community.test").len(), 0);
        assert!(store
            .diagnostics()
            .iter()
            .any(|d| d.code.as_str() == "E-TOML-BAD-OP-ID"));
    }

    #[test]
    fn malformed_toml_produces_parse_error_diagnostic() {
        let file = write_tmp("not-toml-at-all =");
        let mut store = TomlDialectStore::default();
        store.load_file(file.path());
        assert!(store
            .diagnostics()
            .iter()
            .any(|d| d.code.as_str() == "E-TOML-PARSE"));
    }

    #[test]
    fn invalid_op_category_rejects_manifest() {
        let file = write_tmp(
            r#"
dialect = "community.bad_category"
version = "1.0.0"
ops = [ { id = "community.bad_category.scan", category = "Z" } ]
"#,
        );
        let mut store = TomlDialectStore::default();
        store.load_file(file.path());
        assert!(store.dialect("community.bad_category").is_none());
        assert!(store
            .diagnostics()
            .iter()
            .any(|d| d.code.as_str() == "E-TOML-BAD-CATEGORY"));
        assert!(store
            .diagnostics()
            .iter()
            .any(|d| d.code.as_str() == "E-TOML-MANIFEST-REJECTED"));
    }

    #[test]
    fn duplicate_manifest_op_rejects_manifest() {
        let file = write_tmp(
            r#"
dialect = "community.duplicate"
version = "1.0.0"
ops = [
  { id = "community.duplicate.scan", category = "B" },
  { id = "community.duplicate.scan", category = "B" },
]
"#,
        );
        let mut store = TomlDialectStore::default();
        store.load_file(file.path());
        assert!(store.dialect("community.duplicate").is_none());
        assert!(store
            .diagnostics()
            .iter()
            .any(|d| d.code.as_str() == "E-TOML-DUPLICATE-OP"));
    }

    #[test]
    fn shadowed_manifest_keeps_highest_version() {
        let older = write_tmp(
            r#"
dialect = "community.versioned"
version = "1.0.0"
ops = []
"#,
        );
        let newer = write_tmp(
            r#"
dialect = "community.versioned"
version = "2.0.0"
ops = [ { id = "community.versioned.new", category = "B" } ]
"#,
        );
        let mut store = TomlDialectStore::default();
        store.load_file(older.path());
        store.load_file(newer.path());
        assert_eq!(
            store.dialect("community.versioned").unwrap().version,
            "2.0.0"
        );
        assert_eq!(store.ops_in("community.versioned").len(), 1);
    }

    #[test]
    fn shadowed_manifest_is_still_validated() {
        let newer = write_tmp(
            r#"
dialect = "community.versioned_invalid"
version = "2.0.0"
ops = [ { id = "community.versioned_invalid.good", category = "B" } ]
"#,
        );
        let older_invalid = write_tmp(
            r#"
dialect = "community.versioned_invalid"
version = "1.0.0"
ops = [ { id = "wrong.bad", category = "B" } ]
"#,
        );
        let mut store = TomlDialectStore::default();
        store.load_file(newer.path());
        store.load_file(older_invalid.path());
        assert_eq!(store.ops_in("community.versioned_invalid").len(), 1);
        assert!(store
            .diagnostics()
            .iter()
            .any(|d| d.code.as_str() == "E-TOML-BAD-OP-ID"));
    }

    #[test]
    fn env_scan_reports_missing_directories() {
        // VYRE_DIALECT_PATH points at directories that do not exist.
        // Missing Tier-B rule roots are configuration errors: they
        // must not be silently mistaken for an empty knowledge base.
        *dialect_path_override()
            .lock()
            .expect("Fix: dialect path test override lock must not be poisoned") =
            Some("/no/such/dir:/also/not/real".to_string());
        let store = TomlDialectStore::from_env();
        assert!(store.manifests.is_empty());
        assert_eq!(
            store
                .diagnostics
                .iter()
                .filter(|d| d.code.as_str() == "E-TOML-DIALECT-DIR-MISSING")
                .count(),
            2
        );
        *dialect_path_override()
            .lock()
            .expect("Fix: dialect path test override lock must not be poisoned") = None;
    }

    #[test]
    fn code_constants_are_stable() {
        assert_eq!(CODE_PARSE.as_str(), "E-TOML-PARSE");
    }
}
