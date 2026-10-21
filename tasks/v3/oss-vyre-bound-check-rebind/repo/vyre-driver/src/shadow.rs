//! Exhaustive CPU-vs-backend conformance for compiled pipelines.
//!
//! The old sampled shadow path compared a runtime sample of live
//! dispatches. That could never prove soundness: a backend bug whose
//! divergence rate stayed below the sample rate would slip through.
//!
//! This module replaces sampling with an explicit conformance matrix.
//! Callers build a deterministic set of witness cases, run the backend
//! and reference on every case, and require byte-identical outputs for
//! every tuple. The canonical witness inventory lives in
//! `vyre-conform-spec`; this module stays substrate-neutral by
//! accepting the concrete matrix as input rather than depending on a
//! particular op inventory at runtime.

use std::sync::Arc;

use vyre_foundation::ir::Program;

use crate::backend::{BackendError, CompiledPipeline, DispatchConfig};

type ReferenceRunFn =
    dyn Fn(&Program, &[Vec<u8>]) -> Result<Vec<Vec<u8>>, BackendError> + Send + Sync;

/// Executor that runs `program` on a CPU-side reference interpreter.
///
/// `vyre-reference::reference_eval` is the canonical implementation. A host
/// wires an adapter into this wrapper so the conformance path stays
/// substrate-neutral (no vyre-driver → vyre-reference dep cycle).
#[derive(Clone)]
pub struct ReferenceExecutor {
    run: Arc<ReferenceRunFn>,
}

impl ReferenceExecutor {
    /// Build a concrete reference-execution adapter.
    pub fn new<F>(run: F) -> Self
    where
        F: Fn(&Program, &[Vec<u8>]) -> Result<Vec<Vec<u8>>, BackendError> + Send + Sync + 'static,
    {
        Self { run: Arc::new(run) }
    }

    /// Execute `program` against `inputs`, returning the byte-level
    /// output buffers in the same order the backend would emit.
    ///
    /// # Errors
    ///
    /// Returns a [`BackendError`] when the reference rejects the
    /// program or any witness tuple.
    pub fn run(&self, program: &Program, inputs: &[Vec<u8>]) -> Result<Vec<Vec<u8>>, BackendError> {
        (self.run)(program, inputs)
    }
}

/// One deterministic witness case in an exhaustive conformance run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConformanceCase {
    label: String,
    inputs: Vec<Vec<u8>>,
}

impl ConformanceCase {
    /// Build one named witness tuple.
    #[must_use]
    pub fn new(label: impl Into<String>, inputs: Vec<Vec<u8>>) -> Self {
        Self {
            label: label.into(),
            inputs,
        }
    }

    /// Stable label used in diagnostics.
    #[must_use]
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Input buffers in declaration order.
    #[must_use]
    pub fn inputs(&self) -> &[Vec<u8>] {
        &self.inputs
    }
}

/// Deterministic witness inventory for a compiled pipeline.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ConformanceMatrix {
    cases: Vec<ConformanceCase>,
}

impl ConformanceMatrix {
    /// Build a matrix from an explicit witness list.
    #[must_use]
    pub fn new(cases: Vec<ConformanceCase>) -> Self {
        Self { cases }
    }

    /// Append one witness case.
    pub fn push(&mut self, case: ConformanceCase) {
        self.cases.push(case);
    }

    /// Borrow the deterministic witness list.
    #[must_use]
    pub fn cases(&self) -> &[ConformanceCase] {
        &self.cases
    }

    /// Whether the matrix is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.cases.is_empty()
    }
}

/// Structured divergence surfaced by the exhaustive matrix.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DivergenceEvent {
    /// Stable label of the witness tuple that diverged.
    pub case_label: String,
    /// blake3 fingerprint of the Program's canonical wire bytes.
    pub program_fingerprint: [u8; 32],
    /// Input buffers supplied to the dispatch, in declaration order.
    pub inputs: Vec<Vec<u8>>,
    /// Outputs the backend produced.
    pub backend_output: Vec<Vec<u8>>,
    /// Outputs the reference produced.
    pub reference_output: Vec<Vec<u8>>,
}

