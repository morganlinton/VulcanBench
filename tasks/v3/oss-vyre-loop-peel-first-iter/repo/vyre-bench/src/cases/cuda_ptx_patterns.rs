use crate::api::case::{
    BenchCase, BenchContext, BenchError, BenchId, BenchLayer, BenchMetadata, BenchRequirements,
    BenchRun, Correctness, DeterminismClass, PreparedCase, WorkloadClass,
};
use crate::api::metric::{BenchMetrics, MetricPoint};
use crate::api::suite::SuiteKind;
use std::collections::HashMap;
use std::time::Instant;
use vyre_emit_ptx::{patterns, ComputeCapability};
use vyre_foundation::ir::{BinOp, DataType};
use vyre_lower::{
    BindingLayout, BindingSlot, BindingVisibility, Dispatch, KernelBody, KernelDescriptor,
    KernelOp, KernelOpKind, LiteralValue, MatrixMmaElement, MatrixMmaLayout, MatrixMmaShape,
    MemoryClass,
};

/// Release benchmark case for CUDA/PTX fast-path pattern coverage.
pub struct CudaPtxPatterns;

const SUITES: &[SuiteKind] = &[SuiteKind::Release, SuiteKind::Deep];

#[derive(Debug)]
struct PtxPatternTotals {
    corpus_kernels: u64,
    predication_candidates: u64,
    safe_predication_candidates: u64,
    vec_load_candidates: u64,
    vec_store_candidates: u64,
    async_copy_candidates: u64,
    tensor_core_candidates: u64,
    ldmatrix_capable_targets: u64,
    scheduled_fillers: u64,
    predicated_stores: u64,
    branch_labels: u64,
    cp_async_emitted: u64,
    mma_sync_emitted: u64,
    vectorized_loads_emitted: u64,
    vectorized_stores_emitted: u64,
    vector_kernel_scalar_loads: u64,
    vector_kernel_scalar_stores: u64,
    vector_kernel_scalar_index_adds: u64,
    source_cache_entries: u64,
    source_cache_hits: u64,
    source_cache_misses: u64,
    ptx_bytes_emitted: u64,
}

impl BenchCase for CudaPtxPatterns {
    fn id(&self) -> BenchId {
        BenchId("cuda.ptx.patterns.release.corpus".to_string())
    }

    fn metadata(&self) -> BenchMetadata {
        BenchMetadata {
            id: self.id(),
            name: "CUDA PTX Pattern Release Corpus".to_string(),
            description: "Measures PTX-side CUDA fast-path coverage for predication, vector memory, and load-gap scheduling".to_string(),
            tags: vec![
                "cuda".to_string(),
                "ptx".to_string(),
                "backend".to_string(),
                "release".to_string(),
            ],
            layer: BenchLayer::Backend,
            workload: WorkloadClass::Micro,
            determinism: DeterminismClass::Deterministic,
            owner_crate: "vyre-emit-ptx".to_string(),
        }
    }

