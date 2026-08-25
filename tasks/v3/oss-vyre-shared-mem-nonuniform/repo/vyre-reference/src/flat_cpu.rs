//! Flat byte adapter that turns every CPU reference into a uniform byte-in,
//! byte-out contract.
//!
//! The parity engine compares raw bytes, not structured values. This module
//! exists so primitive ops can be tested with the same binary-diff harness
//! regardless of their internal Value representation.

use vyre::ir::{BufferAccess, DataType, Program};

use crate::reference_eval;
use crate::value::Value;
/// Execute a program from a concatenated single-case byte payload.
///
/// Fixed-width input buffers consume exactly their declared static element
/// count from `input` (`count == 0` is treated as one element for legacy
/// runtime-sized flat cases). Read-write output buffers are initialized to the
/// same declared fixed-width storage size and appended to `output` after
/// interpretation. Extra or truncated input bytes are rejected so malformed
/// conformance vectors cannot be hidden by padding or ignored suffixes.
///
/// # Errors
///
/// Returns [`vyre::error::Error`] if the program is invalid or execution fails.
///
/// # Examples
///
/// ```rust,ignore
/// let mut out = Vec::new();
/// vyre::reference::flat_cpu::run_flat(&program, &input_bytes, &mut out)?;
/// ```
pub fn run_flat(program: &Program, input: &[u8], output: &mut Vec<u8>) -> Result<(), vyre::Error> {
    let mut offset = 0usize;
    let mut values = Vec::new();
    for buffer in program.buffers() {
        match buffer.access() {
            BufferAccess::ReadOnly | BufferAccess::Uniform => {
                let width = buffer_flat_width(buffer.name(), buffer.element(), buffer.count())?;
                let remaining = input.len().saturating_sub(offset);
                if remaining < width {
                    return Err(vyre::Error::interp(format!(
                        "flat CPU input for buffer `{}` is truncated: expected {width} byte(s), got {remaining}. Fix: provide the declared fixed-width element count for every ReadOnly/Uniform buffer before invoking the reference backend.",
                        buffer.name()
                    )));
                }
                let mut bytes = vec![0; width];
                bytes.copy_from_slice(&input[offset..offset + width]);
                offset += width;
                values.push(Value::from(bytes));
            }
            BufferAccess::ReadWrite => {
                values.push(Value::from(vec![
                    0;
                    buffer_flat_width(
                        buffer.name(),
                        buffer.element(),
                        buffer.count()
                    )?
                ]));
            }
            BufferAccess::Workgroup => {}
            _ => {}
        }
    }
    if offset != input.len() {
        let trailing = input.len() - offset;
        return Err(vyre::Error::interp(format!(
            "flat CPU input has {trailing} trailing byte(s) after consuming declared ReadOnly/Uniform buffers. Fix: provide exactly one fixed-width element per flat input buffer or split multi-case payloads before invoking the reference backend."
        )));
    }
    let values = reference_eval(program, &values)?;
    output.clear();
    for value in values {
        value.extend_bytes_width(0, output)?;
    }
    Ok(())
}

fn output_width(buffer_name: &str, data_type: DataType) -> Result<usize, vyre::Error> {
    let min_bytes = data_type.min_bytes();
    if min_bytes == 0 {
        return Err(vyre::Error::interp(format!(
            "flat CPU buffer `{buffer_name}` uses variable-width element type {data_type:?}. Fix: use a fixed-width element type or route dynamic buffers through the structured reference evaluator."
        )));
    }
    Ok(min_bytes.max(4))
}

fn buffer_flat_width(
    buffer_name: &str,
    data_type: DataType,
    count: u32,
) -> Result<usize, vyre::Error> {
    output_width(buffer_name, data_type)?
        .checked_mul(count.max(1) as usize)
        .ok_or_else(|| {
            vyre::Error::interp(
                "flat CPU buffer byte width overflows usize. Fix: split the flat conformance case or reduce the declared buffer count.",
            )
        })
}
