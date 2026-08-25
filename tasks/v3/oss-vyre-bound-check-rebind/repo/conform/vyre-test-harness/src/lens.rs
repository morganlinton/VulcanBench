//! Reusable conform lenses: ways of comparing backend output to a truth
//! oracle, one primitive per semantic.
//!
//! Every parity test in the workspace ultimately does one of:
//! - *witness*  -  run on the CPU reference, assert equality to
//!   `expected_output`.
//! - *cpu_vs_backend*  -  run on both, assert byte-identity (or ULP
//!   tolerance) between them.
//! - *fixpoint*  -  dispatch the backend in a loop until a convergence
//!   flag clears, then compare the final state to the CPU reference.
//!
//! Each test picks a lens, passes an op iterator, and the shared code
//! does the rest with missing coverage represented as failure.

use vyre_driver::{BackendError, DispatchConfig, Error, VyreBackend};
use vyre_foundation::ir::{BufferAccess, Program};
use vyre_foundation::program_caps;
use vyre_libs::harness::{convergence_contract, fixpoint_contract, FixpointContract, OpEntry};
use vyre_reference::value::Value;

use crate::fp_parity::{compare_output_buffers, BufferParity};

/// Outcome of running one lens against one op.
#[derive(Debug)]
pub enum LensOutcome {
    /// Lens passed  -  op output matched the oracle for every case.
    Pass {
        /// Number of input cases that were compared.
        cases: usize,
    },
    /// Lens failed  -  op diverged from the oracle on the referenced case.
    Fail {
        /// Zero-based case index of the first divergence.
        case_index: usize,
        /// Rendered failure detail.
        detail: String,
    },
}

impl LensOutcome {
    /// True only when the lens passed (ran and matched the oracle).
    ///
    /// Missing coverage is represented as [`LensOutcome::Fail`], so a
    /// passing lens always performed real comparisons.
    #[must_use]
    pub fn is_ok(&self) -> bool {
        matches!(self, LensOutcome::Pass { .. })
    }

    /// True only when the lens actually ran and matched the oracle.
    #[must_use]
    pub fn is_pass(&self) -> bool {
        matches!(self, LensOutcome::Pass { .. })
    }
}

fn run_cpu(program: &Program, inputs: &[Vec<u8>]) -> Result<Vec<Vec<u8>>, Error> {
    let values: Vec<Value> = inputs.iter().cloned().map(Value::from).collect();
    let outputs = vyre_reference::reference_eval(program, &values)?;
    Ok(outputs.into_iter().map(|value| value.to_bytes()).collect())
}

fn dispatch_config_for(program: &Program) -> Result<DispatchConfig, String> {
    let mut config = DispatchConfig::default();
    let workgroup = program.workgroup_size();
    for (axis, size) in workgroup.into_iter().enumerate() {
        if size == 0 {
            return Err(format!(
                "workgroup_size[{axis}] is 0. Fix: parity dispatch requires every workgroup dimension to be >= 1 before backend dispatch."
            ));
        }
    }
    if workgroup[1] == 1 && workgroup[2] == 1 {
        return Ok(config);
    }

    let lanes = u64::from(workgroup[0])
        .checked_mul(u64::from(workgroup[1]))
        .and_then(|lanes| lanes.checked_mul(u64::from(workgroup[2])))
        .ok_or_else(|| {
            format!(
                "workgroup_size {workgroup:?} overflows u64 lane accounting. Fix: use a valid backend workgroup shape."
            )
        })?;
    let max_writable_count = program
        .buffers()
        .iter()
        .filter(|decl| matches!(decl.access(), BufferAccess::ReadWrite) || decl.is_output())
        .map(|decl| u64::from(decl.count()))
        .max()
        .unwrap_or(1);

    if max_writable_count > lanes {
        return Err(format!(
            "non-1D workgroup_size {workgroup:?} has {lanes} lanes but the largest writable buffer has {max_writable_count} elements. Fix: register an explicit dispatch grid for this op instead of relying on the one-workgroup parity fixture path."
        ));
    }

    config.grid_override = Some([1, 1, 1]);
    Ok(config)
}

