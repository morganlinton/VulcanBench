//! Parity-test-only raw WGSL probes.
//!
//! These helpers deliberately bypass vyre IR validation so conformance tests can
//! measure the backend's native WGSL transcendental behavior. They are compiled
//! only with the `parity-testing` feature and are not part of the production
//! dispatch path.

use crate::numeric::usize_to_u64;
use crate::staging_reserve::reserve_backend_vec;
use crate::WgpuBackend;
use crossbeam_channel::RecvTimeoutError;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::time::{Duration, Instant};

const F32_BYTES: usize = std::mem::size_of::<f32>();
const BATCH_WORKGROUP_SIZE: u32 = 64;
const PROBE_TIMEOUT: Duration = Duration::from_secs(30);

impl WgpuBackend {
    /// Dispatch a canonical one-op f32 unary probe and return raw output bytes.
    ///
    /// # Errors
    ///
    /// Returns a backend error when `input` is not one f32, the op is not a
    /// supported f32 unary probe, or the WGSL dispatch/readback fails.
    pub fn probe_op(
        &self,
        op: vyre_foundation::ir::UnOp,
        input: &[u8],
    ) -> Result<Vec<u8>, vyre_driver::BackendError> {
        if input.len() != F32_BYTES {
            return Err(vyre_driver::BackendError::new(format!(
                "probe_op expects exactly 4 input bytes for one f32, got {}. Fix: pass f32::to_bits().to_le_bytes().",
                input.len()
            )));
        }
        let mut raw = [0_u8; F32_BYTES];
        raw.copy_from_slice(input);
        let output = self.probe_op_many(op, &[f32::from_bits(u32::from_le_bytes(raw))])?;
        let Some(sample) = output.into_iter().next() else {
            return Err(vyre_driver::BackendError::new(
                "probe_op produced no output for one input. Fix: inspect parity probe dispatch/readback sizing.",
            ));
        };
        Ok(sample.to_bits().to_le_bytes().to_vec())
    }

    /// Dispatch a canonical f32 unary probe over a batch of inputs.
    ///
    /// This keeps parity tests from paying one GPU submission and readback per
    /// scalar sample. The generated WGSL is keyed by operation, so the backend
    /// pipeline cache reuses it across calls.
    ///
    /// # Errors
    ///
    /// Returns a backend error when the operation is unsupported, the batch is
    /// too large for WebGPU dispatch dimensions, or dispatch/readback fails.
    pub fn probe_op_many(
        &self,
        op: vyre_foundation::ir::UnOp,
        inputs: &[f32],
    ) -> Result<Vec<f32>, vyre_driver::BackendError> {
        if inputs.is_empty() {
            return Ok(Vec::new());
        }
        let input_words: u32 = inputs.len().try_into().map_err(|_| {
            vyre_driver::BackendError::new(format!(
                "probe_op_many received {} f32 samples, exceeding u32 dispatch dimensions. Fix: split the parity probe batch.",
                inputs.len()
            ))
        })?;
        let output_size = inputs.len().checked_mul(F32_BYTES).ok_or_else(|| {
            vyre_driver::BackendError::new(format!(
                "probe_op_many output size overflow for {} samples. Fix: split the parity probe batch.",
                inputs.len()
            ))
        })?;
        let input_bytes = f32_batch_bytes(inputs)?;
        let output = dispatch_probe_wgsl(
            &self.current_device_queue(),
            &probe_wgsl(op, BATCH_WORKGROUP_SIZE)?,
            &input_bytes,
            output_size,
            input_words,
        )?;
        decode_f32_batch(&output, input_words)
    }
}

fn probe_wgsl(
    op: vyre_foundation::ir::UnOp,
    workgroup_size: u32,
) -> Result<String, vyre_driver::BackendError> {
    let wgsl_body = match op {
        vyre_foundation::ir::UnOp::Sin => "sin(x)",
        vyre_foundation::ir::UnOp::Cos => "cos(x)",
        vyre_foundation::ir::UnOp::Sqrt => "sqrt(x)",
        vyre_foundation::ir::UnOp::Reciprocal => "1.0 / x",
        vyre_foundation::ir::UnOp::Exp => "exp(x)",
        vyre_foundation::ir::UnOp::Log => "log(x)",
        other => {
            return Err(vyre_driver::BackendError::new(format!(
                "unsupported f32 probe op {other:?}. Fix: use Sin, Cos, Sqrt, Reciprocal, Exp, or Log."
            )));
        }
    };

    Ok(format!(
        r#"
@group(0) @binding(0) var<storage, read> input: array<u32>;
@group(0) @binding(1) var<storage, read_write> output: array<u32>;
@group(1) @binding(2) var<uniform> params: vec4<u32>;

@compute @workgroup_size({workgroup_size})
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {{
    if (gid.x >= params.y) {{
        return;
    }}
    let x = bitcast<f32>(input[gid.x]);
    let y = {wgsl_body};
    output[gid.x] = bitcast<u32>(y);
}}
"#
    ))
}

