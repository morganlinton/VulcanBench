//! Minimal release-surface example for the portable WGPU backend.

fn main() {
    println!(
        "WGPU backend type: {}",
        std::any::type_name::<vyre_driver_wgpu::WgpuBackend>()
    );
    println!("Acquire with vyre_driver_wgpu::WgpuBackend::acquire() when validating the portable GPU backend.");
}