/// Exhaustive conformance failures.
#[derive(Debug, thiserror::Error)]
pub enum ConformanceError {
    /// The caller supplied no witness tuples.
    #[error(
        "conformance matrix is empty. Fix: populate every op with at least one witness tuple from vyre-conform-spec before asserting backend parity."
    )]
    EmptyMatrix,
    /// The backend rejected a witness tuple.
    #[error(
        "backend rejected witness `{case_label}`: {source}. Fix: the backend must accept every witness tuple the reference accepts for this Program."
    )]
    BackendRejected {
        /// Stable label of the failing witness tuple.
        case_label: String,
        /// Backend error.
        #[source]
        source: BackendError,
    },
    /// The reference rejected a witness tuple.
    #[error(
        "reference rejected witness `{case_label}`: {source}. Fix: inspect the Program body or witness tuple; the reference is the contract oracle for exhaustive conformance."
    )]
    ReferenceRejected {
        /// Stable label of the failing witness tuple.
        case_label: String,
        /// Reference error.
        #[source]
        source: BackendError,
    },
    /// Backend and reference both ran but produced different bytes.
    #[error(
        "backend diverged from the reference on witness `{event_case_label}`. Fix: inspect the embedded outputs and repair the backend until every witness tuple is byte-identical."
    )]
    Diverged {
        /// Detailed byte-level divergence.
        event: Box<DivergenceEvent>,
        /// Shadow field used by the display impl without reformatting the full event.
        event_case_label: String,
    },
}

/// Run the backend and reference across every witness tuple in `matrix`.
///
/// This is intentionally exhaustive over the supplied cases: if a caller wants
/// "sampled" behaviour, they must sample before constructing the matrix. The
/// conformance harness itself never drops a case.
///
/// # Errors
///
/// Returns the first [`ConformanceError`] after every witness has been
/// executed.
pub fn assert_exhaustive_byte_identity(
    pipeline: &dyn CompiledPipeline,
    program: &Program,
    reference: &ReferenceExecutor,
    matrix: &ConformanceMatrix,
    config: &DispatchConfig,
) -> Result<(), ConformanceError> {
    if matrix.is_empty() {
        return Err(ConformanceError::EmptyMatrix);
    }

    let program_fingerprint = program_fingerprint(program);
    let mut first_error = None;
    for case in matrix.cases() {
        let backend_output = match pipeline.dispatch(case.inputs(), config) {
            Ok(output) => output,
            Err(source) => {
                first_error.get_or_insert(ConformanceError::BackendRejected {
                    case_label: case.label().to_string(),
                    source,
                });
                continue;
            }
        };
        let reference_output = match reference.run(program, case.inputs()) {
            Ok(output) => output,
            Err(source) => {
                first_error.get_or_insert(ConformanceError::ReferenceRejected {
                    case_label: case.label().to_string(),
                    source,
                });
                continue;
            }
        };
        if backend_output != reference_output {
            let event = DivergenceEvent {
                case_label: case.label().to_string(),
                program_fingerprint,
                inputs: case.inputs().to_vec(),
                backend_output,
                reference_output,
            };
            first_error.get_or_insert(ConformanceError::Diverged {
                event_case_label: event.case_label.clone(),
                event: Box::new(event),
            });
        }
    }

    first_error.map_or(Ok(()), Err)
}

