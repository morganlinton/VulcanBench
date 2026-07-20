use std::{borrow::Cow, sync::Arc};

use serde::{Deserialize, Serialize};
use vyre::{DispatchConfig, VyreBackend};
use vyre_driver::{BackendError, CompiledPipeline};
pub use vyre_spec::DeterminismClass;

use super::metric::BenchMetrics;
use super::suite::SuiteKind;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct BenchId(pub String);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BenchLayer {
    Foundation,
    Reference,
    Runtime,
    Libs,
    Backend,
    Conform,
    Competition,
    Honest,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WorkloadClass {
    Micro,
    Macro,
    Adversarial,
    Honest,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchMetadata {
    pub id: BenchId,
    pub name: String,
    pub description: String,
    pub tags: Vec<String>,
    pub layer: BenchLayer,
    pub workload: WorkloadClass,
    pub determinism: DeterminismClass,
    pub owner_crate: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BaselineClass {
    CpuSota,
    GpuSota,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BaselineTarget {
    pub name: String,
    pub crate_name: String,
    pub class: BaselineClass,
    pub min_speedup_x: f64,
    pub backend_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceContract {
    pub primitive: String,
    pub baselines: Vec<BaselineTarget>,
}

impl PerformanceContract {
    pub fn cpu_sota_min_speedup(
        primitive: impl Into<String>,
        crate_name: impl Into<String>,
        baseline: impl Into<String>,
        min_speedup_x: f64,
    ) -> Self {
        Self::cpu_sota_min_speedup_for_backends(
            primitive,
            crate_name,
            baseline,
            min_speedup_x,
            ["cuda", "wgpu"],
        )
    }

    fn cpu_sota_min_speedup_for_backends(
        primitive: impl Into<String>,
        crate_name: impl Into<String>,
        baseline: impl Into<String>,
        min_speedup_x: f64,
        backend_ids: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        Self {
            primitive: primitive.into(),
            baselines: vec![BaselineTarget {
                name: baseline.into(),
                crate_name: crate_name.into(),
                class: BaselineClass::CpuSota,
                min_speedup_x,
                backend_ids: backend_ids.into_iter().map(Into::into).collect(),
            }],
        }
    }

    pub fn cpu_sota_100x(
        primitive: impl Into<String>,
        crate_name: impl Into<String>,
        baseline: impl Into<String>,
    ) -> Self {
        Self::cpu_sota_min_speedup(primitive, crate_name, baseline, 100.0)
    }

    pub fn cpu_sota_10x(
        primitive: impl Into<String>,
        crate_name: impl Into<String>,
        baseline: impl Into<String>,
    ) -> Self {
        Self::cpu_sota_min_speedup(primitive, crate_name, baseline, 10.0)
    }

    pub fn cpu_sota_3x(
        primitive: impl Into<String>,
        crate_name: impl Into<String>,
        baseline: impl Into<String>,
    ) -> Self {
        Self::cpu_sota_min_speedup(primitive, crate_name, baseline, 3.0)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceEvaluation {
    pub speedup_x: Option<f64>,
    pub contract_passed: bool,
    pub violations: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchRequirements {
    pub needs_gpu: bool,
    pub needs_network: bool,
    pub min_vram_bytes: Option<u64>,
    pub min_input_bytes: Option<u64>,
    pub feature_set: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Correctness {
    Exact,
    Toleranced {
        ulp_budget: u32,
        max_observed_ulp: u32,
    },
    Certificate {
        digest: [u8; 32],
    },
    Invalid {
        reason: String,
    },
}

pub struct ScratchPool {
    pub buffer: Vec<u8>,
}

pub struct OptimizerPipeline {}

pub struct CpuReference {}

impl CpuReference {
    pub fn dispatch(
        &self,
        prog: &vyre::ir::Program,
        inputs: &[Vec<u8>],
        _config: &vyre::DispatchConfig,
    ) -> Result<Vec<Vec<u8>>, String> {
        let ref_inputs: Vec<vyre_reference::value::Value> = inputs
            .iter()
            .map(|b| vyre_reference::value::Value::Bytes(std::sync::Arc::from(b.clone())))
            .collect();
        vyre_reference::reference_eval(prog, &ref_inputs)
            .map(|values| values.iter().map(|v| v.to_bytes()).collect())
            .map_err(|e| format!("{:?}", e))
    }
}

pub struct BenchContext {
    pub backends: Vec<Box<dyn VyreBackend>>,
    pub preferred_backend: Arc<dyn VyreBackend>,
    pub compiled_pipeline: Option<Arc<dyn CompiledPipeline>>,
    pub compiled_program_fingerprint: Option<[u8; 32]>,
    pub reference: CpuReference,
    pub optimizer: OptimizerPipeline,
    pub scratch: ScratchPool,
    pub rng: rand::rngs::StdRng,
    pub dispatch_config: DispatchConfig,
    pub evolve_candidate: Option<vyre::ir::Program>,
    pub include_baseline_outputs: bool,
}

impl BenchContext {
    pub fn compiled_pipeline_for(
        &self,
        prog: &vyre::ir::Program,
    ) -> Result<Option<&dyn CompiledPipeline>, vyre_driver::BackendError> {
        if !self
            .compiled_program_fingerprint
            .is_some_and(|fingerprint| fingerprint == prog.fingerprint())
        {
            return Ok(None);
        }

        self.compiled_pipeline
            .as_deref()
            .map(Some)
            .ok_or_else(|| {
                vyre_driver::BackendError::new(
                    "compiled program fingerprint was set without a compiled pipeline. Fix: keep BenchContext compiled pipeline state coherent.",
                )
            })
    }

    pub fn dispatch(
        &self,
        prog: &vyre::ir::Program,
        inputs: &[Vec<u8>],
        config: &DispatchConfig,
    ) -> Result<Vec<Vec<u8>>, vyre_driver::BackendError> {
        let config = dispatch_config_with_inferred_grid(prog, inputs, config)?;
        vyre_driver::validate_program_for_backend(self.preferred_backend.as_ref(), prog, &config)?;
        if let Some(pipeline) = self.compiled_pipeline_for(prog)? {
            let borrowed_inputs: Vec<&[u8]> = inputs.iter().map(Vec::as_slice).collect();
            pipeline.dispatch_borrowed(&borrowed_inputs, &config)
        } else {
            let borrowed_inputs: Vec<&[u8]> = inputs.iter().map(Vec::as_slice).collect();
            self.preferred_backend
                .dispatch_borrowed(prog, &borrowed_inputs, &config)
        }
    }

    pub fn dispatch_timed(
        &self,
        prog: &vyre::ir::Program,
        inputs: &[Vec<u8>],
        config: &DispatchConfig,
    ) -> Result<vyre_driver::TimedDispatchResult, vyre_driver::BackendError> {
        let config = dispatch_config_with_inferred_grid(prog, inputs, config)?;
        vyre_driver::validate_program_for_backend(self.preferred_backend.as_ref(), prog, &config)?;
        let borrowed_inputs: Vec<&[u8]> = inputs.iter().map(Vec::as_slice).collect();
        if let Some(pipeline) = self.compiled_pipeline_for(prog)? {
            pipeline.dispatch_borrowed_timed(&borrowed_inputs, &config)
        } else {
            self.preferred_backend
                .dispatch_borrowed_timed(prog, &borrowed_inputs, &config)
        }
    }
}

/// Return a dispatch config with the benchmark's backend-neutral grid inference applied.
pub fn dispatch_config_with_inferred_grid<'a>(
    prog: &vyre::ir::Program,
    inputs: &[Vec<u8>],
    config: &'a DispatchConfig,
) -> Result<Cow<'a, DispatchConfig>, BackendError> {
    if config.grid_override.is_some() {
        return Ok(Cow::Borrowed(config));
    }

    let mut inferred_config = config.clone();
    inferred_config.grid_override = Some(vyre_driver::program_walks::infer_dispatch_grid(
        prog, inputs, config,
    )?);
    Ok(Cow::Owned(inferred_config))
}

pub type PreparedCase = Box<dyn std::any::Any>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchRun {
    pub metrics: BenchMetrics,
    pub baseline_metrics: Option<BenchMetrics>,
    pub outputs: Vec<Vec<u8>>,
    pub baseline_outputs: Option<Vec<Vec<u8>>>,
}

impl BenchRun {
    pub fn verify_exact_outputs(&self) -> Result<Correctness, BenchError> {
        let baseline = self.baseline_outputs.as_ref().ok_or_else(|| {
            BenchError::CorrectnessViolation(
                "benchmark did not capture a baseline output; cannot claim exact correctness"
                    .to_string(),
            )
        })?;
        if self.outputs == *baseline {
            return Ok(Correctness::Exact);
        }
        Err(BenchError::CorrectnessViolation(first_output_difference(
            &self.outputs,
            baseline,
        )))
    }

    pub fn verify_f32_outputs_with_ulp(&self, ulp_budget: u32) -> Result<Correctness, BenchError> {
        let baseline = self.baseline_outputs.as_ref().ok_or_else(|| {
            BenchError::CorrectnessViolation(
                "benchmark did not capture a baseline output; cannot claim f32 ULP correctness"
                    .to_string(),
            )
        })?;
        if self.outputs.len() != baseline.len() {
            return Err(BenchError::CorrectnessViolation(format!(
                "f32 output count mismatch: backend returned {}, baseline returned {}",
                self.outputs.len(),
                baseline.len()
            )));
        }

        let mut max_observed_ulp = 0u32;
        for (buffer_index, (actual, expected)) in self.outputs.iter().zip(baseline).enumerate() {
            if actual.len() != expected.len() {
                return Err(BenchError::CorrectnessViolation(format!(
                    "f32 output buffer {buffer_index} length mismatch: backend returned {} bytes, baseline returned {} bytes",
                    actual.len(),
                    expected.len()
                )));
            }
            if actual.len() % 4 != 0 {
                return Err(BenchError::CorrectnessViolation(format!(
                    "f32 output buffer {buffer_index} has non-f32 byte length {}",
                    actual.len()
                )));
            }
            for (value_index, (actual_chunk, expected_chunk)) in actual
                .chunks_exact(4)
                .zip(expected.chunks_exact(4))
                .enumerate()
            {
                let actual_value = f32::from_le_bytes(actual_chunk.try_into().map_err(|_| {
                    BenchError::CorrectnessViolation(
                        "backend f32 output chunk was not 4 bytes".to_string(),
                    )
                })?);
                let expected_value =
                    f32::from_le_bytes(expected_chunk.try_into().map_err(|_| {
                        BenchError::CorrectnessViolation(
                            "baseline f32 output chunk was not 4 bytes".to_string(),
                        )
                    })?);
                let distance = f32_ulp_distance(actual_value, expected_value).ok_or_else(|| {
                    BenchError::CorrectnessViolation(format!(
                        "f32 output buffer {buffer_index} value {value_index} contains NaN: backend={actual_value:?}, baseline={expected_value:?}"
                    ))
                })?;
                max_observed_ulp = max_observed_ulp.max(distance);
                if distance > ulp_budget {
                    return Err(BenchError::CorrectnessViolation(format!(
                        "f32 output buffer {buffer_index} value {value_index} exceeded ULP budget {ulp_budget}: observed {distance}, backend={actual_value:?}, baseline={expected_value:?}"
                    )));
                }
            }
        }
        Ok(Correctness::Toleranced {
            ulp_budget,
            max_observed_ulp,
        })
    }
}

pub fn prepared_program(prepared: &PreparedCase) -> Result<&vyre::ir::Program, BenchError> {
    prepared.downcast_ref::<vyre::ir::Program>().ok_or_else(|| {
        BenchError::ExecutionFailed(
            "prepared benchmark payload was not a vyre::ir::Program".to_string(),
        )
    })
}

fn first_output_difference(outputs: &[Vec<u8>], baseline: &[Vec<u8>]) -> String {
    if outputs.len() != baseline.len() {
        return format!(
            "output count mismatch: backend returned {}, baseline returned {}",
            outputs.len(),
            baseline.len()
        );
    }
    for (buffer_index, (actual, expected)) in outputs.iter().zip(baseline).enumerate() {
        if actual.len() != expected.len() {
            return format!(
                "output buffer {buffer_index} length mismatch: backend returned {} bytes, baseline returned {} bytes",
                actual.len(),
                expected.len()
            );
        }
        if let Some(byte_index) = actual
            .iter()
            .zip(expected)
            .position(|(actual_byte, expected_byte)| actual_byte != expected_byte)
        {
            let window_end = actual.len().min(byte_index.saturating_add(16));
            return format!(
                "output buffer {buffer_index} differs at byte {byte_index}: backend=0x{:02x}, baseline=0x{:02x}, backend_window={:02x?}, baseline_window={:02x?}",
                actual[byte_index],
                expected[byte_index],
                &actual[byte_index..window_end],
                &expected[byte_index..window_end]
            );
        }
    }
    "backend output differs from baseline".to_string()
}

fn f32_ulp_distance(actual: f32, expected: f32) -> Option<u32> {
    if actual.to_bits() == expected.to_bits() {
        return Some(0);
    }
    if actual.is_nan() || expected.is_nan() {
        return None;
    }
    let actual_ordered = ordered_f32_bits(actual);
    let expected_ordered = ordered_f32_bits(expected);
    Some(
        actual_ordered
            .abs_diff(expected_ordered)
            .min(u64::from(u32::MAX)) as u32,
    )
}

fn ordered_f32_bits(value: f32) -> i64 {
    let bits = value.to_bits();
    if bits & 0x8000_0000 == 0 {
        i64::from(bits | 0x8000_0000)
    } else {
        i64::from(!bits)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum BenchError {
    #[error("Environment invalid: {0}")]
    EnvironmentInvalid(String),
    #[error("Execution failed: {0}")]
    ExecutionFailed(String),
    #[error("GPU probe failed for GPU-required benchmark: {0}. Fix: run `nvidia-smi`, verify CUDA/WGPU backend acquisition, and rerun the benchmark.")]
    GpuProbeFailed(String),
    #[error("Backend failed: {0}")]
    BackendFailed(String),
    #[error("Correctness violation: {0}")]
    CorrectnessViolation(String),
}

pub trait BenchCase: Send + Sync {
    fn id(&self) -> BenchId;
    fn metadata(&self) -> BenchMetadata;
    fn suites(&self) -> &'static [SuiteKind] {
        &[]
    }
    fn active_in_suite(&self, suite: SuiteKind) -> bool {
        let suites = self.suites();
        suites.is_empty() || suites.contains(&suite)
    }
    fn requirements(&self) -> BenchRequirements;
    fn performance_contract(&self) -> Option<PerformanceContract> {
        None
    }
    fn prepare(&self, ctx: &mut BenchContext) -> Result<PreparedCase, BenchError>;
    fn program<'a>(&self, prepared: &'a PreparedCase) -> Option<&'a vyre::ir::Program> {
        prepared_program(prepared).ok()
    }
    fn workload_fingerprint_bytes(&self, prepared: &PreparedCase) -> Option<[u8; 32]> {
        self.program(prepared).map(vyre::ir::Program::fingerprint)
    }
    fn run(
        &self,
        ctx: &mut BenchContext,
        prepared: &mut PreparedCase,
    ) -> Result<BenchRun, BenchError>;
    fn verify(&self, ctx: &mut BenchContext, run: &BenchRun) -> Result<Correctness, BenchError>;
    fn bytes_touched(&self, prepared: &PreparedCase) -> (u64, u64) {
        prepared_program(prepared)
            .map(static_program_bytes_touched)
            .unwrap_or((0, 0))
    }
}

fn static_program_bytes_touched(program: &vyre::ir::Program) -> (u64, u64) {
    let mut read_bytes = 0_u64;
    let mut write_bytes = 0_u64;
    for buffer in program.buffers() {
        let bytes = buffer
            .element()
            .size_bytes()
            .map(|element_bytes| (element_bytes as u64).saturating_mul(u64::from(buffer.count())))
            .unwrap_or(0);
        match buffer.access() {
            vyre::ir::BufferAccess::ReadOnly | vyre::ir::BufferAccess::Uniform => {
                read_bytes = read_bytes.saturating_add(bytes);
            }
            vyre::ir::BufferAccess::ReadWrite => {
                read_bytes = read_bytes.saturating_add(bytes);
                write_bytes = write_bytes.saturating_add(bytes);
            }
            vyre::ir::BufferAccess::WriteOnly => {
                write_bytes = write_bytes.saturating_add(bytes);
            }
            vyre::ir::BufferAccess::Workgroup => {}
            _ => {}
        }
    }
    (read_bytes, write_bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn f32_bytes(values: &[f32]) -> Vec<u8> {
        vyre_primitives::wire::pack_f32_slice(values)
    }

    #[test]
    fn cpu_sota_contract_applies_to_cuda_and_wgpu_release_backends() {
        let contract = PerformanceContract::cpu_sota_100x("primitive", "vyre", "cpu baseline");
        let backend_ids = &contract.baselines[0].backend_ids;

        for backend in ["cuda", "wgpu"] {
            assert!(
                backend_ids.iter().any(|candidate| candidate == backend),
                "Fix: CPU-SOTA release contracts must apply to `{backend}` evidence."
            );
        }
    }

    #[test]
    fn dispatch_config_infers_grid_from_input_bindings_not_sparse_outputs() {
        let program = vyre::ir::Program::wrapped(
            vec![
                vyre::ir::BufferDecl::output("out_count", 0, vyre::ir::DataType::U32).with_count(1),
                vyre::ir::BufferDecl::storage(
                    "records",
                    1,
                    vyre::ir::BufferAccess::ReadOnly,
                    vyre::ir::DataType::U32,
                )
                .with_count(1024),
            ],
            [256, 1, 1],
            vec![vyre::ir::Node::let_bind(
                "_slot",
                vyre::ir::Expr::atomic_add(
                    "out_count",
                    vyre::ir::Expr::u32(0),
                    vyre::ir::Expr::load("records", vyre::ir::Expr::InvocationId { axis: 0 }),
                ),
            )],
        );
        let inputs = vec![vec![0u8; 1024 * 4]];
        let default_config = DispatchConfig::default();

        let inferred = dispatch_config_with_inferred_grid(&program, &inputs, &default_config)
            .expect("Fix: benchmark dispatch grid inference must handle sparse-output cases.");

        assert_eq!(
            inferred.grid_override,
            Some([4, 1, 1]),
            "Fix: resident sparse-output benchmarks must launch over input records, not the one-word output counter."
        );
    }

    #[test]
    fn f32_ulp_verifier_accepts_budgeted_difference() {
        let one = 1.0f32;
        let next = f32::from_bits(one.to_bits() + 1);
        let run = BenchRun {
            metrics: BenchMetrics::default(),
            baseline_metrics: None,
            outputs: vec![f32_bytes(&[next])],
            baseline_outputs: Some(vec![f32_bytes(&[one])]),
        };

        assert!(matches!(
            run.verify_f32_outputs_with_ulp(1).unwrap(),
            Correctness::Toleranced {
                ulp_budget: 1,
                max_observed_ulp: 1
            }
        ));
    }

    #[test]
    fn f32_ulp_verifier_rejects_over_budget_difference() {
        let one = 1.0f32;
        let far = f32::from_bits(one.to_bits() + 8);
        let run = BenchRun {
            metrics: BenchMetrics::default(),
            baseline_metrics: None,
            outputs: vec![f32_bytes(&[far])],
            baseline_outputs: Some(vec![f32_bytes(&[one])]),
        };

        let error = run.verify_f32_outputs_with_ulp(2).unwrap_err();
        assert!(
            error.to_string().contains("exceeded ULP budget"),
            "Fix: over-budget f32 mismatch should be actionable: {error}"
        );
    }
}
