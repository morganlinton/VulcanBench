//! Runtime values accepted and returned by the core reference interpreter.

use std::sync::Arc;

/// A concrete value passed into or returned from the reference interpreter.
#[non_exhaustive]
#[derive(Debug, Clone)]
pub enum Value {
    /// Unsigned 32-bit integer.
    U32(u32),
    /// Signed 32-bit integer.
    I32(i32),
    /// Unsigned 64-bit integer.
    U64(u64),
    /// Boolean value.
    Bool(bool),
    /// Raw little-endian storage bytes.
    Bytes(Arc<[u8]>),
    /// Floating-point value represented with stable host bits.
    Float(f64),
    /// Fixed-size array of values.
    Array(Vec<Value>),
}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::U32(a), Self::U32(b)) => a == b,
            (Self::I32(a), Self::I32(b)) => a == b,
            (Self::U64(a), Self::U64(b)) => a == b,
            (Self::Bool(a), Self::Bool(b)) => a == b,
            (Self::Bytes(a), Self::Bytes(b)) => a == b,
            (Self::Float(a), Self::Float(b)) => a.to_bits() == b.to_bits(),
            (Self::Array(a), Self::Array(b)) => a == b,
            _ => false,
        }
    }
}

impl Eq for Value {}

impl Value {
    /// Interpret the value using the IR truth convention.
    #[must_use]
    pub fn truthy(&self) -> bool {
        match self {
            Self::Array(values) => !values.is_empty(),
            Self::Float(value) => *value != 0.0,
            _ => self.try_as_u32().unwrap_or(1) != 0,
        }
    }

    /// Return this value as little-endian bytes for buffer initialization.
    #[must_use]
    pub fn to_bytes(&self) -> Vec<u8> {
        match self {
            Self::U32(value) => value.to_le_bytes().to_vec(),
            Self::I32(value) => value.to_le_bytes().to_vec(),
            Self::U64(value) => value.to_le_bytes().to_vec(),
            Self::Bool(value) => u32::from(*value).to_le_bytes().to_vec(),
            Self::Bytes(bytes) => bytes.to_vec(),
            Self::Float(value) => value.to_le_bytes().to_vec(),
            Self::Array(values) => values.iter().flat_map(Self::to_bytes).collect(),
        }
    }

    /// Return this value encoded at the declared input width.
    #[must_use]
    pub fn to_bytes_width(&self, declared_width: usize) -> Vec<u8> {
        let mut bytes = self.to_bytes();
        if declared_width == 0 {
            return bytes;
        }
        bytes.resize(declared_width, 0);
        bytes.truncate(declared_width);
        bytes
    }

    /// Write this value into an existing fixed-width byte slot.
    ///
    /// For non-empty targets, this is equivalent to
    /// `target.copy_from_slice(&self.to_bytes_width(target.len()))` but avoids
    /// allocating a temporary vector on store-heavy reference paths. Empty
    /// targets are a no-op because they cannot carry the variable-width
    /// `to_bytes_width(0)` payload.
    pub fn write_bytes_width_into(&self, target: &mut [u8]) {
        target.fill(0);
        let mut cursor = 0usize;
        self.copy_raw_bytes_prefix(target, &mut cursor);
    }

    /// Append this value encoded at the declared input width without
    /// allocating a temporary byte vector for the caller.
    ///
    /// # Errors
    ///
    /// Returns an error if the destination length would overflow.
    pub fn extend_bytes_width(
        &self,
        declared_width: usize,
        out: &mut Vec<u8>,
    ) -> Result<(), vyre::Error> {
        let start_len = out.len();
        let fixed_next_len = if declared_width == 0 {
            None
        } else {
            Some(start_len.checked_add(declared_width).ok_or_else(|| {
                vyre::Error::interp(
                    "encoded value byte size overflows usize. Fix: reduce the argument count or byte payload size.",
                )
            })?)
        };
        match self {
            Self::U32(value) => extend_fixed_width(&value.to_le_bytes(), declared_width, out),
            Self::I32(value) => extend_fixed_width(&value.to_le_bytes(), declared_width, out),
            Self::U64(value) => extend_fixed_width(&value.to_le_bytes(), declared_width, out),
            Self::Bool(value) => {
                extend_fixed_width(&u32::from(*value).to_le_bytes(), declared_width, out);
            }
            Self::Bytes(bytes) => extend_fixed_width(bytes, declared_width, out),
            Self::Float(value) => extend_fixed_width(&value.to_le_bytes(), declared_width, out),
            Self::Array(values) => {
                for value in values {
                    value.extend_bytes_width(0, out)?;
                }
                if let Some(next_len) = fixed_next_len {
                    out.truncate(start_len + declared_width.min(out.len() - start_len));
                    out.resize(next_len, 0);
                }
            }
        }
        if let Some(next_len) = fixed_next_len {
            debug_assert_eq!(out.len(), next_len);
        }
        Ok(())
    }

