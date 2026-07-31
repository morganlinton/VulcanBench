//! `interpreter.bytecode.dispatch.10m`  -  Threaded bytecode interpreter.
//!
//! Executes a 10M-instruction trace of a simple stack-based bytecode VM.
//! GPU kernel interprets opcodes in parallel over independent program instances.
//! CPU baseline uses a hand-tuned switch-dispatch loop.
//!
//! This is deeply CPU-favorable: branch prediction makes CPU interpreters
//! fast despite being serial. The GPU must amortize branch divergence via
//! massive parallelism over independent program instances.

use crate::api::case::{
    BenchCase, BenchContext, BenchError, BenchId, BenchLayer, BenchMetadata, BenchRequirements,
    BenchRun, Correctness, DeterminismClass, PerformanceContract, PreparedCase, WorkloadClass,
};
use crate::api::metric::BenchMetrics;
use crate::api::suite::SuiteKind;
use rand::{RngExt, SeedableRng};
use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

/// Number of independent program instances to run in parallel.
const INSTANCE_COUNT: u32 = 4096;
/// Instructions per instance.
const INSTRS_PER_INSTANCE: u32 = 2500;
/// Total instruction words = INSTANCE_COUNT * INSTRS_PER_INSTANCE = 10M
const TOTAL_INSTRS: u32 = INSTANCE_COUNT * INSTRS_PER_INSTANCE;

// Opcodes (encoded in low 8 bits of u32 instruction word)
const OP_PUSH: u32 = 0;
const OP_ADD: u32 = 1;
const OP_MUL: u32 = 2;
const OP_DUP: u32 = 3;
const OP_SWAP: u32 = 4;

const HONEST_SUITES: &[SuiteKind] = &[
    SuiteKind::Honest,
    SuiteKind::Deep,
    SuiteKind::Release,
    SuiteKind::Smoke,
];

pub struct BytecodeDispatch;

impl BenchCase for BytecodeDispatch {
    fn id(&self) -> BenchId {
        BenchId("interpreter.bytecode.dispatch.10m".to_string())
    }