/// CPU-only witness lens.
///
/// Executes the op's `test_inputs` through `vyre_reference::reference_eval` and
/// compares the result byte-for-byte against its declared
/// `expected_output`. The oracle lives next to the op; the lens just
/// runs it.
pub fn witness(entry: &OpEntry) -> LensOutcome {
    let Some(test_inputs) = entry.test_inputs else {
        return LensOutcome::Fail {
            case_index: 0,
            detail: format!(
                "{}: no test_inputs  -  witness lens has nothing to run. Fix: register a fixture.",
                entry.id
            ),
        };
    };
    let Some(expected_fn) = entry.expected_output else {
        return LensOutcome::Fail {
            case_index: 0,
            detail: format!(
                "{}: no expected_output  -  witness lens has no oracle. Fix: register a fixture.",
                entry.id
            ),
        };
    };

    let program = (entry.build)();
    let cases = test_inputs();
    let expected = expected_fn();
    if cases.is_empty() {
        return LensOutcome::Fail {
            case_index: 0,
            detail: format!(
                "{}: empty test_inputs fixture. Fix: empty fixtures are zero coverage.",
                entry.id
            ),
        };
    }
    if expected.is_empty() {
        return LensOutcome::Fail {
            case_index: 0,
            detail: format!(
                "{}: empty expected_output fixture. Fix: empty oracles are zero coverage.",
                entry.id
            ),
        };
    }
    if cases.len() != expected.len() {
        return LensOutcome::Fail {
            case_index: 0,
            detail: format!(
                "witness vector count mismatch: {} test_inputs vs {} expected_output sets.",
                cases.len(),
                expected.len()
            ),
        };
    }

    for (index, (inputs, expected_buffers)) in cases.iter().zip(expected.iter()).enumerate() {
        match run_cpu(&program, inputs) {
            Ok(outputs) => {
                if outputs != *expected_buffers {
                    return LensOutcome::Fail {
                        case_index: index,
                        detail: format!(
                            "CPU reference output diverged from declared expected_output.\nACTUAL:\n{:?}\nEXPECTED:\n{:?}\nFix: regenerate the witness via `cargo xtask trace-f32 {}` or \
                             repair the reference.",
                            outputs, expected_buffers, entry.id
                        ),
                    };
                }
            }
            Err(error) => {
                return LensOutcome::Fail {
                    case_index: index,
                    detail: format!("CPU reference failed: {error}"),
                };
            }
        }
    }

    LensOutcome::Pass { cases: cases.len() }
}

/// CPU-vs-backend byte-identity lens.
///
/// Dispatches the op on both the CPU reference and the supplied
/// backend, and asserts byte-identity (modulo the op's declared ULP
/// tolerance). Missing fixtures and missing backend capabilities are hard
/// failures. Fixpoint ops are routed to [`fixpoint`] instead.
pub fn cpu_vs_backend(entry: &OpEntry, backend: &dyn VyreBackend) -> LensOutcome {
    // Fixpoint ops need a convergence loop; route them to the fixpoint
    // lens automatically instead of skipping.
    if fixpoint_contract(entry.id).is_some() {
        return fixpoint(entry, backend);
    }
    // Convergence-contract ops need iterative dispatch until the state
    // stabilises; route them to the convergence lens.
    if convergence_contract(entry.id).is_some() {
        return convergence(entry, backend);
    }
    let Some(test_inputs) = entry.test_inputs else {
        return LensOutcome::Fail {
            case_index: 0,
            detail: format!("{}: no test_inputs  -  byte-identity lens has nothing to run. Fix: register a fixture.", entry.id),
        };
    };

    let program = (entry.build)();
    let required = program_caps::scan(&program);
    if let Err(missing) = program_caps::check_backend_capabilities(
        backend.id(),
        backend.supports_subgroup_ops(),
        backend.supports_f16(),
        backend.supports_bf16(),
        backend.supports_indirect_dispatch(),
        true,
        backend.supports_distributed_collectives(),
        backend.max_workgroup_size(),
        &required,
    ) {
        return LensOutcome::Fail {
            case_index: 0,
            detail: format!(
                "{}: missing backend capability: {missing}. Fix: wire the capability or the op.",
                entry.id
            ),
        };
    }
    let config = match dispatch_config_for(&program) {
        Ok(config) => config,
        Err(detail) => {
            return LensOutcome::Fail {
                case_index: 0,
                detail,
            };
        }
    };

    let cases = test_inputs();
    if cases.is_empty() {
        return LensOutcome::Fail {
            case_index: 0,
            detail: format!(
                "{}: empty test_inputs fixture. Fix: byte-identity parity requires at least one backend witness.",
                entry.id
            ),
        };
    }
    for (index, inputs) in cases.iter().enumerate() {
        let cpu = match run_cpu(&program, inputs) {
            Ok(outputs) => outputs,
            Err(error) => {
                return LensOutcome::Fail {
                    case_index: index,
                    detail: format!("CPU reference failed: {error}"),
                };
            }
        };
        let borrowed_inputs: Vec<&[u8]> = inputs.iter().map(Vec::as_slice).collect();
        let gpu = match backend.dispatch_borrowed(&program, &borrowed_inputs, &config) {
            Ok(outputs) => outputs,
            Err(error) => {
                return LensOutcome::Fail {
                    case_index: index,
                    detail: format!("backend `{}` dispatch failed: {error}", backend.id()),
                };
            }
        };
        if let BufferParity::Mismatch(detail) = compare_output_buffers(&program, &cpu, &gpu) {
            return LensOutcome::Fail {
                case_index: index,
                detail: format!(
                    "backend `{}` diverged from CPU reference on case {index}: {detail}",
                    backend.id(),
                ),
            };
        }
    }

    LensOutcome::Pass { cases: cases.len() }
}