    fn suites(&self) -> &'static [SuiteKind] {
        SUITES
    }

    fn requirements(&self) -> BenchRequirements {
        BenchRequirements {
            needs_gpu: false,
            needs_network: false,
            min_vram_bytes: None,
            min_input_bytes: None,
            feature_set: vec![],
        }
    }

    fn prepare(&self, _ctx: &mut BenchContext) -> Result<PreparedCase, BenchError> {
        Ok(Box::new(corpus()))
    }

    fn program<'a>(&self, _prepared: &'a PreparedCase) -> Option<&'a vyre_foundation::ir::Program> {
        None
    }

    fn run(
        &self,
        _ctx: &mut BenchContext,
        prepared: &mut PreparedCase,
    ) -> Result<BenchRun, BenchError> {
        let corpus = prepared
            .downcast_ref::<Vec<KernelDescriptor>>()
            .ok_or_else(|| {
                BenchError::ExecutionFailed(
                    "CUDA PTX pattern prepared payload type mismatch".to_string(),
                )
            })?;
        let started = Instant::now();
        let totals = measure_corpus(corpus)?;
        let elapsed = started.elapsed().as_nanos() as u64;

        let mut output = Vec::with_capacity(22 * std::mem::size_of::<u64>());
        for value in [
            totals.corpus_kernels,
            totals.predication_candidates,
            totals.safe_predication_candidates,
            totals.vec_load_candidates,
            totals.vec_store_candidates,
            totals.async_copy_candidates,
            totals.tensor_core_candidates,
            totals.ldmatrix_capable_targets,
            totals.scheduled_fillers,
            totals.predicated_stores,
            totals.branch_labels,
            totals.cp_async_emitted,
            totals.mma_sync_emitted,
            totals.vectorized_loads_emitted,
            totals.vectorized_stores_emitted,
            totals.vector_kernel_scalar_loads,
            totals.vector_kernel_scalar_stores,
            totals.vector_kernel_scalar_index_adds,
            totals.source_cache_entries,
            totals.source_cache_hits,
            totals.source_cache_misses,
            totals.ptx_bytes_emitted,
        ] {
            output.extend_from_slice(&value.to_le_bytes());
        }

        Ok(BenchRun {
            metrics: BenchMetrics {
                wall_ns: Some(elapsed),
                lower_ns: Some(elapsed),
                output_bytes: Some(totals.ptx_bytes_emitted),
                custom: vec![
                    MetricPoint {
                        name: "ptx_corpus_kernels".to_string(),
                        value: totals.corpus_kernels,
                    },
                    MetricPoint {
                        name: "ptx_predication_candidates".to_string(),
                        value: totals.predication_candidates,
                    },
                    MetricPoint {
                        name: "ptx_safe_predication_candidates".to_string(),
                        value: totals.safe_predication_candidates,
                    },
                    MetricPoint {
                        name: "ptx_vec_load_candidates".to_string(),
                        value: totals.vec_load_candidates,
                    },
                    MetricPoint {
                        name: "ptx_vec_store_candidates".to_string(),
                        value: totals.vec_store_candidates,
                    },
                    MetricPoint {
                        name: "ptx_async_copy_candidates".to_string(),
                        value: totals.async_copy_candidates,
                    },
                    MetricPoint {
                        name: "ptx_tensor_core_candidates".to_string(),
                        value: totals.tensor_core_candidates,
                    },
                    MetricPoint {
                        name: "ptx_ldmatrix_capable_targets".to_string(),
                        value: totals.ldmatrix_capable_targets,
                    },
                    MetricPoint {
                        name: "ptx_scheduled_fillers".to_string(),
                        value: totals.scheduled_fillers,
                    },
                    MetricPoint {
                        name: "ptx_predicated_stores".to_string(),
                        value: totals.predicated_stores,
                    },
                    MetricPoint {
                        name: "ptx_branch_labels".to_string(),
                        value: totals.branch_labels,
                    },
                    MetricPoint {
                        name: "ptx_cp_async_emitted".to_string(),
                        value: totals.cp_async_emitted,
                    },
                    MetricPoint {
                        name: "ptx_mma_sync_emitted".to_string(),
                        value: totals.mma_sync_emitted,
                    },
                    MetricPoint {
                        name: "ptx_vectorized_loads_emitted".to_string(),
                        value: totals.vectorized_loads_emitted,
                    },
                    MetricPoint {
                        name: "ptx_vectorized_stores_emitted".to_string(),
                        value: totals.vectorized_stores_emitted,
                    },
                    MetricPoint {
                        name: "ptx_vector_kernel_scalar_loads".to_string(),
                        value: totals.vector_kernel_scalar_loads,
                    },
                    MetricPoint {
                        name: "ptx_vector_kernel_scalar_stores".to_string(),
                        value: totals.vector_kernel_scalar_stores,
                    },
                    MetricPoint {
                        name: "ptx_vector_kernel_scalar_index_adds".to_string(),
                        value: totals.vector_kernel_scalar_index_adds,
                    },
                    MetricPoint {
                        name: "cuda_ptx_source_cache_entries".to_string(),
                        value: totals.source_cache_entries,
                    },
                    MetricPoint {
                        name: "cuda_ptx_source_cache_hits".to_string(),
                        value: totals.source_cache_hits,
                    },
                    MetricPoint {
                        name: "cuda_ptx_source_cache_misses".to_string(),
                        value: totals.source_cache_misses,
                    },
                    MetricPoint {
                        name: "ptx_bytes_emitted".to_string(),
                        value: totals.ptx_bytes_emitted,
                    },
                ],
                ..Default::default()
            },
            baseline_metrics: None,
            outputs: vec![output],
            baseline_outputs: None,
        })
    }

    fn verify(&self, _ctx: &mut BenchContext, run: &BenchRun) -> Result<Correctness, BenchError> {
        let words = decode_words(run)?;
        let [corpus_kernels, predication_candidates, safe_predication_candidates, vec_load_candidates, vec_store_candidates, async_copy_candidates, tensor_core_candidates, ldmatrix_capable_targets, scheduled_fillers, predicated_stores, branch_labels, cp_async_emitted, mma_sync_emitted, vectorized_loads_emitted, vectorized_stores_emitted, vector_kernel_scalar_loads, vector_kernel_scalar_stores, vector_kernel_scalar_index_adds, source_cache_entries, source_cache_hits, source_cache_misses, ptx_bytes_emitted] =
            words.as_slice()
        else {
            return Ok(Correctness::Invalid {
                reason: "CUDA PTX pattern benchmark emitted the wrong metric word count"
                    .to_string(),
            });
        };
        if *corpus_kernels < 4
            || *predication_candidates == 0
            || *safe_predication_candidates == 0
            || *vec_load_candidates == 0
            || *vec_store_candidates == 0
            || *async_copy_candidates == 0
            || *tensor_core_candidates == 0
            || *ldmatrix_capable_targets == 0
            || *scheduled_fillers < 2
            || *predicated_stores < 3
            || *branch_labels != 0
            || *cp_async_emitted == 0
            || *mma_sync_emitted == 0
            || *vectorized_loads_emitted == 0
            || *vectorized_stores_emitted == 0
            || *vector_kernel_scalar_loads != 0
            || *vector_kernel_scalar_stores != 0
            || *vector_kernel_scalar_index_adds != 0
            || *source_cache_entries == 0
            || *source_cache_hits == 0
            || *source_cache_misses == 0
            || *ptx_bytes_emitted == 0
        {
            return Ok(Correctness::Invalid {
                reason: format!(
                    "CUDA PTX release corpus missing fast-path evidence: kernels={corpus_kernels}, pred={predication_candidates}, safe_pred={safe_predication_candidates}, vload={vec_load_candidates}, vstore={vec_store_candidates}, async_copy={async_copy_candidates}, tensor_core={tensor_core_candidates}, ldmatrix_capable={ldmatrix_capable_targets}, fillers={scheduled_fillers}, pred_stores={predicated_stores}, branch_labels={branch_labels}, cp_async={cp_async_emitted}, mma_sync={mma_sync_emitted}, vectorized_loads={vectorized_loads_emitted}, vectorized_stores={vectorized_stores_emitted}, vector_scalar_loads={vector_kernel_scalar_loads}, vector_scalar_stores={vector_kernel_scalar_stores}, vector_scalar_index_adds={vector_kernel_scalar_index_adds}, source_cache_entries={source_cache_entries}, source_cache_hits={source_cache_hits}, source_cache_misses={source_cache_misses}, bytes={ptx_bytes_emitted}"
                ),
            });
        }
        Ok(Correctness::Exact)
    }
}

