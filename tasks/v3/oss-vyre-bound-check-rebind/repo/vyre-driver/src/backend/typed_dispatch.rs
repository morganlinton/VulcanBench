//! Typed dispatch helpers layered over the frozen backend contract.

use std::mem;

use bytemuck::Pod;
use smallvec::SmallVec;
use vyre_foundation::ir::Program;

use crate::backend::{BackendError, DispatchConfig, OutputBuffers, VyreBackend};

/// Extension methods for callers that work with typed POD buffers instead of
/// manually packing and unpacking byte vectors.
pub trait TypedDispatchExt: VyreBackend {
    /// Dispatch borrowed byte slices.
    ///
    /// This is a naming convenience over [`VyreBackend::dispatch_borrowed`]
    /// for call sites that are migrating away from owned `Vec<u8>` inputs.
    ///
    /// # Errors
    ///
    /// Returns [`BackendError`] when the backend rejects the program, inputs,
    /// or dispatch.
    fn dispatch_bytes(
        &self,
        program: &Program,
        inputs: &[&[u8]],
        config: &DispatchConfig,
    ) -> Result<Vec<Vec<u8>>, BackendError> {
        self.dispatch_borrowed(program, inputs, config)
    }

    /// Dispatch borrowed typed POD inputs and decode each output as `T`.
    ///
    /// # Errors
    ///
    /// Returns [`BackendError`] when an output byte length is not a whole
    /// number of `T` values or when the backend dispatch fails.
    fn dispatch_pod<T: Pod>(
        &self,
        program: &Program,
        inputs: &[&[T]],
        config: &DispatchConfig,
    ) -> Result<Vec<Vec<T>>, BackendError> {
        let byte_inputs = pod_input_byte_slices(inputs)?;
        let outputs = self.dispatch_borrowed(program, &byte_inputs, config)?;
        decode_pod_outputs(outputs)
    }

    /// Dispatch borrowed typed POD inputs and decode each output as `T` into
    /// caller-owned storage.
    ///
    /// `raw_outputs` retains the backend byte buffers between calls and
    /// `typed_outputs` retains decoded POD slots. Hot loops should use this
    /// instead of [`TypedDispatchExt::dispatch_pod`] to avoid rebuilding both
    /// output shells on every launch.
    ///
    /// # Errors
    ///
    /// Returns [`BackendError`] when an output byte length is not a whole
    /// number of `T` values or when the backend dispatch fails.
    fn dispatch_pod_into<T: Pod>(
        &self,
        program: &Program,
        inputs: &[&[T]],
        config: &DispatchConfig,
        raw_outputs: &mut OutputBuffers,
        typed_outputs: &mut Vec<Vec<T>>,
    ) -> Result<(), BackendError> {
        let byte_inputs = pod_input_byte_slices(inputs)?;
        self.dispatch_borrowed_into(program, &byte_inputs, config, raw_outputs)?;
        decode_pod_outputs_into(raw_outputs, typed_outputs)
    }

    /// Dispatch borrowed `u32` inputs and decode each output as `u32`.
    ///
    /// # Errors
    ///
    /// Returns [`BackendError`] on backend failure or malformed output length.
    fn dispatch_u32(
        &self,
        program: &Program,
        inputs: &[&[u32]],
        config: &DispatchConfig,
    ) -> Result<Vec<Vec<u32>>, BackendError> {
        self.dispatch_pod(program, inputs, config)
    }

    /// Dispatch borrowed `u32` inputs and decode outputs into caller-owned
    /// typed storage.
    ///
    /// # Errors
    ///
    /// Returns [`BackendError`] on backend failure or malformed output length.
    fn dispatch_u32_into(
        &self,
        program: &Program,
        inputs: &[&[u32]],
        config: &DispatchConfig,
        raw_outputs: &mut OutputBuffers,
        typed_outputs: &mut Vec<Vec<u32>>,
    ) -> Result<(), BackendError> {
        self.dispatch_pod_into(program, inputs, config, raw_outputs, typed_outputs)
    }

