//! Lowering stage: thin orchestrator over the `vyre-libs` lowering substrate.

use vyre::ir::Program;
use vyre_libs::parsing::rust::lower as rust_lower;
use vyre_libs::parsing::rust::parse::Module;
use vyre_libs::parsing::rust::sema::Resolution;

use crate::RustFrontendError;

/// Lower a resolved module to Vyre IR via the reusable lowering substrate.
pub fn lower(
    module: &Module,
    resolution: &Resolution,
    lane_count: Option<u32>,
) -> Result<Program, RustFrontendError> {
    let result = match lane_count {
        Some(lanes) => rust_lower::lower_batched(module, resolution, lanes),
        None => rust_lower::lower(module, resolution),
    };
    result.map_err(|e| RustFrontendError::Lower(e.to_string()))
}