    fn metadata(&self) -> BenchMetadata {
        BenchMetadata {
            id: self.id(),
            name: "Bytecode Interpreter 10M".to_string(),
            description: "Stack-based bytecode VM: 4096 instances × 2500 instructions each"
                .to_string(),
            tags: vec![
                "honest".to_string(),
                "branch-heavy".to_string(),
                "serial".to_string(),
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
            min_vram_bytes: Some((TOTAL_INSTRS as u64) * 4 + (INSTANCE_COUNT as u64) * 4),
            min_input_bytes: None,
            feature_set: vec![],
        }
    }

    fn performance_contract(&self) -> Option<PerformanceContract> {
        Some(PerformanceContract::cpu_sota_3x(
            "Bytecode interpreter",
            "hand-tuned C threaded interpreter",
            "switch-dispatch loop with computed goto",
        ))
    }

    fn prepare(&self, _ctx: &mut BenchContext) -> Result<PreparedCase, BenchError> {
        let prog = bytecode_program(INSTANCE_COUNT, INSTRS_PER_INSTANCE);
        Ok(Box::new(prog))
    }

    fn run(
        &self,
        ctx: &mut BenchContext,
        prepared: &mut PreparedCase,
    ) -> Result<BenchRun, BenchError> {
        let prog = crate::api::case::prepared_program(prepared)?;
        let mut rng = rand::rngs::StdRng::seed_from_u64(0xCAFE_BABE);

        // Generate random instruction trace
        let mut instrs = vec![0u32; TOTAL_INSTRS as usize];
        for instr in &mut instrs {
            let op = rng.random_range(0..5u32);
            let imm = if op == OP_PUSH {
                rng.random_range(1..256u32)
            } else {
                0
            };
            *instr = op | (imm << 8);
        }

        let instrs_bytes = vyre_primitives::wire::pack_u32_slice(&instrs);
        let inputs = vec![instrs_bytes];

        // GPU dispatch
        let timed = ctx
            .dispatch_timed(prog, &inputs, &ctx.dispatch_config)
            .map_err(|e| BenchError::BackendFailed(e.to_string()))?;
        let outputs = timed.outputs;

        // CPU baseline: interpret the same bytecode on CPU
        let start_ref = std::time::Instant::now();
        let cpu_results = cpu_interpret(
            &instrs,
            INSTANCE_COUNT as usize,
            INSTRS_PER_INSTANCE as usize,
        );
        let elapsed_ref = start_ref.elapsed().as_nanos() as u64;

        Ok(BenchRun {
            metrics: BenchMetrics {
                wall_ns: Some(timed.wall_ns),
                dispatch_ns: timed.device_ns,
                input_bytes: Some(inputs.iter().map(Vec::len).sum::<usize>() as u64),
                output_bytes: Some(outputs.iter().map(Vec::len).sum::<usize>() as u64),
                ..Default::default()
            },
            baseline_metrics: Some(BenchMetrics {
                wall_ns: Some(elapsed_ref),
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

fn bytecode_program(instance_count: u32, instrs_per_instance: u32) -> Program {
    let total_instrs = instance_count.saturating_mul(instrs_per_instance);
    Program::wrapped(
        vec![
            BufferDecl::storage("instrs", 0, BufferAccess::ReadOnly, DataType::U32)
                .with_count(total_instrs),
            BufferDecl::output("results", 1, DataType::U32).with_count(instance_count),
        ],
        [256, 1, 1],
        vec![
            Node::let_bind("tid", Expr::gid_x()),
            Node::if_then(
                Expr::lt(Expr::var("tid"), Expr::u32(instance_count)),
                vec![
                    Node::let_bind(
                        "base",
                        Expr::mul(Expr::var("tid"), Expr::u32(instrs_per_instance)),
                    ),
                    Node::let_bind("s0", Expr::u32(0)),
                    Node::let_bind("s1", Expr::u32(0)),
                    Node::let_bind("s2", Expr::u32(0)),
                    Node::let_bind("s3", Expr::u32(0)),
                    Node::Loop {
                        var: "pc".into(),
                        from: Expr::u32(0),
                        to: Expr::u32(instrs_per_instance),
                        body: vec![
                            Node::let_bind(
                                "instr",
                                Expr::load("instrs", Expr::add(Expr::var("base"), Expr::var("pc"))),
                            ),
                            Node::let_bind("op", Expr::bitand(Expr::var("instr"), Expr::u32(0xFF))),
                            Node::let_bind("imm", Expr::shr(Expr::var("instr"), Expr::u32(8))),
                            Node::if_then(
                                Expr::eq(Expr::var("op"), Expr::u32(OP_PUSH)),
                                vec![
                                    Node::assign("s3", Expr::var("s2")),
                                    Node::assign("s2", Expr::var("s1")),
                                    Node::assign("s1", Expr::var("s0")),
                                    Node::assign("s0", Expr::var("imm")),
                                ],
                            ),
                            Node::if_then(
                                Expr::eq(Expr::var("op"), Expr::u32(OP_ADD)),
                                vec![
                                    Node::assign("s0", Expr::add(Expr::var("s0"), Expr::var("s1"))),
                                    Node::assign("s1", Expr::var("s2")),
                                    Node::assign("s2", Expr::var("s3")),
                                    Node::assign("s3", Expr::u32(0)),
                                ],
                            ),
                            Node::if_then(
                                Expr::eq(Expr::var("op"), Expr::u32(OP_MUL)),
                                vec![
                                    Node::assign("s0", Expr::mul(Expr::var("s0"), Expr::var("s1"))),
                                    Node::assign("s1", Expr::var("s2")),
                                    Node::assign("s2", Expr::var("s3")),
                                    Node::assign("s3", Expr::u32(0)),
                                ],
                            ),
                            Node::if_then(
                                Expr::eq(Expr::var("op"), Expr::u32(OP_DUP)),
                                vec![
                                    Node::assign("s3", Expr::var("s2")),
                                    Node::assign("s2", Expr::var("s1")),
                                    Node::assign("s1", Expr::var("s0")),
                                ],
                            ),
                            Node::if_then(
                                Expr::eq(Expr::var("op"), Expr::u32(OP_SWAP)),
                                vec![
                                    Node::let_bind("tmp", Expr::var("s0")),
                                    Node::assign("s0", Expr::var("s1")),
                                    Node::assign("s1", Expr::var("tmp")),
                                ],
                            ),
                        ],
                    },
                    Node::store("results", Expr::var("tid"), Expr::var("s0")),
                ],
            ),
        ],
    )
}

/// CPU interpreter  -  processes bytecode with a simple switch loop.
fn cpu_interpret(instrs: &[u32], instances: usize, instrs_per: usize) -> Vec<u8> {
    let mut results = vec![0u32; instances];
    for instance in 0..instances {
        let base = instance * instrs_per;
        let mut s = [0u32; 4];
        for pc in 0..instrs_per {
            let instr = instrs[base + pc];
            let op = instr & 0xFF;
            let imm = instr >> 8;
            match op {
                0 => {
                    // PUSH
                    s[3] = s[2];
                    s[2] = s[1];
                    s[1] = s[0];
                    s[0] = imm;
                }
                1 => {
                    // ADD
                    s[0] = s[0].wrapping_add(s[1]);
                    s[1] = s[2];
                    s[2] = s[3];
                    s[3] = 0;
                }
                2 => {
                    // MUL
                    s[0] = s[0].wrapping_mul(s[1]);
                    s[1] = s[2];
                    s[2] = s[3];
                    s[3] = 0;
                }
                3 => {
                    // DUP
                    s[3] = s[2];
                    s[2] = s[1];
                    s[1] = s[0];
                }
                4 => {
                    // SWAP
                    s.swap(0, 1);
                }
                _ => {}
            }
        }
        results[instance] = s[0];
    }
    vyre_primitives::wire::pack_u32_slice(&results)
}

inventory::submit! {
    &BytecodeDispatch as &'static dyn BenchCase
}

#[cfg(test)]
mod tests {
    use super::*;
    use vyre_driver::{DispatchConfig, VyreBackend};

    fn stack_carrier_snapshot_instrs() -> Vec<u32> {
        vec![
            OP_SWAP,
            OP_SWAP,
            OP_PUSH | (192 << 8),
            OP_ADD,
            OP_PUSH | (222 << 8),
            OP_SWAP,
            OP_MUL,
        ]
    }

    #[test]
    fn bytecode_program_matches_cpu_reference_on_stack_ops() {
        let instrs = vec![
            OP_PUSH | (2 << 8),
            OP_PUSH | (3 << 8),
            OP_ADD,
            OP_DUP,
            OP_PUSH | (5 << 8),
            OP_SWAP,
            OP_MUL,
        ];
        let inputs = vec![vyre_primitives::wire::pack_u32_slice(&instrs)];
        let program = bytecode_program(1, instrs.len() as u32);
        let outputs = vyre_driver_reference::CpuRefBackend
            .dispatch(&program, &inputs, &DispatchConfig::default())
            .expect("Fix: cpu-ref bytecode VM dispatch must succeed");
        let expected = cpu_interpret(&instrs, 1, instrs.len());

        assert_eq!(
            outputs,
            vec![expected],
            "official IR reference semantics must match the bytecode benchmark baseline"
        );
    }

    #[test]
    fn bytecode_program_wgpu_matches_seeded_cpu_trace() {
        let instrs = stack_carrier_snapshot_instrs();
        let backend = vyre_driver_wgpu::WgpuBackend::new()
            .expect("Fix: wgpu backend must initialize on the release GPU machine");
        let inputs = vec![vyre_primitives::wire::pack_u32_slice(&instrs)];
        let program = bytecode_program(1, instrs.len() as u32);
        let outputs = backend
            .dispatch(&program, &inputs, &DispatchConfig::default())
            .expect("Fix: wgpu bytecode VM dispatch must succeed");
        let expected = cpu_interpret(&instrs, 1, instrs.len());

        assert_eq!(
            outputs,
            vec![expected],
            "wgpu must snapshot stack carriers during SWAP instead of aliasing later carrier writes"
        );
    }

    #[test]
    fn bytecode_program_cuda_matches_seeded_cpu_trace() {
        let instrs = stack_carrier_snapshot_instrs();
        let backend = vyre_driver_cuda::CudaBackend::acquire()
            .expect("Fix: CUDA backend must initialize on the release GPU machine");
        let inputs = vec![vyre_primitives::wire::pack_u32_slice(&instrs)];
        let program = bytecode_program(1, instrs.len() as u32);
        let outputs = backend
            .dispatch(&program, &inputs, &DispatchConfig::default())
            .expect("Fix: CUDA bytecode VM dispatch must succeed");
        let expected = cpu_interpret(&instrs, 1, instrs.len());

        assert_eq!(
            outputs,
            vec![expected],
            "CUDA must snapshot stack carriers during SWAP instead of aliasing later carrier writes"
        );
    }
}
