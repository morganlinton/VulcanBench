//! Frozen IR data-type tags shared by signatures, validators, and wire metadata.
// TAG RESERVATIONS: U32=0x01, I32=0x02, U64=0x03, Vec2U32=0x04,
// Vec4U32=0x05, Bool=0x06, Bytes=0x07, Array=0x08, F16=0x09,
// BF16=0x0A, F32=0x0B, F64=0x0C, Tensor=0x0D, U8=0x0E, U16=0x0F,
// I8=0x10, I16=0x11, I64=0x12, Handle=0x13, Vec=0x14,
// TensorShaped=0x15, SparseCsr=0x16, SparseCoo=0x17, SparseBsr=0x18,
// F8E4M3=0x19, F8E5M2=0x1A, I4=0x1B, FP4=0x1C, NF4=0x1D,
// DeviceMesh=0x1E, Quantized=0x1F, 0x20..=0x7F reserved, Opaque=0x80.

use crate::extension::ExtensionDataTypeId;

mod display;
mod layout;
mod validation;

/// Stable handle type id for backend-owned GPU resources.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Deserialize, serde::Serialize)]
pub struct TypeId(pub u32);

impl TypeId {
    /// Return the raw stable handle type id.
    #[must_use]
    pub const fn as_u32(self) -> u32 {
        self.0
    }
}

/// Scale metadata layout for a quantized tensor or vector.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Deserialize, serde::Serialize)]
pub enum QuantizationScale {
    /// One scale value for the whole buffer.
    PerTensor,
    /// One scale value per slice along `axis`.
    PerChannel {
        /// Tensor axis carrying independent scale values.
        axis: u32,
    },
    /// One scale value per contiguous group.
    PerGroup {
        /// Number of logical elements per quantization group.
        group_size: u32,
    },
}

/// Zero-point metadata layout for affine quantization.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Deserialize, serde::Serialize)]
pub enum QuantizationZeroPoint {
    /// Symmetric quantization; zero point is implicitly zero.
    Absent,
    /// One zero point for the whole buffer.
    PerTensor,
    /// One zero point per slice along `axis`.
    PerChannel {
        /// Tensor axis carrying independent zero-point values.
        axis: u32,
    },
    /// One zero point per contiguous quantization group.
    PerGroup {
        /// Number of logical elements per quantization group.
        group_size: u32,
    },
}

