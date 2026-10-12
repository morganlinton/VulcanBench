//! Display implementations for frozen data-type contracts.

use core::fmt;

use super::{DataType, QuantizationScale, QuantizationZeroPoint};

impl fmt::Display for DataType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::U8 => f.write_str("u8"),
            Self::U16 => f.write_str("u16"),
            Self::U32 => f.write_str("u32"),
            Self::I8 => f.write_str("i8"),
            Self::I16 => f.write_str("i16"),
            Self::I32 => f.write_str("i32"),
            Self::I64 => f.write_str("i64"),
            Self::U64 => f.write_str("u64"),
            Self::Vec2U32 => f.write_str("vec2<u32>"),
            Self::Vec4U32 => f.write_str("vec4<u32>"),
            Self::Bool => f.write_str("bool"),
            Self::Bytes => f.write_str("bytes"),
            Self::Array { element_size } => write!(f, "array<{element_size}B>"),
            Self::F16 => f.write_str("f16"),
            Self::BF16 => f.write_str("bf16"),
            Self::F32 => f.write_str("f32"),
            Self::F64 => f.write_str("f64"),
            Self::Tensor => f.write_str("tensor"),
            Self::Handle(id) => write!(f, "handle<{:#010x}>", id.as_u32()),
            Self::Vec { element, count } => write!(f, "vec<{element};{count}>"),
            Self::TensorShaped { element, shape } => {
                write!(f, "tensor<{element};")?;
                for (idx, dim) in shape.iter().enumerate() {
                    if idx > 0 {
                        f.write_str("x")?;
                    }
                    write!(f, "{dim}")?;
                }
                f.write_str(">")
            }
            Self::Opaque(id) => write!(f, "opaque<{:#010x}>", id.as_u32()),
            Self::F8E4M3 => f.write_str("f8e4m3"),
            Self::F8E5M2 => f.write_str("f8e5m2"),
            Self::I4 => f.write_str("i4"),
            Self::FP4 => f.write_str("fp4"),
            Self::NF4 => f.write_str("nf4"),
            Self::SparseCsr { element } => write!(f, "sparse_csr<{element}>"),
            Self::SparseCoo { element } => write!(f, "sparse_coo<{element}>"),
            Self::SparseBsr {
                element,
                block_rows,
                block_cols,
            } => write!(f, "sparse_bsr<{element};{block_rows}x{block_cols}>"),
            Self::DeviceMesh { axes } => {
                f.write_str("device_mesh<")?;
                for (idx, axis) in axes.iter().enumerate() {
                    if idx > 0 {
                        f.write_str("x")?;
                    }
                    write!(f, "{axis}")?;
                }
                f.write_str(">")
            }
            Self::Quantized {
                storage,
                scale,
                zero_point,
            } => write!(f, "quantized<{storage};{scale};{zero_point}>"),
        }
    }
}

impl fmt::Display for QuantizationScale {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PerTensor => f.write_str("scale:per_tensor"),
            Self::PerChannel { axis } => write!(f, "scale:per_channel(axis={axis})"),
            Self::PerGroup { group_size } => write!(f, "scale:per_group(size={group_size})"),
        }
    }
}

impl fmt::Display for QuantizationZeroPoint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Absent => f.write_str("zp:absent"),
            Self::PerTensor => f.write_str("zp:per_tensor"),
            Self::PerChannel { axis } => write!(f, "zp:per_channel(axis={axis})"),
            Self::PerGroup { group_size } => write!(f, "zp:per_group(size={group_size})"),
        }
    }
}
