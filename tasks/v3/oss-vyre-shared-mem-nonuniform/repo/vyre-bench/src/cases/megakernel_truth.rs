use crate::api::case::{
    BenchCase, BenchContext, BenchError, BenchId, BenchLayer, BenchMetadata, BenchRequirements,
    BenchRun, Correctness, DeterminismClass, PreparedCase, WorkloadClass,
};
use crate::api::metric::{BenchMetrics, MetricPoint};
use crate::api::resident::{
    dispatch_compiled_timed, input_bytes_total, transfer_accounting, ResidentInputPool,
};
use std::sync::Arc;
use std::time::Instant;
use vyre_driver::CompiledPipeline;
use vyre_runtime::megakernel::{self, protocol, MegakernelWorkItem};

pub struct MegakernelTruth;

const WORK_ITEM_COUNT: usize = 1024;
const WORKER_COUNT: u32 = 256;
const RESIDENT_SAMPLE_SETS: usize = 8;
const SUITES: &[crate::api::suite::SuiteKind] = &[
    crate::api::suite::SuiteKind::Release,
    crate::api::suite::SuiteKind::Gpu,
    crate::api::suite::SuiteKind::Deep,
];

struct MegakernelTruthPrepared {
    program: Arc<vyre_foundation::ir::Program>,
    work_items: Vec<MegakernelWorkItem>,
    inputs: Vec<Vec<u8>>,
    input_bytes_total: u64,
    compiled: Option<Arc<dyn CompiledPipeline>>,
    resident: Option<ResidentInputPool>,
}

impl BenchCase for MegakernelTruth {
    fn id(&self) -> BenchId {
        BenchId("runtime.megakernel.truth.1024".to_string())
    }

    fn metadata(&self) -> BenchMetadata {
        BenchMetadata {
            id: self.id(),
            name: "Megakernel Truth 1024 WorkItems".to_string(),
            description:
                "Actual megakernel dispatcher path with queue planning, publication, and backend timing"
                    .to_string(),
            tags: vec![
                "runtime".to_string(),
                "megakernel".to_string(),
                "truth".to_string(),
                "release".to_string(),
            ],
            layer: BenchLayer::Runtime,
            workload: WorkloadClass::Macro,
            determinism: DeterminismClass::Deterministic,
            owner_crate: "vyre-runtime".to_string(),
        }
    }

    fn suites(&self) -> &'static [crate::api::suite::SuiteKind] {
        SUITES
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

