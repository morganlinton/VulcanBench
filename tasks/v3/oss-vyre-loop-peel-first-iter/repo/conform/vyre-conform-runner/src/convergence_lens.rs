//! CPU↔GPU convergence lens for fixpoint ops.
//!
//! Drives a transfer program and a `bitset_fixpoint` program in a loop
//! until the changed flag clears.

use vyre::ir::{BufferAccess, Program};
use vyre::VyreBackend;
use vyre_reference::value::Value;

use crate::dispatch_grid;

/// Error from the convergence loop.
#[derive(Debug)]
pub enum ConvergenceError {
    /// Backend or reference dispatch failed.
    Dispatch(String),
    /// Did not converge within the iteration budget.
    DidNotConverge {
        /// Max iterations that were attempted.
        max_iterations: u32,
    },
    /// The program's buffer layout is incompatible with the fixpoint
    /// convergence protocol.
    IncompatibleLayout(String),
}

impl std::fmt::Display for ConvergenceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConvergenceError::Dispatch(msg) => write!(f, "dispatch failed: {msg}"),
            ConvergenceError::DidNotConverge { max_iterations } => {
                write!(f, "did not converge in {max_iterations} iterations")
            }
            ConvergenceError::IncompatibleLayout(msg) => {
                write!(f, "incompatible fixpoint layout: {msg}")
            }
        }
    }
}

impl std::error::Error for ConvergenceError {}

/// Run a backend through a fixpoint convergence loop.
///
/// The program is dispatched repeatedly; after each dispatch a
/// `bitset_fixpoint` pass checks whether `current` and `next`
/// differ. If they do, the buffers are swapped and the loop
/// continues. Returns the final RW outputs of `program`.
pub fn run_fixpoint_to_convergence(
    backend: &dyn VyreBackend,
    program: &Program,
    inputs: &[Vec<u8>],
    max_iterations: u32,
) -> Result<Vec<Vec<u8>>, ConvergenceError> {
    let (current_name, next_name, words) = infer_fixpoint_buffers(program)?;
    let mut program_config = dispatch_grid::config_for_program(program).map_err(|error| {
        ConvergenceError::IncompatibleLayout(format!(
            "program dispatch grid cannot be inferred: {error}"
        ))
    })?;
    program_config.grid_override = Some(convergence_grid(program)?);
    let changed_name = "fp_changed";
    let bitset_program = vyre_primitives::fixpoint::bitset_fixpoint::bitset_fixpoint(
        current_name,
        next_name,
        changed_name,
        words,
    );
    let bitset_config = dispatch_grid::config_for_program(&bitset_program).map_err(|error| {
        ConvergenceError::IncompatibleLayout(format!(
            "bitset dispatch grid cannot be inferred: {error}"
        ))
    })?;

    let mut state: Vec<Vec<u8>> = Vec::with_capacity(inputs.len());
    state.extend(inputs.iter().cloned());
    let mut changed_buf = vec![0u8; 4];
    let mut rw_outputs: Vec<Vec<u8>> = Vec::with_capacity(program.buffers().len());

    let current_idx = index_of_buffer(program, current_name).ok_or_else(|| {
        ConvergenceError::IncompatibleLayout(format!(
            "buffer `{current_name}` not found in program"
        ))
    })?;
    let next_idx = index_of_buffer(program, next_name).ok_or_else(|| {
        ConvergenceError::IncompatibleLayout(format!("buffer `{next_name}` not found in program"))
    })?;
    for _ in 0..max_iterations {
        let borrowed: Vec<&[u8]> = state.iter().map(Vec::as_slice).collect();
        let transfer_outputs = backend
            .dispatch_borrowed(program, &borrowed, &program_config)
            .map_err(|e| ConvergenceError::Dispatch(e.to_string()))?;
        merge_rw(&mut state, transfer_outputs.as_slice(), program);

        changed_buf.fill(0);
        let converged = {
            let current = &state[current_idx][..];
            let next = &state[next_idx][..];
            let bitset_inputs = [current, next, &changed_buf[..]];
            let bitset_outputs = backend
                .dispatch_borrowed(&bitset_program, &bitset_inputs, &bitset_config)
                .map_err(|e| ConvergenceError::Dispatch(e.to_string()))?;

            if let Some(flag) = bitset_outputs.first() {
                if let Some(bytes) = flag.get(0..4) {
                    changed_buf.copy_from_slice(bytes);
                } else {
                    changed_buf.fill(0);
                }
            }
            flag_word(&changed_buf) == 0
        };
        if converged {
            extract_rw(program, &state, &mut rw_outputs);
            return Ok(rw_outputs);
        }

        clone_slot(&mut state, current_idx, next_idx);
    }

    Err(ConvergenceError::DidNotConverge { max_iterations })
}

