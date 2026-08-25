//! `hashtable.openaddr.probe.10m`  -  Open-addressing hash table probe.
//!
//! Probes a prebuilt 10M-key table with 1M random lookups. GPU uses
//! open-addressing with linear probing on a power-of-2 table. CPU baseline uses
//! a prebuilt hashbrown table (robin-hood hashing, SIMD probing).
//!
//! This is CPU-favorable territory: hash tables are latency-bound with
//! pointer-chasing patterns that exploit CPU caches. The GPU must overcome
//! random-access memory latency via massive parallelism.

use crate::api::case::{
    BenchCase, BenchContext, BenchError, BenchId, BenchLayer, BenchMetadata, BenchRequirements,
    BenchRun, Correctness, DeterminismClass, PerformanceContract, PreparedCase, WorkloadClass,
};
use crate::api::metric::BenchMetrics;
use crate::api::resident::{
    dispatch_program_timed, input_bytes_total, transfer_accounting, ResidentInputSet,
};
use crate::api::suite::SuiteKind;
use hashbrown::HashMap;
use rand::{RngExt, SeedableRng};
use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

const KEY_COUNT: u32 = 10_000_000;
const PROBE_COUNT: u32 = 1_000_000;
const TABLE_SIZE: u32 = 16_777_216; // 2^24, load factor ~0.6

const HONEST_SUITES: &[SuiteKind] = &[
    SuiteKind::Honest,
    SuiteKind::Deep,
    SuiteKind::Release,
    SuiteKind::Smoke,
];

pub struct HashtableProbe;

struct HashtableProbePrepared {
    program: Program,
    inputs: Vec<Vec<u8>>,
    input_bytes_total: u64,
    probe_keys: Vec<u32>,
    cpu_table: HashMap<u32, u32>,
    resident: Option<ResidentInputSet>,
}

impl BenchCase for HashtableProbe {
    fn id(&self) -> BenchId {
        BenchId("hashtable.openaddr.probe.10m".to_string())
    }

