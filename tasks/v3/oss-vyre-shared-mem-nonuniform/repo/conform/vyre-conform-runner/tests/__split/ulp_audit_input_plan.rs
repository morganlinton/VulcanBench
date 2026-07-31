use super::*;

#[derive(Clone)]
pub(crate) enum BackendInputSource {
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

pub(crate) struct BackendDispatchPlan {
    sources: Vec<BackendInputSource>,
    zeroed_inputs: Vec<Vec<u8>>,
    buffer_len: usize,
}

pub(crate) fn backend_dispatch_plan(program: &Program) -> Result<BackendDispatchPlan, String> {
    let mut sources = Vec::with_capacity(program.buffers().len());
    let mut zeroed_inputs = Vec::with_capacity(program.buffers().len());
    let mut fixture_index = 0usize;
    for (buffer_index, buffer) in program.buffers().iter().enumerate() {
        if buffer.kind() == vyre::ir::MemoryKind::Shared
            || buffer.is_output()
            || (buffer.is_pipeline_live_out() && matches!(buffer.access(), BufferAccess::ReadWrite))
        {
            continue;
        }
        if matches!(buffer.access(), BufferAccess::ReadWrite) {
            let byte_len = fixture_backed_byte_len(buffer)?;
            let zero_index = if let Some(byte_len) = byte_len {
                let zero_index = zeroed_inputs.len();
                zeroed_inputs.push(vec![0u8; byte_len]);
                Some(zero_index)
            } else {
                None
            };
            sources.push(BackendInputSource::ReadWriteOrZero {
                fixture_index,
                buffer_index,
                zero_index,
                byte_len,
            });
            fixture_index += 1;
            continue;
        }
        let byte_len = fixture_backed_byte_len(buffer)?;
        sources.push(BackendInputSource::Fixture {
            fixture_index,
            buffer_index,
            byte_len,
        });
        fixture_index += 1;
    }

    Ok(BackendDispatchPlan {
        sources,
        zeroed_inputs,
        buffer_len: program.buffers().len(),
    })
}

fn fixture_backed_byte_len(buffer: &BufferDecl) -> Result<Option<usize>, String> {
    buffer.static_byte_len().map_err(|error| {
        format!(
            "ULP audit witness buffer `{}` static byte length could not be computed: {error}. Fix: use a fixed-width buffer type or provide concrete fixture bytes.",
            buffer.name()
        )
    })
}

pub(crate) fn backend_inputs_from_fixture_into<'a>(
    fixture: &'a [Vec<u8>],
    plan: &'a BackendDispatchPlan,
    outputs: &mut Vec<&'a [u8]>,
) -> Result<(), String> {
    if fixture.len() > plan.buffer_len {
        return Err(format!(
            "ULP audit witness fixture provided {} buffer(s) but Program declares {}. Fix: fixture cases must not exceed Program::buffers order.",
            fixture.len(),
            plan.buffer_len
        ));
    }

    outputs.clear();
    outputs.reserve(plan.sources.len());
    for source in &plan.sources {
        match source {
            BackendInputSource::Fixture {
                fixture_index,
                buffer_index,
                byte_len,
            } => {
                if let Some(bytes) =
                    matching_fixture_bytes(fixture, *buffer_index, *fixture_index, *byte_len)
                {
                    outputs.push(bytes.as_slice());
                    continue;
                }
                return Err(format!(
                    "ULP audit witness omitted required input buffer at fixture index `{fixture_index}` / program index `{buffer_index}`. Fix: every non-output read-only/uniform buffer must be present in the witness case."
                ));
            }
            BackendInputSource::ReadWriteOrZero {
                fixture_index,
                buffer_index,
                zero_index,
                byte_len,
            } => {
                if let Some(bytes) =
                    matching_fixture_bytes(fixture, *buffer_index, *fixture_index, *byte_len)
                {
                    outputs.push(bytes.as_slice());
                    continue;
                }
                if let Some(zero_index) = zero_index {
                    if let Some(bytes) = plan.zeroed_inputs.get(*zero_index) {
                        outputs.push(bytes.as_slice());
                        continue;
                    }
                    return Err(
                        "internal ULP audit plan mismatch: zeroed input index is invalid."
                            .to_string(),
                    );
                }
                return Err(format!(
                    "ULP audit witness omitted runtime-sized read-write buffer at fixture index `{fixture_index}` / program index `{buffer_index}`. Fix: provide concrete fixture bytes because dynamic read-write buffers cannot be zero-initialized without a byte length."
                ));
            }
        }
    }
    Ok(())
}