/// Fixpoint lens: dispatch the op repeatedly until its convergence flag
/// clears, then compare the final state to the CPU reference.
///
/// The contract comes from [`fixpoint_contract`] (`converged_flag_buffer`,
/// `max_iterations`). Each dispatch: zero the flag, run the program,
/// read the flag's first word; if zero, the op has converged. The CPU
/// reference is expected to reach the same final state after iterating
/// under the same loop.
pub fn fixpoint(entry: &OpEntry, backend: &dyn VyreBackend) -> LensOutcome {
    let Some(contract) = fixpoint_contract(entry.id) else {
        return LensOutcome::Fail {
            case_index: 0,
            detail: format!("{}: no FixpointContract registered for this op. Fix: register a contract or use the cpu_vs_backend lens.", entry.id),
        };
    };
    let Some(test_inputs) = entry.test_inputs else {
        return LensOutcome::Fail {
            case_index: 0,
            detail: format!(
                "{}: no test_inputs  -  fixpoint lens has nothing to run. Fix: register a fixture.",
                entry.id
            ),
        };
    };

    let program = (entry.build)();
    let required = program_caps::scan(&program);
    if let Err(missing) = program_caps::check_backend_capabilities(
        backend.id(),
        backend.supports_subgroup_ops(),
        backend.supports_f16(),
        backend.supports_bf16(),
        backend.supports_indirect_dispatch(),
        true,
        backend.supports_distributed_collectives(),
        backend.max_workgroup_size(),
        &required,
    ) {
        return LensOutcome::Fail {
            case_index: 0,
            detail: format!(
                "{}: missing backend capability: {missing}. Fix: wire the capability or the op.",
                entry.id
            ),
        };
    }
    let config = match dispatch_config_for(&program) {
        Ok(config) => config,
        Err(detail) => {
            return LensOutcome::Fail {
                case_index: 0,
                detail,
            };
        }
    };

    let Some(flag_index) = index_of_buffer(&program, contract.converged_flag_buffer) else {
        return LensOutcome::Fail {
            case_index: 0,
            detail: format!(
                "program does not declare buffer `{}` named by FixpointContract.",
                contract.converged_flag_buffer
            ),
        };
    };

    let cases = test_inputs();
    if cases.is_empty() {
        return LensOutcome::Fail {
            case_index: 0,
            detail: format!(
                "{}: empty test_inputs fixture. Fix: fixpoint parity requires at least one initial state.",
                entry.id
            ),
        };
    }
    for (index, inputs) in cases.iter().enumerate() {
        let cpu_final = match cpu_fixpoint(&program, inputs, flag_index, contract) {
            Ok(outputs) => outputs,
            Err(LoopError::Reference(error)) => {
                return LensOutcome::Fail {
                    case_index: index,
                    detail: format!("CPU reference failed inside fixpoint loop: {error}"),
                };
            }
            Err(LoopError::DidNotConverge) => {
                return LensOutcome::Fail {
                    case_index: index,
                    detail: format!(
                        "CPU reference did not converge in {} iterations. \
                         Fix: raise the FixpointContract max_iterations or shrink the fixture.",
                        contract.max_iterations
                    ),
                };
            }
            Err(LoopError::Backend(error)) => {
                return LensOutcome::Fail {
                    case_index: index,
                    detail: format!("backend failed inside fixpoint loop: {error}"),
                };
            }
        };
        let gpu_final = match gpu_fixpoint(backend, &program, inputs, flag_index, contract, &config)
        {
            Ok(outputs) => outputs,
            Err(LoopError::Reference(error)) => {
                return LensOutcome::Fail {
                    case_index: index,
                    detail: format!("CPU reference failed inside fixpoint loop: {error}"),
                };
            }
            Err(LoopError::DidNotConverge) => {
                return LensOutcome::Fail {
                    case_index: index,
                    detail: format!(
                        "backend `{}` did not converge in {} iterations.",
                        backend.id(),
                        contract.max_iterations
                    ),
                };
            }
            Err(LoopError::Backend(error)) => {
                return LensOutcome::Fail {
                    case_index: index,
                    detail: format!(
                        "backend `{}` fixpoint dispatch failed: {error}",
                        backend.id()
                    ),
                };
            }
        };
        let cpu_outputs = match project_output_buffers(&program, &cpu_final) {
            Ok(outputs) => outputs,
            Err(detail) => {
                return LensOutcome::Fail {
                    case_index: index,
                    detail: format!("CPU reference {detail}"),
                };
            }
        };
        let gpu_outputs = match project_output_buffers(&program, &gpu_final) {
            Ok(outputs) => outputs,
            Err(detail) => {
                return LensOutcome::Fail {
                    case_index: index,
                    detail: format!("backend `{}` {detail}", backend.id()),
                };
            }
        };
        if let BufferParity::Mismatch(detail) =
            compare_output_buffers(&program, &cpu_outputs, &gpu_outputs)
        {
            return LensOutcome::Fail {
                case_index: index,
                detail: format!(
                    "backend `{}` final state diverged from CPU reference after fixpoint loop: {detail}",
                    backend.id()
                ),
            };
        }
    }

    LensOutcome::Pass { cases: cases.len() }
}