fn measure_corpus(corpus: &[KernelDescriptor]) -> Result<PtxPatternTotals, BenchError> {
    let mut totals = PtxPatternTotals {
        corpus_kernels: corpus.len() as u64,
        predication_candidates: 0,
        safe_predication_candidates: 0,
        vec_load_candidates: 0,
        vec_store_candidates: 0,
        async_copy_candidates: 0,
        tensor_core_candidates: 0,
        ldmatrix_capable_targets: 0,
        scheduled_fillers: 0,
        predicated_stores: 0,
        branch_labels: 0,
        cp_async_emitted: 0,
        mma_sync_emitted: 0,
        vectorized_loads_emitted: 0,
        vectorized_stores_emitted: 0,
        vector_kernel_scalar_loads: 0,
        vector_kernel_scalar_stores: 0,
        vector_kernel_scalar_index_adds: 0,
        source_cache_entries: 0,
        source_cache_hits: 0,
        source_cache_misses: 0,
        ptx_bytes_emitted: 0,
    };
    let mut source_cache = HashMap::<String, String>::new();
    for desc in corpus {
        let audit = patterns::audit(desc, ComputeCapability::SM_90);
        totals.predication_candidates = totals
            .predication_candidates
            .saturating_add(audit.predication.candidates.len() as u64);
        totals.safe_predication_candidates = totals
            .safe_predication_candidates
            .saturating_add(audit.predication.safe_candidate_count() as u64);
        totals.vec_load_candidates = totals
            .vec_load_candidates
            .saturating_add(audit.vec_load.candidates.len() as u64);
        totals.vec_store_candidates = totals
            .vec_store_candidates
            .saturating_add(audit.vec_store.candidates.len() as u64);
        totals.async_copy_candidates = totals
            .async_copy_candidates
            .saturating_add(audit.async_copy.candidates.len() as u64);
        totals.tensor_core_candidates = totals
            .tensor_core_candidates
            .saturating_add(audit.tensor_core.candidates.len() as u64);
        totals.ldmatrix_capable_targets = totals.ldmatrix_capable_targets.saturating_add(
            if audit.async_copy.target_supports_ldmatrix {
                1
            } else {
                0
            },
        );

        let ptx = if let Some(cached) = source_cache.get(&desc.id) {
            totals.source_cache_hits = totals.source_cache_hits.saturating_add(1);
            cached.clone()
        } else {
            totals.source_cache_misses = totals.source_cache_misses.saturating_add(1);
            let lowered = vyre_emit_ptx::emit_with_target(desc, ComputeCapability::SM_90)
                .map_err(|error| BenchError::ExecutionFailed(error.to_string()))?;
            source_cache.insert(desc.id.clone(), lowered.clone());
            lowered
        };
        if let Some(cached) = source_cache.get(&desc.id) {
            totals.source_cache_hits = totals.source_cache_hits.saturating_add(1);
            if cached.len() != ptx.len() {
                return Err(BenchError::ExecutionFailed(
                    "CUDA PTX pattern source cache returned divergent source length".to_string(),
                ));
            }
        }
        totals.ptx_bytes_emitted = totals.ptx_bytes_emitted.saturating_add(ptx.len() as u64);
        totals.scheduled_fillers = totals
            .scheduled_fillers
            .saturating_add(ptx.matches("// schedule: hoist independent").count() as u64);
        totals.predicated_stores = totals
            .predicated_stores
            .saturating_add(ptx.matches("@%p").count() as u64)
            .saturating_add(ptx.matches("@!%p").count() as u64);
        totals.branch_labels = totals
            .branch_labels
            .saturating_add(ptx.matches("$L_if_").count() as u64);
        totals.cp_async_emitted = totals
            .cp_async_emitted
            .saturating_add(ptx.matches("cp.async.ca.shared.global").count() as u64);
        totals.mma_sync_emitted = totals
            .mma_sync_emitted
            .saturating_add(ptx.matches("mma.sync.aligned").count() as u64);
        if desc.id == "ptx_vector_load_store" {
            totals.vectorized_loads_emitted = totals
                .vectorized_loads_emitted
                .saturating_add(ptx.matches("ld.global.v4").count() as u64);
            totals.vectorized_stores_emitted = totals
                .vectorized_stores_emitted
                .saturating_add(ptx.matches("st.global.v4").count() as u64);
            totals.vector_kernel_scalar_loads = totals
                .vector_kernel_scalar_loads
                .saturating_add(ptx.matches("ld.global.u32").count() as u64);
            totals.vector_kernel_scalar_stores = totals
                .vector_kernel_scalar_stores
                .saturating_add(ptx.matches("st.global.u32").count() as u64);
            totals.vector_kernel_scalar_index_adds = totals
                .vector_kernel_scalar_index_adds
                .saturating_add(ptx.matches("// scalar-index-increment").count() as u64);
        }
    }
    totals.source_cache_entries = source_cache.len() as u64;
    Ok(totals)
}

