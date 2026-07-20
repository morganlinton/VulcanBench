use crate::api::case::{
    BenchCase, BenchContext, BenchError, BenchId, BenchLayer, BenchMetadata, BenchRequirements,
    BenchRun, Correctness, DeterminismClass, PerformanceContract, PreparedCase, WorkloadClass,
};
use crate::api::metric::{BenchMetrics, MetricPoint};
use crate::api::resident::{
    dispatch_compiled_timed, input_bytes_total, transfer_accounting, ResidentInputPool,
};
use std::sync::Arc;
use vyre_driver::autotune_store::{AutotuneRecord, AutotuneStore};
use vyre_driver::specialization::SpecCacheKey;
use vyre_driver::speculate::SpeculativeVariantKeys;
use vyre_driver::speculation_substrate::SpeculationVerdict;
use vyre_driver::CompiledPipeline;
use vyre_runtime::megakernel::{
    self, control, slot, PairedSpeculationSample, PairedSpeculationWindow, SLOT_WORDS, STATUS_WORD,
};

pub struct MegakernelLatency;

const SLOT_COUNT: u32 = 256;
const WORKGROUP_SIZE: u32 = 256;
const RESIDENT_SAMPLE_SETS: usize = 8;

struct MegakernelLatencyPrepared {
    program: vyre_foundation::ir::Program,
    inputs: Vec<Vec<u8>>,
    input_bytes_total: u64,
    compiled: Option<Arc<dyn CompiledPipeline>>,
    resident: Option<ResidentInputPool>,
}

impl BenchCase for MegakernelLatency {
    fn id(&self) -> BenchId {
        BenchId("runtime.megakernel.dispatch.256".to_string())
    }

    fn metadata(&self) -> BenchMetadata {
        BenchMetadata {
            id: self.id(),
            name: "Megakernel Dispatch 256 Slots".to_string(),
            description: "Finite one-pass persistent-megakernel slot drain latency".to_string(),
            tags: vec![
                "runtime".to_string(),
                "megakernel".to_string(),
                "latency".to_string(),
            ],
            layer: BenchLayer::Runtime,
            workload: WorkloadClass::Macro,
            determinism: DeterminismClass::Deterministic,
            owner_crate: "vyre-runtime".to_string(),
        }
    }

    fn requirements(&self) -> BenchRequirements {
        BenchRequirements {
            needs_gpu: true,
            needs_network: false,
            min_vram_bytes: None,
            min_input_bytes: None,
            feature_set: vec![],
        }
    }

    fn performance_contract(&self) -> Option<PerformanceContract> {
        Some(PerformanceContract::cpu_sota_10x(
            "resident megakernel slot dispatch",
            "vyre-runtime",
            "single-threaded CPU slot-drain simulator",
        ))
    }

    fn prepare(&self, ctx: &mut BenchContext) -> Result<PreparedCase, BenchError> {
        let program = megakernel::build_program_sharded_once_slots(WORKGROUP_SIZE, SLOT_COUNT, &[]);
        let control_bytes = megakernel::encode_control(false, 1, 0)
            .map_err(|error| BenchError::ExecutionFailed(error.to_string()))?;
        let ring_bytes = published_ring(SLOT_COUNT)?;
        let debug_bytes = megakernel::encode_empty_debug_log(megakernel::debug::RECORD_CAPACITY)
            .map_err(|error| BenchError::ExecutionFailed(error.to_string()))?;
        let io_bytes = megakernel::io::try_encode_empty_io_queue(megakernel::io::IO_SLOT_COUNT)
            .map_err(|error| BenchError::ExecutionFailed(error.to_string()))?;
        let inputs = vec![control_bytes, ring_bytes, debug_bytes, io_bytes];
        let input_bytes_total = input_bytes_total(&inputs);
        let resident = ResidentInputPool::upload_optional(
            ctx,
            &inputs,
            RESIDENT_SAMPLE_SETS,
            "megakernel latency bench",
        )?;
        Ok(Box::new(MegakernelLatencyPrepared {
            program,
            inputs,
            input_bytes_total,
            compiled: None,
            resident,
        }))
    }