/// Convergence lens: dispatch the op repeatedly until the RW state
/// stabilises, then compare the final state to the CPU reference.
///
/// Used for ops that register a [`ConvergenceContract`] (e.g. security
/// graph-traversal steps whose Program performs ONE transfer step).
/// The lens infers the `current` (RO input) and `next` (RW output)
/// buffers, copies `next` → `current` between iterations, and stops
/// when `next` stops changing.
pub fn convergence(entry: &OpEntry, backend: &dyn VyreBackend) -> LensOutcome {
    let Some(contract) = convergence_contract(entry.id) else {
        return LensOutcome::Fail {
            case_index: 0,
            detail: format!(
                "{}: no ConvergenceContract registered for this op. \
                 Fix: register a contract or use the cpu_vs_backend lens.",
                entry.id
            ),
        };
    };
    let Some(test_inputs) = entry.test_inputs else {
        return LensOutcome::Fail {
            case_index: 0,
            detail: format!(
                "{}: no test_inputs  -  convergence lens has nothing to run. Fix: register a fixture.",
                entry.id
            ),
        };
    };

    let program = (entry.build)();
    let required = program_caps::scan(&program);
    if let Err(missing) = program_caps::check_backend_capabilities(
        backend.id(),
        backend.supports_subgroup_ops(),
        backend.supports_f16(),
        backend.supports_bf16(),
        backend.supports_indirect_dispatch(),
        true,
        backend.supports_distributed_collectives(),
        backend.max_workgroup_size(),
        &required,
    ) {
        return LensOutcome::Fail {
            case_index: 0,
            detail: format!(
                "{}: missing backend capability: {missing}. Fix: wire the capability or the op.",
                entry.id
            ),
        };
    }
    let config = match dispatch_config_for(&program) {
        Ok(config) => config,
        Err(detail) => {
            return LensOutcome::Fail {
                case_index: 0,
                detail,
            };
        }
    };

    let Ok((current_name, next_name, _words)) = infer_fixpoint_buffers(&program) else {
        return LensOutcome::Fail {
            case_index: 0,
            detail: format!(
                "{}: could not infer fixpoint current/next buffers from program layout. \
                 Fix: ensure one RO buffer matches the last RW buffer in count.",
                entry.id
            ),
        };
    };
    let Some(current_idx) = index_of_buffer(&program, current_name) else {
        return LensOutcome::Fail {
            case_index: 0,
            detail: format!(
                "{}: inferred current buffer '{current_name}' is absent from the program buffer table. \
                 Fix: keep fixpoint inference and buffer declarations in the same program contract.",
                entry.id
            ),
        };
    };
    let Some(next_idx) = index_of_buffer(&program, next_name) else {
        return LensOutcome::Fail {
            case_index: 0,
            detail: format!(
                "{}: inferred next buffer '{next_name}' is absent from the program buffer table. \
                 Fix: keep fixpoint inference and buffer declarations in the same program contract.",
                entry.id
            ),
        };
    };

    let cases = test_inputs();
    if cases.is_empty() {
        return LensOutcome::Fail {
            case_index: 0,
            detail: format!(
                "{}: empty test_inputs fixture. Fix: convergence parity requires at least one initial state.",
                entry.id
            ),
        };
    }
    for (index, inputs) in cases.iter().enumerate() {
        let cpu_final = match cpu_convergence(
            &program,
            inputs,
            contract.max_iterations,
            current_idx,
            next_idx,
        ) {
            Ok(outputs) => outputs,
            Err(LoopError::Reference(error)) => {
                return LensOutcome::Fail {
                    case_index: index,
                    detail: format!("CPU reference failed inside convergence loop: {error}"),
                };
            }
            Err(LoopError::DidNotConverge) => {
                return LensOutcome::Fail {
                    case_index: index,
                    detail: format!(
                        "CPU reference did not converge in {} iterations. \
                         Fix: raise the ConvergenceContract max_iterations or shrink the fixture.",
                        contract.max_iterations
                    ),
                };
            }
            Err(LoopError::Backend(error)) => {
                return LensOutcome::Fail {
                    case_index: index,
                    detail: format!("backend failed inside convergence loop: {error}"),
                };
            }
        };
        let gpu_final = match gpu_convergence(
            backend,
            &program,
            inputs,
            contract.max_iterations,
            current_idx,
            next_idx,
            &config,
        ) {
            Ok(outputs) => outputs,
            Err(LoopError::Reference(error)) => {
                return LensOutcome::Fail {
                    case_index: index,
                    detail: format!("CPU reference failed inside convergence loop: {error}"),
                };
            }
            Err(LoopError::DidNotConverge) => {
                return LensOutcome::Fail {
                    case_index: index,
                    detail: format!(
                        "backend `{}` did not converge in {} iterations.",
                        backend.id(),
                        contract.max_iterations
                    ),
                };
            }
            Err(LoopError::Backend(error)) => {
                return LensOutcome::Fail {
                    case_index: index,
                    detail: format!(
                        "backend `{}` convergence dispatch failed: {error}",
                        backend.id()
                    ),
                };
            }
        };
        let cpu_outputs = match project_output_buffers(&program, &cpu_final) {
            Ok(outputs) => outputs,
            Err(detail) => {
                return LensOutcome::Fail {
                    case_index: index,
                    detail: format!("CPU reference {detail}"),
                };
            }
        };
        let gpu_outputs = match project_output_buffers(&program, &gpu_final) {
            Ok(outputs) => outputs,
            Err(detail) => {
                return LensOutcome::Fail {
                    case_index: index,
                    detail: format!("backend `{}` {detail}", backend.id()),
                };
            }
        };
        if let BufferParity::Mismatch(detail) =
            compare_output_buffers(&program, &cpu_outputs, &gpu_outputs)
        {
            return LensOutcome::Fail {
                case_index: index,
                detail: format!(
                    "backend `{}` final state diverged from CPU reference after convergence loop: {detail}",
                    backend.id()
                ),
            };
        }
    }

    LensOutcome::Pass { cases: cases.len() }
}

