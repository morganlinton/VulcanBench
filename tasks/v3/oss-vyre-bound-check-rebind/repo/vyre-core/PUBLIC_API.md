pub mod vyre
pub use vyre::BackendError
pub use vyre::BackendRegistration
pub use vyre::ByteRange
pub use vyre::CompiledPipeline
pub use vyre::DispatchConfig
pub use vyre::Error
pub use vyre::Executable
pub use vyre::InterpCtx
pub use vyre::Match
pub use vyre::Memory
pub use vyre::MemoryOrdering
pub use vyre::MemoryRef
pub use vyre::NodeId
pub use vyre::NodeStorage
pub use vyre::OpId
pub use vyre::OutputBuffers
pub use vyre::PersistentThreadMode
pub use vyre::Program
pub use vyre::ResidentGraphReuseTelemetry
pub use vyre::ResidentGraphReuseTelemetryError
pub use vyre::SpeculationMode
pub use vyre::TypedDispatchExt
pub use vyre::Value
pub use vyre::VyreBackend
pub use vyre::backend
pub use vyre::diagnostics
pub use vyre::error
pub use vyre::execution_plan
pub use vyre::ir
pub use vyre::match_result
pub use vyre::memory_model
pub use vyre::optimizer
pub use vyre::pipeline
pub use vyre::routing
pub use vyre::soundness
pub use vyre::validate
pub mod vyre::lower
pub use vyre::lower::<<vyre_lower::*>>
pub use vyre::lower::lower
pub const vyre::OPTIMIZE_CACHE_CAPACITY: usize
pub fn vyre::optimize(program: vyre_foundation::ir_inner::model::program::core::Program) -> core::result::Result<vyre_foundation::ir_inner::model::program::core::Program, vyre_foundation::optimizer::OptimizerError>
pub fn vyre::optimize_for_backend(program: vyre_foundation::ir_inner::model::program::core::Program, backend: &dyn vyre_driver::backend::vyre_backend::VyreBackend) -> core::result::Result<vyre_foundation::ir_inner::model::program::core::Program, vyre_foundation::optimizer::OptimizerError>
pub fn vyre::optimize_for_device(program: vyre_foundation::ir_inner::model::program::core::Program, profile: &vyre_driver::device_profile::DeviceProfile) -> core::result::Result<vyre_foundation::ir_inner::model::program::core::Program, vyre_foundation::optimizer::OptimizerError>