    fn prepare(&self, ctx: &mut BenchContext) -> Result<PreparedCase, BenchError> {
        let work_items = make_work_items(WORK_ITEM_COUNT)?;
        let slot_count = u32::try_from(WORK_ITEM_COUNT).map_err(|source| {
            BenchError::ExecutionFailed(format!(
                "megakernel truth work item count cannot fit u32: {source}"
            ))
        })?;
        let program = megakernel::build_program_sharded_once_slots_control_report_shared(
            WORKER_COUNT,
            slot_count,
            &[],
        );
        let control_bytes = megakernel::encode_control(false, 1, 0)
            .map_err(|error| BenchError::ExecutionFailed(error.to_string()))?;
        let mut ring_words = Vec::new();
        vyre_runtime::megakernel::Megakernel::encode_work_items_ring_words_into(
            slot_count,
            0,
            &work_items,
            &mut ring_words,
        )
        .map_err(|error| BenchError::ExecutionFailed(error.to_string()))?;
        let mut ring_bytes = Vec::with_capacity(ring_words.len().saturating_mul(4));
        for word in &ring_words {
            ring_bytes.extend_from_slice(&word.to_le_bytes());
        }
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
            "megakernel truth bench",
        )?;
        Ok(Box::new(MegakernelTruthPrepared {
            program,
            work_items,
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
            .downcast_mut::<MegakernelTruthPrepared>()
            .ok_or_else(|| {
                BenchError::ExecutionFailed(
                    "megakernel truth prepared payload type mismatch".to_string(),
                )
            })?;

        let mut dispatch_config = ctx.dispatch_config.clone();
        dispatch_config.grid_override =
            Some([(WORK_ITEM_COUNT as u32).div_ceil(WORKER_COUNT), 1, 1]);
        if prepared.compiled.is_none() {
            let compiled = vyre_driver::pipeline::compile_with_telemetry(
                Arc::clone(&ctx.preferred_backend),
                prepared.program.as_ref(),
                &dispatch_config,
            )
            .map_err(|error| BenchError::BackendFailed(error.to_string()))?
            .pipeline;
            prepared.compiled = Some(compiled);
        }
        let compiled = prepared.compiled.as_ref().ok_or_else(|| {
            BenchError::ExecutionFailed(
                "megakernel truth compiled pipeline missing after compile".to_string(),
            )
        })?;
        let dispatch = dispatch_compiled_timed(
            compiled.as_ref(),
            prepared.resident.as_mut(),
            &prepared.inputs,
            &dispatch_config,
        )?;
        let resident_used = dispatch.resident_used;
        let wall_ns = dispatch.timed.wall_ns;
        let outputs = dispatch.timed.outputs;
        let done_count = read_done_count(&outputs)?;
        let baseline_start = Instant::now();
        let baseline_processed = simulate_cpu_drain(&prepared.work_items);
        let baseline_ns = u64::try_from(baseline_start.elapsed().as_nanos()).unwrap_or(u64::MAX);
        let output_bytes_total = outputs.iter().map(Vec::len).sum::<usize>() as u64;
        let accounting = transfer_accounting(
            prepared.input_bytes_total,
            output_bytes_total,
            resident_used,
        );

        Ok(BenchRun {
            metrics: BenchMetrics {
                wall_ns: Some(wall_ns),
                dispatch_ns: Some(wall_ns),
                kernel_queue_submit_ns: Some(0),
                input_bytes: Some(prepared.input_bytes_total),
                output_bytes: Some(output_bytes_total),
                bytes_read: Some(accounting.bytes_read),
                bytes_written: Some(accounting.bytes_written),
                bytes_touched: Some(accounting.bytes_touched),
                atomic_op_count: Some((WORK_ITEM_COUNT as u64).saturating_mul(2)),
                custom: vec![
                    MetricPoint {
                        name: "megakernel_backend_dispatch_ns".to_string(),
                        value: wall_ns,
                    },
                    MetricPoint {
                        name: "megakernel_published_items".to_string(),
                        value: WORK_ITEM_COUNT as u64,
                    },
                    MetricPoint {
                        name: "megakernel_items_processed".to_string(),
                        value: done_count,
                    },
                    MetricPoint {
                        name: "megakernel_items_remaining".to_string(),
                        value: (WORK_ITEM_COUNT as u64).saturating_sub(done_count),
                    },
                    MetricPoint {
                        name: "megakernel_kernel_launches".to_string(),
                        value: 1,
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
                        name: "megakernel_backend_neutral_cuda_path".to_string(),
                        value: u64::from(ctx.preferred_backend.id() == "cuda"),
                    },
                ],
                ..Default::default()
            },
            baseline_metrics: Some(BenchMetrics {
                wall_ns: Some(baseline_ns),
                cpu_ns: Some(baseline_ns),
                input_bytes: Some(prepared.input_bytes_total),
                bytes_read: Some(prepared.input_bytes_total),
                bytes_touched: Some(prepared.input_bytes_total),
                custom: vec![MetricPoint {
                    name: "megakernel_items_processed".to_string(),
                    value: baseline_processed,
                }],
                ..Default::default()
            }),
            outputs: vec![done_count.to_le_bytes().to_vec()],
            baseline_outputs: Some(vec![baseline_processed.to_le_bytes().to_vec()]),
        })
    }

    fn verify(&self, _ctx: &mut BenchContext, run: &BenchRun) -> Result<Correctness, BenchError> {
        run.verify_exact_outputs()
    }

    fn bytes_touched(&self, prepared: &PreparedCase) -> (u64, u64) {
        prepared
            .downcast_ref::<MegakernelTruthPrepared>()
            .map(|prepared| (prepared.input_bytes_total, 0))
            .unwrap_or((0, 0))
    }
}

fn read_done_count(outputs: &[Vec<u8>]) -> Result<u64, BenchError> {
    let control = outputs.first().ok_or_else(|| {
        BenchError::CorrectnessViolation(
            "megakernel truth dispatch produced no control output".to_string(),
        )
    })?;
    let done = vyre_runtime::megakernel::try_read_done_count(control)
        .map_err(|error| BenchError::CorrectnessViolation(error.to_string()))?;
    Ok(u64::from(done))
}

fn make_work_items(count: usize) -> Result<Vec<MegakernelWorkItem>, BenchError> {
    let mut items = Vec::with_capacity(count);
    for index in 0..count {
        let word = u32::try_from(index).map_err(|_| {
            BenchError::ExecutionFailed(
                "megakernel truth work item index exceeded u32::MAX".to_string(),
            )
        })?;
        items.push(MegakernelWorkItem {
            op_handle: protocol::opcode::NOP,
            input_handle: word,
            output_handle: word,
            param: word,
        });
    }
    Ok(items)
}

fn simulate_cpu_drain(items: &[MegakernelWorkItem]) -> u64 {
    items.iter().fold(0_u64, |count, item| {
        count.saturating_add(if item.op_handle == protocol::opcode::NOP {
            1
        } else {
            0
        })
    })
}

inventory::submit! {
    &MegakernelTruth as &'static dyn BenchCase
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn work_items_are_unique_for_dedupe_truth() {
        let items = make_work_items(64).expect("Fix: fixture");
        let mut deduped = Vec::new();
        let report = vyre_runtime::megakernel::try_prune_redundant_work_items_into(
            &items,
            &mut deduped,
        )
        .expect("Fix: dedupe truth fixture must satisfy megakernel planner invariants.");

        assert!(report.is_empty());
        assert!(deduped.is_empty());
    }

    #[test]
    fn cpu_drain_counts_nop_items() {
        let items = make_work_items(8).expect("Fix: fixture");

        assert_eq!(simulate_cpu_drain(&items), 8);
    }
}
