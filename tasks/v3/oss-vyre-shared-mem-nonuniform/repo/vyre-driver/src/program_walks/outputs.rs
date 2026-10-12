//! Output-buffer readback layout and budget checks.

use std::sync::Arc;

use vyre_foundation::ir::{BufferDecl, DataType, Program};

use crate::backend::{BackendError, DispatchConfig};

/// Enforces [`DispatchConfig::max_output_bytes`] against materialized readback buffers.
///
/// # Errors
///
/// Returns when the summed output length exceeds the configured cap.
pub fn enforce_actual_output_budget(
    config: &DispatchConfig,
    outputs: &[Vec<u8>],
) -> Result<(), BackendError> {
    let Some(limit) = config.max_output_bytes else {
        return Ok(());
    };
    let actual = outputs.iter().try_fold(0usize, |sum, output| {
        sum.checked_add(output.len()).ok_or_else(|| {
            BackendError::new(
                "actual readback size overflows usize. Fix: split the Program output before dispatch.",
            )
        })
    })?;
    if actual > limit {
        return Err(BackendError::new(format!(
            "actual readback size {actual} exceeds DispatchConfig.max_output_bytes {limit}. Fix: narrow BufferDecl::output_byte_range or raise max_output_bytes."
        )));
    }
    Ok(())
}

/// Output readback layout derived from a program's declared output range.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OutputLayout {
    /// Full output buffer byte size allocated on the GPU.
    pub full_size: usize,
    /// Consumer-visible byte count returned from dispatch.
    pub read_size: usize,
    /// Aligned source offset copied from the GPU output buffer.
    pub copy_offset: usize,
    /// Aligned staging-buffer byte size.
    pub copy_size: usize,
    /// Offset within the staging buffer where the requested range starts.
    pub trim_start: usize,
}

/// Readback and allocation metadata for one writable buffer.
#[derive(Clone, Debug)]
pub struct OutputBindingLayout {
    /// Buffer binding slot.
    pub binding: u32,
    /// Buffer name for diagnostics.
    pub name: Arc<str>,
    /// Full readback/copy layout for this binding.
    pub layout: OutputLayout,
    /// Rounded-up 32-bit word count used for allocation and clears.
    pub word_count: usize,
}

/// Derive output readback layout for a program.
///
/// # Errors
///
/// Returns a backend error when the program has no output buffer or declares
/// an out-of-bounds output byte range.
pub fn output_layout_from_program(program: &Program) -> Result<OutputLayout, BackendError> {
    let Some(&index) = program.output_buffer_indices().first() else {
        return Err(BackendError::new(
            "program has no output buffer. Fix: declare exactly one output buffer in the vyre Program.",
        ));
    };
    let output = program.buffers().get(index as usize).ok_or_else(|| {
        BackendError::new(format!(
            "output buffer index {index} is out of bounds. Fix: rebuild the Program so writable buffer metadata stays consistent."
        ))
    })?;
    output_binding_layout(output).map(|output| output.layout)
}

/// All output-buffer binding layouts for `program`, in declaration order.
///
/// # Errors
///
/// Returns when there is no output buffer, an index is invalid, or layout
/// math fails.
pub fn output_binding_layouts(program: &Program) -> Result<Vec<OutputBindingLayout>, BackendError> {
    let mut outputs = reserved_output_layout_slots(program.output_buffer_indices().len())?;
    output_binding_layouts_into(program, &mut outputs)?;
    Ok(outputs)
}

/// Write output-buffer binding layouts into caller-owned storage.
///
/// # Errors
///
/// Returns when there is no output buffer, an index is invalid, or layout
/// math fails.
pub fn output_binding_layouts_into(
    program: &Program,
    outputs: &mut Vec<OutputBindingLayout>,
) -> Result<(), BackendError> {
    outputs.clear();
    reserve_output_layout_slots(outputs, program.output_buffer_indices().len())?;
    for &index in program.output_buffer_indices() {
        let output = program.buffers().get(index as usize).ok_or_else(|| {
            BackendError::new(
                format!(
                    "output buffer index {index} is out of bounds. Fix: rebuild the Program so writable buffer metadata stays consistent."
                ),
            )
        })?;
        outputs.push(output_binding_layout(output)?);
    }
    if outputs.is_empty() {
        return Err(BackendError::new(
            "program has no output buffer. Fix: declare at least one writable buffer in the vyre Program.",
        ));
    }
    Ok(())
}

/// Per-output binding layout for a single declared output buffer.
///
/// # Errors
///
/// Returns when counts, element size, or declared byte range are inconsistent.
pub fn output_binding_layout(output: &BufferDecl) -> Result<OutputBindingLayout, BackendError> {
    let count = usize::try_from(output.count()).map_err(|_| {
        BackendError::new(
            "program output element count exceeds usize. Fix: split the dispatch into smaller output buffers.",
        )
    })?;
    output.element.validate_layout().map_err(|error| {
        BackendError::new(format!(
            "program output `{}` has malformed data-type layout metadata: {error}",
            output.name()
        ))
    })?;
    let full_size = output.element.packed_size_bytes(count).map_err(|error| {
        BackendError::new(format!(
            "program output `{}` byte size could not be computed: {error}",
            output.name()
        ))
    })?.ok_or_else(|| {
        BackendError::new(
            "program output element type has no fixed packed byte size. Fix: validate the Program and flatten variable-size outputs before backend pipeline compilation.",
        )
    })?;
    let layout = output_layout(output, full_size)?;
    let word_count = full_size
        .checked_add(3)
        .and_then(|n| n.checked_div(4))
        .ok_or_else(|| {
            BackendError::new(
                "program output word count overflows usize. Fix: split the dispatch into smaller output buffers.",
            )
        })?
        .max(1);
    Ok(OutputBindingLayout {
        binding: output.binding(),
        name: Arc::clone(&output.name),
        layout,
        word_count,
    })
}

