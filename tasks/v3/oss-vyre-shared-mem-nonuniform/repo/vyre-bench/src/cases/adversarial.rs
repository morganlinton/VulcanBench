use crate::api::case::{
    BenchCase, BenchContext, BenchId, BenchLayer, BenchMetadata, BenchRequirements, BenchRun,
    Correctness, DeterminismClass, WorkloadClass,
};
use crate::api::metric::BenchMetrics;
use crate::api::suite::SuiteKind;
use rand::{RngExt, SeedableRng};
use vyre_foundation::ir::*;

pub struct RegisterExhaustionCase;

const REGISTER_EXHAUSTION_SUITES: &[SuiteKind] =
    &[SuiteKind::Adversarial, SuiteKind::Deep, SuiteKind::Release];

impl BenchCase for RegisterExhaustionCase {
    fn id(&self) -> BenchId {
        BenchId("adversarial.register_exhaustion.u32_1024".to_string())
    }

    fn metadata(&self) -> BenchMetadata {
        BenchMetadata {
            id: self.id(),
            name: "Register Exhaustion".to_string(),
            description: "Generates a highly-nested set of independent live variables to stress-test register allocators".to_string(),
            tags: vec!["adversarial".to_string(), "compiler".to_string()],
            layer: BenchLayer::Backend,
            workload: WorkloadClass::Adversarial,
            determinism: DeterminismClass::Deterministic,
            owner_crate: "vyre-bench".to_string(),
        }
    }

    fn suites(&self) -> &'static [SuiteKind] {
        REGISTER_EXHAUSTION_SUITES
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

    fn prepare(
        &self,
        _ctx: &mut BenchContext,
    ) -> Result<crate::api::case::PreparedCase, crate::api::case::BenchError> {
        let mut body = Vec::new();
        body.push(Node::let_bind("tid", Expr::gid_x()));

        // Initialize variables
        for i in 0..100 {
            body.push(Node::let_bind(
                format!("v{i}"),
                Expr::add(Expr::var("tid"), Expr::u32(i as u32)),
            ));
        }

        // Dummy mixing loop to keep them alive
        let mut loop_body = Vec::new();
        for i in 0..100 {
            let next = (i + 1) % 100;
            loop_body.push(Node::assign(
                format!("v{i}"),
                Expr::add(Expr::var(format!("v{i}")), Expr::var(format!("v{next}"))),
            ));
        }

        body.push(Node::Loop {
            var: "iter".into(),
            from: Expr::u32(0),
            to: Expr::u32(10),
            body: loop_body,
        });

        // Final reduce tree to prevent DCE
        let mut reduce_expr = Expr::var("v0");
        for i in 1..100 {
            reduce_expr = Expr::add(reduce_expr, Expr::var(format!("v{i}")));
        }

        body.push(Node::store("out", Expr::var("tid"), reduce_expr));

        Ok(Box::new(Program::wrapped(
            vec![
                BufferDecl::storage("in", 0, BufferAccess::ReadOnly, DataType::U32)
                    .with_count(1024),
                BufferDecl::output("out", 1, DataType::U32).with_count(1024),
            ],
            [256, 1, 1],
            body,
        )))
    }

    fn run(
        &self,
        ctx: &mut BenchContext,
        prepared: &mut crate::api::case::PreparedCase,
    ) -> Result<crate::api::case::BenchRun, crate::api::case::BenchError> {
        let program = crate::api::case::prepared_program(prepared)?;

        let mut input = vec![0u8; 1024 * 4];
        let mut rng = rand::rngs::StdRng::seed_from_u64(1337);
        rng.fill(input.as_mut_slice());

        let timed = ctx
            .dispatch_timed(program, &[input.clone()], &vyre::DispatchConfig::default())
            .map_err(|e| crate::api::case::BenchError::ExecutionFailed(e.to_string()))?;

        let start_ref = std::time::Instant::now();
        let baseline = cpu_register_exhaustion_outputs(1024);
        let elapsed_ref = start_ref.elapsed().as_nanos() as u64;

        Ok(crate::api::case::BenchRun {
            metrics: BenchMetrics {
                wall_ns: Some(timed.wall_ns),
                dispatch_ns: timed.device_ns,
                input_bytes: Some(input.len() as u64),
                output_bytes: Some(timed.outputs.iter().map(Vec::len).sum::<usize>() as u64),
                bytes_read: Some(input.len() as u64),
                bytes_written: Some(timed.outputs.iter().map(Vec::len).sum::<usize>() as u64),
                ..Default::default()
            },
            baseline_metrics: Some(BenchMetrics {
                wall_ns: Some(elapsed_ref),
                ..Default::default()
            }),
            outputs: timed.outputs,
            baseline_outputs: Some(vec![baseline]),
        })
    }

    fn verify(
        &self,
        _ctx: &mut BenchContext,
        run: &BenchRun,
    ) -> Result<Correctness, crate::api::case::BenchError> {
        run.verify_exact_outputs()
    }
}

fn cpu_register_exhaustion_outputs(lanes: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(lanes * 4);
    for tid in 0..lanes {
        let tid = tid as u32;
        let mut values = [0u32; 100];
        for (i, value) in values.iter_mut().enumerate() {
            *value = tid.wrapping_add(i as u32);
        }
        for _ in 0..10 {
            for i in 0..100 {
                let next = (i + 1) % 100;
                values[i] = values[i].wrapping_add(values[next]);
            }
        }
        let reduced = values
            .iter()
            .copied()
            .fold(0u32, |acc, value| acc.wrapping_add(value));
        out.extend_from_slice(&reduced.to_le_bytes());
    }
    out
}

inventory::submit! {
    &RegisterExhaustionCase as &dyn BenchCase
}
