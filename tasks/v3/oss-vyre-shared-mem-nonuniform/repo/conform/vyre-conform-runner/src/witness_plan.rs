//! Shared witness-input planner for release conformance paths.
//!
//! Fixtures are authored in logical input order, while backend dispatch expects
//! one slice per executable program input. The planner is the single production
//! contract that expands logical witnesses into the stream consumed by both
//! `vyre_reference::reference_eval` and backend `dispatch_borrowed`.

use vyre::ir::{BufferAccess, BufferDecl, MemoryKind, Program};

#[derive(Clone)]
enum WitnessInputSource {
    Fixture {
        fixture_index: usize,
        buffer_index: usize,
        byte_len: Option<usize>,
    },
    ReadWriteOrZero {
        fixture_index: usize,
        buffer_index: usize,
        zero_index: Option<usize>,
        byte_len: Option<usize>,
    },
}

/// Planned mapping from logical witness fixtures to backend/reference inputs.
#[derive(Clone)]
pub struct WitnessInputPlan {
    sources: Vec<WitnessInputSource>,
    zeroed_inputs: Vec<Vec<u8>>,
    buffer_len: usize,
}

impl WitnessInputPlan {
    /// Build the logical witness-input plan for a Program.
    ///
    /// Shared memory, declared output buffers, and pipeline live-out read-write
    /// buffers are not witness inputs. Static read-write buffers can be omitted
    /// from the fixture and are then zero-filled; runtime-sized read-write
    /// buffers require explicit fixture bytes.
    pub fn for_program(program: &Program) -> Result<Self, String> {
        let mut sources = Vec::with_capacity(program.buffers().len());
        let mut zeroed_inputs = Vec::with_capacity(program.buffers().len());
        let mut fixture_index = 0usize;
        for (buffer_index, buffer) in program.buffers().iter().enumerate() {
            if buffer.kind() == MemoryKind::Shared
                || buffer.is_output()
                || (buffer.is_pipeline_live_out()
                    && matches!(buffer.access(), BufferAccess::ReadWrite))
            {
                continue;
            }
            if matches!(buffer.access(), BufferAccess::ReadWrite) {
                let byte_len = fixture_backed_byte_len(buffer, "read-write witness buffer")?;
                let zero_index = if let Some(byte_len) = byte_len {
                    let zero_index = zeroed_inputs.len();
                    zeroed_inputs.push(vec![0u8; byte_len]);
                    Some(zero_index)
                } else {
                    None
                };
                sources.push(WitnessInputSource::ReadWriteOrZero {
                    fixture_index,
                    buffer_index,
                    zero_index,
                    byte_len,
                });
                fixture_index += 1;
                continue;
            }
            let byte_len = fixture_backed_byte_len(buffer, "input witness buffer")?;
            sources.push(WitnessInputSource::Fixture {
                fixture_index,
                buffer_index,
                byte_len,
            });
            fixture_index += 1;
        }

        Ok(Self {
            sources,
            zeroed_inputs,
            buffer_len: program.buffers().len(),
        })
    }

    /// Number of executable input slices produced by this plan.
    pub fn source_count(&self) -> usize {
        self.sources.len()
    }

    /// Number of static read-write inputs that this plan can synthesize.
    pub fn zeroed_input_count(&self) -> usize {
        self.zeroed_inputs.len()
    }
}

fn fixture_backed_byte_len(buffer: &BufferDecl, role: &str) -> Result<Option<usize>, String> {
    buffer
        .static_byte_len()
        .map_err(|error| format!("{role} `{}`: {error}", buffer.name()))
}

/// Static byte length required when synthesising a complete fixture.
pub fn static_buffer_byte_len(buffer: &BufferDecl, role: &str) -> Result<usize, String> {
    buffer
        .static_byte_len()
        .map_err(|error| format!("{role} `{}`: {error}", buffer.name()))?
        .ok_or_else(|| {
            format!(
                "{role} `{}` is runtime-sized. Fix: provide explicit witness bytes for dynamically sized buffers.",
                buffer.name()
            )
        })
}