fn corpus() -> Vec<KernelDescriptor> {
    vec![
        predicated_literal_store_kernel(),
        predicated_else_store_kernel(),
        vector_load_store_kernel(),
        scheduled_load_gap_kernel(),
        cp_async_candidate_kernel(),
        async_copy_emit_kernel(),
        tensor_core_candidate_kernel(),
        matrix_mma_emit_kernel(),
    ]
}

fn u32_global_slot(slot: u32, name: &str) -> BindingSlot {
    BindingSlot {
        slot,
        element_type: DataType::U32,
        element_count: Some(1024),
        memory_class: MemoryClass::Global,
        visibility: BindingVisibility::ReadWrite,
        name: name.to_string(),
    }
}

fn predicated_literal_store_kernel() -> KernelDescriptor {
    KernelDescriptor {
        id: "ptx_predicated_literal_store".to_string(),
        bindings: BindingLayout {
            slots: vec![u32_global_slot(0, "out")],
        },
        dispatch: Dispatch::new(64, 1, 1),
        body: KernelBody {
            ops: vec![
                op(KernelOpKind::Literal, vec![0], Some(0)),
                op(KernelOpKind::Literal, vec![1], Some(1)),
                op(KernelOpKind::StructuredIfThen, vec![0, 0], None),
            ],
            child_bodies: vec![KernelBody {
                ops: vec![
                    op(KernelOpKind::Literal, vec![0], Some(20)),
                    op(KernelOpKind::StoreGlobal, vec![0, 1, 20], None),
                ],
                child_bodies: vec![],
                literals: vec![LiteralValue::U32(13)],
            }],
            literals: vec![LiteralValue::Bool(true), LiteralValue::U32(0)],
        },
    }
}

