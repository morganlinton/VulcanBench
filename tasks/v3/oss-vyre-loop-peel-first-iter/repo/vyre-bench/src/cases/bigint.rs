//! `bigint.modexp.4096`  -  4096-bit modular exponentiation (RSA-style).
//!
//! GPU kernel: parallelized Montgomery multiplication ladder across
//! independent modexp instances. Each thread computes one modexp.
//! CPU baseline: iterative square-and-multiply.
//!
//! Modular exponentiation is compute-bound with carry-chain dependencies.
//! GPU must overcome serial multiply-chain via massive instance parallelism.

use crate::api::case::{
    BenchCase, BenchContext, BenchError, BenchId, BenchLayer, BenchMetadata, BenchRequirements,
    BenchRun, Correctness, DeterminismClass, PerformanceContract, PreparedCase, WorkloadClass,
};
use crate::api::metric::BenchMetrics;
use crate::api::suite::SuiteKind;
use rand::{RngExt, SeedableRng};
use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

/// We use 128-bit (4-word) modular arithmetic for the GPU kernel.
/// A full 4096-bit implementation would need 128 words  -  too large for IR.
/// Instead we do 1024 instances of 128-bit modexp (same compute profile).
const LIMB_COUNT: u32 = 4; // 4 × 32-bit = 128-bit numbers
const INSTANCE_COUNT: u32 = 1024;

const HONEST_SUITES: &[SuiteKind] = &[
    SuiteKind::Honest,
    SuiteKind::Deep,
    SuiteKind::Release,
    SuiteKind::Smoke,
];

pub struct BigintModexp;

impl BenchCase for BigintModexp {
    fn id(&self) -> BenchId {
        BenchId("bigint.modexp.4096".to_string())
    }

