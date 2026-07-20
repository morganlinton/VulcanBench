use crate::api::case::{
    BenchCase, BenchContext, BenchError, BenchId, BenchLayer, BenchMetadata, BenchRequirements,
    BenchRun, Correctness, DeterminismClass, PerformanceContract, PreparedCase, WorkloadClass,
};
use crate::api::metric::BenchMetrics;
use crate::api::suite::SuiteKind;
use std::hint::black_box;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

pub struct FlakyCase;

static FLAKY_RUN_COUNTER: AtomicU64 = AtomicU64::new(0);

impl BenchCase for FlakyCase {
    fn id(&self) -> BenchId {
        BenchId("synthetic.flaky".to_string())
    }

    fn metadata(&self) -> BenchMetadata {
        BenchMetadata {
            id: self.id(),
            name: "Flaky Synthetic".to_string(),
            description: "A case that fluctuates randomly".to_string(),
            tags: vec!["synthetic".to_string()],
            layer: BenchLayer::Foundation,
            workload: WorkloadClass::Micro,
            determinism: DeterminismClass::NonDeterministic,
            owner_crate: "vyre-bench".to_string(),
        }
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

    fn performance_contract(&self) -> Option<PerformanceContract> {
        None
    }

    fn active_in_suite(&self, suite: SuiteKind) -> bool {
        matches!(suite, SuiteKind::Custom(name) if name == "flaky_test")
    }

    fn prepare(&self, _ctx: &mut BenchContext) -> Result<PreparedCase, BenchError> {
        Ok(Box::new(()))
    }

    fn run(
        &self,
        _ctx: &mut BenchContext,
        _prepared: &mut PreparedCase,
    ) -> Result<BenchRun, BenchError> {
        let baseline_start = Instant::now();
        let mut baseline_acc = 0u64;
        for i in 0..4_096u64 {
            baseline_acc = baseline_acc.wrapping_add(black_box(i.rotate_left((i % 31) as u32)));
        }
        black_box(baseline_acc);
        let baseline_wall_ns = baseline_start.elapsed().as_nanos() as u64;

        let started = Instant::now();
        let mut acc = 0u64;
        let run_index = FLAKY_RUN_COUNTER.fetch_add(1, Ordering::Relaxed);
        let measured_block = run_index.saturating_sub(1) / 30;
        let iterations = if measured_block % 2 == 0 {
            8_192u64
        } else {
            262_144u64
        };
        for i in 0..iterations {
            acc = acc.wrapping_add(black_box(i.rotate_left((i % 31) as u32)));
        }
        black_box(acc);
        let wall_ns = started.elapsed().as_nanos() as u64;

        Ok(BenchRun {
            metrics: BenchMetrics {
                wall_ns: Some(wall_ns.max(1)),
                ..Default::default()
            },
            baseline_metrics: Some(BenchMetrics {
                wall_ns: Some(baseline_wall_ns.max(1)),
                ..Default::default()
            }),
            outputs: vec![],
            baseline_outputs: Some(vec![]),
        })
    }

    fn verify(&self, _ctx: &mut BenchContext, run: &BenchRun) -> Result<Correctness, BenchError> {
        run.verify_exact_outputs()
    }
}

inventory::submit! {
    &FlakyCase as &'static dyn BenchCase
}
