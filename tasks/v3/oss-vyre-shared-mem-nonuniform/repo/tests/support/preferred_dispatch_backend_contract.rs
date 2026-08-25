/// Shared preferred-dispatch backend registry contracts for driver crates.
use vyre_driver::backend::{
    acquire_preferred_dispatch_backend, backend_dispatches, backend_precedence,
};

pub(crate) fn assert_backend_registry_metadata<LinkRegistration: 'static>(
    backend_id: &str,
    expected_precedence: u32,
    dispatch_message: &str,
    precedence_message: &str,
) {
    let _link_inventory_registration = std::any::TypeId::of::<LinkRegistration>();

    assert!(backend_dispatches(backend_id), "{dispatch_message}");
    assert_eq!(
        backend_precedence(backend_id),
        expected_precedence,
        "{precedence_message}"
    );
}

pub(crate) fn assert_preferred_dispatch_selects<LinkRegistration: 'static>(
    expected_backend_id: &str,
    acquisition_message: &str,
    selection_message: &str,
) {
    let _link_inventory_registration = std::any::TypeId::of::<LinkRegistration>();
    let backend = acquire_preferred_dispatch_backend().expect(acquisition_message);
    let actual_id = backend.id();

    assert_eq!(
        actual_id, expected_backend_id,
        "{selection_message}; preferred dispatch got `{actual_id}`"
    );
    assert_ne!(actual_id, "reference");
    assert_ne!(actual_id, "cpu-ref");
}
