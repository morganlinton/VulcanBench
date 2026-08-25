use crate::api::case::{
    BenchCase, BenchContext, BenchError, BenchId, BenchLayer, BenchMetadata, BenchRequirements,
    BenchRun, Correctness, DeterminismClass, PerformanceContract, PreparedCase, WorkloadClass,
};
use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

pub struct Histogram;

impl BenchCase for Histogram {
    fn id(&self) -> BenchId {
        BenchId("foundation.histogram.u32_256.1m".to_string())
    }

    fn metadata(&self) -> BenchMetadata {
        BenchMetadata {
            id: self.id(),
            name: "Histogram U32 256-bin 1M".to_string(),
            description: "Atomic 256-bin histogram over 1M u32 values".to_string(),
            tags: vec![
                "memory-bound".to_string(),
                "atomics".to_string(),
                "histogram".to_string(),
            ],
            layer: BenchLayer::Foundation,
            workload: WorkloadClass::Micro,
            determinism: DeterminismClass::Deterministic,
            owner_crate: "vyre-bench".to_string(),
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
        None
    }

    fn prepare(&self, _ctx: &mut BenchContext) -> Result<PreparedCase, BenchError> {
        let count = 1_000_000u32;
        let prog = Program::wrapped(
            vec![
                BufferDecl::output("bins", 0, DataType::U32).with_count(256),
                BufferDecl::storage("values", 1, BufferAccess::ReadOnly, DataType::U32)
                    .with_count(count),
            ],
            [256, 1, 1],
            vec![
                Node::let_bind("idx", Expr::gid_x()),
                Node::if_then(
                    Expr::lt(Expr::var("idx"), Expr::u32(count)),
                    vec![Node::let_bind(
                        "old",
                        Expr::atomic_add(
                            "bins",
                            Expr::bitand(Expr::load("values", Expr::var("idx")), Expr::u32(255)),
                            Expr::u32(1),
                        ),
                    )],
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
        let count = 1_000_000usize;
        let mut values = vec![0u8; count * 4];
        for i in 0..count {
            values[i * 4..i * 4 + 4].copy_from_slice(&((i * 31 % 256) as u32).to_le_bytes());
        }
        let inputs = vec![values];

        crate::cases::gpu_case::run_gpu_with_cpu_baseline(
            ctx,
            prepared,
            inputs,
            count as u64,
            |inputs| {
                vec![crate::cases::cpu_baselines::histogram_u32_256_bytes(
                    &inputs[0],
                )]
            },
        )
    }

    fn verify(&self, _ctx: &mut BenchContext, run: &BenchRun) -> Result<Correctness, BenchError> {
        run.verify_exact_outputs()
    }
}

inventory::submit! {
    &Histogram as &'static dyn BenchCase
}
