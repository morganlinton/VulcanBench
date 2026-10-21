//! Size and packing rules for frozen data-type contracts.

use super::DataType;

#[allow(clippy::match_same_arms)]
impl DataType {
    /// Minimum byte count to represent one value of this type.
    #[must_use]
    pub const fn min_bytes(&self) -> usize {
        match self {
            Self::U16 | Self::I16 | Self::F16 | Self::BF16 => 2,
            Self::Bool | Self::U32 | Self::I32 | Self::F32 | Self::Handle(_) => 4,
            Self::I64 | Self::U64 | Self::Vec2U32 | Self::F64 => 8,
            Self::Vec4U32 => 16,
            Self::Vec { element, count } => element.min_bytes().saturating_mul(*count as usize),
            Self::Bytes | Self::Array { .. } | Self::Tensor | Self::TensorShaped { .. } => 0,
            // Quantized / compressed scalar families. F8/F4 = 1 byte rounded up;
            // I4 / NF4 = 1 byte rounded up (two values share a byte in practice,
            // but the conservative minimum is one byte per logical value).
            Self::U8
            | Self::I8
            | Self::F8E4M3
            | Self::F8E5M2
            | Self::I4
            | Self::FP4
            | Self::NF4 => 1,
            // Sparse layouts + device-mesh handles are unbounded at the
            // spec level; runtime asks the extension for a concrete size.
            Self::SparseCsr { .. } | Self::SparseCoo { .. } | Self::SparseBsr { .. } => 0,
            Self::DeviceMesh { .. } => 0,
            Self::Quantized { storage, .. } => storage.min_bytes(),
            // Opaque: conservative sentinel. Real value via ExtensionDataType::min_bytes.
            Self::Opaque(_) => 0,
        }
    }

    /// Maximum byte count for one value of this type.
    ///
    /// Returns `None` for truly unbounded types; currently all variants
    /// have a hard ceiling. Fixed-width types return `Some(min_bytes())`.
    #[must_use]
    pub const fn max_bytes(&self) -> Option<usize> {
        match self {
            Self::U8 | Self::I8 => Some(1),
            Self::U16 | Self::I16 | Self::F16 | Self::BF16 => Some(2),
            Self::U32 | Self::I32 | Self::Bool => Some(4),
            Self::I64 | Self::U64 | Self::Vec2U32 | Self::F64 => Some(8),
            Self::Vec4U32 => Some(16),
            Self::F32 => Some(4),
            Self::Handle(_) => Some(4),
            Self::Vec { element, count } => match element.max_bytes() {
                Some(bytes) => bytes.checked_mul(*count as usize),
                None => None,
            },
            Self::Bytes => Some(64 * 1024 * 1024),
            Self::Array { .. } | Self::Tensor => Some(256 * 1024 * 1024),
            Self::TensorShaped { .. } => None,
            Self::F8E4M3 | Self::F8E5M2 => Some(1),
            Self::I4 | Self::FP4 | Self::NF4 => Some(1),
            Self::SparseCsr { .. } | Self::SparseCoo { .. } | Self::SparseBsr { .. } => None,
            Self::DeviceMesh { .. } => Some(4),
            Self::Quantized { storage, .. } => storage.max_bytes(),
            // Opaque: unbounded at the spec level. Real ceiling via ExtensionDataType::max_bytes.
            Self::Opaque(_) => None,
        }
    }

    /// Element size for array-typed outputs, or `None` for scalar types.
    #[must_use]
    pub const fn element_size(&self) -> Option<usize> {
        match self {
            Self::Array { element_size } => Some(*element_size),
            Self::Vec { element, .. }
            | Self::TensorShaped { element, .. }
            | Self::SparseCsr { element }
            | Self::SparseCoo { element }
            | Self::SparseBsr { element, .. } => element.size_bytes(),
            Self::Quantized { storage, .. } => storage.size_bytes(),
            Self::Opaque(_) => None,
            _ => None,
        }
    }

