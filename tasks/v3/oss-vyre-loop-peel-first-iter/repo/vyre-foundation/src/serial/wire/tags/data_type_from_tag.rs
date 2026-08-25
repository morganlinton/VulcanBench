use crate::ir::DataType;

/// Decode a [`DataType`] from its VIR0 wire-format tag byte.
///
/// # Preconditions
///
/// `tag` must be a tag assigned by the VIR0 specification. Values outside
/// the defined tag space indicate a version mismatch or malformed input.
///
/// # Returns
///
/// `Ok(DataType)` on a recognized scalar or tensor tag. The mapping is stable
/// and skips tag `12` (array) because arrays require an additional
/// `element_size` payload that must be read by the caller.
///
/// # Failure mode
///
/// * Tag `12` (array) returns `Err` directing the caller to use
///   [`Reader::data_type`](crate::serial::wire::Reader::data_type) instead,
///   because the bare tag is insufficient to reconstruct the full type.
/// * Any other unrecognized tag returns `Err("Fix: unknown data type tag ...")`
///   so callers reject the blob with an actionable diagnostic.
#[inline]
pub(crate) fn data_type_from_tag(tag: u8) -> Result<DataType, String> {
    match tag {
        0x01 => Ok(DataType::U32),
        0x02 => Ok(DataType::I32),
        0x03 => Ok(DataType::U64),
        0x04 => Ok(DataType::Vec2U32),
        0x05 => Ok(DataType::Vec4U32),
        0x06 => Ok(DataType::Bool),
        0x07 => Ok(DataType::Bytes),
        0x08 => Err(
            "Fix: array data type tag requires a VIR0 element_size payload; use Reader::data_type instead of data_type_from_tag."
                .to_string(),
        ),
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
        0x13 => Err(
            "Fix: handle data type tag requires a VIR0 type-id payload; use Reader::data_type instead of data_type_from_tag."
                .to_string(),
        ),
        0x14 => Err(
            "Fix: vector data type tag requires element and count payloads; use Reader::data_type instead of data_type_from_tag."
                .to_string(),
        ),
        0x15 => Err(
            "Fix: shaped tensor data type tag requires element and shape payloads; use Reader::data_type instead of data_type_from_tag."
                .to_string(),
        ),
        0x16 => Err(
            "Fix: sparse-CSR data type tag requires an element-tag payload; use Reader::data_type instead of data_type_from_tag."
                .to_string(),
        ),
        0x17 => Err(
            "Fix: sparse-COO data type tag requires an element-tag payload; use Reader::data_type instead of data_type_from_tag."
                .to_string(),
        ),
        0x18 => Err(
            "Fix: sparse-BSR data type tag requires element + block_rows + block_cols payloads; use Reader::data_type instead of data_type_from_tag."
                .to_string(),
        ),
        0x19 => Ok(DataType::F8E4M3),
        0x1A => Ok(DataType::F8E5M2),
        0x1B => Ok(DataType::I4),
        0x1C => Ok(DataType::FP4),
        0x1D => Ok(DataType::NF4),
        0x1E => Err(
            "Fix: device-mesh data type tag requires an axes payload; use Reader::data_type instead of data_type_from_tag."
                .to_string(),
        ),
        0x1F => Err(
            "Fix: quantized data type tag requires storage, scale, and zero-point payloads; use Reader::data_type instead of data_type_from_tag."
                .to_string(),
        ),
        0x80 => Err(
            "Fix: opaque data type tag 0x80 requires a u32 extension id payload; use Reader::data_type instead of data_type_from_tag."
                .to_string(),
        ),
        _ => Err(format!(
            "Fix: unknown data type tag {tag}; use a compatible IR serializer."
        )),
    }
}