    /// Dispatch borrowed `f32` inputs and decode each output as `f32`.
    ///
    /// # Errors
    ///
    /// Returns [`BackendError`] on backend failure or malformed output length.
    fn dispatch_f32(
        &self,
        program: &Program,
        inputs: &[&[f32]],
        config: &DispatchConfig,
    ) -> Result<Vec<Vec<f32>>, BackendError> {
        self.dispatch_pod(program, inputs, config)
    }

    /// Dispatch borrowed `f32` inputs and decode outputs into caller-owned
    /// typed storage.
    ///
    /// # Errors
    ///
    /// Returns [`BackendError`] on backend failure or malformed output length.
    fn dispatch_f32_into(
        &self,
        program: &Program,
        inputs: &[&[f32]],
        config: &DispatchConfig,
        raw_outputs: &mut OutputBuffers,
        typed_outputs: &mut Vec<Vec<f32>>,
    ) -> Result<(), BackendError> {
        self.dispatch_pod_into(program, inputs, config, raw_outputs, typed_outputs)
    }
}

impl<T: VyreBackend + ?Sized> TypedDispatchExt for T {}

fn pod_input_byte_slices<'a, T: Pod>(
    inputs: &'a [&'a [T]],
) -> Result<SmallVec<[&'a [u8]; 8]>, BackendError> {
    let mut byte_inputs = SmallVec::<[&[u8]; 8]>::new();
    byte_inputs.try_reserve(inputs.len()).map_err(|error| {
        BackendError::InvalidProgram {
            fix: format!(
                "Fix: typed dispatch could not reserve {} POD input byte slice(s): {error}. Reuse caller-owned byte slices or shard the typed dispatch.",
                inputs.len()
            ),
        }
    })?;
    byte_inputs.extend(
        inputs
            .iter()
            .map(|input| bytemuck::cast_slice::<T, u8>(input)),
    );
    Ok(byte_inputs)
}

fn decode_pod_outputs<T: Pod>(outputs: Vec<Vec<u8>>) -> Result<Vec<Vec<T>>, BackendError> {
    let width = mem::size_of::<T>();
    if width == 0 {
        return Err(BackendError::InvalidProgram {
            fix: "Fix: typed dispatch does not support zero-sized POD outputs.".to_string(),
        });
    }
    let mut typed_outputs = Vec::new();
    crate::backend::resize_typed_output_slots(
        &mut typed_outputs,
        outputs.len(),
        "typed POD output",
    )?;
    for (index, (bytes, slot)) in outputs
        .into_iter()
        .zip(typed_outputs.iter_mut())
        .enumerate()
    {
        decode_pod_output_into(index, &bytes, width, slot)?;
    }
    Ok(typed_outputs)
}

fn decode_pod_outputs_into<T: Pod>(
    raw_outputs: &[Vec<u8>],
    typed_outputs: &mut Vec<Vec<T>>,
) -> Result<(), BackendError> {
    let width = mem::size_of::<T>();
    if width == 0 {
        return Err(BackendError::InvalidProgram {
            fix: "Fix: typed dispatch does not support zero-sized POD outputs.".to_string(),
        });
    }
    crate::backend::resize_typed_output_slots(
        typed_outputs,
        raw_outputs.len(),
        "typed POD output",
    )?;
    for (index, (bytes, slot)) in raw_outputs.iter().zip(typed_outputs.iter_mut()).enumerate() {
        decode_pod_output_into(index, bytes, width, slot)?;
    }
    Ok(())
}