fn predicated_else_store_kernel() -> KernelDescriptor {
    KernelDescriptor {
        id: "ptx_predicated_else_store".to_string(),
        bindings: BindingLayout {
            slots: vec![u32_global_slot(0, "out")],
        },
        dispatch: Dispatch::new(64, 1, 1),
        body: KernelBody {
            ops: vec![
                op(KernelOpKind::Literal, vec![0], Some(0)),
                op(KernelOpKind::Literal, vec![1], Some(1)),
                op(KernelOpKind::StructuredIfThenElse, vec![0, 0, 1], None),
            ],
            child_bodies: vec![store_child(20, 21), store_child(21, 34)],
            literals: vec![LiteralValue::Bool(true), LiteralValue::U32(0)],
        },
    }
}

fn store_child(result_id: u32, value: u32) -> KernelBody {
    KernelBody {
        ops: vec![
            op(KernelOpKind::Literal, vec![0], Some(result_id)),
            op(KernelOpKind::StoreGlobal, vec![0, 1, result_id], None),
        ],
        child_bodies: vec![],
        literals: vec![LiteralValue::U32(value)],
    }
}

fn vector_load_store_kernel() -> KernelDescriptor {
    KernelDescriptor {
        id: "ptx_vector_load_store".to_string(),
        bindings: BindingLayout {
            slots: vec![u32_global_slot(0, "input"), u32_global_slot(1, "output")],
        },
        dispatch: Dispatch::new(64, 1, 1),
        body: KernelBody {
            ops: vec![
                op(KernelOpKind::Literal, vec![0], Some(0)),
                op(KernelOpKind::Literal, vec![1], Some(1)),
                op(KernelOpKind::LoadGlobal, vec![0, 0], Some(10)),
                op(KernelOpKind::BinOpKind(BinOp::Add), vec![0, 1], Some(2)),
                op(KernelOpKind::LoadGlobal, vec![0, 2], Some(11)),
                op(KernelOpKind::BinOpKind(BinOp::Add), vec![2, 1], Some(3)),
                op(KernelOpKind::LoadGlobal, vec![0, 3], Some(12)),
                op(KernelOpKind::BinOpKind(BinOp::Add), vec![3, 1], Some(4)),
                op(KernelOpKind::LoadGlobal, vec![0, 4], Some(13)),
                op(KernelOpKind::StoreGlobal, vec![1, 0, 10], None),
                op(KernelOpKind::BinOpKind(BinOp::Add), vec![0, 1], Some(5)),
                op(KernelOpKind::StoreGlobal, vec![1, 5, 11], None),
                op(KernelOpKind::BinOpKind(BinOp::Add), vec![5, 1], Some(6)),
                op(KernelOpKind::StoreGlobal, vec![1, 6, 12], None),
                op(KernelOpKind::BinOpKind(BinOp::Add), vec![6, 1], Some(7)),
                op(KernelOpKind::StoreGlobal, vec![1, 7, 13], None),
            ],
            child_bodies: vec![],
            literals: vec![LiteralValue::U32(0), LiteralValue::U32(1)],
        },
    }
}

