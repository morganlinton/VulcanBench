use std::mem;

pub(crate) fn mark_program_outputs(
    program: vyre_foundation::ir::Program,
    names: &[&str],
) -> vyre_foundation::ir::Program {
    mark_program_outputs_readback(program, names, true)
}

pub(crate) fn mark_program_outputs_readback(
    mut program: vyre_foundation::ir::Program,
    names: &[&str],
    readback: bool,
) -> vyre_foundation::ir::Program {
    mark_program_outputs_with_policy(&mut program, names, readback);
    program
}

fn mark_program_outputs_with_policy(
    program: &mut vyre_foundation::ir::Program,
    names: &[&str],
    readback: bool,
) {
    use vyre_foundation::ir::BufferAccess;

    let mut result_marked = false;
    for buffer in std::sync::Arc::make_mut(&mut program.buffers) {
        if buffer_name_is_selected(buffer.name.as_ref(), names) {
            buffer.access = BufferAccess::ReadWrite;
            buffer.pipeline_live_out = true;
            if result_marked {
                buffer.is_output = false;
            } else {
                buffer.is_output = true;
                result_marked = true;
            }
            if !readback {
                buffer.output_byte_range = Some(0..0);
            }
        }
    }
}

fn buffer_name_is_selected(buffer_name: &str, names: &[&str]) -> bool {
    names.iter().any(|name| buffer_name == *name)
}

pub(crate) fn suppress_readwrite_readback(
    mut program: vyre_foundation::ir::Program,
    names: &[&str],
) -> vyre_foundation::ir::Program {
    for buffer in std::sync::Arc::make_mut(&mut program.buffers) {
        if names.iter().any(|name| buffer.name.as_ref() == *name) {
            buffer.output_byte_range = Some(0..0);
        }
    }
    program
}

pub(crate) fn drop_suppressed_readbacks(outputs: &mut Vec<Vec<u8>>) {
    outputs.retain(|output| !output.is_empty());
}

pub(crate) fn take_exact_output(
    stage: &str,
    outputs: &mut Vec<Vec<u8>>,
) -> Result<Vec<u8>, String> {
    if outputs.len() != 1 {
        return Err(format!(
            "{stage} returned {} output buffer(s), expected exactly 1. Fix: backend must return the declared stage output only.",
            outputs.len()
        ));
    }
    let mut output = Vec::new();
    mem::swap(&mut output, &mut outputs[0]);
    Ok(output)
}

pub(crate) fn take_last_output_into<F>(
    outputs: &mut Vec<Vec<u8>>,
    output: &mut Vec<u8>,
    missing: F,
) -> Result<(), String>
where
    F: FnOnce() -> String,
{
    let mut next = outputs.pop().ok_or_else(missing)?;
    outputs.clear();
    mem::swap(output, &mut next);
    outputs.push(next);
    Ok(())
}

pub(crate) fn is_input_buffer(buf: &vyre_foundation::ir::BufferDecl) -> bool {
    use vyre_foundation::ir::BufferAccess;
    if buf.is_output {
        return false;
    }
    if buf.pipeline_live_out && buf.access == BufferAccess::ReadWrite {
        return false;
    }
    matches!(
        buf.access,
        BufferAccess::ReadOnly | BufferAccess::ReadWrite | BufferAccess::Uniform
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Program};

    fn program_with_outputs() -> Program {
        Program::wrapped(
            vec![
                BufferDecl::storage("input", 0, BufferAccess::ReadOnly, DataType::U32)
                    .with_count(4),
                BufferDecl::storage("out_a", 1, BufferAccess::ReadOnly, DataType::U32)
                    .with_count(4),
                BufferDecl::storage("out_b", 2, BufferAccess::ReadOnly, DataType::U32)
                    .with_count(4),
            ],
            [1, 1, 1],
            vec![],
        )
    }

    #[test]
    fn mark_program_outputs_selects_only_first_named_buffer_as_result() {
        let program = mark_program_outputs(program_with_outputs(), &["out_a", "out_b"]);
        let out_a = program
            .buffers
            .iter()
            .find(|buffer| buffer.name.as_ref() == "out_a")
            .expect("Fix: pipeline outputs must declare out_a before bind; validate program outputs at compile time - out_a exists");
        let out_b = program
            .buffers
            .iter()
            .find(|buffer| buffer.name.as_ref() == "out_b")
            .expect("Fix: pipeline outputs must declare out_b before bind; validate program outputs at compile time - out_b exists");

        assert!(out_a.is_output);
        assert!(out_a.pipeline_live_out);
        assert_eq!(out_a.access, BufferAccess::ReadWrite);
        assert_eq!(out_a.output_byte_range, None);
        assert!(!out_b.is_output);
        assert!(out_b.pipeline_live_out);
        assert_eq!(out_b.access, BufferAccess::ReadWrite);
        assert_eq!(out_b.output_byte_range, None);
    }

    #[test]
    fn mark_program_outputs_readback_false_suppresses_all_named_readbacks() {
        let program =
            mark_program_outputs_readback(program_with_outputs(), &["out_a", "out_b"], false);
        let ranges: Vec<_> = program
            .buffers
            .iter()
            .filter(|buffer| buffer.name.as_ref().starts_with("out_"))
            .map(|buffer| buffer.output_byte_range.clone())
            .collect();

        assert_eq!(ranges, vec![Some(0..0), Some(0..0)]);
    }

    #[test]
    fn drop_suppressed_readbacks_preserves_nonempty_output_slots() {
        let mut outputs = vec![vec![1, 2], Vec::new(), vec![3], Vec::new()];

        drop_suppressed_readbacks(&mut outputs);

        assert_eq!(outputs, vec![vec![1, 2], vec![3]]);
    }

    #[test]
    fn take_last_output_into_preserves_reusable_output_slot() {
        let mut outputs = vec![vec![1, 2, 3, 4]];
        let original_capacity = outputs[0].capacity();
        let mut output = Vec::with_capacity(32);
        output.extend_from_slice(&[9, 9]);

        take_last_output_into(&mut outputs, &mut output, || "missing".to_string()).unwrap();

        assert_eq!(output, vec![1, 2, 3, 4]);
        assert_eq!(outputs.len(), 1);
        assert!(outputs[0].capacity() >= 32);
        assert!(output.capacity() >= original_capacity);
    }

    #[test]
    fn take_exact_output_rejects_missing_and_extra_buffers() {
        let mut none = Vec::new();
        let mut extra = vec![vec![1], vec![2]];
        let mut exact = vec![vec![3, 4]];

        assert!(take_exact_output("stage", &mut none).is_err());
        assert!(take_exact_output("stage", &mut extra).is_err());
        assert_eq!(take_exact_output("stage", &mut exact).unwrap(), vec![3, 4]);
    }
}