/// Canonical data types supported by the vyre IR frozen data contract.
///
/// Integer-first by design. GPU floating-point is nondeterministic across
/// vendors through different rounding, fused multiply-add, and subnormal
/// handling. Integer arithmetic is deterministic everywhere. F32 is supported
/// for primitives that require it, with conformance validated per-backend.
/// `vyre::ir::DataType` re-exports this same type; conformance metadata should
/// use this canonical contract path. Example: `DataType::Vec4U32` records a
/// four-word lane value and has a minimum byte width of 16.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Deserialize, serde::Serialize)]
#[non_exhaustive]
pub enum DataType {
    /// Unsigned 8-bit integer.
    U8,
    /// Unsigned 16-bit integer.
    U16,
    /// Unsigned 32-bit integer. The fundamental GPU word.
    U32,
    /// Signed 8-bit integer.
    I8,
    /// Signed 16-bit integer.
    I16,
    /// Signed 32-bit integer.
    I32,
    /// Signed 64-bit integer.
    I64,
    /// Unsigned 64-bit integer, emulated as `vec2<u32>` with low and high words.
    U64,
    /// Two-component `u32` vector.
    Vec2U32,
    /// Four-component `u32` vector.
    Vec4U32,
    /// Boolean value stored as a GPU word.
    Bool,
    /// Variable-length byte buffer.
    Bytes,
    /// Fixed-element-size array.
    ///
    /// Each element is `element_size` bytes. The total byte count is
    /// `N * element_size` where N is encoded by the value.
    Array {
        /// Byte size of each element.
        element_size: usize,
    },
    /// Strict IEEE 754 binary16 floating-point.
    F16,
    /// Strict bfloat16 floating-point.
    BF16,
    /// IEEE 754 binary32 floating-point.
    F32,
    /// Strict IEEE 754 binary64 floating-point.
    F64,
    /// Multi-dimensional tensor value.
    Tensor,
    /// Opaque backend resource handle.
    Handle(TypeId),
    /// Generic fixed-lane vector.
    Vec {
        /// Lane element type.
        element: Box<Self>,
        /// Lane count.
        count: u8,
    },
    /// Tensor with explicit element type and rank-limited shape.
    TensorShaped {
        /// Tensor element type.
        element: Box<Self>,
        /// Tensor dimensions. Four dimensions stay inline.
        shape: smallvec::SmallVec<[u32; 4]>,
    },
    /// Sparse-CSR tensor: compressed sparse row layout. Element type
    /// lives in the dense values buffer; structure (indptr + `col_idx`)
    /// is laid out separately by the consumer per the documented CSR
    /// contract. Size depends on nnz; conservative sentinel applies.
    ///
    /// Wire encoding: tag `0x16` followed by the element type tag.
    SparseCsr {
        /// Element type of the dense values buffer.
        element: Box<Self>,
    },
    /// Sparse-COO tensor: coordinate-list layout with (row, col, val)
    /// triples. Simpler than CSR but less cache-friendly; lowering
    /// passes typically convert COO → CSR before dispatch.
    ///
    /// Wire encoding: tag `0x17` followed by the element type tag.
    SparseCoo {
        /// Element type of each triple's value.
        element: Box<Self>,
    },
    /// Sparse-BSR tensor: block-sparse rows with fixed block size.
    /// Favored by quantized LLM weight matrices (50%+ sparsity at
    /// block-granularity retains line-rate GEMM).
    ///
    /// Wire encoding: tag `0x18` followed by `block_rows u32`,
    /// `block_cols u32`, then the element type tag.
    SparseBsr {
        /// Element type.
        element: Box<Self>,
        /// Block height in elements.
        block_rows: u32,
        /// Block width in elements.
        block_cols: u32,
    },
    /// 8-bit float (E4M3 format, per FP8 spec) for quantized inference.
    F8E4M3,
    /// 8-bit float (E5M2 format, per FP8 spec)  -  wider range than E4M3.
    F8E5M2,
    /// 4-bit signed integer for aggressive LLM weight quantization.
    I4,
    /// 4-bit float for LLM-class inference.
    FP4,
    /// 4-bit "normal-float" (per `QLoRA` paper) for LLM weight compression.
    NF4,
    /// Device-mesh handle  -  topology identifier consumed by
    /// collective ops (`all_reduce`, `all_gather`, `reduce_scatter`,
    /// broadcast). Shape is informational; actual topology is
    /// resolved through the backend's mesh registry.
    DeviceMesh {
        /// Device count along each mesh axis. 1-D = pure ring/tree;
        /// 2-D = torus; higher-D = hypercube.
        axes: smallvec::SmallVec<[u32; 3]>,
    },
    /// First-class quantized value domain.
    ///
    /// `storage` is the physical packed element family (`I4`, `I8`, `U8`,
    /// `F8E4M3`, `NF4`, etc.). `scale` and `zero_point` describe the
    /// sidecar buffers needed to dequantize, operate, and optionally requantize
    /// without losing the stable IR type. This closes RFC-0003 at the spec
    /// layer; concrete ops still choose whether to lower to tensor-core MMA,
    /// scalar dequantize-op-requantize, or a backend-specific packed path.
    Quantized {
        /// Physical storage element type.
        storage: Box<Self>,
        /// Scale sidecar layout.
        scale: QuantizationScale,
        /// Optional zero-point sidecar layout.
        zero_point: QuantizationZeroPoint,
    },
    /// Extension-declared data type.
    ///
    /// The `ExtensionDataTypeId` is stable across process runs and
    /// resolves to a `&'static dyn ExtensionDataType` via
    /// `vyre::dialect::extension::resolve_data_type` (in vyre-core).
    /// Wire encoding of Opaque is `0x80 ++ u32 extension_id`  -  see
    /// `docs/wire-format.md` §Extensions.
    ///
    /// The builtin const methods on `DataType` (`min_bytes`, `max_bytes`,
    /// `size_bytes`, `is_float_family`) return conservative sentinels for
    /// Opaque because the real values live behind the trait and are not
    /// known at compile time. Consumers that need the actual values
    /// should resolve the trait via the vyre-core registry.
    Opaque(ExtensionDataTypeId),
}