    fn copy_raw_bytes_prefix(&self, target: &mut [u8], cursor: &mut usize) {
        match self {
            Self::U32(value) => copy_bytes_prefix(&value.to_le_bytes(), target, cursor),
            Self::I32(value) => copy_bytes_prefix(&value.to_le_bytes(), target, cursor),
            Self::U64(value) => copy_bytes_prefix(&value.to_le_bytes(), target, cursor),
            Self::Bool(value) => {
                copy_bytes_prefix(&u32::from(*value).to_le_bytes(), target, cursor);
            }
            Self::Bytes(bytes) => copy_bytes_prefix(bytes, target, cursor),
            Self::Float(value) => copy_bytes_prefix(&value.to_le_bytes(), target, cursor),
            Self::Array(values) => {
                for value in values {
                    if *cursor >= target.len() {
                        break;
                    }
                    value.copy_raw_bytes_prefix(target, cursor);
                }
            }
        }
    }

    /// Try to interpret the value as the IR's scalar `u32` word.
    #[must_use]
    pub fn try_as_u32(&self) -> Option<u32> {
        match self {
            Self::U32(value) => Some(*value),
            Self::I32(value) => u32::try_from(*value).ok(),
            Self::U64(value) => u32::try_from(*value).ok(),
            Self::Bool(value) => Some(u32::from(*value)),
            Self::Bytes(bytes) => (bytes.len() <= 4).then(|| read_u32_prefix(bytes)),
            Self::Float(value) => f64_to_u32(*value),
            Self::Array(_) => None,
        }
    }

    /// Interpret the value as the IR's scalar `u32` word.
    #[must_use]
    pub fn as_u32(&self) -> u32 {
        self.try_as_u32().unwrap_or(0)
    }

    /// Try to interpret the value as a full `u64`.
    #[must_use]
    pub fn try_as_u64(&self) -> Option<u64> {
        match self {
            Self::U32(value) => Some(u64::from(*value)),
            Self::I32(value) => u64::try_from(*value).ok(),
            Self::U64(value) => Some(*value),
            Self::Bool(value) => Some(u64::from(*value)),
            Self::Bytes(bytes) => (bytes.len() <= 8).then(|| read_u64_prefix(bytes)),
            Self::Float(value) => f64_to_u64(*value),
            Self::Array(_) => None,
        }
    }

    /// Interpret the value as a full `u64`.
    #[must_use]
    pub fn as_u64(&self) -> u64 {
        self.try_as_u64().unwrap_or(0)
    }

    /// Try to interpret the value as an `f32`.
    #[must_use]
    pub fn try_as_f32(&self) -> Option<f32> {
        match self {
            Self::Float(value) => Some(*value as f32),
            Self::U32(value) => Some(f32::from_bits(*value)),
            _ => None,
        }
    }

    /// Return the full value payload as little-endian bytes.
    #[must_use]
    pub fn wide_bytes(&self) -> Vec<u8> {
        self.to_bytes()
    }

    /// Create a zero value for the given data type.
    #[must_use]
    pub fn zero_for(ty: vyre::ir::DataType) -> Self {
        Self::try_zero_for(ty).unwrap_or_else(|| Self::Bytes(Arc::from([])))
    }

    /// Try to create a zero value for the given data type.
    #[must_use]
    pub fn try_zero_for(ty: vyre::ir::DataType) -> Option<Self> {
        match ty {
            vyre::ir::DataType::U32 => Some(Self::U32(0)),
            vyre::ir::DataType::I32 => Some(Self::I32(0)),
            vyre::ir::DataType::U64 => Some(Self::U64(0)),
            vyre::ir::DataType::Bool => Some(Self::Bool(false)),
            vyre::ir::DataType::Bytes => Some(Self::Bytes(Arc::from([]))),
            vyre::ir::DataType::F32 => Some(Self::Float(0.0)),
            vyre::ir::DataType::F64 => Some(Self::Float(0.0)),
            vyre::ir::DataType::Vec2U32 => Some(Self::Bytes(Arc::from(vec![0; 8]))),
            vyre::ir::DataType::Vec4U32 => Some(Self::Bytes(Arc::from(vec![0; 16]))),
            _ => {
                fixed_scalar_storage_width(&ty).map(|width| Self::Bytes(Arc::from(vec![0; width])))
            }
        }
    }

