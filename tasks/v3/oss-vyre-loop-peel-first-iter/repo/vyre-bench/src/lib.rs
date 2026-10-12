#![allow(unsafe_code)]

//! Benchmark runner library surface for the vyre engine.
//!
//! Provides the core CLI parse hooks, registry integration,
//! regression monitoring, and release matrices.

#[cfg(not(target_os = "macos"))]
use vyre_driver_cuda as _;
use vyre_driver_metal as _;
use vyre_driver_reference as _;
use vyre_driver_spirv as _;
use vyre_driver_wgpu as _;

/// Retain benchmark backend registration crates in the final binary.
pub fn link_benchmark_backend_registrations() {
    let cpu_ref_id = <vyre_driver_reference::CpuRefBackend as vyre_driver::VyreBackend>::id(
        &vyre_driver_reference::CpuRefBackend,
    );
    let wgpu_acquire: fn() -> Result<vyre_driver_wgpu::WgpuBackend, vyre_driver::BackendError> =
        vyre_driver_wgpu::WgpuBackend::acquire;
    let metal_acquire: fn()
        -> Result<Box<dyn vyre_driver::VyreBackend>, vyre_driver::BackendError> =
        vyre_driver_metal::acquire;
    let spirv_factory: fn()
        -> Result<Box<dyn vyre_driver::VyreBackend>, vyre_driver::BackendError> =
        vyre_driver_spirv::spirv_factory;
    std::hint::black_box(cpu_ref_id);
    std::hint::black_box(wgpu_acquire);
    std::hint::black_box(metal_acquire);
    std::hint::black_box(spirv_factory);
    #[cfg(not(target_os = "macos"))]
    {
        let cuda_factory: fn() -> Result<
            Box<dyn vyre_driver::VyreBackend>,
            vyre_driver::BackendError,
        > = vyre_driver_cuda::cuda_factory;
        std::hint::black_box(cuda_factory);
    }
}

/// API definitions for external benchmark drivers.
#[allow(missing_docs)]
pub mod api;
/// Reference test cases and standard regression suites.
#[allow(missing_docs)]
pub mod cases;
/// Command-line interface definition and argument parsing.
#[allow(missing_docs)]
pub mod cli;
/// Evolutionary solver benchmarks and auto-tuners.
#[allow(missing_docs)]
pub mod evolve;
/// Target device capability and telemetry probes.
#[allow(missing_docs)]
pub mod probes;
/// The benchmark registry and metadata catalog.
#[allow(missing_docs)]
pub mod registry;
/// Parity release matrix verification logic.
#[allow(missing_docs)]
pub mod release_matrix;
/// HTML/Markdown report formatting and artifact writing.
#[allow(missing_docs)]
pub mod report;
/// Context and thread coordination for the test runner.
#[allow(missing_docs)]
pub mod runner;
