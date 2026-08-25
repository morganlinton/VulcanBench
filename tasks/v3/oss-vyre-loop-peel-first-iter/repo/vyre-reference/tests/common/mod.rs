#![allow(unused_imports)]

use vyre_reference::value::Value;

pub(crate) use vyre_primitives::wire::pack_u32_slice as u32_bytes;

pub(crate) fn bytes_to_u32(value: &Value) -> Vec<u32> {
    vyre_primitives::wire::decode_u32_le_bytes_all(&value.to_bytes())
}
