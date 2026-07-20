//! Out-of-bounds rules enforced by the parity engine.
//!
//! GPU drivers differ on what happens when a shader indexes past the end of a
//! buffer: some clamp, some return zero, some crash. The reference interpreter
//! eliminates that ambiguity by defining one deterministic behavior  -  defined-type
//! zero-fill for scalar loads, empty slice for `Bytes`, and silent no-op for stores.
//! Any backend that diverges from these rules fails the conform gate.

use vyre::ir::DataType as IrDataType;

use crate::value::Value;
use vyre::ir::DataType;

use std::sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard};

/// Typed bytes backing one declared IR buffer.
///
/// This struct exists to give the reference interpreter a single place to enforce
/// stride-correct indexing and OOB semantics, independent of how any GPU driver
/// handles buffer bounds.
#[derive(Debug, Clone)]
pub struct Buffer {
    pub(crate) bytes: Arc<RwLock<Vec<u8>>>,
    pub(crate) element: IrDataType,
}

impl Buffer {
    /// Create a buffer from typed bytes.
    #[must_use]
    pub fn new(bytes: Vec<u8>, element: DataType) -> Self {
        Self {
            bytes: Arc::new(RwLock::new(bytes)),
            element,
        }
    }

    /// Acquire the byte buffer for reading, failing closed on poison.
    ///
    /// A poisoned lock means a writer panicked mid-store, leaving the bytes
    /// inconsistent. Silently recovering with `into_inner()` would let the CPU
    /// reference oracle emit corrupt golden values that the conform gate then
    /// trusts as truth — a silent correctness fallback (Law 10). Surface it.
    fn read_bytes(&self) -> RwLockReadGuard<'_, Vec<u8>> {
        self.bytes
            .read()
            .unwrap_or_else(|_| panic!("reference Buffer byte lock was poisoned"))
    }

    /// Acquire the byte buffer for writing, failing closed on poison (see
    /// [`Buffer::read_bytes`]).
    fn write_bytes(&self) -> RwLockWriteGuard<'_, Vec<u8>> {
        self.bytes
            .write()
            .unwrap_or_else(|_| panic!("reference Buffer byte lock was poisoned"))
    }

    pub(crate) fn len(&self) -> u32 {
        let bytes_guard = self.read_bytes();
        let count = if let Some(bits) = self.element.bit_width() {
            bytes_guard
                .len()
                .checked_mul(8)
                .map(|total_bits| total_bits / bits)
                .unwrap_or(usize::MAX)
        } else if let Some(stride) = self.element.size_bytes() {
            if stride == 0 {
                bytes_guard.len()
            } else {
                bytes_guard.len() / stride
            }
        } else {
            bytes_guard.len()
        };
        match u32::try_from(count) {
            Ok(value) => value,
            Err(_) => {
                debug_assert!(
                    false,
                    "Buffer::len overflowed u32::MAX for byte_len={}; element={:?}. \
                     Fix: split or downsize the buffer so per-element indexing remains representable.",
                    bytes_guard.len(),
                    self.element
                );
                u32::MAX
            }
        }
    }

    pub(crate) fn byte_len(&self) -> usize {
        self.read_bytes().len()
    }

    pub(crate) fn element(&self) -> &IrDataType {
        &self.element
    }

    pub(crate) fn zero_fill(&self) {
        self.write_bytes().fill(0);
    }

    pub(crate) fn into_bytes(self) -> Vec<u8> {
        // Same poison policy as the guard helpers: a poisoned lock is a corrupt
        // reference buffer, never silently laundered.
        std::sync::Arc::try_unwrap(self.bytes)
            .map(|rw| {
                rw.into_inner()
                    .unwrap_or_else(|_| panic!("reference Buffer byte lock was poisoned"))
            })
            .unwrap_or_else(|shared| {
                shared
                    .read()
                    .unwrap_or_else(|_| panic!("reference Buffer byte lock was poisoned"))
                    .clone()
            })
    }

    /// Consume this buffer and return its contents as a Value.
    #[must_use]
    pub fn to_value(self) -> crate::value::Value {
        crate::value::Value::from(self.into_bytes())
    }
}