fn f32_batch_bytes(inputs: &[f32]) -> Result<Vec<u8>, vyre_driver::BackendError> {
    let byte_len = inputs.len().checked_mul(F32_BYTES).ok_or_else(|| {
        vyre_driver::BackendError::new(format!(
            "parity probe f32 input byte length overflow for {} samples. Fix: split the parity probe batch.",
            inputs.len()
        ))
    })?;
    let mut bytes = Vec::new();
    reserve_backend_vec(&mut bytes, byte_len, "parity probe f32 input staging")?;
    for value in inputs {
        bytes.extend_from_slice(&value.to_bits().to_le_bytes());
    }
    Ok(bytes)
}

fn dispatch_probe_wgsl(
    device_queue: &Arc<(wgpu::Device, wgpu::Queue)>,
    wgsl: &str,
    input: &[u8],
    output_size: usize,
    output_words: u32,
) -> Result<Vec<u8>, vyre_driver::BackendError> {
    let (device, queue) = &**device_queue;
    let input_size = usize_to_u64(input.len().max(F32_BYTES), "parity probe input size")?;
    let output_size_u64 = usize_to_u64(output_size.max(F32_BYTES), "parity probe output size")?;
    let input_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("vyre parity probe input"),
        size: input_size,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    queue.write_buffer(&input_buffer, 0, input);
    let output_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("vyre parity probe output"),
        size: output_size_u64,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    });
    let readback_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("vyre parity probe readback"),
        size: output_size_u64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let input_len = u32::try_from(input.len()).map_err(|_| {
        vyre_driver::BackendError::new(
            "parity probe input length exceeds u32::MAX bytes. Fix: split the probe input before dispatch.",
        )
    })?;
    let params = [input_len, output_words, 0_u32, 0_u32];
    let params_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("vyre parity probe params"),
        size: usize_to_u64(
            params.len()
                .checked_mul(std::mem::size_of::<u32>())
                .ok_or_else(|| {
                    vyre_driver::BackendError::new(
                        "parity probe params byte length overflowed usize. Fix: reduce parameter words before probe dispatch.",
                    )
                })?,
            "parity probe params size",
        )?,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    queue.write_buffer(&params_buffer, 0, bytemuck::cast_slice(&params));

    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("vyre parity probe shader"),
        source: wgpu::ShaderSource::Wgsl(wgsl.into()),
    });
    let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("vyre parity probe pipeline"),
        layout: None,
        module: &module,
        entry_point: Some("main"),
        compilation_options: wgpu::PipelineCompilationOptions::default(),
        cache: None,
    });
    let group0_layout = pipeline.get_bind_group_layout(0);
    let group1_layout = pipeline.get_bind_group_layout(1);
    let group0 = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("vyre parity probe storage bind group"),
        layout: &group0_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: input_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: output_buffer.as_entire_binding(),
            },
        ],
    });
    let group1 = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("vyre parity probe uniform bind group"),
        layout: &group1_layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 2,
            resource: params_buffer.as_entire_binding(),
        }],
    });

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("vyre parity probe encoder"),
    });
    {
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("vyre parity probe compute"),
            timestamp_writes: None,
        });
        pass.set_pipeline(&pipeline);
        pass.set_bind_group(0, &group0, &[]);
        pass.set_bind_group(1, &group1, &[]);
        pass.dispatch_workgroups(output_words.div_ceil(BATCH_WORKGROUP_SIZE), 1, 1);
    }
    encoder.copy_buffer_to_buffer(&output_buffer, 0, &readback_buffer, 0, output_size_u64);
    queue.submit(std::iter::once(encoder.finish()));

    let slice = readback_buffer.slice(0..output_size_u64);
    let (sender, receiver) = crossbeam_channel::bounded(1);
    let ready = Arc::new(AtomicBool::new(false));
    let ready_cb = Arc::clone(&ready);
    slice.map_async(wgpu::MapMode::Read, move |result| {
        if let Err(error) = sender.send(result) {
            tracing::warn!("wgpu parity probe readback notification failed: {error}");
        }
        ready_cb.store(true, Ordering::Release);
    });

    let deadline = Instant::now() + PROBE_TIMEOUT;
    let mut backoff = crate::wait_backoff::AdaptiveWaitBackoff::from_micros(64, 2, 50, 5);
    while !ready.load(Ordering::Acquire) {
        crate::runtime::device::poll_device_once(device)?;
        let now = Instant::now();
        if now >= deadline {
            return Err(vyre_driver::BackendError::new(
                "parity probe readback did not complete within 30s. Fix: inspect wgpu device polling and direct-dispatch readback liveness.",
            ));
        }
        backoff.idle_for(deadline.duration_since(now));
    }
    let remaining = deadline
        .checked_duration_since(Instant::now())
        .ok_or_else(|| {
            vyre_driver::BackendError::new(
                "parity probe readback became ready after its receive deadline. Fix: inspect wgpu callback scheduling latency and raise the probe timeout deliberately.",
            )
        })?;
    receiver
        .recv_timeout(remaining)
        .map_err(|error| match error {
            RecvTimeoutError::Timeout => vyre_driver::BackendError::new(
                "parity probe readback callback timed out after readiness. Fix: keep callback receiver and readiness flag synchronized.",
            ),
            RecvTimeoutError::Disconnected => vyre_driver::BackendError::new(
                "parity probe readback callback disconnected. Fix: keep map_async callback sender alive until collection.",
            ),
        })?
        .map_err(|error| {
            vyre_driver::BackendError::new(format!(
                "parity probe readback mapping failed: {error:?}. Fix: verify readback buffer MAP_READ/COPY_DST usage."
            ))
        })?;
    let mapped = slice.get_mapped_range();
    let mut bytes = Vec::new();
    reserve_backend_vec(&mut bytes, output_size, "parity probe readback staging")?;
    bytes.extend_from_slice(&mapped[..output_size]);
    drop(mapped);
    readback_buffer.unmap();
    Ok(bytes)
}

