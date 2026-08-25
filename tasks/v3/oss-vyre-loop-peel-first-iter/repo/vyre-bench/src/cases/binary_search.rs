//! `search.binary.u32.1m`  -  divergent binary search over a sorted table.

use super::byte_pack::u32_bytes;
use crate::api::case::{
    BenchCase, BenchContext, BenchError, BenchId, BenchLayer, BenchMetadata, BenchRequirements,
    BenchRun, Correctness, DeterminismClass, PerformanceContract, PreparedCase, WorkloadClass,
};
use crate::api::metric::BenchMetrics;
use crate::api::resident::{
    dispatch_program_timed, input_bytes_total, transfer_accounting, ResidentInputSet,
};
use crate::api::suite::SuiteKind;
use rayon::prelude::*;
use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

const KEY_COUNT: u32 = 1 << 20;
const QUERY_COUNT: u32 = 1 << 20;
const MISS: u32 = u32::MAX;
const SEARCH_STEPS: u32 = 21;

const HONEST_SUITES: &[SuiteKind] = &[SuiteKind::Honest, SuiteKind::Deep, SuiteKind::Release];

pub struct BinarySearchU32;

struct BinarySearchPrepared {
    program: Program,
    keys: Vec<u32>,
    queries: Vec<u32>,
    inputs: Vec<Vec<u8>>,
    input_bytes_total: u64,
    resident: Option<ResidentInputSet>,
}

impl BenchCase for BinarySearchU32 {
    fn id(&self) -> BenchId {
        BenchId("search.binary.u32.1m".to_string())
    }

    fn metadata(&self) -> BenchMetadata {
        BenchMetadata {
            id: self.id(),
            name: "Binary Search U32 1M".to_string(),
            description: "Divergent binary search: 1M queries against a sorted 1M-entry u32 table"
                .to_string(),
            tags: vec![
                "honest".to_string(),
                "cpu-favorable".to_string(),
                "branchy".to_string(),
                "cache".to_string(),
            ],
            layer: BenchLayer::Honest,
            workload: WorkloadClass::Honest,
            determinism: DeterminismClass::Deterministic,
            owner_crate: "vyre-bench".to_string(),
        }
    }