fn infer_fixpoint_buffers(program: &Program) -> Result<(&str, &str, u32), String> {
    let ro_buffers: Vec<_> = program
        .buffers()
        .iter()
        .filter(|d| d.access() == BufferAccess::ReadOnly)
        .collect();
    let rw_buffers: Vec<_> = program
        .buffers()
        .iter()
        .filter(|d| d.access() == BufferAccess::ReadWrite)
        .collect();

    let next = rw_buffers
        .last()
        .ok_or_else(|| "no ReadWrite buffer found for fixpoint next".to_string())?
        .name();

    let next_count = rw_buffers
        .last()
        .ok_or_else(|| "no ReadWrite buffer found for fixpoint next".to_string())?
        .count();

    let current_decl = ro_buffers
        .iter()
        .copied()
        .filter(|decl| decl.count() == next_count)
        .min_by_key(|decl| fixpoint_current_score(decl.name(), next))
        .ok_or_else(|| {
            format!("no ReadOnly fixpoint current buffer matches `{next}` count={next_count}")
        })?;
    let current = current_decl.name();
    let current_count = current_decl.count();

    if current_count != next_count {
        return Err(format!(
            "fixpoint buffers `{current}` (count={current_count}) and `{next}` (count={next_count}) must match",
        ));
    }

    Ok((current, next, current_count))
}

fn fixpoint_current_score(current: &str, next: &str) -> u8 {
    if let Some(expected) = next.strip_suffix("out").map(|prefix| format!("{prefix}in")) {
        if current == expected {
            return 0;
        }
    }
    let expected_current = next.replace("next", "current");
    if expected_current != next && current == expected_current {
        return 0;
    }
    if current.contains("current") || current.contains("frontier") || current.ends_with("in") {
        return 1;
    }
    if current.contains("tag") || current.contains("kind") || current.contains("offset") {
        return 8;
    }
    4
}

fn cpu_convergence(
    program: &Program,
    initial_inputs: &[Vec<u8>],
    max_iterations: u32,
    current_idx: usize,
    next_idx: usize,
) -> Result<Vec<Vec<u8>>, LoopError> {
    let mut state: Vec<Vec<u8>> = initial_inputs.to_vec();
    let mut prev_next: Vec<u8> = Vec::new();
    for _ in 0..max_iterations {
        let outputs = run_cpu(program, &state).map_err(LoopError::Reference)?;
        merge_rw(&mut state, &outputs, program);
        if state.get(next_idx) == Some(&prev_next) {
            return Ok(state);
        }
        prev_next = state[next_idx].clone();
        state[current_idx] = state[next_idx].clone();
    }
    Err(LoopError::DidNotConverge)
}

