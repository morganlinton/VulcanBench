//! Integration test crate for the containing Vyre package.

#![allow(deprecated)]
use vyre_driver_wgpu::WgpuBackend;

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
fn wgpu_registry_metadata_is_dispatch_capable_gpu() {
    assert_backend_registry_metadata::<WgpuBackend>(
        "wgpu",
        30,
        "Fix: WGPU backend must register dispatches=true for preferred GPU dispatch selection",
        "Fix: WGPU precedence rank must stay below CUDA but above reference/oracle backends",
    );
}

#[test]
fn preferred_dispatch_backend_selects_wgpu_not_reference_oracle() {
    assert_preferred_dispatch_selects::<WgpuBackend>(
        "wgpu",
        "Fix: preferred dispatch acquisition must initialize the linked WGPU backend on the local GPU host.",
        "Fix: preferred dispatch must select the linked GPU backend",
    );
}