/// CPU-side fixpoint driver using `vyre_reference`.
pub fn run_cpu_fixpoint_to_convergence(
    program: &Program,
    inputs: &[Vec<u8>],
    max_iterations: u32,
) -> Result<Vec<Vec<u8>>, ConvergenceError> {
    let (current_name, next_name, _words) = infer_fixpoint_buffers(program)?;

    let mut state: Vec<Vec<u8>> = Vec::with_capacity(inputs.len());
    state.extend(inputs.iter().cloned());
    let mut values = Vec::with_capacity(state.len());
    let mut transfer_outputs = Vec::with_capacity(program.buffers().len());

    let current_idx = index_of_buffer(program, current_name).ok_or_else(|| {
        ConvergenceError::IncompatibleLayout(format!(
            "buffer `{current_name}` not found in program"
        ))
    })?;
    let next_idx = index_of_buffer(program, next_name).ok_or_else(|| {
        ConvergenceError::IncompatibleLayout(format!("buffer `{next_name}` not found in program"))
    })?;

    for _ in 0..max_iterations {
        let transfer_slice = run_cpu(program, &state, &mut values, &mut transfer_outputs)
            .map_err(|e| ConvergenceError::Dispatch(e.to_string()))?;
        merge_rw(&mut state, transfer_slice, program);

        if state[current_idx] == state[next_idx] {
            extract_rw(program, &state, &mut transfer_outputs);
            return Ok(transfer_outputs);
        }

        clone_slot(&mut state, current_idx, next_idx);
    }

    Err(ConvergenceError::DidNotConverge { max_iterations })
}

fn run_cpu<'a>(
    program: &Program,
    inputs: &[Vec<u8>],
    values: &'a mut Vec<Value>,
    outputs: &'a mut Vec<Vec<u8>>,
) -> Result<&'a [Vec<u8>], vyre::Error> {
    values.clear();
    for input in inputs {
        values.push(Value::from(input.as_slice()));
    }

    let evaluated = vyre_reference::reference_eval(program, values)?;
    outputs.clear();
    outputs.extend(evaluated.into_iter().map(|value| value.to_bytes()));
    Ok(outputs.as_slice())
}

fn convergence_grid(program: &Program) -> Result<[u32; 3], ConvergenceError> {
    let workgroup = program.workgroup_size();
    if workgroup[0] == 0 || workgroup[1] != 1 || workgroup[2] != 1 {
        return Err(ConvergenceError::IncompatibleLayout(format!(
            "convergence lens requires a nonzero 1D workgroup, got {workgroup:?}"
        )));
    }
    let element_count = program
        .buffers()
        .iter()
        .map(|decl| decl.count())
        .max()
        .unwrap_or(1)
        .max(1);
    let grid_x = element_count.div_ceil(workgroup[0]).max(1);
    Ok([grid_x, 1, 1])
}

fn infer_fixpoint_buffers(program: &Program) -> Result<(&str, &str, u32), ConvergenceError> {
    let mut next: Option<&str> = None;
    let mut next_count: Option<u32> = None;
    for buffer in program.buffers().iter().rev() {
        if buffer.access() == BufferAccess::ReadWrite && next.is_none() {
            next = Some(buffer.name());
            next_count = Some(buffer.count());
            break;
        }
    }
    let next = next.ok_or_else(|| {
        ConvergenceError::IncompatibleLayout(
            "no ReadWrite buffer found for fixpoint next".to_string(),
        )
    })?;
    let next_count = next_count.ok_or_else(|| {
        ConvergenceError::IncompatibleLayout(
            "no ReadWrite buffer found for fixpoint next".to_string(),
        )
    })?;

    let mut current_decl = None;
    let mut best_score = u8::MAX;
    for decl in program.buffers().iter() {
        if decl.access() != BufferAccess::ReadOnly {
            continue;
        }
        if decl.count() != next_count {
            continue;
        }
        let score = fixpoint_current_score(decl.name(), next);
        if score < best_score {
            best_score = score;
            current_decl = Some(decl);
        }
    }

    let current_decl = current_decl.ok_or_else(|| {
        ConvergenceError::IncompatibleLayout(format!(
            "no ReadOnly fixpoint current buffer matches `{next}` count={next_count}"
        ))
    })?;
    let current = current_decl.name();
    let current_count = current_decl.count();

    if current_count != next_count {
        return Err(ConvergenceError::IncompatibleLayout(format!(
            "fixpoint buffers `{current}` (count={current_count}) and `{next}` (count={next_count}) must match",
        )));
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

fn merge_rw(state: &mut [Vec<u8>], outputs: &[Vec<u8>], program: &Program) {
    let mut out_iter = outputs.iter();
    for (slot, decl) in state.iter_mut().zip(program.buffers().iter()) {
        if decl.access() == BufferAccess::ReadWrite {
            if let Some(next) = out_iter.next() {
                slot.clone_from(next);
            }
        }
    }
}

#[inline]
fn clone_slot(state: &mut [Vec<u8>], dst: usize, src: usize) {
    if dst == src {
        return;
    }

    if dst < src {
        let (head, tail) = state.split_at_mut(src);
        head[dst].clone_from(&tail[0]);
    } else {
        let (head, tail) = state.split_at_mut(dst);
        tail[0].clone_from(&head[src]);
    }
}

fn extract_rw(program: &Program, state: &[Vec<u8>], outputs: &mut Vec<Vec<u8>>) {
    outputs.clear();
    for (decl, buf) in program.buffers().iter().zip(state.iter()) {
        if decl.access() == BufferAccess::ReadWrite {
            outputs.push(buf.clone());
        }
    }
}

fn flag_word(buffer: &[u8]) -> u32 {
    buffer
        .get(0..4)
        .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
        .unwrap_or(0)
}

fn index_of_buffer(program: &Program, name: &str) -> Option<usize> {
    program
        .buffers()
        .iter()
        .position(|decl| decl.name() == name)
}
