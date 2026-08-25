use super::TypeKey;
use crate::ir::DataType;

impl From<DataType> for TypeKey {
    fn from(value: DataType) -> Self {
        Self::from(&value)
    }
}

impl From<&DataType> for TypeKey {
    fn from(value: &DataType) -> Self {
        // Discriminant tags mirror the wire-format `data_type_tag`
        // (0x01..0x80). Parameterised variants pack their parameters
        // into the high bits so distinct shapes produce distinct
        // keys  -  the CSE intern table uses TypeKey for structural
        // equality, and a collapsing fallback would let CSE merge
        // semantically distinct typed expressions (e.g. F16 atomics
        // with F64 atomics) onto the same id.
        match value {
            DataType::U32 => Self(0x01),
            DataType::I32 => Self(0x02),
            DataType::U64 => Self(0x03),
            DataType::Vec2U32 => Self(0x04),
            DataType::Vec4U32 => Self(0x05),
            DataType::Bool => Self(0x06),
            DataType::Bytes => Self(0x07),
            DataType::F16 => Self(0x09),
            DataType::BF16 => Self(0x0A),
            DataType::F32 => Self(0x0B),
            DataType::F64 => Self(0x0C),
            DataType::Tensor => Self(0x0D),
            DataType::U8 => Self(0x0E),
            DataType::U16 => Self(0x0F),
            DataType::I8 => Self(0x10),
            DataType::I16 => Self(0x11),
            DataType::I64 => Self(0x12),
            DataType::Array { element_size } => {
                Self(0x08 | (u64::try_from(*element_size).unwrap_or(u64::MAX) << 8))
            }
            DataType::Handle(id) => Self(0x13 | (u64::from(id.as_u32()) << 8)),
            DataType::Vec { element, count } => {
                Self(0x14 | (u64::from(*count) << 8) | (Self::from(element.as_ref()).0 << 16))
            }
            // TensorShaped's element + shape contribute to the key so
            // tensor<f32; 32x32> does not CSE-merge with tensor<f16; 8>.
            DataType::TensorShaped { element, shape } => {
                let mut h: u64 = 0x15;
                h |= Self::from(element.as_ref()).0 << 8;
                let mut shape_acc: u64 = 0xcbf2_9ce4_8422_2325;
                for dim in shape {
                    shape_acc ^= u64::from(*dim);
                    shape_acc = shape_acc.wrapping_mul(0x100_0000_01b3);
                }
                Self(h ^ (shape_acc << 24))
            }
            DataType::SparseCsr { element } => Self(0x16 | (Self::from(element.as_ref()).0 << 8)),
            DataType::SparseCoo { element } => Self(0x17 | (Self::from(element.as_ref()).0 << 8)),
            DataType::SparseBsr {
                element,
                block_rows,
                block_cols,
            } => Self(
                0x18 | (Self::from(element.as_ref()).0 << 8)
                    | ((u64::from(*block_rows) & 0xFFFF) << 32)
                    | ((u64::from(*block_cols) & 0xFFFF) << 48),
            ),
            DataType::F8E4M3 => Self(0x19),
            DataType::F8E5M2 => Self(0x1A),
            DataType::I4 => Self(0x1B),
            DataType::FP4 => Self(0x1C),
            DataType::NF4 => Self(0x1D),
            DataType::DeviceMesh { axes } => {
                let mut acc: u64 = 0xcbf2_9ce4_8422_2325;
                for axis in axes {
                    acc ^= u64::from(*axis);
                    acc = acc.wrapping_mul(0x100_0000_01b3);
                }
                Self(0x1E | (acc << 8))
            }
            DataType::Quantized {
                storage,
                scale,
                zero_point,
            } => {
                let mut acc = 0x1F | (Self::from(storage.as_ref()).0 << 8);
                acc ^= quantization_scale_key(scale).rotate_left(17);
                acc ^= quantization_zero_point_key(zero_point).rotate_left(37);
                Self(acc)
            }
            DataType::Opaque(id) => Self(0x80 | (u64::from(id.as_u32()) << 8)),
            // Any future variant must extend this table  -  keep the
            // sentinel as a hard-fail beacon (0xFFFF_FFFF distinguishes
            // "unknown" from any real assigned tag).
            _ => Self(0xFFFF_FFFF),
        }
    }
}

fn quantization_scale_key(scale: &vyre_spec::QuantizationScale) -> u64 {
    match scale {
        vyre_spec::QuantizationScale::PerTensor => 0,
        vyre_spec::QuantizationScale::PerChannel { axis } => 1 | (u64::from(*axis) << 8),
        vyre_spec::QuantizationScale::PerGroup { group_size } => 2 | (u64::from(*group_size) << 8),
    }
}

fn quantization_zero_point_key(zero_point: &vyre_spec::QuantizationZeroPoint) -> u64 {
    match zero_point {
        vyre_spec::QuantizationZeroPoint::Absent => 0,
        vyre_spec::QuantizationZeroPoint::PerTensor => 1,
        vyre_spec::QuantizationZeroPoint::PerChannel { axis } => 2 | (u64::from(*axis) << 8),
        vyre_spec::QuantizationZeroPoint::PerGroup { group_size } => {
            3 | (u64::from(*group_size) << 8)
        }
    }
}
