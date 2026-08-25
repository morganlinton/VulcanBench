//! Frozen backend intrinsic-name tables for Category C operations.

use crate::BackendId;

/// One backend-specific intrinsic spelling.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntrinsicLowering {
    /// Opaque backend id owned by a concrete driver crate.
    pub backend: BackendId,
    /// Intrinsic or instruction spelling used by that backend.
    pub name: &'static str,
}

impl IntrinsicLowering {
    /// Construct a backend intrinsic spelling row.
    #[must_use]
    pub fn new(backend: impl Into<BackendId>, name: &'static str) -> Self {
        Self {
            backend: backend.into(),
            name,
        }
    }
}

/// Backend intrinsic names for a Category C operation in the frozen contract.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct IntrinsicTable {
    /// Backend-specific spellings registered by concrete driver crates.
    pub lowerings: Vec<IntrinsicLowering>,
}

impl IntrinsicTable {
    /// Return whether `backend` has a non-empty intrinsic spelling.
    #[must_use]
    pub fn has_backend(&self, backend: &BackendId) -> bool {
        self.lowerings
            .iter()
            .any(|row| row.backend == *backend && !intrinsic_name_is_empty(Some(row.name)))
    }

    /// Return the missing backend ids from a caller-supplied required set.
    pub fn missing_backends<'a>(
        &'a self,
        required: &'a [BackendId],
    ) -> impl Iterator<Item = &'a str> + 'a {
        required
            .iter()
            .filter(|backend| !self.has_backend(backend))
            .map(BackendId::as_str)
    }
}

fn intrinsic_name_is_empty(value: Option<&str>) -> bool {
    value.map(str::trim).unwrap_or_default().is_empty()
}