fn gpu_convergence(
    backend: &dyn VyreBackend,
    program: &Program,
    initial_inputs: &[Vec<u8>],
    max_iterations: u32,
    current_idx: usize,
    next_idx: usize,
    config: &DispatchConfig,
) -> Result<Vec<Vec<u8>>, LoopError> {
    let mut state: Vec<Vec<u8>> = initial_inputs.to_vec();
    let mut prev_next: Vec<u8> = Vec::new();
    for _ in 0..max_iterations {
        let borrowed_state: Vec<&[u8]> = state.iter().map(Vec::as_slice).collect();
        let outputs = backend
            .dispatch_borrowed(program, &borrowed_state, config)
            .map_err(LoopError::Backend)?;
        merge_rw(&mut state, &outputs, program);
        if state.get(next_idx) == Some(&prev_next) {
            return Ok(state);
        }
        prev_next = state[next_idx].clone();
        state[current_idx] = state[next_idx].clone();
    }
    Err(LoopError::DidNotConverge)
}

#[derive(Debug)]
enum LoopError {
    Reference(Error),
    Backend(BackendError),
    DidNotConverge,
}

fn cpu_fixpoint(
    program: &Program,
    initial_inputs: &[Vec<u8>],
    flag_index: usize,
    contract: &FixpointContract,
) -> Result<Vec<Vec<u8>>, LoopError> {
    let mut state: Vec<Vec<u8>> = initial_inputs.to_vec();
    for _ in 0..contract.max_iterations {
        // Zero the convergence flag buffer (first u32) before the step.
        if let Some(buffer) = state.get_mut(flag_index) {
            if buffer.len() >= 4 {
                buffer[0..4].copy_from_slice(&0u32.to_le_bytes());
            }
        }
        let outputs = run_cpu(program, &state).map_err(LoopError::Reference)?;
        // `vyre_reference::reference_eval` returns the RW buffers in the same
        // declaration order as the inputs. Merge the RW outputs back
        // into `state` by index.
        merge_rw(&mut state, &outputs, program);
        if flag_word(&state, flag_index) == 0 {
            return Ok(state);
        }
    }
    Err(LoopError::DidNotConverge)
}

fn gpu_fixpoint(
    backend: &dyn VyreBackend,
    program: &Program,
    initial_inputs: &[Vec<u8>],
    flag_index: usize,
    contract: &FixpointContract,
    config: &DispatchConfig,
) -> Result<Vec<Vec<u8>>, LoopError> {
    let mut state: Vec<Vec<u8>> = initial_inputs.to_vec();
    for _ in 0..contract.max_iterations {
        if let Some(buffer) = state.get_mut(flag_index) {
            if buffer.len() >= 4 {
                buffer[0..4].copy_from_slice(&0u32.to_le_bytes());
            }
        }
        let borrowed_state: Vec<&[u8]> = state.iter().map(Vec::as_slice).collect();
        let outputs = backend
            .dispatch_borrowed(program, &borrowed_state, config)
            .map_err(LoopError::Backend)?;
        merge_rw(&mut state, &outputs, program);
        if flag_word(&state, flag_index) == 0 {
            return Ok(state);
        }
    }
    Err(LoopError::DidNotConverge)
}

fn merge_rw(state: &mut [Vec<u8>], outputs: &[Vec<u8>], program: &Program) {
    // `vyre_reference::reference_eval` (and `backend.dispatch`) return only the
    // ReadWrite buffers in declaration order. Walk the declarations in
    // the same order and splice each RW output back into the
    // corresponding slot in `state`.
    let mut out_iter = outputs.iter();
    for (slot, decl) in state.iter_mut().zip(program.buffers().iter()) {
        if matches!(decl.access(), BufferAccess::ReadWrite) {
            if let Some(next) = out_iter.next() {
                *slot = next.clone();
            }
        }
    }
}

fn flag_word(state: &[Vec<u8>], flag_index: usize) -> u32 {
    state
        .get(flag_index)
        .filter(|buffer| buffer.len() >= 4)
        .map(|buffer| u32::from_le_bytes([buffer[0], buffer[1], buffer[2], buffer[3]]))
        .unwrap_or(0)
}

fn index_of_buffer(program: &Program, name: &str) -> Option<usize> {
    program
        .buffers()
        .iter()
        .position(|decl| decl.name() == name)
}

