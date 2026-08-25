//! Name resolution stage: thin orchestrator over the `vyre-libs` sema substrate.

use vyre_libs::parsing::rust::parse::Module;
use vyre_libs::parsing::rust::sema::{self, Resolution};

use crate::RustFrontendError;

/// Resolve names in a module against `source`, via the reusable sema substrate.
pub fn resolve(module: &Module, source: &[u8]) -> Result<Resolution, RustFrontendError> {
    sema::resolve(module, source).map_err(|e| RustFrontendError::Resolve(e.to_string()))
}