fn program_fingerprint(program: &Program) -> [u8; 32] {
    vyre_foundation::optimizer::pipeline_fingerprint_bytes(program)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

    type FakeRun = dyn Fn(&[Vec<u8>]) -> Result<Vec<Vec<u8>>, BackendError> + Send + Sync;

    struct FakePipeline {
        id: String,
        run: Arc<FakeRun>,
    }

    impl crate::backend::private::Sealed for FakePipeline {}

    impl CompiledPipeline for FakePipeline {
        fn id(&self) -> &str {
            &self.id
        }

        fn dispatch(
            &self,
            inputs: &[Vec<u8>],
            _config: &DispatchConfig,
        ) -> Result<Vec<Vec<u8>>, BackendError> {
            (self.run)(inputs)
        }
    }

    fn sample_program() -> Program {
        Program::wrapped(
            vec![
                BufferDecl::storage("input", 0, BufferAccess::ReadOnly, DataType::U32)
                    .with_count(1),
                BufferDecl::storage("output", 1, BufferAccess::ReadWrite, DataType::U32)
                    .with_count(1),
            ],
            [1, 1, 1],
            vec![Node::store(
                "output",
                Expr::u32(0),
                Expr::load("input", Expr::u32(0)),
            )],
        )
    }

    fn witness_matrix() -> ConformanceMatrix {
        ConformanceMatrix::new(
            u32_witnesses()
                .into_iter()
                .map(|witness| {
                    ConformanceCase::new(
                        format!("u32:{witness:#010x}"),
                        vec![witness.to_le_bytes().to_vec()],
                    )
                })
                .collect(),
        )
    }

    #[test]
    fn empty_matrix_is_rejected() {
        let pipeline: Arc<dyn CompiledPipeline> = Arc::new(FakePipeline {
            id: "fake".into(),
            run: Arc::new(|inputs| Ok(inputs.to_vec())),
        });
        let reference = ReferenceExecutor::new(|_, inputs| Ok(inputs.to_vec()));

        let error = assert_exhaustive_byte_identity(
            pipeline.as_ref(),
            &sample_program(),
            &reference,
            &ConformanceMatrix::default(),
            &DispatchConfig::default(),
        )
        .expect_err("empty witness inventories must be rejected");

        assert!(matches!(error, ConformanceError::EmptyMatrix));
    }

    #[test]
    fn exhaustive_matrix_passes_matching_outputs() {
        let pipeline: Arc<dyn CompiledPipeline> = Arc::new(FakePipeline {
            id: "fake".into(),
            run: Arc::new(|inputs| Ok(inputs.to_vec())),
        });
        let reference = ReferenceExecutor::new(|_, inputs| Ok(inputs.to_vec()));

        assert_exhaustive_byte_identity(
            pipeline.as_ref(),
            &sample_program(),
            &reference,
            &witness_matrix(),
            &DispatchConfig::default(),
        )
        .expect("Fix: matching backend/reference outputs must pass the exhaustive matrix; restore this invariant before continuing.");
    }

    #[test]
    fn exhaustive_matrix_catches_divergence_hidden_by_sampling() {
        let hidden_witness = 0xDEAD_BEEF_u32.to_le_bytes().to_vec();
        let seen = Arc::new(Mutex::new(Vec::<Vec<u8>>::new()));
        let seen_clone = Arc::clone(&seen);
        let pipeline: Arc<dyn CompiledPipeline> = Arc::new(FakePipeline {
            id: "fake".into(),
            run: Arc::new(move |inputs| {
                seen_clone.lock().unwrap().push(inputs[0].clone());
                if inputs[0] == hidden_witness {
                    Ok(vec![0_u32.to_le_bytes().to_vec()])
                } else {
                    Ok(inputs.to_vec())
                }
            }),
        });
        let reference = ReferenceExecutor::new(|_, inputs| Ok(inputs.to_vec()));

        let error = assert_exhaustive_byte_identity(
            pipeline.as_ref(),
            &sample_program(),
            &reference,
            &witness_matrix(),
            &DispatchConfig::default(),
        )
        .expect_err("one divergent witness must fail exhaustive conformance");

        match error {
            ConformanceError::Diverged { event, .. } => {
                assert_eq!(event.case_label, "u32:0xdeadbeef");
                assert_eq!(event.inputs, vec![0xDEAD_BEEF_u32.to_le_bytes().to_vec()]);
                assert_eq!(event.backend_output, vec![0_u32.to_le_bytes().to_vec()]);
                assert_eq!(
                    event.reference_output,
                    vec![0xDEAD_BEEF_u32.to_le_bytes().to_vec()]
                );
            }
            other => panic!("expected divergence event, got {other:?}"),
        }

        assert_eq!(
            seen.lock().unwrap().len(),
            u32_witnesses().len(),
            "the conformance matrix must execute every witness tuple exactly once"
        );
    }

    fn u32_witnesses() -> Vec<u32> {
        let mut out = vec![
            0u32,
            1,
            2,
            3,
            u32::MAX,
            u32::MAX - 1,
            0x8000_0000,
            0x7FFF_FFFF,
            0xAAAA_AAAA,
            0x5555_5555,
            0xDEAD_BEEF,
            0xCAFE_F00D,
        ];
        let mut state = 0xD5E4_A7B9_3C6D_102Fu64;
        for _ in 0..24 {
            state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
            let mut z = state;
            z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
            z ^= z >> 31;
            out.push((z as u32) ^ ((z >> 32) as u32));
        }
        out
    }
}
