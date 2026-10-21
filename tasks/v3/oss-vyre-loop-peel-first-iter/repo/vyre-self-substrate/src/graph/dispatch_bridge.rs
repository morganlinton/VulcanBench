//! Shared graph primitive dispatch bridge.
//!
//! Graph self-substrate modules should own dispatch orchestration, not copied
//! primitive shape logic or repeated byte-buffer plumbing. This bridge keeps
//! the backend boundary in one place while each wrapper remains responsible for
//! selecting the primitive program and primitive-returned layout.

mod inputs;
mod resident;
mod u32_outputs;

#[cfg(test)]
mod tests;

pub(crate) use crate::hardware::dispatch_program_cache::ProgramCache;
pub(crate) use inputs::{
    fingerprint_u32_slice, refresh_keyed_dispatch_inputs, write_dispatch_input, DispatchInput,
    U32SliceFingerprint,
};
pub(crate) use resident::{
    alloc_resident_buffers, resident_dispatch_two_u32_outputs_into,
    resident_sequence_single_u32_output_into, upload_resident_dispatch_inputs,
};
pub(crate) use u32_outputs::{
    dispatch_single_u32_output_from_prepared_into, dispatch_two_u32_outputs_from_prepared_into,
};

use vyre_foundation::ir::Program;

/// Cached primitive dispatch program shared by graph wrappers.
///
/// Most graph facades cache exactly one specialized [`Program`] per primitive
/// layout key. Keeping this value type centralized prevents each wrapper from
/// defining its own one-field `Cached*Program` shell while preserving typed
/// keys at the cache boundary.
#[derive(Debug)]
pub(crate) struct CachedProgram {
    pub(crate) program: Program,
}
