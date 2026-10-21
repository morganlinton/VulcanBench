//! Frozen backend extension contract.
//!
//! Vyre treats GPU compute as a target-agnostic intermediate representation.
//! This module defines the narrow interface that every backend or the
//! pure-Rust reference interpreter must implement. Frontends
//! emit `Program` values without knowing which backend will execute them, and
//! backends compete on implementation quality without negotiating API changes.
//! The trait signature is frozen under the five-year stability contract from
//! `ARCHITECTURE.md`.

mod capability;
mod dialect_supported_ops;
pub mod lowering;
mod registry;
pub mod validation;

mod compiled_pipeline;
mod device_buffer;
mod dispatch_config;
mod dispatch_result;
mod error;
mod pending_dispatch;
mod regex_accelerator;
mod resource;
mod typed_dispatch;
mod vyre_backend;

pub use capability::{Backend, Executable, Memory, MemoryRef, Streamable};
pub use dialect_supported_ops::{dialect_and_language_supported_ops, dialect_only_supported_ops};
pub use registry::{
    acquire, acquire_preferred_dispatch_backend, backend_dispatches, backend_precedence,
    core_supported_ops, registered_backends, registered_backends_by_precedence,
    registered_backends_by_precedence_slice, BackendCapability, BackendPrecedence,
    BackendRegistration,
};
pub use validation::{
    default_supported_ops, default_supported_ops_with_trap, node_op_id, validate_program,
};
// `validate_program_for_backend` lives at the crate root in
// `crate::validation` (the cross-backend variant), not under the
// per-backend submodule. Re-export it here so legacy call sites that
// reach `vyre_driver::backend::validate_program_for_backend` keep
// resolving against the same path.
pub use crate::validation::validate_program_for_backend;

pub use compiled_pipeline::CompiledPipeline;
pub use device_buffer::{
    default_dispatch_with_device_buffers, validate_buffer_ownership, DeviceBuffer, HostShimBuffer,
    DEVICE_BUFFER_FEATURE,
};
pub use dispatch_config::DispatchConfig;
pub use dispatch_result::{
    replace_output_buffers_preserving_slots,
    replace_output_buffers_preserving_slots_with_memory_stats,
    replace_output_buffers_preserving_slots_with_stats, OutputBuffers, OutputReplacementStats,
    OutputSlotByteStats, OutputSlotStats, TimedDispatchResult,
};
pub use error::{BackendError, ErrorCode};
pub use pending_dispatch::PendingDispatch;
pub use regex_accelerator::{
    RegexAcceleratorCapability, RegexAcceleratorClass, RegexAcceleratorEvidence,
    RegexAcceleratorMatchSchema, RegexAcceleratorStreamMode,
    REGEX_ACCELERATOR_EVIDENCE_SCHEMA_VERSION,
};
pub use resource::Resource;
pub use typed_dispatch::TypedDispatchExt;
pub use vyre_backend::{
    ResidentDispatchStep, ResidentReadRange, ResidentSequenceTiming, VyreBackend,
};

#[doc(hidden)]
pub mod private {
    pub trait Sealed {}
}

pub(crate) fn clone_borrowed_inputs_for_dispatch(
    inputs: &[&[u8]],
    field: &'static str,
) -> Result<smallvec::SmallVec<[Vec<u8>; 8]>, BackendError> {
    let mut owned = smallvec::SmallVec::<[Vec<u8>; 8]>::new();
    vyre_foundation::allocation::try_reserve_smallvec_to_capacity(&mut owned, inputs.len())
        .map_err(|error| {
        BackendError::InvalidProgram {
            fix: format!(
                "Fix: failed to reserve {field} for {} borrowed input buffer(s): {error}. Use a backend-native borrowed dispatch path or shard the dispatch.",
                inputs.len()
            ),
        }
    })?;
    for (index, input) in inputs.iter().enumerate() {
        let mut cloned = Vec::new();
        crate::allocation::try_reserve_vec_to_capacity(&mut cloned, input.len()).map_err(
            |error| {
            BackendError::InvalidProgram {
                fix: format!(
                    "Fix: failed to reserve {field} bytes for borrowed input {index} with length {}: {error}. Keep hot inputs resident on the GPU or shard the dispatch.",
                    input.len()
                ),
            }
        })?;
        cloned.extend_from_slice(input);
        owned.push(cloned);
    }
    Ok(owned)
}