fn decode_f32_batch(
    output: &[u8],
    expected_words: u32,
) -> Result<Vec<f32>, vyre_driver::BackendError> {
    let expected_words = usize::try_from(expected_words).map_err(|_| {
        vyre_driver::BackendError::new(
            "parity probe expected word count cannot fit host usize. Fix: split probe readback into smaller batches.",
        )
    })?;
    let expected_bytes = expected_words
        .checked_mul(F32_BYTES)
        .ok_or_else(|| {
            vyre_driver::BackendError::new(
                "parity probe expected byte count overflowed usize. Fix: split probe readback into smaller batches.",
            )
        })?;
    if output.len() != expected_bytes {
        return Err(vyre_driver::BackendError::new(format!(
            "batch probe returned {} bytes for {expected_words} f32 samples. Fix: keep dispatch_wgsl readback size synchronized with probe batch length.",
            output.len()
        )));
    }
    let mut values = Vec::new();
    reserve_backend_vec(
        &mut values,
        expected_words,
        "parity probe decoded f32 staging",
    )?;
    for chunk in output.chunks_exact(F32_BYTES) {
        let mut raw = [0_u8; F32_BYTES];
        raw.copy_from_slice(chunk);
        values.push(f32::from_bits(u32::from_le_bytes(raw)));
    }
    Ok(values)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn batch_probe_matches_singleton_probe() {
        let backend = WgpuBackend::acquire()
            .expect("Fix: parity probe tests require the local GPU-backed wgpu backend");
        let inputs = [0.0_f32, 0.5, 1.0, -2.0];
        let batch = backend
            .probe_op_many(vyre_foundation::ir::UnOp::Cos, &inputs)
            .expect("Fix: batched parity probe must dispatch successfully");
        assert_eq!(batch.len(), inputs.len());

        for (index, input) in inputs.iter().enumerate() {
            let singleton = backend
                .probe_op(
                    vyre_foundation::ir::UnOp::Cos,
                    &input.to_bits().to_le_bytes(),
                )
                .expect("Fix: singleton parity probe must dispatch successfully");
            let mut raw = [0_u8; F32_BYTES];
            raw.copy_from_slice(&singleton);
            assert_eq!(
                batch[index].to_bits(),
                f32::from_bits(u32::from_le_bytes(raw)).to_bits(),
                "Fix: batched probe result at lane {index} must match singleton probe"
            );
        }
    }

    #[test]
    fn parity_probe_uses_fallible_staging() {
        let src =
            std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/parity_probe.rs"))
                .expect("Fix: parity probe source must be readable");
        let production = src
            .split("#[cfg(test)]")
            .next()
            .expect("Fix: meta-test scans production sources; update fixture path if module moved - production section must exist");
        assert!(
            !production.contains("Vec::with_capacity("),
            "parity probe staging must use reserve_backend_vec"
        );
        assert!(production.contains("reserve_backend_vec"));
        assert!(production.contains("f32_batch_bytes(inputs)?"));
    }
}