/// Project a full convergence/fixpoint state vector down to just the
/// program's declared output buffers (`ReadWrite`/`WriteOnly`), in
/// `output_buffer_indices` order — the exact shape
/// [`compare_output_buffers`] requires (it asserts the comparison vectors
/// have one entry per declared output).
///
/// The iterative lenses carry the FULL buffer state across iterations
/// (every read-only input plus the read-write frontier) so they can copy
/// `next` → `current` between steps. The only buffers the backend
/// actually computes are the outputs; the read-only inputs are
/// host-managed and identical on both sides by construction, so comparing
/// them adds nothing. Returns an explicit `Err` (never a silently short
/// vector) if `state` is missing a declared output slot, so a malformed
/// fixture surfaces loudly instead of degrading into a confusing
/// length-mismatch downstream.
fn project_output_buffers(program: &Program, state: &[Vec<u8>]) -> Result<Vec<Vec<u8>>, String> {
    program
        .output_buffer_indices()
        .iter()
        .map(|&index| {
            let slot = index as usize;
            state.get(slot).cloned().ok_or_else(|| {
                format!(
                    "convergence state has {} buffer(s) but the program declares an output at \
                     index {slot}; the fixture must supply an initial value for every program buffer.",
                    state.len()
                )
            })
        })
        .collect()
}

#[cfg(test)]
mod convergence_tests {
    use super::*;
    use vyre_foundation::ir::{BufferDecl, DataType, Expr, Node, Program};

    #[test]
    fn infer_fixpoint_buffers_rejects_no_rw() {
        let program = Program::wrapped(
            vec![BufferDecl::storage("x", 0, BufferAccess::ReadOnly, DataType::U32).with_count(1)],
            [1, 1, 1],
            vec![],
        );
        assert!(infer_fixpoint_buffers(&program).is_err());
    }

    #[test]
    fn infer_fixpoint_buffers_matches_in_out_pair() {
        // Simulate the buffer layout of flows_to / sanitized_by.
        let program = Program::wrapped(
            vec![
                BufferDecl::storage("pg_nodes", 0, BufferAccess::ReadOnly, DataType::U32)
                    .with_count(4),
                BufferDecl::storage("fin", 1, BufferAccess::ReadOnly, DataType::U32).with_count(1),
                BufferDecl::storage("fout", 2, BufferAccess::ReadWrite, DataType::U32)
                    .with_count(1),
            ],
            [1, 1, 1],
            vec![],
        );
        let (current, next, count) = infer_fixpoint_buffers(&program).expect("Fix: inference");
        assert_eq!(current, "fin");
        assert_eq!(next, "fout");
        assert_eq!(count, 1);
    }

    #[test]
    fn convergence_contract_ops_are_discoverable() {
        // Every op with a ConvergenceContract must be discoverable and
        // must NOT also have a FixpointContract.
        let convergent_ids: Vec<&str> = vyre_libs::harness::all_entries()
            .filter_map(|e| convergence_contract(e.id).map(|_| e.id))
            .collect();
        assert!(
            !convergent_ids.is_empty(),
            "expected at least one ConvergenceContract-registered op"
        );
        for id in &convergent_ids {
            assert!(
                fixpoint_contract(id).is_none(),
                "{id}: must not register BOTH ConvergenceContract and FixpointContract"
            );
        }
    }

    #[test]
    fn cpu_convergence_reaches_fixpoint_on_accumulating_or() {
        // Synthetic program: each invocation ORs current into next.
        // Iteration 1: next = current | next = 1 | 2 = 3
        // Iteration 2: current = 3, next = 3 | 3 = 3 → converged
        let program = Program::wrapped(
            vec![
                BufferDecl::storage("current", 0, BufferAccess::ReadOnly, DataType::U32)
                    .with_count(1),
                BufferDecl::storage("next", 1, BufferAccess::ReadWrite, DataType::U32)
                    .with_count(1),
            ],
            [1, 1, 1],
            vec![Node::store(
                "next",
                Expr::u32(0),
                Expr::bitor(
                    Expr::load("current", Expr::u32(0)),
                    Expr::load("next", Expr::u32(0)),
                ),
            )],
        );
        let initial = vec![
            vec![1u8, 0, 0, 0], // current = 1
            vec![2u8, 0, 0, 0], // next = 2
        ];
        let result = cpu_convergence(&program, &initial, 10, 0, 1).unwrap();
        let final_next =
            u32::from_le_bytes([result[1][0], result[1][1], result[1][2], result[1][3]]);
        assert_eq!(final_next, 3, "should converge to stable OR of all inputs");
    }

    #[test]
    fn cpu_convergence_respects_max_iterations() {
        // Program that never converges: next = next + 1
        let program = Program::wrapped(
            vec![
                BufferDecl::storage("current", 0, BufferAccess::ReadOnly, DataType::U32)
                    .with_count(1),
                BufferDecl::storage("next", 1, BufferAccess::ReadWrite, DataType::U32)
                    .with_count(1),
            ],
            [1, 1, 1],
            vec![Node::store(
                "next",
                Expr::u32(0),
                Expr::add(Expr::load("next", Expr::u32(0)), Expr::u32(1)),
            )],
        );
        let initial = vec![vec![0u8; 4], vec![0u8; 4]];
        assert!(
            cpu_convergence(&program, &initial, 5, 0, 1).is_err(),
            "non-convergent program should exhaust max_iterations"
        );
    }

