//! WGPU backend resident-buffer API contracts.

use vyre_driver::{Resource, VyreBackend};
use vyre_driver_wgpu::WgpuBackend;

fn backend_impl_source() -> String {
    std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/backend_impl.rs"))
        .expect("Fix: resident-buffer contract must read WGPU backend implementation source")
}

fn resident_upload_source() -> String {
    std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/resident_upload.rs"
    ))
    .expect("Fix: resident-buffer contract must read WGPU resident upload implementation source")
}

fn resident_download_source() -> String {
    std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/resident_download.rs"
    ))
    .expect("Fix: resident-buffer contract must read WGPU resident download implementation source")
}

fn resident_resource_source() -> String {
    std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/resident_resource.rs"
    ))
    .expect("Fix: resident-buffer contract must read WGPU resident resource implementation source")
}

fn buffer_handle_source() -> String {
    std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/buffer/handle.rs"))
        .expect("Fix: resident-buffer contract must read WGPU buffer handle implementation source")
}

fn record_and_readback_source() -> String {
    std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/engine/record_and_readback/readback.rs"
    ))
    .expect("Fix: resident-buffer contract must read WGPU record-and-readback collector source")
}

fn readback_ring_source() -> String {
    std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/runtime/readback_ring.rs"
    ))
    .expect("Fix: resident-buffer contract must read WGPU readback ring source")
}

fn backend() -> WgpuBackend {
    WgpuBackend::new().expect(
        "Fix: live WGPU backend required for resident-buffer contracts; missing GPU is a configuration bug.",
    )
}

#[path = "resident_buffer_contracts/source_shape_contracts.rs"]
mod source_shape_contracts;
#[path = "resident_buffer_contracts/lifecycle_range_contracts.rs"]
mod lifecycle_range_contracts;
#[path = "resident_buffer_contracts/ranged_batch_contracts.rs"]
mod ranged_batch_contracts;
#[path = "resident_buffer_contracts/validation_atomicity_contracts.rs"]
mod validation_atomicity_contracts;
