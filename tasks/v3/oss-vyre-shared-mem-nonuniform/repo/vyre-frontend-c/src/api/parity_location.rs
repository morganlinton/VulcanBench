//! Normalized source-location and provenance model for parity comparison.

use std::path::{Component, Path, PathBuf};

/// One normalized source point.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ParitySourcePoint {
    /// Normalized source file path or clang pseudo-file such as `<built-in>`.
    pub file: String,
    /// One-based source line.
    pub line: u32,
    /// One-based source column.
    pub column: u32,
}

/// One normalized source span.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ParitySourceSpan {
    /// Inclusive span start.
    pub start: ParitySourcePoint,
    /// Exclusive span end when known; point spans use the same value as `start`.
    pub end: ParitySourcePoint,
}

/// Normalized source provenance for a parity fact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParitySourceProvenance {
    /// Expansion or primary source span.
    pub expansion: ParitySourceSpan,
    /// Macro spelling span when the fact came from macro expansion.
    pub spelling: Option<ParitySourceSpan>,
    /// Include-stack spans from including context to included location.
    pub include_stack: Vec<ParitySourceSpan>,
}

impl ParitySourcePoint {
    /// Parses a clang-style `file:line:column` point.
    #[must_use]
    pub fn parse_clang(raw: &str) -> Option<Self> {
        let mut pieces = raw.rsplitn(3, ':');
        let column = pieces.next()?.parse::<u32>().ok()?;
        let line = pieces.next()?.parse::<u32>().ok()?;
        let file = normalize_source_file(pieces.next()?);
        Some(Self { file, line, column })
    }
}

impl ParitySourceSpan {
    /// Creates a point span from one source point.
    #[must_use]
    pub fn point(point: ParitySourcePoint) -> Self {
        Self {
            start: point.clone(),
            end: point,
        }
    }

    /// Parses a clang-style point span.
    #[must_use]
    pub fn parse_clang_point(raw: &str) -> Option<Self> {
        ParitySourcePoint::parse_clang(raw).map(Self::point)
    }
}

impl ParitySourceProvenance {
    /// Builds provenance from clang primary, optional spelling, and include-stack locations.
    #[must_use]
    pub fn from_clang_locations(
        expansion: &str,
        spelling: Option<&str>,
        include_stack: impl IntoIterator<Item = impl AsRef<str>>,
    ) -> Option<Self> {
        Some(Self {
            expansion: ParitySourceSpan::parse_clang_point(expansion)?,
            spelling: spelling.and_then(ParitySourceSpan::parse_clang_point),
            include_stack: include_stack
                .into_iter()
                .filter_map(|raw| ParitySourceSpan::parse_clang_point(raw.as_ref()))
                .collect(),
        })
    }
}

/// Normalizes a source file path for parity comparisons.
#[must_use]
pub fn normalize_source_file(raw: &str) -> String {
    if raw.starts_with('<') && raw.ends_with('>') {
        return raw.to_string();
    }
    let path = Path::new(raw);
    if let Ok(canonical) = std::fs::canonicalize(path) {
        return canonical.to_string_lossy().into_owned();
    }
    normalize_existing_independent_path(path)
        .to_string_lossy()
        .into_owned()
}

fn normalize_existing_independent_path(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            Component::Normal(part) => normalized.push(part),
            Component::RootDir | Component::Prefix(_) => normalized.push(component.as_os_str()),
        }
    }
    normalized
}