pub(crate) fn backend_inputs_from_fixture_into_owned(
    fixture: &[Vec<u8>],
    plan: &BackendDispatchPlan,
    outputs: &mut Vec<Vec<u8>>,
) -> Result<(), String> {
    let mut borrowed = Vec::with_capacity(plan.sources.len());
    backend_inputs_from_fixture_into(fixture, plan, &mut borrowed)?;
    outputs.clear();
    outputs.reserve(borrowed.len());
    outputs.extend(borrowed.into_iter().map(<[u8]>::to_vec));
    Ok(())
}

fn matching_fixture_bytes<'a>(
    fixture_inputs: &'a [Vec<u8>],
    buffer_index: usize,
    fixture_index: usize,
    byte_len: Option<usize>,
) -> Option<&'a Vec<u8>> {
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
            .or_else(|| fixture_inputs.get(buffer_index));
    }
    fixture_inputs
        .get(fixture_index)
        .or_else(|| fixture_inputs.get(buffer_index))
}

pub(crate) fn backend_input_buffer_indices(plan: &BackendDispatchPlan) -> Vec<usize> {
    plan.sources
        .iter()
        .map(|source| match source {
            BackendInputSource::Fixture { buffer_index, .. }
            | BackendInputSource::ReadWriteOrZero { buffer_index, .. } => *buffer_index,
        })
        .collect()
}

#[test]
fn ulp_audit_input_plan_accepts_logical_fixture_order_after_output_buffer() {
    let program = Program::wrapped(
        vec![
            BufferDecl::output("out", 0, DataType::U32).with_count(1),
            BufferDecl::storage("input", 1, BufferAccess::ReadOnly, DataType::U32).with_count(2),
        ],
        [1, 1, 1],
        Vec::<Node>::new(),
    );
    let plan = backend_dispatch_plan(&program)
        .expect("Fix: ULP audit logical input planning must succeed.");
    let case = vec![vec![1, 0, 0, 0, 2, 0, 0, 0]];
    let mut backend_inputs = Vec::new();

    backend_inputs_from_fixture_into(&case, &plan, &mut backend_inputs).expect(
        "Fix: ULP audit must route logical fixture bytes even when outputs precede inputs.",
    );

    assert_eq!(
        backend_inputs,
        vec![case[0].as_slice()],
        "Fix: ULP audit must use logical fixture order, not raw Program::buffers indices."
    );
}

#[test]
fn ulp_audit_input_plan_uses_zeroed_static_read_write_inputs() {
    let program = Program::wrapped(
        vec![
            BufferDecl::storage("input", 0, BufferAccess::ReadOnly, DataType::U32).with_count(1),
            BufferDecl::storage("scratch", 1, BufferAccess::ReadWrite, DataType::U32).with_count(1),
        ],
        [1, 1, 1],
        Vec::<Node>::new(),
    );
    let plan = backend_dispatch_plan(&program)
        .expect("Fix: ULP audit static read-write zero-fill planning must succeed.");
    let case = vec![1u32.to_le_bytes().to_vec()];
    let mut backend_inputs = Vec::new();

    backend_inputs_from_fixture_into(&case, &plan, &mut backend_inputs)
        .expect("Fix: ULP audit must synthesize static read-write zero inputs.");

    assert_eq!(
        backend_inputs,
        vec![case[0].as_slice(), &[0, 0, 0, 0][..]],
        "Fix: ULP audit and release conformance must expand read-write witness inputs identically."
    );
}

#[test]
fn ulp_audit_input_plan_rejects_omitted_runtime_sized_read_write_input() {
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
    let plan = backend_dispatch_plan(&program)
        .expect("Fix: dynamic read-write buffers may be fixture-backed per ULP case.");
    let mut backend_inputs = Vec::new();

    let error = backend_inputs_from_fixture_into(&[], &plan, &mut backend_inputs)
        .expect_err("Fix: omitted dynamic read-write inputs must not be silently zeroed.");

    assert!(
        error.contains("runtime-sized read-write buffer"),
        "Fix: ULP audit error must preserve dynamic read-write fixture guidance, got: {error}"
    );
}