pub(crate) fn load(buffer: &Buffer, index: u32) -> Value {
    let bytes_guard = buffer.read_bytes();
    let stride = buffer.element.min_bytes();
    let ty = ir_to_conform_type(buffer.element.clone());
    if matches!(buffer.element, IrDataType::Bytes) {
        let offset = index as usize;
        if offset > bytes_guard.len() {
            return Value::from(Vec::new());
        }
        return Value::from(&bytes_guard[offset..]);
    }
    let Some(offset) = byte_offset(index, stride) else {
        return Value::try_zero_for(ty).unwrap_or_else(|| Value::from(Vec::new()));
    };
    if stride == 0 || offset + stride > bytes_guard.len() {
        return Value::try_zero_for(ty).unwrap_or_else(|| Value::from(Vec::new()));
    }
    read_element(ty.clone(), &bytes_guard[offset..offset + stride])
        .unwrap_or_else(|_| Value::try_zero_for(ty).unwrap_or_else(|| Value::from(Vec::new())))
}

pub(crate) fn store(buffer: &mut Buffer, index: u32, value: &Value) {
    let mut bytes_guard = buffer.write_bytes();
    let stride = buffer.element.min_bytes();
    if matches!(buffer.element, IrDataType::Bytes) {
        let offset = index as usize;
        if offset >= bytes_guard.len() {
            return;
        }
        let bytes = value.to_bytes();
        let available = bytes_guard.len() - offset;
        let write_len = bytes.len().min(available);
        bytes_guard[offset..offset + write_len].copy_from_slice(&bytes[..write_len]);
        return;
    }
    let Some(offset) = byte_offset(index, stride) else {
        return;
    };
    if stride == 0 || offset + stride > bytes_guard.len() {
        return;
    }
    write_element(
        buffer.element.clone(),
        &mut bytes_guard[offset..offset + stride],
        value,
    );
}

pub(crate) fn atomic_load(buffer: &Buffer, index: u32) -> Option<u32> {
    let bytes_guard = buffer.read_bytes();
    let stride = buffer.element.min_bytes().max(4);
    let offset = byte_offset(index, stride)?;
    if offset + 4 > bytes_guard.len() {
        None
    } else {
        Some(read_u32(&bytes_guard[offset..offset + 4]))
    }
}

pub(crate) fn atomic_store(buffer: &mut Buffer, index: u32, value: u32) {
    let mut bytes_guard = buffer.write_bytes();
    let stride = buffer.element.min_bytes().max(4);
    let Some(offset) = byte_offset(index, stride) else {
        return;
    };
    if offset + 4 <= bytes_guard.len() {
        write_u32(&mut bytes_guard[offset..offset + 4], value);
    }
}

fn byte_offset(index: u32, stride: usize) -> Option<usize> {
    (index as usize).checked_mul(stride)
}

fn write_element(element: IrDataType, target: &mut [u8], value: &Value) {
    match element {
        IrDataType::U32 => {
            value.write_bytes_width_into(target);
        }
        IrDataType::I32 => {
            value.write_bytes_width_into(target);
        }
        IrDataType::Bool => {
            value.write_bytes_width_into(target);
        }
        IrDataType::U64 => {
            value.write_bytes_width_into(target);
        }
        IrDataType::F32 => {
            // Value::Float carries an f64; the GPU buffer is four bytes
            // of f32, so narrow via `as f32` before writing. Dropping the
            // upper four bytes of `v.to_le_bytes()` (what the default
            // to_bytes_width path does) would mangle the f32 bit pattern.
            let v = match value {
                Value::Float(v) => *v as f32,
                Value::U32(v) => f32::from_bits(*v),
                _ => 0.0,
            };
            let v = crate::execution::typed_ops::canonical_f32(v);
            target.copy_from_slice(&v.to_le_bytes());
        }
        IrDataType::Bytes | IrDataType::Vec2U32 | IrDataType::Vec4U32 => {
            value.write_bytes_width_into(target);
        }
        _ => {
            value.write_bytes_width_into(target);
        }
    }
}