    /// Create a value from element bytes for the given data type.
    ///
    /// # Errors
    ///
    /// Returns an error if the byte slice is too short for the declared type.
    pub fn from_element_bytes(ty: vyre::ir::DataType, bytes: &[u8]) -> Result<Self, String> {
        match ty {
            vyre::ir::DataType::U32 => {
                if bytes.len() < 4 {
                    return Err("u32 requires 4 bytes".to_string());
                }
                Ok(Self::U32(u32::from_le_bytes([
                    bytes[0], bytes[1], bytes[2], bytes[3],
                ])))
            }
            vyre::ir::DataType::I32 => {
                if bytes.len() < 4 {
                    return Err("i32 requires 4 bytes".to_string());
                }
                Ok(Self::I32(i32::from_le_bytes([
                    bytes[0], bytes[1], bytes[2], bytes[3],
                ])))
            }
            vyre::ir::DataType::U64 => {
                if bytes.len() < 8 {
                    return Err("u64 requires 8 bytes".to_string());
                }
                Ok(Self::U64(u64::from_le_bytes([
                    bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
                ])))
            }
            vyre::ir::DataType::Bool => {
                if bytes.len() < 4 {
                    return Err("bool requires 4 bytes".to_string());
                }
                Ok(Self::Bool(
                    u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) != 0,
                ))
            }
            vyre::ir::DataType::Vec2U32 => {
                if bytes.len() < 8 {
                    return Err("vec2u32 requires 8 bytes".to_string());
                }
                Ok(Self::Bytes(Arc::from(&bytes[..8])))
            }
            vyre::ir::DataType::Vec4U32 => {
                if bytes.len() < 16 {
                    return Err("vec4u32 requires 16 bytes".to_string());
                }
                Ok(Self::Bytes(Arc::from(&bytes[..16])))
            }
            vyre::ir::DataType::F32 => {
                if bytes.len() < 4 {
                    return Err("f32 requires 4 bytes".to_string());
                }
                let value = f32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
                Ok(Self::Float(f64::from(
                    crate::execution::typed_ops::canonical_f32(value),
                )))
            }
            vyre::ir::DataType::F64 => {
                if bytes.len() < 8 {
                    return Err("f64 requires 8 bytes".to_string());
                }
                Ok(Self::Float(f64::from_le_bytes([
                    bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
                ])))
            }
            vyre::ir::DataType::Bytes => Ok(Self::Bytes(Arc::from(bytes))),
            _ => match fixed_scalar_storage_width(&ty) {
                Some(width) => {
                    if bytes.len() < width {
                        return Err(format!("{ty} requires {width} bytes"));
                    }
                    Ok(Self::Bytes(Arc::from(&bytes[..width])))
                }
                None => Ok(Self::Bytes(Arc::from(bytes))),
            },
        }
    }
}

fn fixed_scalar_storage_width(ty: &vyre::ir::DataType) -> Option<usize> {
    match ty {
        vyre::ir::DataType::U8
        | vyre::ir::DataType::I8
        | vyre::ir::DataType::F8E4M3
        | vyre::ir::DataType::F8E5M2
        | vyre::ir::DataType::I4
        | vyre::ir::DataType::FP4
        | vyre::ir::DataType::NF4 => Some(1),
        vyre::ir::DataType::U16
        | vyre::ir::DataType::I16
        | vyre::ir::DataType::F16
        | vyre::ir::DataType::BF16 => Some(2),
        vyre::ir::DataType::Handle(_) | vyre::ir::DataType::DeviceMesh { .. } => Some(4),
        vyre::ir::DataType::I64 => Some(8),
        vyre::ir::DataType::Array { element_size } => Some(*element_size),
        vyre::ir::DataType::Vec { element, count } => fixed_scalar_storage_width(element)
            .and_then(|width| width.checked_mul(usize::from(*count))),
        vyre::ir::DataType::TensorShaped { element, shape } => {
            let element_width = fixed_scalar_storage_width(element)?;
            shape
                .iter()
                .try_fold(element_width, |width, &dim| width.checked_mul(dim as usize))
        }
        vyre::ir::DataType::Quantized { storage, .. } => fixed_scalar_storage_width(storage),
        _ => None,
    }
}