    fn program<'a>(&self, _prepared: &'a PreparedCase) -> Option<&'a vyre_foundation::ir::Program> {
        None
    }

    fn run(
        &self,
        ctx: &mut BenchContext,
        prepared: &mut PreparedCase,
    ) -> Result<BenchRun, BenchError> {
        let prepared = prepared
            .downcast_mut::<MegakernelLatencyPrepared>()
            .ok_or_else(|| {
                BenchError::ExecutionFailed(
                    "megakernel latency prepared payload type mismatch".to_string(),
                )
            })?;
        let mut config = ctx.dispatch_config.clone();
        config.grid_override = Some([1, 1, 1]);

        if prepared.compiled.is_none() {
            let compiled = vyre_driver::pipeline::compile_with_telemetry(
                Arc::clone(&ctx.preferred_backend),
                &prepared.program,
                &config,
            )
            .map_err(|error| BenchError::BackendFailed(error.to_string()))?
            .pipeline;
            prepared.compiled = Some(compiled);
        }
        let compiled = prepared.compiled.as_ref().ok_or_else(|| {
            BenchError::ExecutionFailed(
                "megakernel latency compiled pipeline missing after compile".to_string(),
            )
        })?;
        let dispatch = dispatch_compiled_timed(
            compiled.as_ref(),
            prepared.resident.as_mut(),
            &prepared.inputs,
            &config,
        )?;
        let resident_used = dispatch.resident_used;
        let elapsed = dispatch.timed.wall_ns;
        let dispatch_ns = dispatch.timed.device_ns;
        let outputs = dispatch.timed.outputs;
        let output_bytes_total = outputs.iter().map(Vec::len).sum::<usize>() as u64;
        let accounting = transfer_accounting(
            prepared.input_bytes_total,
            output_bytes_total,
            resident_used,
        );
        let device_ns = dispatch_ns.unwrap_or(elapsed);
        let wall_gb_s = gb_per_second(accounting.bytes_touched, elapsed);
        let device_gb_s = gb_per_second(accounting.bytes_touched, device_ns);
        let start_ref = std::time::Instant::now();
        let baseline_outputs = simulate_sharded_once_outputs(&prepared.inputs)?;
        let baseline_ns = start_ref.elapsed().as_nanos() as u64;
        let baseline_output_bytes = baseline_outputs.iter().map(Vec::len).sum::<usize>() as u64;
        let baseline_bytes_touched = prepared
            .input_bytes_total
            .saturating_add(baseline_output_bytes);
        let speculation = record_speculation_probe()?;

        Ok(BenchRun {
            metrics: BenchMetrics {
                wall_ns: Some(elapsed),
                dispatch_ns,
                input_bytes: Some(prepared.input_bytes_total),
                output_bytes: Some(output_bytes_total),
                bytes_touched: Some(accounting.bytes_touched),
                bytes_read: Some(accounting.bytes_read),
                bytes_written: Some(accounting.bytes_written),
                atomic_op_count: Some(u64::from(SLOT_COUNT).saturating_mul(2)),
                wall_throughput_gb_s: Some(wall_gb_s),
                device_throughput_gb_s: Some(device_gb_s),
                custom: vec![
                    MetricPoint {
                        name: "megakernel_slots".to_string(),
                        value: u64::from(SLOT_COUNT),
                    },
                    MetricPoint {
                        name: "megakernel_dispatch_latency_ns".to_string(),
                        value: device_ns,
                    },
                    MetricPoint {
                        name: "megakernel_slots_per_sec_x1000".to_string(),
                        value: rate_per_second_x1000(u64::from(SLOT_COUNT), device_ns),
                    },
                    MetricPoint {
                        name: "megakernel_roundtrip_buffers".to_string(),
                        value: outputs.len() as u64,
                    },
                    MetricPoint {
                        name: "megakernel_resident_input_pool_sets".to_string(),
                        value: if resident_used {
                            RESIDENT_SAMPLE_SETS as u64
                        } else {
                            0
                        },
                    },
                    MetricPoint {
                        name: "megakernel_speculation_samples".to_string(),
                        value: speculation.samples,
                    },
                    MetricPoint {
                        name: "megakernel_speculation_adopted".to_string(),
                        value: speculation.adopted,
                    },
                    MetricPoint {
                        name: "megakernel_speculation_rejected".to_string(),
                        value: speculation.rejected,
                    },
                    MetricPoint {
                        name: "megakernel_speculation_side_compile_cost_ns".to_string(),
                        value: speculation.side_compile_cost_ns,
                    },
                    MetricPoint {
                        name: "megakernel_speculation_autotune_records".to_string(),
                        value: speculation.autotune_records,
                    },
                ],
                ..Default::default()
            },
            baseline_metrics: Some(BenchMetrics {
                wall_ns: Some(baseline_ns),
                input_bytes: Some(prepared.input_bytes_total),
                output_bytes: Some(baseline_output_bytes),
                bytes_touched: Some(baseline_bytes_touched),
                bytes_read: Some(prepared.input_bytes_total),
                bytes_written: Some(baseline_output_bytes),
                ..Default::default()
            }),
            outputs,
            baseline_outputs: Some(baseline_outputs),
        })
    }

    fn verify(&self, _ctx: &mut BenchContext, run: &BenchRun) -> Result<Correctness, BenchError> {
        run.verify_exact_outputs()?;
        verify_megakernel_outputs(&run.outputs)
    }
}

