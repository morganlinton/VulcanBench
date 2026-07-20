//! Inventory streams contributed by linked backend crates.

use std::collections::HashSet;

use vyre_foundation::ir::OpId;

use super::grid_sync_split::wrap_grid_sync_split;
use crate::backend::{BackendError, VyreBackend};

/// One backend constructor contributed by a linked backend crate.
///
/// Backend construction can fail (missing GPU adapter, unsupported driver),
/// so the factory returns a [`BackendError`] rather than panicking. Callers
/// iterate [`registered_backends`] and skip backends whose factory fails on
/// this host.
pub struct BackendRegistration {
    /// Stable backend identifier, matching [`VyreBackend::id`].
    pub id: &'static str,
    /// Factory that constructs the backend implementation.
    ///
    /// Returns `Err(BackendError)` when the backend cannot initialize on
    /// this host. The error message must include a `Fix:` remediation section
    /// per the frozen `BackendError` contract.
    pub factory: fn() -> Result<Box<dyn VyreBackend>, BackendError>,
    /// Operation ids supported by this backend.
    pub supported_ops: fn() -> &'static HashSet<OpId>,
}

impl BackendRegistration {
    /// Construct this registered backend through the shared driver boundary.
    ///
    /// This preserves the raw factory ABI while ensuring registration-based
    /// callers receive the same dispatch wrapper as [`crate::backend::acquire`]
    /// and [`crate::backend::acquire_preferred_dispatch_backend`].
    ///
    /// # Errors
    ///
    /// Returns the backend factory error when the concrete backend cannot
    /// initialize on this host.
    pub fn acquire(&self) -> Result<Box<dyn VyreBackend>, BackendError> {
        (self.factory)().map(wrap_grid_sync_split)
    }
}

inventory::collect!(BackendRegistration);

/// Per-backend precedence rank registered alongside its
/// [`BackendRegistration`]. Lower rank wins in router selection.
///
/// Conventional ranks are backend-owned. A backend that does not submit a
/// `BackendPrecedence` entry is treated as `u32::MAX`.
pub struct BackendPrecedence {
    /// Backend identifier; must match the corresponding
    /// [`BackendRegistration::id`].
    pub id: &'static str,
    /// Sort key. Lower means higher priority.
    pub rank: u32,
}

inventory::collect!(BackendPrecedence);

/// Backend capability declaration: whether a backend owns a live dispatch
/// stack on this host.
pub struct BackendCapability {
    /// Backend identifier; must match the corresponding
    /// [`BackendRegistration::id`].
    pub id: &'static str,
    /// `true` when this backend's `dispatch` can execute a Program and return
    /// real outputs; `false` when the backend is emission-only.
    pub dispatches: bool,
}

inventory::collect!(BackendCapability);

/// Return all backend registrations linked into the current binary.
///
/// Iteration order is unspecified. Callers that need a specific backend
/// should look it up by [`BackendRegistration::id`].
///
/// # Runtime cost
///
/// First call walks the link-time inventory and freezes the result into a
/// process-wide `OnceLock<Box<[&'static BackendRegistration]>>`. Every
/// subsequent call is one atomic load and returns the frozen slice with
/// zero allocation.
#[must_use]
pub fn registered_backends() -> &'static [&'static BackendRegistration] {
    static FROZEN: std::sync::OnceLock<Box<[&'static BackendRegistration]>> =
        std::sync::OnceLock::new();
    FROZEN.get_or_init(|| {
        // HOT-PATH-OK: inventory::iter runs only during OnceLock
        // initialization; registered_backends returns the frozen slice after
        // first access.
        let registration_count = inventory::iter::<BackendRegistration>.into_iter().count();
        let mut registrations = Vec::new();
        let _ = registrations.try_reserve_exact(registration_count);
        // HOT-PATH-OK: this second inventory walk materializes the same
        // init-only frozen backend slice after capacity has been reserved.
        registrations.extend(inventory::iter::<BackendRegistration>);
        registrations.into_boxed_slice()
    })
}