fn output_layout(output: &BufferDecl, full_size: usize) -> Result<OutputLayout, BackendError> {
    let range = output.output_byte_range().unwrap_or(0..full_size);
    if range.start > range.end || range.end > full_size {
        return Err(BackendError::new(format!(
            "output byte range {:?} is outside output buffer size {full_size}. Fix: declare a range within the output buffer.",
            range
        )));
    }
    let copy_offset = range.start & !3;
    let copy_end = align_up_to_u32_word(range.end)?.min(full_size.max(4));
    let copy_size = copy_end.checked_sub(copy_offset).ok_or_else(|| {
        BackendError::new(format!(
            "aligned output copy range underflowed: copy_end={copy_end}, copy_offset={copy_offset}. Fix: declare output_byte_range inside the output buffer."
        ))
    })?.max(4);
    Ok(OutputLayout {
        full_size,
        read_size: range.end - range.start,
        copy_offset,
        copy_size,
        trim_start: range.start - copy_offset,
    })
}

fn reserve_output_layout_slots(
    outputs: &mut Vec<OutputBindingLayout>,
    capacity: usize,
) -> Result<(), BackendError> {
    crate::allocation::try_reserve_vec_to_capacity(outputs, capacity).map_err(|error| {
        BackendError::new(format!(
            "output binding layout planning could not reserve {capacity} output slot(s): {error}. Fix: split the Program output set or reuse caller-owned output layout scratch."
        ))
    })
}

fn reserved_output_layout_slots(capacity: usize) -> Result<Vec<OutputBindingLayout>, BackendError> {
    let mut outputs = Vec::new();
    reserve_output_layout_slots(&mut outputs, capacity)?;
    Ok(outputs)
}

fn align_up_to_u32_word(value: usize) -> Result<usize, BackendError> {
    value.checked_add(3).map(|end| end & !3).ok_or_else(|| {
        BackendError::new(format!(
            "aligned output copy end overflows usize for byte offset {value}. Fix: declare a smaller output_byte_range before backend readback planning."
        ))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use vyre_foundation::ir::{BufferDecl, DataType};

    #[test]
    fn output_layout_planning_uses_fallible_modular_reservation_and_alignment() {
        let source = include_str!("outputs.rs");
        let production = source
            .split("#[cfg(test)]")
            .next()
            .expect("Fix: output-layout source must contain production section before tests");

        assert!(
            production.contains("fn reserve_output_layout_slots")
                && production.contains("fn align_up_to_u32_word")
                && production.contains("try_reserve_vec_to_capacity"),
            "Fix: output layout planning must keep reservation and alignment as modular fallible helpers."
        );
        assert!(
            !production.contains("Vec::with_capacity")
                && !production.contains(".reserve(program.output_buffer_indices().len())")
                && !production.contains(".next_multiple_of(4)")
                && !production.contains(".unwrap_or(full_size)"),
            "Fix: output layout planning must not allocate infallibly or hide overflow in release paths."
        );
    }

    #[test]
    fn output_layout_alignment_rejects_usize_overflow() {
        let error =
            align_up_to_u32_word(usize::MAX).expect_err("max byte offset cannot align upward");
        assert!(
            error.to_string().contains("Fix:"),
            "alignment overflow must be actionable: {error}"
        );
    }

    #[test]
    fn output_layout_uses_packed_size_for_subbyte_elements() {
        let output = BufferDecl::output("packed_i4", 0, DataType::I4).with_count(3);
        let layout = output_binding_layout(&output)
            .expect("Fix: packed I4 output layout should use packed byte sizing");

        assert_eq!(layout.layout.full_size, 2);
        assert_eq!(layout.layout.read_size, 2);
        assert_eq!(layout.word_count, 1);
    }

    #[test]
    fn output_layout_rejects_malformed_data_type_layouts() {
        let output = BufferDecl::output(
            "bad_bsr",
            0,
            DataType::SparseBsr {
                element: Box::new(DataType::F32),
                block_rows: 0,
                block_cols: 4,
            },
        )
        .with_count(1);

        let error = output_binding_layout(&output)
            .expect_err("zero-height BSR blocks must not enter output planning");
        assert!(
            error
                .to_string()
                .contains("SparseBsr block_rows must be > 0"),
            "Fix: malformed output data-type layout diagnostics must remain actionable: {error}"
        );
    }
}

/// Fixed scalar element size in bytes for [`DataType`].
///
/// # Errors
///
/// Returns when the type has no fixed size (e.g. unsized or dynamic).
pub fn element_size_bytes(data_type: &DataType) -> Result<usize, BackendError> {
    data_type.size_bytes().ok_or_else(|| {
        BackendError::new(
            "output buffer element type has no fixed scalar element size. Fix: validate the Program and flatten variable-size outputs before backend pipeline compilation.",
        )
    })
}