fn read_element(ty: DataType, bytes: &[u8]) -> Result<Value, String> {
    Value::from_element_bytes(ty, bytes)
}

fn read_u32(bytes: &[u8]) -> u32 {
    u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
}

fn write_u32(bytes: &mut [u8], value: u32) {
    bytes.copy_from_slice(&value.to_le_bytes());
}

fn ir_to_conform_type(ty: IrDataType) -> DataType {
    match ty {
        IrDataType::U32 => DataType::U32,
        IrDataType::I32 => DataType::I32,
        IrDataType::U64 => DataType::U64,
        IrDataType::F32 => DataType::F32,
        IrDataType::F64 => DataType::F64,
        IrDataType::Vec2U32 => DataType::Vec2U32,
        IrDataType::Vec4U32 => DataType::Vec4U32,
        IrDataType::Bool => DataType::U32,
        IrDataType::Bytes => DataType::Bytes,
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn f32_bits(value: Value) -> u32 {
        match value {
            Value::Float(value) => (value as f32).to_bits(),
            other => {
                let bytes = other.to_bytes();
                u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
            }
        }
    }

    #[test]
    fn f32_load_canonicalizes_subnormal_and_nan_payloads() {
        let positive_subnormal = Buffer::new(1u32.to_le_bytes().to_vec(), DataType::F32);
        assert_eq!(f32_bits(load(&positive_subnormal, 0)), 0x0000_0000);

        let negative_subnormal = Buffer::new(0x8000_0001u32.to_le_bytes().to_vec(), DataType::F32);
        assert_eq!(f32_bits(load(&negative_subnormal, 0)), 0x8000_0000);

        let payload_nan = Buffer::new(0x7fa0_0001u32.to_le_bytes().to_vec(), DataType::F32);
        assert_eq!(f32_bits(load(&payload_nan, 0)), 0x7fc0_0000);
    }

    #[test]
    fn f32_store_canonicalizes_subnormal_and_nan_payloads() {
        let mut subnormal = Buffer::new(vec![0; 4], DataType::F32);
        store(
            &mut subnormal,
            0,
            &Value::Float(f64::from(f32::from_bits(0x8000_0001))),
        );
        assert_eq!(f32_bits(subnormal.to_value()), 0x8000_0000);

        let mut payload_nan = Buffer::new(vec![0; 4], DataType::F32);
        store(&mut payload_nan, 0, &Value::U32(0x7fa0_0001));
        assert_eq!(f32_bits(payload_nan.to_value()), 0x7fc0_0000);
    }

    #[test]
    fn poisoned_reference_buffer_lock_is_not_silently_recovered() {
        // A writer that panics mid-store poisons the lock. The reference oracle
        // must fail closed on a subsequent access rather than handing back the
        // half-mutated bytes (which would silently produce a corrupt golden
        // value the conform gate then trusts) — Law 10.
        let buffer = Buffer::new(vec![0u8; 8], DataType::U32);
        let poisoner = buffer.clone();
        let _ = std::thread::spawn(move || {
            let _guard = poisoner.write_bytes();
            panic!("poison reference buffer lock mid-store");
        })
        .join();

        let panic = std::panic::catch_unwind(|| {
            let _ = buffer.len();
        })
        .expect_err("poisoned reference Buffer lock must panic instead of recovering");
        let message = panic
            .downcast_ref::<String>()
            .map(String::as_str)
            .or_else(|| panic.downcast_ref::<&'static str>().copied())
            .unwrap_or("<non-string panic>");
        assert!(
            message.contains("reference Buffer byte lock was poisoned"),
            "panic must name the poisoned reference buffer lock, got: {message}"
        );
    }
}