    fn metadata(&self) -> BenchMetadata {
        BenchMetadata {
            id: self.id(),
            name: "Hashtable Probe 10M".to_string(),
            description: "Open-addressing hash table: probe 1M random lookups against a prebuilt 10M-key table"
                .to_string(),
            tags: vec![
                "honest".to_string(),
                "latency-bound".to_string(),
                "random-access".to_string(),
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
            min_vram_bytes: Some((TABLE_SIZE as u64) * 8 + (PROBE_COUNT as u64) * 4),
            min_input_bytes: None,
            feature_set: vec![],
        }
    }

    fn performance_contract(&self) -> Option<PerformanceContract> {
        Some(PerformanceContract::cpu_sota_10x(
            "Hash table probe",
            "hashbrown",
            "hashbrown 0.17.0 prebuilt SwissTable probe",
        ))
    }

    fn bytes_touched(&self, _prepared: &PreparedCase) -> (u64, u64) {
        // Read: table (key+value per slot) + probe keys
        // Write: probe results
        let read = (TABLE_SIZE as u64) * 8 + (PROBE_COUNT as u64) * 4;
        let write = (PROBE_COUNT as u64) * 4;
        (read, write)
    }

    fn prepare(&self, ctx: &mut BenchContext) -> Result<PreparedCase, BenchError> {
        // GPU kernel: linear-probed open-addressing lookup.
        // Buffer layout:
        //   slot 0: table_keys[TABLE_SIZE]   (u32, 0 = empty)
        //   slot 1: table_vals[TABLE_SIZE]   (u32)
        //   slot 2: probe_keys[PROBE_COUNT]  (u32, keys to look up)
        //   slot 3: results[PROBE_COUNT]     (u32, found values or 0)
        //
        // Each thread handles one probe key via linear probing.
        let program = Program::wrapped(
            vec![
                BufferDecl::storage("table_keys", 0, BufferAccess::ReadOnly, DataType::U32)
                    .with_count(TABLE_SIZE),
                BufferDecl::storage("table_vals", 1, BufferAccess::ReadOnly, DataType::U32)
                    .with_count(TABLE_SIZE),
                BufferDecl::storage("probe_keys", 2, BufferAccess::ReadOnly, DataType::U32)
                    .with_count(PROBE_COUNT),
                BufferDecl::output("results", 3, DataType::U32).with_count(PROBE_COUNT),
            ],
            [256, 1, 1],
            vec![
                Node::let_bind("tid", Expr::gid_x()),
                Node::if_then(
                    Expr::lt(Expr::var("tid"), Expr::u32(PROBE_COUNT)),
                    vec![
                        // Load probe key
                        Node::let_bind("key", Expr::load("probe_keys", Expr::var("tid"))),
                        // Hash: key * 2654435761 (Knuth multiplicative)
                        Node::let_bind(
                            "hash",
                            Expr::bitand(
                                Expr::mul(Expr::var("key"), Expr::u32(2_654_435_761)),
                                Expr::u32(TABLE_SIZE - 1), // mask for power-of-2
                            ),
                        ),
                        // Linear probe up to 64 slots
                        Node::let_bind("result", Expr::u32(0)),
                        Node::Loop {
                            var: "probe".into(),
                            from: Expr::u32(0),
                            to: Expr::u32(64),
                            body: vec![
                                Node::let_bind(
                                    "slot",
                                    Expr::bitand(
                                        Expr::add(Expr::var("hash"), Expr::var("probe")),
                                        Expr::u32(TABLE_SIZE - 1),
                                    ),
                                ),
                                Node::let_bind(
                                    "slot_key",
                                    Expr::load("table_keys", Expr::var("slot")),
                                ),
                                Node::if_then(
                                    Expr::eq(Expr::var("slot_key"), Expr::var("key")),
                                    vec![Node::assign(
                                        "result",
                                        Expr::load("table_vals", Expr::var("slot")),
                                    )],
                                ),
                            ],
                        },
                        Node::store("results", Expr::var("tid"), Expr::var("result")),
                    ],
                ),
            ],
        );
        let mut rng = rand::rngs::StdRng::seed_from_u64(0xDEAD_BEEF);

        let mut table_keys = vec![0u32; TABLE_SIZE as usize];
        let mut table_vals = vec![0u32; TABLE_SIZE as usize];
        let mask = TABLE_SIZE - 1;
        let mut cpu_table: HashMap<u32, u32> = HashMap::with_capacity(KEY_COUNT as usize);

        let mut inserted_keys = Vec::with_capacity(KEY_COUNT as usize);
        for i in 0..KEY_COUNT {
            let key = rng.random_range(1..u32::MAX); // 0 = empty sentinel
            let val = i + 1;
            let mut slot = key.wrapping_mul(2_654_435_761) & mask;
            for _ in 0..64 {
                if table_keys[slot as usize] == 0 {
                    table_keys[slot as usize] = key;
                    table_vals[slot as usize] = val;
                    inserted_keys.push(key);
                    cpu_table.insert(key, val);
                    break;
                }
                slot = (slot + 1) & mask;
            }
        }

        let mut probe_keys = vec![0u32; PROBE_COUNT as usize];
        for probe_key in &mut probe_keys {
            if rng.random_bool(0.8) && !inserted_keys.is_empty() {
                *probe_key = inserted_keys[rng.random_range(0..inserted_keys.len())];
            } else {
                *probe_key = rng.random_range(1..u32::MAX);
            }
        }

        let table_keys_bytes = vyre_primitives::wire::pack_u32_slice(&table_keys);
        let table_vals_bytes = vyre_primitives::wire::pack_u32_slice(&table_vals);
        let probe_keys_bytes = vyre_primitives::wire::pack_u32_slice(&probe_keys);
        let inputs = vec![table_keys_bytes, table_vals_bytes, probe_keys_bytes];
        let input_bytes_total = input_bytes_total(&inputs);
        let resident = ResidentInputSet::upload_program_ordered_with_zeroed_outputs_optional(
            ctx,
            &program,
            &inputs,
            "hashtable probe bench",
        )?;
        Ok(Box::new(HashtableProbePrepared {
            program,
            inputs,
            input_bytes_total,
            probe_keys,
            cpu_table,
            resident,
        }))
    }

    fn program<'a>(&self, prepared: &'a PreparedCase) -> Option<&'a Program> {
        prepared
            .downcast_ref::<HashtableProbePrepared>()
            .map(|prepared| &prepared.program)
    }

    fn run(
        &self,
        ctx: &mut BenchContext,
        prepared: &mut PreparedCase,
    ) -> Result<BenchRun, BenchError> {
        let prepared = prepared
            .downcast_ref::<HashtableProbePrepared>()
            .ok_or_else(|| {
                BenchError::ExecutionFailed("hashtable prepared payload type mismatch".to_string())
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
        let cpu_results: Vec<u8> = prepared
            .probe_keys
            .iter()
            .flat_map(|key| {
                prepared
                    .cpu_table
                    .get(key)
                    .copied()
                    .unwrap_or(0)
                    .to_le_bytes()
            })
            .collect();
        let elapsed_ref = start_ref.elapsed().as_nanos() as u64;
        let output_bytes = outputs.iter().map(Vec::len).sum::<usize>() as u64;
        let accounting =
            transfer_accounting(prepared.input_bytes_total, output_bytes, resident_used);

        Ok(BenchRun {
            metrics: BenchMetrics {
                wall_ns: Some(timed.wall_ns),
                dispatch_ns: timed.device_ns,
                input_bytes: Some(prepared.input_bytes_total),
                output_bytes: Some(output_bytes),
                bytes_read: Some(accounting.bytes_read),
                bytes_written: Some(accounting.bytes_written),
                bytes_touched: Some(accounting.bytes_touched),
                ..Default::default()
            },
            baseline_metrics: Some(BenchMetrics {
                wall_ns: Some(elapsed_ref),
                input_bytes: Some(prepared.input_bytes_total),
                output_bytes: Some(cpu_results.len() as u64),
                ..Default::default()
            }),
            outputs,
            baseline_outputs: Some(vec![cpu_results]),
        })
    }

    fn verify(&self, _ctx: &mut BenchContext, run: &BenchRun) -> Result<Correctness, BenchError> {
        run.verify_exact_outputs()
    }
}

inventory::submit! {
    &HashtableProbe as &'static dyn BenchCase
}
