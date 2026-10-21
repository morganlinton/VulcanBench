//! Type-checking stage: thin orchestrator over the `vyre-libs` sema substrate.

use vyre_libs::parsing::rust::parse::Module;
use vyre_libs::parsing::rust::sema::{self, Resolution};

use crate::RustFrontendError;

/// Type-check a resolved module via the reusable sema substrate.
pub fn typeck(
    module: &Module,
    source: &[u8],
    resolution: &Resolution,
) -> Result<(), RustFrontendError> {
    sema::typeck(module, source, resolution).map_err(|e| RustFrontendError::Typeck(e.to_string()))
}