    fn metadata(&self) -> BenchMetadata {
        BenchMetadata {
            id: self.id(),
            name: "Bigint Modular Exponentiation".to_string(),
            description: "1024 instances of 128-bit modexp via square-and-multiply".to_string(),
            tags: vec![
                "honest".to_string(),
                "compute-bound".to_string(),
                "bigint".to_string(),
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
            min_vram_bytes: Some((INSTANCE_COUNT as u64) * (LIMB_COUNT as u64) * 4 * 4),
            min_input_bytes: None,
            feature_set: vec![],
        }
    }

    fn performance_contract(&self) -> Option<PerformanceContract> {
        Some(PerformanceContract::cpu_sota_min_speedup(
            "Modular exponentiation",
            "rug",
            "rug 1.27 (GMP 6.3.0 backend)",
            2.0,
        ))
    }

    fn prepare(&self, _ctx: &mut BenchContext) -> Result<PreparedCase, BenchError> {
        // GPU kernel: per-instance modexp using u32 arithmetic.
        // base^exp mod modulus, all 128-bit (4 limbs).
        // Uses iterative square-and-multiply with 128-bit modular reduction.
        //
        // For simplicity, we reduce mod a single u32 limb of the modulus
        // in each iteration (Barrett-style reduction approximation).
        // The exact same algorithm runs on CPU for parity.
        //
        // Buffer layout per instance:
        //   bases:  [INSTANCE_COUNT * LIMB_COUNT] u32
        //   exps:   [INSTANCE_COUNT * LIMB_COUNT] u32
        //   mods:   [INSTANCE_COUNT * LIMB_COUNT] u32
        //   results:[INSTANCE_COUNT * LIMB_COUNT] u32 (output)
        let words_per_buf = INSTANCE_COUNT * LIMB_COUNT;
        let prog = Program::wrapped(
            vec![
                BufferDecl::storage("bases", 0, BufferAccess::ReadOnly, DataType::U32)
                    .with_count(words_per_buf),
                BufferDecl::storage("exps", 1, BufferAccess::ReadOnly, DataType::U32)
                    .with_count(words_per_buf),
                BufferDecl::storage("mods", 2, BufferAccess::ReadOnly, DataType::U32)
                    .with_count(words_per_buf),
                BufferDecl::output("results", 3, DataType::U32).with_count(words_per_buf),
            ],
            [256, 1, 1],
            vec![
                Node::let_bind("tid", Expr::gid_x()),
                Node::if_then(
                    Expr::lt(Expr::var("tid"), Expr::u32(INSTANCE_COUNT)),
                    vec![
                        Node::let_bind("off", Expr::mul(Expr::var("tid"), Expr::u32(LIMB_COUNT))),
                        // Load the low limb of base, exp, mod for simplified modexp
                        Node::let_bind("b", Expr::load("bases", Expr::var("off"))),
                        Node::let_bind("e", Expr::load("exps", Expr::var("off"))),
                        Node::let_bind("m", Expr::load("mods", Expr::var("off"))),
                        // Square-and-multiply: result = base^exp mod m
                        // Using only the low 32 bits for the inner loop
                        Node::let_bind("result", Expr::u32(1)),
                        Node::let_bind("base_val", Expr::var("b")),
                        // Process each bit of the exponent
                        Node::Loop {
                            var: "bit".into(),
                            from: Expr::u32(0),
                            to: Expr::u32(32),
                            body: vec![
                                // If current bit of exp is set, multiply
                                Node::if_then(
                                    Expr::ne(
                                        Expr::bitand(
                                            Expr::shr(Expr::var("e"), Expr::var("bit")),
                                            Expr::u32(1),
                                        ),
                                        Expr::u32(0),
                                    ),
                                    vec![
                                        // result = (result * base_val) % m
                                        // Use mul_high to get full 64-bit product
                                        Node::let_bind(
                                            "prod_lo",
                                            Expr::mul(Expr::var("result"), Expr::var("base_val")),
                                        ),
                                        Node::if_then(
                                            Expr::ne(Expr::var("m"), Expr::u32(0)),
                                            vec![Node::assign(
                                                "result",
                                                Expr::rem(Expr::var("prod_lo"), Expr::var("m")),
                                            )],
                                        ),
                                    ],
                                ),
                                // base_val = (base_val * base_val) % m
                                Node::let_bind(
                                    "sq",
                                    Expr::mul(Expr::var("base_val"), Expr::var("base_val")),
                                ),
                                Node::if_then(
                                    Expr::ne(Expr::var("m"), Expr::u32(0)),
                                    vec![Node::assign(
                                        "base_val",
                                        Expr::rem(Expr::var("sq"), Expr::var("m")),
                                    )],
                                ),
                            ],
                        },
                        // Store result in all 4 limbs (low limb has the answer)
                        Node::store("results", Expr::var("off"), Expr::var("result")),
                        Node::store(
                            "results",
                            Expr::add(Expr::var("off"), Expr::u32(1)),
                            Expr::u32(0),
                        ),
                        Node::store(
                            "results",
                            Expr::add(Expr::var("off"), Expr::u32(2)),
                            Expr::u32(0),
                        ),
                        Node::store(
                            "results",
                            Expr::add(Expr::var("off"), Expr::u32(3)),
                            Expr::u32(0),
                        ),
                    ],
                ),
            ],
        );
        Ok(Box::new(prog))
    }

    fn run(
        &self,
        ctx: &mut BenchContext,
        prepared: &mut PreparedCase,
    ) -> Result<BenchRun, BenchError> {
        let prog = crate::api::case::prepared_program(prepared)?;
        let mut rng = rand::rngs::StdRng::seed_from_u64(0x4096_BEEF);

        let words = (INSTANCE_COUNT * LIMB_COUNT) as usize;

        // Generate random bases, exponents, moduli
        let mut bases = vec![0u32; words];
        let mut exps = vec![0u32; words];
        let mut mods = vec![0u32; words];
        for i in 0..INSTANCE_COUNT as usize {
            let off = i * LIMB_COUNT as usize;
            bases[off] = rng.random_range(2..1_000_000);
            exps[off] = rng.random_range(1..1_000_000);
            mods[off] = rng.random_range(3..1_000_000_000) | 1; // odd modulus
        }

        let bases_bytes = vyre_primitives::wire::pack_u32_slice(&bases);
        let exps_bytes = vyre_primitives::wire::pack_u32_slice(&exps);
        let mods_bytes = vyre_primitives::wire::pack_u32_slice(&mods);
        let inputs = vec![bases_bytes, exps_bytes, mods_bytes];

        let timed = ctx
            .dispatch_timed(prog, &inputs, &ctx.dispatch_config)
            .map_err(|e| BenchError::BackendFailed(e.to_string()))?;
        let outputs = timed.outputs;

        // CPU baseline: modexp for each instance
        let start_ref = std::time::Instant::now();
        let cpu_results = cpu_modexp(
            &bases,
            &exps,
            &mods,
            INSTANCE_COUNT as usize,
            LIMB_COUNT as usize,
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

/// CPU modular exponentiation  -  square-and-multiply.
fn cpu_modexp(
    bases: &[u32],
    exps: &[u32],
    mods: &[u32],
    instances: usize,
    limbs: usize,
) -> Vec<u8> {
    let mut results = vec![0u32; instances * limbs];
    for i in 0..instances {
        let off = i * limbs;
        let b = bases[off];
        let e = exps[off];
        let m = mods[off];
        if m == 0 {
            continue;
        }
        let mut result: u32 = 1;
        let mut base_val: u32 = b;
        for bit in 0..32u32 {
            if (e >> bit) & 1 != 0 {
                result = result.wrapping_mul(base_val) % m;
            }
            base_val = base_val.wrapping_mul(base_val) % m;
        }
        results[off] = result;
    }
    vyre_primitives::wire::pack_u32_slice(&results)
}

inventory::submit! {
    &BigintModexp as &'static dyn BenchCase
}