/// Borrow caller-owned input buffers as dispatch slices with checked SmallVec allocation.
///
/// Backend compiled-pipeline implementations use this at the `dispatch(Vec<u8>)`
/// front door so every backend has the same allocation failure semantics before
/// entering its native borrowed-input path.
///
/// # Errors
/// Returns [`BackendError::InvalidProgram`] if reserving the slice-reference
/// arena fails.
pub fn borrowed_input_slices<'a>(
    inputs: &'a [Vec<u8>],
    field: &'static str,
) -> Result<smallvec::SmallVec<[&'a [u8]; 8]>, BackendError> {
    let mut borrowed = smallvec::SmallVec::<[&[u8]; 8]>::new();
    vyre_foundation::allocation::try_reserve_smallvec_to_capacity(&mut borrowed, inputs.len())
        .map_err(|error| {
        BackendError::InvalidProgram {
            fix: format!(
                "Fix: failed to reserve {field} for {} borrowed input slice(s): {error}. Reuse caller-owned borrowed slices or shard dispatch certification.",
                inputs.len()
            ),
        }
    })?;
    borrowed.extend(inputs.iter().map(Vec::as_slice));
    Ok(borrowed)
}

#[cfg(test)]
mod borrowed_input_slices_tests {
    use super::*;

    #[test]
    fn borrowed_input_slices_reuses_caller_storage() {
        let inputs = vec![vec![1u8, 2, 3], vec![4u8, 5]];
        let borrowed = borrowed_input_slices(&inputs, "test borrowed input").unwrap();
        assert_eq!(borrowed.len(), inputs.len());
        assert_eq!(borrowed[0], inputs[0].as_slice());
        assert_eq!(borrowed[1], inputs[1].as_slice());
        assert_eq!(
            borrowed[0].as_ptr(),
            inputs[0].as_ptr(),
            "compiled dispatch must borrow caller input storage instead of copying"
        );
    }
}

pub(crate) fn reserve_batch_output_slots(
    outputs: &mut Vec<OutputBuffers>,
    batch_len: usize,
    field: &'static str,
) -> Result<(), BackendError> {
    crate::allocation::try_reserve_vec_to_capacity(outputs, batch_len).map_err(|error| {
        BackendError::InvalidProgram {
            fix: format!(
                "Fix: failed to reserve {field} for {batch_len} batch output slot(s): {error}. Split the batch before dispatch or use a backend-native batch path."
            ),
        }
    })
}

pub(crate) fn reserved_batch_output_slots(
    batch_len: usize,
    field: &'static str,
) -> Result<Vec<OutputBuffers>, BackendError> {
    let mut outputs = Vec::new();
    reserve_batch_output_slots(&mut outputs, batch_len, field)?;
    Ok(outputs)
}

pub(crate) fn resize_batch_output_slots(
    outputs: &mut Vec<OutputBuffers>,
    batch_len: usize,
    field: &'static str,
) -> Result<(), BackendError> {
    reserve_batch_output_slots(outputs, batch_len, field)?;
    if outputs.len() < batch_len {
        outputs.resize_with(batch_len, Vec::new);
    } else {
        outputs.truncate(batch_len);
    }
    Ok(())
}

pub(crate) fn resize_typed_output_slots<T>(
    outputs: &mut Vec<Vec<T>>,
    output_len: usize,
    field: &'static str,
) -> Result<(), BackendError> {
    crate::allocation::try_reserve_vec_to_capacity(outputs, output_len).map_err(|error| {
        BackendError::InvalidProgram {
            fix: format!(
                "Fix: failed to reserve {field} for {output_len} typed output slot(s): {error}. Split the dispatch or decode into a caller-owned output arena."
            ),
        }
    })?;
    if outputs.len() < output_len {
        outputs.resize_with(output_len, Vec::new);
    } else {
        outputs.truncate(output_len);
    }
    Ok(())
}

pub(crate) fn checked_elapsed_wall_ns(
    started: std::time::Instant,
    field: &'static str,
) -> Result<u64, BackendError> {
    u64::try_from(started.elapsed().as_nanos()).map_err(|error| BackendError::InvalidProgram {
        fix: format!(
            "Fix: {field} wall-clock timing cannot fit u64 nanoseconds: {error}. Split telemetry windows or report per-dispatch timing."
        ),
    })
}
