//! Semantic invariants enforced while decoding `VIR0` wire payloads.

use super::from_wire::DecodedBuffer;
use crate::ir_inner::model::types::DataType;

pub(crate) fn validate_workgroup_size(workgroup_size: [u32; 3]) -> Result<(), String> {
    for (axis, size) in workgroup_size.into_iter().enumerate() {
        if size == 0 {
            return Err(format!(
                "InvalidDiscriminant: workgroup_size[{axis}] is 0. Fix: reject tampered Program bytes or reserialize with positive workgroup dimensions."
            ));
        }
    }
    Ok(())
}

pub(crate) fn validate_output_range_order(start: usize, end: usize) -> Result<(), String> {
    if start > end {
        return Err(format!(
            "InvalidDiscriminant: output range start {start} exceeds end {end}. Fix: reserialize with Program::to_wire()."
        ));
    }
    Ok(())
}

pub(crate) fn validate_output_range_fits(
    buffer: &DecodedBuffer,
    element: &DataType,
    count_value: u32,
) -> Result<(), String> {
    let Some(range) = buffer.output_byte_range.as_ref() else {
        return Ok(());
    };
    let element_size = element.size_bytes().ok_or_else(|| {
        format!(
            "InvalidDiscriminant: output range on buffer `{}` uses variable-width element type. Fix: serialize only byte ranges over fixed-width buffers.",
            buffer.name
        )
    })?;
    let full_size = usize::try_from(count_value)
        .ok()
        .and_then(|count| count.checked_mul(element_size))
        .ok_or_else(|| {
            format!(
                "InvalidDiscriminant: output range full byte size overflows for buffer `{}`. Fix: split the buffer or reject tampered Program bytes.",
                buffer.name
            )
        })?;
    if range.end > full_size {
        return Err(format!(
            "InvalidDiscriminant: output range end {} exceeds buffer `{}` byte size {full_size}. Fix: reserialize with Program::to_wire().",
            range.end, buffer.name
        ));
    }
    Ok(())
}