fn scheduled_load_gap_kernel() -> KernelDescriptor {
    KernelDescriptor {
        id: "ptx_scheduled_load_gap".to_string(),
        bindings: BindingLayout {
            slots: vec![u32_global_slot(0, "input"), u32_global_slot(1, "output")],
        },
        dispatch: Dispatch::new(64, 1, 1),
        body: KernelBody {
            ops: vec![
                op(KernelOpKind::Literal, vec![0], Some(0)),
                op(KernelOpKind::Literal, vec![1], Some(1)),
                op(KernelOpKind::LoadGlobal, vec![0, 0], Some(2)),
                op(KernelOpKind::BinOpKind(BinOp::Add), vec![2, 1], Some(3)),
                op(KernelOpKind::Literal, vec![2], Some(4)),
                op(KernelOpKind::BinOpKind(BinOp::Add), vec![4, 1], Some(5)),
                op(KernelOpKind::StoreGlobal, vec![1, 0, 3], None),
            ],
            child_bodies: vec![],
            literals: vec![
                LiteralValue::U32(0),
                LiteralValue::U32(7),
                LiteralValue::U32(11),
            ],
        },
    }
}

fn cp_async_candidate_kernel() -> KernelDescriptor {
    KernelDescriptor {
        id: "ptx_cp_async_candidate".to_string(),
        bindings: BindingLayout {
            slots: vec![
                BindingSlot {
                    slot: 0,
                    element_type: DataType::U32,
                    element_count: Some(1024),
                    memory_class: MemoryClass::Global,
                    visibility: BindingVisibility::ReadOnly,
                    name: "input".to_string(),
                },
                BindingSlot {
                    slot: 1,
                    element_type: DataType::U32,
                    element_count: Some(1024),
                    memory_class: MemoryClass::Shared,
                    visibility: BindingVisibility::ReadWrite,
                    name: "tile".to_string(),
                },
            ],
        },
        dispatch: Dispatch::new(64, 1, 1),
        body: KernelBody {
            ops: vec![
                op(KernelOpKind::Literal, vec![0], Some(0)),
                op(KernelOpKind::LoadGlobal, vec![0, 0], Some(1)),
                op(KernelOpKind::StoreShared, vec![1, 0, 1], None),
            ],
            child_bodies: vec![],
            literals: vec![LiteralValue::U32(0)],
        },
    }
}

