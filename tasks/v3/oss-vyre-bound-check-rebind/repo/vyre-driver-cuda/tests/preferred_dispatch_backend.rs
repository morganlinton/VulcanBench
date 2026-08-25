//! Integration test crate for the containing Vyre package.

use vyre_driver_cuda::CudaBackendRegistration;

mod preferred_dispatch_contract {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../tests/support/preferred_dispatch_backend_contract.rs"
    ));
}
use preferred_dispatch_contract::{
    assert_backend_registry_metadata, assert_preferred_dispatch_selects,
};

#[test]
fn cuda_registry_metadata_is_release_preferred_dispatch() {
    assert_backend_registry_metadata::<CudaBackendRegistration>(
        "cuda",
        5,
        "Fix: CUDA backend must register dispatches=true for release dispatch selection",
        "Fix: CUDA must keep canonical release precedence rank 5",
    );
}

#[test]
fn preferred_dispatch_backend_selects_cuda_not_reference_oracle() {
    assert_preferred_dispatch_selects::<CudaBackendRegistration>(
        "cuda",
        "Fix: preferred dispatch acquisition must initialize the linked CUDA backend on the local GPU host.",
        "Fix: CUDA has release precedence when linked",
    );
}
