//! Link-time backend registry.
//!
//! Backend crates submit a [`BackendRegistration`] with `inventory::submit!`.
//! Applications discover every linked backend through [`registered_backends`]
//! without hardcoding crate-specific constructors. Because the registry lives
//! in `vyre-core` and is populated by downstream crates at link time, this
//! crate's own test sees an empty registry; downstream crates assert their
//! own presence.

mod acquire;
mod grid_sync_split;
mod inventory_streams;

pub use acquire::{
    acquire, acquire_preferred_dispatch_backend, backend_dispatches, backend_precedence,
    core_supported_ops, registered_backends_by_precedence, registered_backends_by_precedence_slice,
};
pub use inventory_streams::{
    registered_backends, BackendCapability, BackendPrecedence, BackendRegistration,
};