/// Expand logical fixture bytes into the planned dispatch input stream.
pub fn plan_witness_inputs_into<'a>(
    fixture_inputs: &'a [Vec<u8>],
    plan: &'a WitnessInputPlan,
    backend_inputs: &mut Vec<&'a [u8]>,
) -> Result<(), String> {
    if fixture_inputs.len() > plan.buffer_len {
        return Err(format!(
            "witness fixture provided {} buffer(s) but Program declares {}. Fix: fixture cases must not exceed Program::buffers order.",
            fixture_inputs.len(),
            plan.buffer_len
        ));
    }

    backend_inputs.clear();
    for source in &plan.sources {
        match source {
            WitnessInputSource::Fixture {
                fixture_index,
                buffer_index,
                byte_len,
            } => {
                if let Some(bytes) =
                    matching_fixture_bytes(fixture_inputs, *buffer_index, *fixture_index, *byte_len)
                {
                    backend_inputs.push(bytes);
                    continue;
                }
                return Err(format!(
                    "witness omitted required input buffer at fixture index `{fixture_index}` / program index `{buffer_index}`. Fix: every non-output read-only/uniform buffer must be present in the witness case."
                ));
            }
            WitnessInputSource::ReadWriteOrZero {
                fixture_index,
                buffer_index,
                zero_index,
                byte_len,
            } => {
                if let Some(bytes) =
                    matching_fixture_bytes(fixture_inputs, *buffer_index, *fixture_index, *byte_len)
                {
                    backend_inputs.push(bytes);
                    continue;
                }
                if let Some(zero_index) = zero_index {
                    if let Some(bytes) = plan.zeroed_inputs.get(*zero_index) {
                        backend_inputs.push(bytes.as_slice());
                        continue;
                    }
                    return Err(
                        "internal plan mismatch: zeroed input index is invalid.".to_string()
                    );
                }
                return Err(format!(
                    "witness omitted runtime-sized read-write buffer at fixture index `{fixture_index}` / program index `{buffer_index}`. Fix: provide concrete fixture bytes because dynamic read-write buffers cannot be zero-initialized without a byte length."
                ));
            }
        }
    }
    Ok(())
}

fn matching_fixture_bytes<'a>(
    fixture_inputs: &'a [Vec<u8>],
    buffer_index: usize,
    fixture_index: usize,
    byte_len: Option<usize>,
) -> Option<&'a [u8]> {
    if let Some(byte_len) = byte_len {
        return fixture_inputs
            .get(buffer_index)
            .filter(|bytes| bytes.len() == byte_len)
            .or_else(|| {
                fixture_inputs
                    .get(fixture_index)
                    .filter(|bytes| bytes.len() == byte_len)
            })
            .or_else(|| fixture_inputs.get(fixture_index))
            .or_else(|| fixture_inputs.get(buffer_index))
            .map(Vec::as_slice);
    }
    fixture_inputs
        .get(fixture_index)
        .or_else(|| fixture_inputs.get(buffer_index))
        .map(Vec::as_slice)
}

#[cfg(test)]
mod tests {
    use super::*;
    use vyre::ir::{BufferDecl, DataType, Node};

    #[test]
    fn witness_input_plan_accepts_fixture_backed_runtime_sized_read_input() {
        let program = Program::wrapped(
            vec![
                BufferDecl::storage("input", 0, BufferAccess::ReadOnly, DataType::U32),
                BufferDecl::output("out", 1, DataType::U32).with_count(1),
            ],
            [1, 1, 1],
            Vec::<Node>::new(),
        );
        let plan = WitnessInputPlan::for_program(&program)
            .expect("Fix: runtime-sized read-only buffers must be fixture-backed, not rejected.");
        let case = vec![vec![0xA5; 12]];
        let mut backend_inputs = Vec::new();

        plan_witness_inputs_into(&case, &plan, &mut backend_inputs)
            .expect("Fix: concrete fixture bytes must satisfy a runtime-sized input buffer.");

        assert_eq!(
            backend_inputs,
            vec![case[0].as_slice()],
            "Fix: dynamic fixture-backed inputs must be passed through byte-exactly."
        );
    }

    #[test]
    fn witness_input_plan_uses_zeroed_static_read_write_inputs() {
        let program = Program::wrapped(
            vec![
                BufferDecl::storage("input", 0, BufferAccess::ReadOnly, DataType::U32)
                    .with_count(1),
                BufferDecl::storage("scratch", 1, BufferAccess::ReadWrite, DataType::U32)
                    .with_count(1),
            ],
            [1, 1, 1],
            Vec::<Node>::new(),
        );
        let plan = WitnessInputPlan::for_program(&program)
            .expect("Fix: static read-write zero-fill planning must succeed.");
        let case = vec![1u32.to_le_bytes().to_vec()];
        let mut backend_inputs = Vec::new();

        plan_witness_inputs_into(&case, &plan, &mut backend_inputs)
            .expect("Fix: static read-write buffers may be omitted and zero-filled.");

        assert_eq!(
            backend_inputs,
            vec![case[0].as_slice(), &[0, 0, 0, 0][..]],
            "Fix: backend dispatch input stream must append zero-filled static read-write buffers."
        );
    }

    #[test]
    fn witness_input_plan_rejects_omitted_runtime_sized_read_write_input() {
        let program = Program::wrapped(
            vec![BufferDecl::storage(
                "scratch",
                0,
                BufferAccess::ReadWrite,
                DataType::U32,
            )],
            [1, 1, 1],
            Vec::<Node>::new(),
        );
        let plan = WitnessInputPlan::for_program(&program)
            .expect("Fix: dynamic read-write buffers may be fixture-backed per case.");
        let mut backend_inputs = Vec::new();

        let error = plan_witness_inputs_into(&[], &plan, &mut backend_inputs)
            .expect_err("Fix: omitted dynamic read-write input must not be silently zeroed.");

        assert!(
            error.contains("runtime-sized read-write buffer"),
            "Fix: error must explain that dynamic read-write buffers need concrete fixture bytes, got: {error}"
        );
    }
}