fn extend_fixed_width(bytes: &[u8], declared_width: usize, out: &mut Vec<u8>) {
    if declared_width == 0 {
        out.extend_from_slice(bytes);
        return;
    }
    let copied = bytes.len().min(declared_width);
    out.extend_from_slice(&bytes[..copied]);
    out.resize(out.len() + (declared_width - copied), 0);
}

fn copy_bytes_prefix(bytes: &[u8], target: &mut [u8], cursor: &mut usize) {
    if *cursor >= target.len() {
        return;
    }
    let len = (target.len() - *cursor).min(bytes.len());
    target[*cursor..*cursor + len].copy_from_slice(&bytes[..len]);
    *cursor += len;
}

fn f64_to_u32(value: f64) -> Option<u32> {
    (value.is_finite() && value >= 0.0 && value <= f64::from(u32::MAX)).then(|| value as u32)
}

fn f64_to_u64(value: f64) -> Option<u64> {
    const U64_EXCLUSIVE_MAX_AS_F64: f64 = 18_446_744_073_709_551_616.0;
    (value.is_finite() && value >= 0.0 && value < U64_EXCLUSIVE_MAX_AS_F64).then(|| value as u64)
}

impl From<Vec<u8>> for Value {
    fn from(bytes: Vec<u8>) -> Self {
        Self::Bytes(Arc::from(bytes))
    }
}

impl From<&[u8]> for Value {
    fn from(bytes: &[u8]) -> Self {
        Self::Bytes(Arc::from(bytes))
    }
}

fn read_u32_prefix(bytes: &[u8]) -> u32 {
    let mut padded = [0u8; 4];
    let len = bytes.len().min(4);
    padded[..len].copy_from_slice(&bytes[..len]);
    u32::from_le_bytes(padded)
}

fn read_u64_prefix(bytes: &[u8]) -> u64 {
    let mut padded = [0u8; 8];
    let len = bytes.len().min(8);
    padded[..len].copy_from_slice(&bytes[..len]);
    u64::from_le_bytes(padded)
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn neg_zero_truthiness_is_false() {
        assert!(!Value::Float(-0.0).truthy());
    }

    #[test]
    fn pos_zero_truthiness_is_false() {
        assert!(!Value::Float(0.0).truthy());
    }

    #[test]
    fn nonzero_float_truthiness_is_true() {
        assert!(Value::Float(1.0).truthy());
        assert!(Value::Float(-1.0).truthy());
        assert!(Value::Float(f64::INFINITY).truthy());
        assert!(Value::Float(f64::NEG_INFINITY).truthy());
    }

    #[test]
    fn f32_element_decode_canonicalizes_subnormal_and_nan_payload_bits() {
        let positive_subnormal =
            Value::from_element_bytes(vyre::ir::DataType::F32, &1u32.to_le_bytes())
                .expect("Fix: replace expect with fallible API or document caller precondition; panic only on programmer error - f32 positive subnormal decode must succeed");
        assert_eq!(
            positive_subnormal.try_as_f32().unwrap().to_bits(),
            0x0000_0000
        );

        let negative_subnormal =
            Value::from_element_bytes(vyre::ir::DataType::F32, &0x8000_0001u32.to_le_bytes())
                .expect("Fix: replace expect with fallible API or document caller precondition; panic only on programmer error - f32 negative subnormal decode must succeed");
        assert_eq!(
            negative_subnormal.try_as_f32().unwrap().to_bits(),
            0x8000_0000
        );

        let payload_nan =
            Value::from_element_bytes(vyre::ir::DataType::F32, &0x7fa0_0001u32.to_le_bytes())
                .expect("Fix: replace expect with fallible API or document caller precondition; panic only on programmer error - f32 payload NaN decode must succeed");
        assert_eq!(payload_nan.try_as_f32().unwrap().to_bits(), 0x7fc0_0000);
    }

    proptest! {
        #[test]
        fn neg_zero_select_branches_to_false(
            positive_sign in proptest::bool::ANY,
        ) {
            let zero = if positive_sign { 0.0_f64 } else { -0.0_f64 };
            prop_assert!(!Value::Float(zero).truthy(),
                "Value::Float({zero}).truthy() must be false to match backend bool(0.0)/bool(-0.0) semantics");
        }
    }
}