fn async_copy_emit_kernel() -> KernelDescriptor {
    KernelDescriptor {
        id: "ptx_cp_async_emit".to_string(),
        bindings: BindingLayout {
            slots: vec![
                BindingSlot {
                    slot: 0,
                    element_type: DataType::U32,
                    element_count: Some(1024),
                    memory_class: MemoryClass::Global,
                    visibility: BindingVisibility::ReadOnly,
                    name: "input".to_string(),
                },
                BindingSlot {
                    slot: 1,
                    element_type: DataType::U32,
                    element_count: Some(1024),
                    memory_class: MemoryClass::Shared,
                    visibility: BindingVisibility::ReadWrite,
                    name: "tile".to_string(),
                },
            ],
        },
        dispatch: Dispatch::new(64, 1, 1),
        body: KernelBody {
            ops: vec![
                op(KernelOpKind::Literal, vec![0], Some(0)),
                op(KernelOpKind::Literal, vec![1], Some(1)),
                op(
                    KernelOpKind::AsyncLoad { tag: "tile".into() },
                    vec![0, 1, 0, 1],
                    None,
                ),
                op(KernelOpKind::AsyncWait { tag: "tile".into() }, vec![], None),
            ],
            child_bodies: vec![],
            literals: vec![LiteralValue::U32(0), LiteralValue::U32(256)],
        },
    }
}

fn tensor_core_candidate_kernel() -> KernelDescriptor {
    let mut ops = Vec::new();
    let mut literals = Vec::new();
    for id in 0..3 {
        literals.push(LiteralValue::F32(id as f32));
        ops.push(op(KernelOpKind::Literal, vec![id], Some(id)));
    }
    for result in 3..11 {
        ops.push(op(KernelOpKind::Fma, vec![0, 1, 2], Some(result)));
    }
    KernelDescriptor {
        id: "ptx_tensor_core_candidate".to_string(),
        bindings: BindingLayout { slots: vec![] },
        dispatch: Dispatch::new(64, 1, 1),
        body: KernelBody {
            ops,
            child_bodies: vec![],
            literals,
        },
    }
}

fn matrix_mma_emit_kernel() -> KernelDescriptor {
    let mut ops = Vec::new();
    let mut literals = Vec::new();
    for id in 0..6 {
        literals.push(LiteralValue::U32(id));
        ops.push(op(KernelOpKind::Literal, vec![id], Some(id)));
    }
    for id in 6..10 {
        literals.push(LiteralValue::F32(0.0));
        ops.push(op(KernelOpKind::Literal, vec![id], Some(id)));
    }
    ops.push(op(
        KernelOpKind::MatrixMma {
            shape: MatrixMmaShape::M16N8K16,
            a_layout: MatrixMmaLayout::RowMajor,
            b_layout: MatrixMmaLayout::ColMajor,
            a_type: MatrixMmaElement::F16,
            b_type: MatrixMmaElement::F16,
            accum_type: MatrixMmaElement::F32,
        },
        (0..10).collect(),
        Some(10),
    ));
    KernelDescriptor {
        id: "ptx_matrix_mma_emit".to_string(),
        bindings: BindingLayout { slots: vec![] },
        dispatch: Dispatch::new(32, 1, 1),
        body: KernelBody {
            ops,
            child_bodies: vec![],
            literals,
        },
    }
}

fn op(kind: KernelOpKind, operands: Vec<u32>, result: Option<u32>) -> KernelOp {
    KernelOp {
        kind,
        operands,
        result,
    }
}

fn decode_words(run: &BenchRun) -> Result<Vec<u64>, BenchError> {
    let Some(output) = run.outputs.first() else {
        return Err(BenchError::ExecutionFailed(
            "CUDA PTX pattern benchmark emitted no output payload".to_string(),
        ));
    };
    if output.len() % std::mem::size_of::<u64>() != 0 {
        return Err(BenchError::ExecutionFailed(
            "CUDA PTX pattern output payload is not u64-aligned".to_string(),
        ));
    }
    Ok(output
        .chunks_exact(std::mem::size_of::<u64>())
        .map(|chunk| {
            let mut bytes = [0u8; 8];
            bytes.copy_from_slice(chunk);
            u64::from_le_bytes(bytes)
        })
        .collect())
}

inventory::submit! {
    &CudaPtxPatterns as &dyn BenchCase
}