    /// Mirrors the real `vyre-libs::security::flows_to` layout: many
    /// read-only graph/input buffers plus a single read-write frontier.
    /// The convergence loop carries the FULL state (7 buffers), but
    /// `compare_output_buffers` is contractually sized to the program's
    /// declared outputs. Before the projection fix the lens fed the
    /// 7-buffer state straight into the comparator, which rejected it with
    /// "program declares 1 output buffer(s), compared 7 result buffer(s)"
    /// even when CPU and GPU agreed byte-for-byte — a false parity
    /// failure. These two assertions pin both halves: the raw full state
    /// is rejected, the projected state is accepted.
    fn flows_to_shaped_program() -> Program {
        Program::wrapped(
            vec![
                BufferDecl::storage("pg_nodes", 0, BufferAccess::ReadOnly, DataType::U32)
                    .with_count(4),
                BufferDecl::storage("pg_edges", 1, BufferAccess::ReadOnly, DataType::U32)
                    .with_count(4),
                BufferDecl::storage("edge_offsets", 2, BufferAccess::ReadOnly, DataType::U32)
                    .with_count(4),
                BufferDecl::storage("sources", 3, BufferAccess::ReadOnly, DataType::U32)
                    .with_count(4),
                BufferDecl::storage("dims", 4, BufferAccess::ReadOnly, DataType::U32).with_count(1),
                BufferDecl::storage("reach_in", 5, BufferAccess::ReadOnly, DataType::U32)
                    .with_count(4),
                BufferDecl::storage("reach_out", 6, BufferAccess::ReadWrite, DataType::U32)
                    .with_count(4),
            ],
            [1, 1, 1],
            vec![],
        )
    }

    #[test]
    fn project_output_buffers_selects_only_the_declared_output() {
        let program = flows_to_shaped_program();
        // Full convergence state: a distinct value per buffer so a wrong
        // projection is detectable, not coincidentally equal.
        let full_state: Vec<Vec<u8>> = (0..7u32)
            .map(|index| {
                let byte = (index + 1) as u8;
                vec![byte; if index == 4 { 4 } else { 16 }]
            })
            .collect();
        assert_eq!(program.output_buffer_indices(), &[6]);

        let projected =
            project_output_buffers(&program, &full_state).expect("Fix: projection must succeed");
        assert_eq!(
            projected.len(),
            1,
            "exactly one declared output (reach_out) must survive projection"
        );
        assert_eq!(
            projected[0], full_state[6],
            "the surviving buffer must be reach_out (the RW frontier), byte-for-byte"
        );
    }

    #[test]
    fn full_state_breaks_comparator_but_projection_restores_parity() {
        let program = flows_to_shaped_program();
        let full_state: Vec<Vec<u8>> = (0..7u32)
            .map(|index| vec![(index + 1) as u8; if index == 4 { 4 } else { 16 }])
            .collect();

        // The raw full state (CPU == GPU, both 7 buffers) is STILL rejected
        // by the comparator because it is sized to declared outputs (1).
        // This is the exact false failure the GPU parity test hit.
        match compare_output_buffers(&program, &full_state, &full_state) {
            BufferParity::Mismatch(detail) => assert!(
                detail.contains("declares 1 output buffer(s), compared 7 result buffer(s)"),
                "expected the output-count mismatch, got: {detail}"
            ),
            BufferParity::Ok => {
                panic!("full 7-buffer state must NOT satisfy a 1-output comparator")
            }
        }

        // Projecting to declared outputs first makes identical states pass.
        let cpu = project_output_buffers(&program, &full_state).expect("Fix: projection");
        let gpu = project_output_buffers(&program, &full_state).expect("Fix: projection");
        assert!(
            matches!(compare_output_buffers(&program, &cpu, &gpu), BufferParity::Ok),
            "projected identical outputs must compare equal"
        );
    }

    #[test]
    fn project_output_buffers_errs_loudly_on_missing_output_slot() {
        let program = flows_to_shaped_program();
        // State truncated before the RW output index (6) — a malformed
        // fixture. Projection must surface this, never silently drop it.
        let short_state: Vec<Vec<u8>> = vec![vec![0u8; 16]; 3];
        let err = project_output_buffers(&program, &short_state)
            .expect_err("missing output slot must be an error, not a silent short vector");
        assert!(
            err.contains("declares an output at index 6"),
            "error must name the missing output index: {err}"
        );
    }
}