struct SpeculationProbe {
    samples: u64,
    adopted: u64,
    rejected: u64,
    side_compile_cost_ns: u64,
    autotune_records: u64,
}

fn record_speculation_probe() -> Result<SpeculationProbe, BenchError> {
    let mut store = AutotuneStore::default();
    let adopt_conservative = spec_key(0xA001);
    let adopt_speculative = spec_key(0xA002);
    let reject_conservative = spec_key(0xB001);
    let reject_speculative = spec_key(0xB002);
    let adopt_keys = SpeculativeVariantKeys {
        conservative: &adopt_conservative,
        speculative: &adopt_speculative,
        adapter_id: "release-megakernel-latency",
    };
    let reject_keys = SpeculativeVariantKeys {
        conservative: &reject_conservative,
        speculative: &reject_speculative,
        adapter_id: "release-megakernel-latency",
    };
    let mut adopt_window = PairedSpeculationWindow::new();
    let mut reject_window = PairedSpeculationWindow::new();
    let mut adopt_verdict = SpeculationVerdict::KeepRacing;
    let mut reject_verdict = SpeculationVerdict::KeepRacing;
    for _ in 0..8 {
        adopt_verdict = adopt_window
            .record_sample(
                &mut store,
                adopt_keys,
                speculation_sample(100_000, 50_000, 0),
            )
            .verdict;
        reject_verdict = reject_window
            .record_sample(
                &mut store,
                reject_keys,
                speculation_sample(100_000, 50_000, 1_000_000),
            )
            .verdict;
    }
    if adopt_verdict != SpeculationVerdict::Adopt {
        return Err(BenchError::CorrectnessViolation(format!(
            "megakernel speculation adoption probe returned {adopt_verdict:?}, expected Adopt"
        )));
    }
    if reject_verdict != SpeculationVerdict::Reject {
        return Err(BenchError::CorrectnessViolation(format!(
            "megakernel speculation rejection probe returned {reject_verdict:?}, expected Reject"
        )));
    }
    let reject_observation = reject_window.observation();
    Ok(SpeculationProbe {
        samples: u64::from(adopt_window.len()).saturating_add(u64::from(reject_window.len())),
        adopted: 1,
        rejected: 1,
        side_compile_cost_ns: reject_observation.side_compile_cost_ns,
        autotune_records: store.len() as u64,
    })
}

fn speculation_sample(
    conservative_dispatch_ns: u64,
    speculative_dispatch_ns: u64,
    speculative_compile_ns: u64,
) -> PairedSpeculationSample {
    PairedSpeculationSample {
        conservative_dispatch_ns,
        speculative_dispatch_ns,
        conservative_compile_ns: 0,
        speculative_compile_ns,
        conservative_record: autotune_record(64),
        speculative_record: autotune_record(128),
    }
}

fn autotune_record(workgroup: u32) -> AutotuneRecord {
    AutotuneRecord {
        workgroup_size: [workgroup, 1, 1],
        unroll: 1,
        tile: [0, 0, 0],
        recorded_at: "2026-05-05".to_string(),
    }
}

fn spec_key(id: u64) -> SpecCacheKey {
    SpecCacheKey {
        shader_hash: id,
        binding_sig: id << 8,
        workgroup_size: [WORKGROUP_SIZE, 1, 1],
        spec_hash: id << 16,
    }
}

fn published_ring(slot_count: u32) -> Result<Vec<u8>, BenchError> {
    let mut ring = megakernel::encode_empty_ring(slot_count)
        .map_err(|error| BenchError::ExecutionFailed(error.to_string()))?;
    let slot_bytes = (SLOT_WORDS as usize).checked_mul(4).ok_or_else(|| {
        BenchError::ExecutionFailed("megakernel slot byte width overflowed usize".to_string())
    })?;
    let status_offset = (STATUS_WORD as usize).checked_mul(4).ok_or_else(|| {
        BenchError::ExecutionFailed("megakernel status byte offset overflowed usize".to_string())
    })?;
    for slot in ring.chunks_exact_mut(slot_bytes) {
        slot[status_offset..status_offset + 4].copy_from_slice(&slot::PUBLISHED.to_le_bytes());
    }
    Ok(ring)
}