    /// Fixed scalar element size in bytes, or `None` for variable-size types.
    ///
    /// Scalar types return their natural width (`U32` -> `Some(4)`, `Vec4U32` ->
    /// `Some(16)`). `Bytes` returns `Some(1)` because each element is one byte.
    /// `Array` returns `Some(element_size)`. `Tensor` returns `None` because it
    /// has no fixed per-element size.
    #[must_use]
    pub const fn size_bytes(&self) -> Option<usize> {
        match self {
            Self::U8 | Self::I8 => Some(1),
            Self::U16 | Self::I16 | Self::F16 | Self::BF16 => Some(2),
            Self::Bool | Self::U32 | Self::I32 | Self::F32 => Some(4),
            Self::I64 | Self::U64 | Self::Vec2U32 | Self::F64 => Some(8),
            Self::Vec4U32 => Some(16),
            Self::Handle(_) => Some(4),
            Self::Bytes => Some(1),
            Self::Array { element_size } => Some(*element_size),
            Self::Vec { element, count } => match element.size_bytes() {
                Some(bytes) => bytes.checked_mul(*count as usize),
                None => None,
            },
            Self::Tensor | Self::TensorShaped { .. } => None,
            Self::F8E4M3 | Self::F8E5M2 => Some(1),
            Self::I4 | Self::FP4 | Self::NF4 => Some(1),
            Self::SparseCsr { .. } | Self::SparseCoo { .. } | Self::SparseBsr { .. } => None,
            Self::DeviceMesh { .. } => Some(4),
            Self::Quantized { storage, .. } => storage.size_bytes(),
            // Opaque: real size via ExtensionDataType::size_bytes (runtime).
            Self::Opaque(_) => None,
        }
    }

    /// True element bit width for fixed-width scalar types. Returns `None`
    /// for variable / dynamically-shaped / extension-defined types.
    ///
    /// Sub-byte types (`I4`, `FP4`, `NF4`) report `4` here; `size_bytes`
    /// over-rounds to `1` for safety. Callers that pack two `I4` per
    /// byte (the standard layout for INT4 quantization) need
    /// `bit_width()` to compute correct packed-buffer sizes:
    ///
    /// ```ignore
    /// // Allocate enough bytes to hold `count` packed `I4` values.
    /// let bits = count.checked_mul(DataType::I4.bit_width().unwrap_or(8)).unwrap();
    /// let bytes = bits.div_ceil(8);
    /// ```
    #[must_use]
    pub const fn bit_width(&self) -> Option<usize> {
        match self {
            Self::I4 | Self::FP4 | Self::NF4 => Some(4),
            Self::F8E4M3 | Self::F8E5M2 | Self::U8 | Self::I8 => Some(8),
            Self::U16 | Self::I16 | Self::F16 | Self::BF16 => Some(16),
            Self::Bool | Self::U32 | Self::I32 | Self::F32 | Self::Handle(_) => Some(32),
            Self::I64 | Self::U64 | Self::F64 | Self::Vec2U32 => Some(64),
            Self::Vec4U32 => Some(128),
            Self::DeviceMesh { .. } => Some(32),
            Self::Quantized { storage, .. } => storage.bit_width(),
            Self::Bytes => Some(8),
            // Vec packs `count` elements: total bits scale with the inner
            // element's bit width.
            Self::Vec { element, count } => match element.bit_width() {
                Some(bits) => bits.checked_mul(*count as usize),
                None => None,
            },
            // Variable / extension-defined / dynamically-shaped: no
            // compile-time width.
            Self::Array { .. }
            | Self::Tensor
            | Self::TensorShaped { .. }
            | Self::SparseCsr { .. }
            | Self::SparseCoo { .. }
            | Self::SparseBsr { .. }
            | Self::Opaque(_) => None,
        }
    }