    fn suites(&self) -> &'static [SuiteKind] {
        HONEST_SUITES
    }

    fn requirements(&self) -> BenchRequirements {
        BenchRequirements {
            needs_gpu: true,
            needs_network: false,
            min_vram_bytes: Some((u64::from(KEY_COUNT) + u64::from(QUERY_COUNT) * 2) * 4),
            min_input_bytes: None,
            feature_set: vec![],
        }
    }

    fn performance_contract(&self) -> Option<PerformanceContract> {
        Some(PerformanceContract::cpu_sota_3x(
            "Divergent binary search",
            "std+rayon",
            "Rust slice::binary_search with Rayon parallel query partitioning",
        ))
    }

    fn bytes_touched(&self, prepared: &PreparedCase) -> (u64, u64) {
        let read = prepared
            .downcast_ref::<BinarySearchPrepared>()
            .map(|prepared| prepared.input_bytes_total)
            .unwrap_or_else(|| (u64::from(KEY_COUNT) + u64::from(QUERY_COUNT)) * 4);
        let write = u64::from(QUERY_COUNT) * 4;
        (read, write)
    }

    fn prepare(&self, ctx: &mut BenchContext) -> Result<PreparedCase, BenchError> {
        let program = Program::wrapped(
            vec![
                BufferDecl::storage("keys", 0, BufferAccess::ReadOnly, DataType::U32)
                    .with_count(KEY_COUNT),
                BufferDecl::storage("queries", 1, BufferAccess::ReadOnly, DataType::U32)
                    .with_count(QUERY_COUNT),
                BufferDecl::output("results", 2, DataType::U32).with_count(QUERY_COUNT),
            ],
            [256, 1, 1],
            vec![
                Node::let_bind("tid", Expr::gid_x()),
                Node::if_then(
                    Expr::lt(Expr::var("tid"), Expr::u32(QUERY_COUNT)),
                    vec![
                        Node::let_bind("query", Expr::load("queries", Expr::var("tid"))),
                        Node::let_bind("low", Expr::u32(0)),
                        Node::let_bind("high", Expr::u32(KEY_COUNT)),
                        Node::Loop {
                            var: "step".into(),
                            from: Expr::u32(0),
                            to: Expr::u32(SEARCH_STEPS),
                            body: vec![
                                Node::let_bind(
                                    "mid",
                                    Expr::shr(
                                        Expr::add(Expr::var("low"), Expr::var("high")),
                                        Expr::u32(1),
                                    ),
                                ),
                                Node::let_bind("mid_key", Expr::load("keys", Expr::var("mid"))),
                                Node::let_bind(
                                    "go_right",
                                    Expr::lt(Expr::var("mid_key"), Expr::var("query")),
                                ),
                                Node::let_bind(
                                    "next_low",
                                    Expr::select(
                                        Expr::var("go_right"),
                                        Expr::add(Expr::var("mid"), Expr::u32(1)),
                                        Expr::var("low"),
                                    ),
                                ),
                                Node::let_bind(
                                    "next_high",
                                    Expr::select(
                                        Expr::var("go_right"),
                                        Expr::var("high"),
                                        Expr::var("mid"),
                                    ),
                                ),
                                Node::assign("low", Expr::var("next_low")),
                                Node::assign("high", Expr::var("next_high")),
                            ],
                        },
                        Node::if_then_else(
                            Expr::lt(Expr::var("low"), Expr::u32(KEY_COUNT)),
                            vec![
                                Node::let_bind("candidate", Expr::load("keys", Expr::var("low"))),
                                Node::if_then_else(
                                    Expr::eq(Expr::var("candidate"), Expr::var("query")),
                                    vec![Node::store(
                                        "results",
                                        Expr::var("tid"),
                                        Expr::var("low"),
                                    )],
                                    vec![Node::store("results", Expr::var("tid"), Expr::u32(MISS))],
                                ),
                            ],
                            vec![Node::store("results", Expr::var("tid"), Expr::u32(MISS))],
                        ),
                    ],
                ),
            ],
        );
        let keys: Vec<u32> = (0..KEY_COUNT)
            .map(|value| value.saturating_mul(2))
            .collect();
        let queries = build_queries();
        let inputs = vec![u32_bytes(&keys), u32_bytes(&queries)];
        let input_bytes_total = input_bytes_total(&inputs);
        let resident = ResidentInputSet::upload_optional(ctx, &inputs, "binary search bench")?;

        Ok(Box::new(BinarySearchPrepared {
            program,
            keys,
            queries,
            inputs,
            input_bytes_total,
            resident,
        }))
    }

    fn program<'a>(&self, prepared: &'a PreparedCase) -> Option<&'a Program> {
        prepared
            .downcast_ref::<BinarySearchPrepared>()
            .map(|prepared| &prepared.program)
    }

    fn run(
        &self,
        ctx: &mut BenchContext,
        prepared: &mut PreparedCase,
    ) -> Result<BenchRun, BenchError> {
        let prepared = prepared
            .downcast_ref::<BinarySearchPrepared>()
            .ok_or_else(|| {
                BenchError::ExecutionFailed(
                    "binary search prepared payload type mismatch".to_string(),
                )
            })?;

        let dispatch = dispatch_program_timed(
            ctx,
            &prepared.program,
            prepared.resident.as_ref(),
            &prepared.inputs,
            &ctx.dispatch_config,
        )?;
        let resident_used = dispatch.resident_used;
        let timed = dispatch.timed;
        let outputs = timed.outputs;

        let start_ref = std::time::Instant::now();
        let baseline = cpu_binary_search_results(&prepared.keys, &prepared.queries);
        let elapsed_ref = start_ref.elapsed().as_nanos() as u64;
        let input_bytes = prepared.input_bytes_total;
        let output_bytes = outputs.iter().map(Vec::len).sum::<usize>() as u64;
        let accounting = transfer_accounting(input_bytes, output_bytes, resident_used);

        Ok(BenchRun {
            metrics: BenchMetrics {
                wall_ns: Some(timed.wall_ns),
                dispatch_ns: timed.device_ns,
                input_bytes: Some(input_bytes),
                output_bytes: Some(output_bytes),
                bytes_read: Some(accounting.bytes_read),
                bytes_written: Some(accounting.bytes_written),
                bytes_touched: Some(accounting.bytes_touched),
                ..Default::default()
            },
            baseline_metrics: Some(BenchMetrics {
                wall_ns: Some(elapsed_ref),
                input_bytes: Some(input_bytes),
                output_bytes: Some(baseline.len() as u64),
                ..Default::default()
            }),
            outputs,
            baseline_outputs: Some(vec![baseline]),
        })
    }

    fn verify(&self, _ctx: &mut BenchContext, run: &BenchRun) -> Result<Correctness, BenchError> {
        run.verify_exact_outputs()
    }
}

fn build_queries() -> Vec<u32> {
    (0..QUERY_COUNT)
        .map(|idx| {
            let mixed = idx.wrapping_mul(747_796_405).wrapping_add(2_891_336_453);
            let slot = mixed & (KEY_COUNT - 1);
            if idx % 10 < 7 {
                slot.saturating_mul(2)
            } else {
                slot.saturating_mul(2).saturating_add(1)
            }
        })
        .collect()
}

fn cpu_binary_search_results(keys: &[u32], queries: &[u32]) -> Vec<u8> {
    let results: Vec<u32> = queries
        .par_iter()
        .map(|query| {
            keys.binary_search(query)
                .map(|index| index as u32)
                .unwrap_or(MISS)
        })
        .collect();
    u32_bytes(&results)
}

inventory::submit! {
    &BinarySearchU32 as &'static dyn BenchCase
}