fn gb_per_second(bytes: u64, nanos: u64) -> f64 {
    if nanos == 0 {
        return 0.0;
    }
    bytes as f64 / nanos as f64
}

fn rate_per_second_x1000(units: u64, nanos: u64) -> u64 {
    if nanos == 0 {
        return 0;
    }
    ((u128::from(units) * 1_000_000_000_000u128) / u128::from(nanos)).min(u128::from(u64::MAX))
        as u64
}

fn read_word(bytes: &[u8], word_index: u32) -> Result<u32, BenchError> {
    let offset = usize::try_from(word_index)
        .ok()
        .and_then(|word| word.checked_mul(4))
        .ok_or_else(|| {
            BenchError::CorrectnessViolation("megakernel word index overflowed usize".to_string())
        })?;
    let end = offset.checked_add(4).ok_or_else(|| {
        BenchError::CorrectnessViolation("megakernel word byte range overflowed usize".to_string())
    })?;
    let bytes = bytes.get(offset..end).ok_or_else(|| {
        BenchError::CorrectnessViolation(format!(
            "megakernel word {word_index} is outside output buffer"
        ))
    })?;
    vyre_primitives::wire::read_u32_le_word(bytes, 0, "megakernel latency output")
        .map_err(BenchError::CorrectnessViolation)
}

fn write_word(bytes: &mut [u8], word_index: u32, value: u32) -> Result<(), BenchError> {
    let offset = usize::try_from(word_index)
        .ok()
        .and_then(|word| word.checked_mul(4))
        .ok_or_else(|| {
            BenchError::ExecutionFailed("megakernel word index overflowed usize".to_string())
        })?;
    let end = offset.checked_add(4).ok_or_else(|| {
        BenchError::ExecutionFailed("megakernel word byte range overflowed usize".to_string())
    })?;
    let slot = bytes.get_mut(offset..end).ok_or_else(|| {
        BenchError::ExecutionFailed(format!(
            "megakernel word {word_index} is outside output buffer"
        ))
    })?;
    slot.copy_from_slice(&value.to_le_bytes());
    Ok(())
}

fn simulate_sharded_once_outputs(inputs: &[Vec<u8>]) -> Result<Vec<Vec<u8>>, BenchError> {
    if inputs.len() != 4 {
        return Err(BenchError::ExecutionFailed(format!(
            "megakernel baseline received {} buffers, expected 4",
            inputs.len()
        )));
    }
    let mut control = inputs[0].clone();
    let mut ring = inputs[1].clone();
    let debug = inputs[2].clone();
    let io = inputs[3].clone();

    write_word(&mut control, control::DONE_COUNT, SLOT_COUNT)?;
    write_word(&mut control, control::METRICS_BASE, SLOT_COUNT)?;
    for slot_index in 0..SLOT_COUNT {
        write_word(
            &mut ring,
            slot_index
                .saturating_mul(SLOT_WORDS)
                .saturating_add(STATUS_WORD),
            slot::DONE,
        )?;
    }
    Ok(vec![control, ring, debug, io])
}

fn verify_megakernel_outputs(outputs: &[Vec<u8>]) -> Result<Correctness, BenchError> {
    if outputs.len() != 4 {
        return Err(BenchError::CorrectnessViolation(format!(
            "megakernel dispatch returned {} buffers, expected 4",
            outputs.len()
        )));
    }
    let done_count = read_word(&outputs[0], control::DONE_COUNT)?;
    if done_count != SLOT_COUNT {
        return Err(BenchError::CorrectnessViolation(format!(
            "megakernel DONE_COUNT was {done_count}, expected {SLOT_COUNT}"
        )));
    }
    for slot_index in 0..SLOT_COUNT {
        let status = read_word(
            &outputs[1],
            slot_index
                .saturating_mul(SLOT_WORDS)
                .saturating_add(STATUS_WORD),
        )?;
        if status != slot::DONE {
            return Err(BenchError::CorrectnessViolation(format!(
                "megakernel slot {slot_index} status was {status}, expected DONE"
            )));
        }
    }
    Ok(Correctness::Exact)
}

inventory::submit! {
    &MegakernelLatency as &'static dyn BenchCase
}
