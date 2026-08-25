//! Borrow-checking stage: thin orchestrator over the `vyre-libs` sema substrate.
//!
//! The borrow analysis (mutability rule now; CFG + dataflow conflict rules via
//! weir later) is language-specific substrate in `vyre-libs::parsing::rust::sema`.
//! This stage only orchestrates it.

use vyre_libs::parsing::rust::parse::Module;
use vyre_libs::parsing::rust::sema::{self, Resolution};

use crate::RustFrontendError;

/// Borrow-check a resolved module via the reusable sema substrate.
pub fn borrow_check(module: &Module, resolution: &Resolution) -> Result<(), RustFrontendError> {
    sema::borrow_check(module, resolution).map_err(|e| RustFrontendError::Borrow(e.to_string()))
}