fn decode_pod_output_into<T: Pod>(
    index: usize,
    bytes: &[u8],
    width: usize,
    output: &mut Vec<T>,
) -> Result<(), BackendError> {
    let remainder = bytes.len() % width;
    if remainder != 0 {
        return Err(BackendError::InvalidProgram {
            fix: format!(
                "Fix: output buffer {index} has {} bytes, which is not a whole number of {}-byte typed values.",
                bytes.len(),
                width
            ),
        });
    }
    output.clear();
    let value_count = bytes.len() / width;
    crate::allocation::try_reserve_vec_to_capacity(output, value_count).map_err(|error| {
        BackendError::InvalidProgram {
            fix: format!(
                "Fix: typed dispatch could not reserve {value_count} decoded POD value(s) for output buffer {index}: {error}. Decode into caller-owned output storage or shard the dispatch output."
            ),
        }
    })?;
    output.extend(
        bytes
            .chunks_exact(width)
            .map(bytemuck::pod_read_unaligned::<T>),
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use vyre_foundation::ir::{OpId, Program};

    use super::*;
    use crate::backend::private;

    struct EchoBackend;

    impl private::Sealed for EchoBackend {}

    impl VyreBackend for EchoBackend {
        fn id(&self) -> &'static str {
            "typed-dispatch-test"
        }

        fn supported_ops(&self) -> &HashSet<OpId> {
            static OPS: std::sync::OnceLock<HashSet<OpId>> = std::sync::OnceLock::new();
            OPS.get_or_init(HashSet::new)
        }

        fn dispatch(
            &self,
            _program: &Program,
            inputs: &[Vec<u8>],
            _config: &DispatchConfig,
        ) -> Result<Vec<Vec<u8>>, BackendError> {
            Ok(inputs.to_vec())
        }
    }

    #[test]
    fn dispatch_u32_packs_inputs_and_decodes_outputs() {
        let backend = EchoBackend;
        let input = [1u32, 2, 0x0102_0304];
        let outputs = backend
            .dispatch_u32(&Program::empty(), &[&input], &DispatchConfig::default())
            .unwrap_or_else(|error| panic!("typed u32 dispatch must succeed: {error}"));

        assert_eq!(outputs, vec![input.to_vec()]);
    }

    #[test]
    fn typed_decode_rejects_partial_words() {
        let error = decode_pod_outputs::<u32>(vec![vec![1, 2, 3]])
            .expect_err("partial u32 output must fail");

        assert!(
            error.to_string().contains("whole number of 4-byte"),
            "malformed typed output must produce actionable width error: {error}"
        );
    }

    #[test]
    fn dispatch_u32_into_reuses_raw_and_typed_output_slots() {
        let backend = EchoBackend;
        let input = [1u32, 2, 0x0102_0304];
        let mut raw_outputs = vec![Vec::with_capacity(16)];
        let mut typed_outputs = vec![Vec::with_capacity(3)];
        let raw_outer = raw_outputs.as_ptr();
        let raw_slot = raw_outputs[0].as_ptr();
        let typed_outer = typed_outputs.as_ptr();
        let typed_slot = typed_outputs[0].as_ptr();

        backend
            .dispatch_u32_into(
                &Program::empty(),
                &[&input],
                &DispatchConfig::default(),
                &mut raw_outputs,
                &mut typed_outputs,
            )
            .unwrap_or_else(|error| panic!("typed u32 into dispatch must succeed: {error}"));
        assert_eq!(typed_outputs, vec![input.to_vec()]);
        assert_eq!(raw_outputs.as_ptr(), raw_outer);
        assert_eq!(raw_outputs[0].as_ptr(), raw_slot);
        assert_eq!(typed_outputs.as_ptr(), typed_outer);
        assert_eq!(typed_outputs[0].as_ptr(), typed_slot);

        backend
            .dispatch_u32_into(
                &Program::empty(),
                &[&input],
                &DispatchConfig::default(),
                &mut raw_outputs,
                &mut typed_outputs,
            )
            .unwrap_or_else(|error| panic!("second typed u32 into dispatch must succeed: {error}"));
        assert_eq!(typed_outputs, vec![input.to_vec()]);
        assert_eq!(raw_outputs.as_ptr(), raw_outer);
        assert_eq!(raw_outputs[0].as_ptr(), raw_slot);
        assert_eq!(typed_outputs.as_ptr(), typed_outer);
        assert_eq!(typed_outputs[0].as_ptr(), typed_slot);
    }
}