    /// Checked packed byte count for `element_count` logical values.
    ///
    /// This is the sizing helper CUDA and wire codecs should use for sub-byte
    /// quantized storage. `I4`, `FP4`, and `NF4` pack two logical elements per
    /// byte, so `packed_size_bytes(3)` returns `Some(2)` instead of the
    /// conservative `size_bytes() * 3 == 3`. Variable-width types return
    /// `Ok(None)`; arithmetic overflow returns an actionable error instead of
    /// saturating.
    ///
    /// # Errors
    ///
    /// Returns an error when bit or byte arithmetic overflows host `usize`.
    pub fn packed_size_bytes(&self, element_count: usize) -> Result<Option<usize>, String> {
        if let Some(bits) = self.checked_bit_width_for_packed_size()? {
            let total_bits = bits.checked_mul(element_count).ok_or_else(|| {
                format!(
                    "Fix: packed byte sizing overflowed bits for {self} with {element_count} logical element(s)."
                )
            })?;
            return total_bits
                .checked_add(7)
                .map(|rounded_bits| Some(rounded_bits / 8))
                .ok_or_else(|| {
                    format!(
                        "Fix: packed byte sizing overflowed byte rounding for {self} with {element_count} logical element(s)."
                    )
                });
        }
        if let Some(bytes) = self.checked_size_bytes_for_packed_size()? {
            return bytes
                .checked_mul(element_count)
                .map(Some)
                .ok_or_else(|| {
                    format!(
                        "Fix: packed byte sizing overflowed bytes for {self} with {element_count} logical element(s)."
                    )
                });
        }
        Ok(None)
    }

    fn checked_bit_width_for_packed_size(&self) -> Result<Option<usize>, String> {
        match self {
            Self::I4 | Self::FP4 | Self::NF4 => Ok(Some(4)),
            Self::F8E4M3 | Self::F8E5M2 | Self::U8 | Self::I8 => Ok(Some(8)),
            Self::U16 | Self::I16 | Self::F16 | Self::BF16 => Ok(Some(16)),
            Self::Bool | Self::U32 | Self::I32 | Self::F32 | Self::Handle(_) => Ok(Some(32)),
            Self::I64 | Self::U64 | Self::F64 | Self::Vec2U32 => Ok(Some(64)),
            Self::Vec4U32 => Ok(Some(128)),
            Self::DeviceMesh { .. } => Ok(Some(32)),
            Self::Quantized { storage, .. } => storage.checked_bit_width_for_packed_size(),
            Self::Bytes => Ok(Some(8)),
            Self::Vec { element, count } => {
                let Some(bits) = element.checked_bit_width_for_packed_size()? else {
                    return Ok(None);
                };
                bits.checked_mul(*count as usize).map(Some).ok_or_else(|| {
                    format!("Fix: packed byte sizing overflowed nested bit width for {self}.")
                })
            }
            Self::Array { .. }
            | Self::Tensor
            | Self::TensorShaped { .. }
            | Self::SparseCsr { .. }
            | Self::SparseCoo { .. }
            | Self::SparseBsr { .. }
            | Self::Opaque(_) => Ok(None),
        }
    }

    fn checked_size_bytes_for_packed_size(&self) -> Result<Option<usize>, String> {
        match self {
            Self::U8 | Self::I8 => Ok(Some(1)),
            Self::U16 | Self::I16 | Self::F16 | Self::BF16 => Ok(Some(2)),
            Self::Bool | Self::U32 | Self::I32 | Self::F32 => Ok(Some(4)),
            Self::I64 | Self::U64 | Self::Vec2U32 | Self::F64 => Ok(Some(8)),
            Self::Vec4U32 => Ok(Some(16)),
            Self::Handle(_) => Ok(Some(4)),
            Self::Bytes => Ok(Some(1)),
            Self::Array { element_size } => Ok(Some(*element_size)),
            Self::Vec { element, count } => {
                let Some(bytes) = element.checked_size_bytes_for_packed_size()? else {
                    return Ok(None);
                };
                bytes.checked_mul(*count as usize).map(Some).ok_or_else(|| {
                    format!("Fix: packed byte sizing overflowed nested byte width for {self}.")
                })
            }
            Self::Tensor | Self::TensorShaped { .. } => Ok(None),
            Self::F8E4M3 | Self::F8E5M2 => Ok(Some(1)),
            Self::I4 | Self::FP4 | Self::NF4 => Ok(Some(1)),
            Self::SparseCsr { .. } | Self::SparseCoo { .. } | Self::SparseBsr { .. } => Ok(None),
            Self::DeviceMesh { .. } => Ok(Some(4)),
            Self::Quantized { storage, .. } => storage.checked_size_bytes_for_packed_size(),
            Self::Opaque(_) => Ok(None),
        }
    }
}