#[allow(clippy::match_same_arms)]
impl DataType {
    /// Frozen builtin wire tag for this data type.
    ///
    /// Returns `None` for extension-declared opaque types because their wire
    /// representation is the high-bit extension id, not a core builtin tag.
    #[must_use]
    pub const fn builtin_wire_tag(&self) -> Option<u8> {
        match self {
            Self::U32 => Some(0x01),
            Self::I32 => Some(0x02),
            Self::U64 => Some(0x03),
            Self::Vec2U32 => Some(0x04),
            Self::Vec4U32 => Some(0x05),
            Self::Bool => Some(0x06),
            Self::Bytes => Some(0x07),
            Self::Array { .. } => Some(0x08),
            Self::F16 => Some(0x09),
            Self::BF16 => Some(0x0A),
            Self::F32 => Some(0x0B),
            Self::F64 => Some(0x0C),
            Self::Tensor => Some(0x0D),
            Self::U8 => Some(0x0E),
            Self::U16 => Some(0x0F),
            Self::I8 => Some(0x10),
            Self::I16 => Some(0x11),
            Self::I64 => Some(0x12),
            Self::Handle(_) => Some(0x13),
            Self::Vec { .. } => Some(0x14),
            Self::TensorShaped { .. } => Some(0x15),
            Self::SparseCsr { .. } => Some(0x16),
            Self::SparseCoo { .. } => Some(0x17),
            Self::SparseBsr { .. } => Some(0x18),
            Self::F8E4M3 => Some(0x19),
            Self::F8E5M2 => Some(0x1A),
            Self::I4 => Some(0x1B),
            Self::FP4 => Some(0x1C),
            Self::NF4 => Some(0x1D),
            Self::DeviceMesh { .. } => Some(0x1E),
            Self::Quantized { .. } => Some(0x1F),
            Self::Opaque(_) => None,
        }
    }

    /// Whether this type belongs to the strict floating-point conformance family.
    #[must_use]
    pub const fn is_float_family(&self) -> bool {
        match self {
            Self::F16 | Self::BF16 | Self::F32 | Self::F64 => true,
            Self::F8E4M3 | Self::F8E5M2 | Self::FP4 | Self::NF4 => true,
            Self::Vec { element, .. }
            | Self::TensorShaped { element, .. }
            | Self::SparseCsr { element }
            | Self::SparseCoo { element }
            | Self::SparseBsr { element, .. } => element.is_float_family(),
            Self::Quantized { .. } => false,
            _ => false,
        }
    }

    /// Whether this type carries first-class quantization sidecar metadata.
    #[must_use]
    pub const fn is_quantized(&self) -> bool {
        match self {
            Self::Quantized { .. } => true,
            Self::Vec { element, .. }
            | Self::TensorShaped { element, .. }
            | Self::SparseCsr { element }
            | Self::SparseCoo { element }
            | Self::SparseBsr { element, .. } => element.is_quantized(),
            _ => false,
        }
    }

    /// Whether this type is valid as the storage field of `DataType::Quantized`.
    #[must_use]
    pub const fn is_quantized_storage(&self) -> bool {
        matches!(
            self,
            Self::I4
                | Self::I8
                | Self::I16
                | Self::U8
                | Self::U16
                | Self::F8E4M3
                | Self::F8E5M2
                | Self::FP4
                | Self::NF4
        )
    }
}
