//! Wire decode payload readers for typed memory-region metadata.

use super::{LebReader, Reader};
use crate::ir::{CacheLocality, DataType, MemoryHints, MemoryKind};

pub(super) fn read_dense_quantization_scale(
    reader: &mut Reader<'_>,
) -> Result<vyre_spec::QuantizationScale, String> {
    let tag = reader.leb_u64()?;
    let param = u32::try_from(reader.leb_u64()?).map_err(|err| {
        format!("TruncatedPayload: quantization scale parameter cannot fit u32 ({err}). Fix: reject this payload.")
    })?;
    match tag {
        0 => {
            if param != 0 {
                return Err(format!(
                    "InvalidDiscriminant: PerTensor quantization scale parameter must be 0, got {param}. Fix: reject non-canonical Program bytes."
                ));
            }
            Ok(vyre_spec::QuantizationScale::PerTensor)
        }
        1 => Ok(vyre_spec::QuantizationScale::PerChannel { axis: param }),
        2 => {
            if param == 0 {
                return Err(
                    "InvalidDiscriminant: PerGroup quantization scale requires group_size > 0. Fix: reject non-canonical Program bytes."
                        .to_string(),
                );
            }
            Ok(vyre_spec::QuantizationScale::PerGroup { group_size: param })
        }
        other => Err(format!(
            "InvalidDiscriminant: unknown quantization scale tag {other}. Fix: use a compatible IR serializer."
        )),
    }
}

pub(super) fn read_dense_quantization_zero_point(
    reader: &mut Reader<'_>,
) -> Result<vyre_spec::QuantizationZeroPoint, String> {
    let tag = reader.leb_u64()?;
    let param = u32::try_from(reader.leb_u64()?).map_err(|err| {
        format!("TruncatedPayload: quantization zero-point parameter cannot fit u32 ({err}). Fix: reject this payload.")
    })?;
    match tag {
        0 => {
            if param != 0 {
                return Err(format!(
                    "InvalidDiscriminant: absent quantization zero-point parameter must be 0, got {param}. Fix: reject non-canonical Program bytes."
                ));
            }
            Ok(vyre_spec::QuantizationZeroPoint::Absent)
        }
        1 => {
            if param != 0 {
                return Err(format!(
                    "InvalidDiscriminant: PerTensor quantization zero-point parameter must be 0, got {param}. Fix: reject non-canonical Program bytes."
                ));
            }
            Ok(vyre_spec::QuantizationZeroPoint::PerTensor)
        }
        2 => Ok(vyre_spec::QuantizationZeroPoint::PerChannel { axis: param }),
        3 => {
            if param == 0 {
                return Err(
                    "InvalidDiscriminant: PerGroup quantization zero-point requires group_size > 0. Fix: reject non-canonical Program bytes."
                        .to_string(),
                );
            }
            Ok(vyre_spec::QuantizationZeroPoint::PerGroup {
                group_size: param,
            })
        }
        other => Err(format!(
            "InvalidDiscriminant: unknown quantization zero-point tag {other}. Fix: use a compatible IR serializer."
        )),
    }
}

pub(super) fn read_hints(reader: &mut Reader<'_>) -> Result<MemoryHints, String> {
    let coalesce_axis = match reader.u8()? {
        0 => None,
        1 => Some(reader.u8()?),
        value => {
            return Err(format!(
                "InvalidDiscriminant: field coalesce_axis tag has value {value}. Fix: reserialize with Program::to_wire()."
            ));
        }
    };
    let preferred_alignment = reader.u32()?;
    let cache_locality = match reader.u8()? {
        0 => CacheLocality::Streaming,
        1 => CacheLocality::Temporal,
        2 => CacheLocality::Random,
        value => {
            return Err(format!(
                "InvalidDiscriminant: field cache_locality has value {value}. Fix: reserialize with Program::to_wire()."
            ));
        }
    };
    Ok(MemoryHints {
        coalesce_axis,
        preferred_alignment,
        cache_locality,
    })
}

pub(super) fn memory_kind_from_tag(tag: u8) -> Result<MemoryKind, String> {
    match tag {
        0 => Ok(MemoryKind::Global),
        1 => Ok(MemoryKind::Shared),
        2 => Ok(MemoryKind::Uniform),
        3 => Ok(MemoryKind::Local),
        4 => Ok(MemoryKind::Readonly),
        5 => Ok(MemoryKind::Push),
        6 => Ok(MemoryKind::Persistent),
        value => Err(format!(
            "InvalidDiscriminant: field kind has value {value}. Fix: use a defined MemoryKind discriminant."
        )),
    }
}

pub(super) fn data_type_from_tag(tag: u8) -> Result<DataType, String> {
    match tag {
        0x01 => Ok(DataType::U32),
        0x02 => Ok(DataType::I32),
        0x03 => Ok(DataType::U64),
        0x04 => Ok(DataType::Vec2U32),
        0x05 => Ok(DataType::Vec4U32),
        0x06 => Ok(DataType::Bool),
        0x07 => Ok(DataType::Bytes),
        0x08 => Err("InvalidDiscriminant: Array element tag requires shape payload. Fix: include array element_size in the Dense shape payload.".to_string()),
        0x09 => Ok(DataType::F16),
        0x0A => Ok(DataType::BF16),
        0x0B => Ok(DataType::F32),
        0x0C => Ok(DataType::F64),
        0x0D => Ok(DataType::Tensor),
        0x0E => Ok(DataType::U8),
        0x0F => Ok(DataType::U16),
        0x10 => Ok(DataType::I8),
        0x11 => Ok(DataType::I16),
        0x12 => Ok(DataType::I64),
        0x13 => Err("InvalidDiscriminant: Handle element tag requires shape payload. Fix: include handle type id in the Dense shape payload.".to_string()),
        0x14 => Err("InvalidDiscriminant: Vec element tag is not valid for a Dense memory element. Fix: serialize vectors as scalar lanes or extend the Dense shape payload.".to_string()),
        0x15 => Err("InvalidDiscriminant: TensorShaped element tag is not valid for a Dense memory element. Fix: use DataType::Tensor in this VIR0 schema.".to_string()),
        0x19 => Ok(DataType::F8E4M3),
        0x1A => Ok(DataType::F8E5M2),
        0x1B => Ok(DataType::I4),
        0x1C => Ok(DataType::FP4),
        0x1D => Ok(DataType::NF4),
        value => Err(format!(
            "InvalidDiscriminant: field element has value {value}. Fix: use a defined DataType discriminant."
        )),
    }
}
