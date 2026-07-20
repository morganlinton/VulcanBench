use crate::ir::DataType;
use crate::serial::wire::encode::WireEncodeErr;
use crate::serial::wire::framing::{put_u32, put_u8};
use crate::serial::wire::{MAX_MESH_AXES, MAX_TENSOR_RANK};

/// Encode a [`DataType`] into its stable VIR0 wire-format tag byte.
///
/// # Preconditions
///
/// `value` must be a variant known to the VIR0 encoder. Because `DataType` is
/// `#[non_exhaustive]`, spec additions must receive a tag here before
/// they can round-trip through the wire format.
///
/// # Returns
///
/// `Ok(u8)` containing the tag value. Scalar and tensor types map to a single
/// byte; `Array` maps to tag `12` and the caller must follow up with the
/// `element_size` payload via [`put_data_type`].
///
/// # Failure mode
///
/// Returns `Err("unknown DataType variant")` when the variant has no
/// registered tag, preventing silent data loss on round-trip.
/// Wire tag reserved for extension `DataTypes`. The tag byte is `0x80`;
/// the u32 extension id follows immediately (little-endian). See
/// `docs/wire-format.md` §Extensions.
pub(crate) const DATA_TYPE_TAG_OPAQUE: u8 = 0x80;

#[inline]
pub(crate) fn data_type_tag(value: &DataType) -> Result<u8, WireEncodeErr> {
    match value {
        DataType::U32 => Ok(0x01),
        DataType::I32 => Ok(0x02),
        DataType::U64 => Ok(0x03),
        DataType::Vec2U32 => Ok(0x04),
        DataType::Vec4U32 => Ok(0x05),
        DataType::Bool => Ok(0x06),
        DataType::Bytes => Ok(0x07),
        DataType::Array { .. } => Ok(0x08),
        DataType::F16 => Ok(0x09),
        DataType::BF16 => Ok(0x0A),
        DataType::F32 => Ok(0x0B),
        DataType::F64 => Ok(0x0C),
        DataType::Tensor => Ok(0x0D),
        DataType::U8 => Ok(0x0E),
        DataType::U16 => Ok(0x0F),
        DataType::I8 => Ok(0x10),
        DataType::I16 => Ok(0x11),
        DataType::I64 => Ok(0x12),
        DataType::Handle(_) => Ok(0x13),
        DataType::Vec { .. } => Ok(0x14),
        DataType::TensorShaped { .. } => Ok(0x15),
        DataType::SparseCsr { .. } => Ok(0x16),
        DataType::SparseCoo { .. } => Ok(0x17),
        DataType::SparseBsr { .. } => Ok(0x18),
        DataType::F8E4M3 => Ok(0x19),
        DataType::F8E5M2 => Ok(0x1A),
        DataType::I4 => Ok(0x1B),
        DataType::FP4 => Ok(0x1C),
        DataType::NF4 => Ok(0x1D),
        DataType::DeviceMesh { .. } => Ok(0x1E),
        DataType::Quantized { .. } => Ok(0x1F),
        DataType::Opaque(_) => Ok(DATA_TYPE_TAG_OPAQUE),
        _ => Err(WireEncodeErr::static_msg("unknown DataType variant")),
    }
}

/// Write a [`DataType`] tag and any required payload into the output buffer.
///
/// # Preconditions
///
/// `value` must be a variant known to the VIR0 encoder. `out` is the byte
/// accumulator for the current wire-format message.
///
/// # Returns
///
/// `Ok(())` after appending the tag byte (and, for `Array`, the `element_size`
/// as a little-endian `u32`).
///
/// # Failure mode
///
/// * Returns the same error as [`data_type_tag`] if the variant is unknown.
/// * Returns `Err("Fix: array element_size ... cannot fit the VIR0 u32 payload")`
///   when `element_size` exceeds `u32::MAX`, which would truncate the payload.
#[inline]
pub(crate) fn put_data_type(out: &mut Vec<u8>, value: &DataType) -> Result<(), WireEncodeErr> {
    put_u8(out, data_type_tag(value)?);
    match value {
        DataType::Array { element_size } => {
            let encoded = u32::try_from(*element_size).map_err(|_| {
                WireEncodeErr::fmt_usize(
                    "Fix: array element_size ",
                    *element_size,
                    " cannot fit the VIR0 u32 payload; cap the element size or extend the wire format.",
                )
            })?;
            put_u32(out, encoded);
        }
        DataType::Opaque(id) => {
            // Opaque payload = u32 extension id (little-endian).
            put_u32(out, id.as_u32());
        }
        DataType::Handle(id) => {
            put_u32(out, id.as_u32());
        }
        DataType::Vec { element, count } => {
            put_data_type(out, element)?;
            put_u8(out, *count);
        }
        DataType::TensorShaped { element, shape } => {
            put_data_type(out, element)?;
            // Encoder/decoder limit symmetry: the decoder rejects ranks above
            // MAX_TENSOR_RANK, so refuse to emit an over-limit (undecodable) blob
            // here instead — fail loudly at encode with the actual rank and the
            // limit named (cf. MAX_OPAQUE_PAYLOAD_LEN's encoder/decoder pairing).
            if shape.len() > MAX_TENSOR_RANK {
                return Err(WireEncodeErr::fmt_usize2(
                    "Fix: tensor shape rank ",
                    shape.len(),
                    " exceeds the wire-format limit ",
                    MAX_TENSOR_RANK,
                    "; cap rank before serialization (the decoder rejects ranks above this).",
                ));
            }
            let len = u32::try_from(shape.len()).map_err(|_| {
                WireEncodeErr::fmt_usize(
                    "Fix: tensor shape rank ",
                    shape.len(),
                    " cannot fit the VIR0 u32 payload; cap rank before serialization.",
                )
            })?;
            put_u32(out, len);
            for dim in shape {
                put_u32(out, *dim);
            }
        }
        DataType::SparseCsr { element } | DataType::SparseCoo { element } => {
            put_data_type(out, element)?;
        }
        DataType::SparseBsr {
            element,
            block_rows,
            block_cols,
        } => {
            put_data_type(out, element)?;
            put_u32(out, *block_rows);
            put_u32(out, *block_cols);
        }
        DataType::DeviceMesh { axes } => {
            // Encoder/decoder limit symmetry (see TensorShaped above).
            if axes.len() > MAX_MESH_AXES {
                return Err(WireEncodeErr::fmt_usize2(
                    "Fix: device-mesh axes count ",
                    axes.len(),
                    " exceeds the wire-format limit ",
                    MAX_MESH_AXES,
                    "; cap mesh rank before serialization (the decoder rejects counts above this).",
                ));
            }
            let len = u32::try_from(axes.len()).map_err(|_| {
                WireEncodeErr::fmt_usize(
                    "Fix: device-mesh axes count ",
                    axes.len(),
                    " cannot fit the VIR0 u32 payload; cap mesh rank before serialization.",
                )
            })?;
            put_u32(out, len);
            for axis in axes {
                put_u32(out, *axis);
            }
        }
        DataType::Quantized {
            storage,
            scale,
            zero_point,
        } => {
            if !storage.is_quantized_storage() {
                return Err(WireEncodeErr::static_msg(
                    "Fix: DataType::Quantized storage must be I4/I8/I16/U8/U16/F8E4M3/F8E5M2/FP4/NF4.",
                ));
            }
            put_data_type(out, storage)?;
            put_quantization_scale(out, scale)?;
            put_quantization_zero_point(out, zero_point)?;
        }
        // Fixed-width scalar and vector types consume zero payload bytes
        // beyond the tag byte `data_type_tag` returned above.
        DataType::U8
        | DataType::U16
        | DataType::U32
        | DataType::I8
        | DataType::I16
        | DataType::I32
        | DataType::I64
        | DataType::U64
        | DataType::F32
        | DataType::F16
        | DataType::BF16
        | DataType::F64
        | DataType::Bool
        | DataType::Bytes
        | DataType::Tensor
        | DataType::Vec2U32
        | DataType::Vec4U32
        | DataType::F8E4M3
        | DataType::F8E5M2
        | DataType::I4
        | DataType::FP4
        | DataType::NF4 => {}
        // `DataType` is `#[non_exhaustive]` in vyre-spec; extension
        // variants added there must not break the existing encoder. Any
        // new variant must also add a payload-emission arm above before
        // being released, or encoding will fail fast here.
        _ => {
            return Err(WireEncodeErr::static_msg(
                "Fix: unknown DataType variant has no wire-format payload emitter. Add a match arm in put_data_type when the variant is introduced in vyre-spec.",
            ));
        }
    }
    Ok(())
}

fn put_quantization_scale(
    out: &mut Vec<u8>,
    scale: &vyre_spec::QuantizationScale,
) -> Result<(), WireEncodeErr> {
    match scale {
        vyre_spec::QuantizationScale::PerTensor => {
            put_u8(out, 0);
            put_u32(out, 0);
        }
        vyre_spec::QuantizationScale::PerChannel { axis } => {
            put_u8(out, 1);
            put_u32(out, *axis);
        }
        vyre_spec::QuantizationScale::PerGroup { group_size } => {
            if *group_size == 0 {
                return Err(WireEncodeErr::static_msg(
                    "Fix: quantized PerGroup scale requires group_size > 0.",
                ));
            }
            put_u8(out, 2);
            put_u32(out, *group_size);
        }
    }
    Ok(())
}

fn put_quantization_zero_point(
    out: &mut Vec<u8>,
    zero_point: &vyre_spec::QuantizationZeroPoint,
) -> Result<(), WireEncodeErr> {
    match zero_point {
        vyre_spec::QuantizationZeroPoint::Absent => {
            put_u8(out, 0);
            put_u32(out, 0);
        }
        vyre_spec::QuantizationZeroPoint::PerTensor => {
            put_u8(out, 1);
            put_u32(out, 0);
        }
        vyre_spec::QuantizationZeroPoint::PerChannel { axis } => {
            put_u8(out, 2);
            put_u32(out, *axis);
        }
        vyre_spec::QuantizationZeroPoint::PerGroup { group_size } => {
            if *group_size == 0 {
                return Err(WireEncodeErr::static_msg(
                    "Fix: quantized PerGroup zero-point requires group_size > 0.",
                ));
            }
            put_u8(out, 3);
            put_u32(out, *group_size);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::put_data_type;
    use crate::ir::DataType;
    use crate::serial::wire::Reader;
    use smallvec::smallvec;

    #[test]
    fn bool_data_type_wire_payload_is_single_u8_tag() {
        let mut encoded = Vec::new();
        put_data_type(&mut encoded, &DataType::Bool)
            .expect("Fix: DataType::Bool must encode as one u8 tag");
        assert_eq!(encoded, vec![0x06]);
    }

    /// Every DataType variant the encoder accepts must also round-trip
    /// through the decoder with bit-exact equality. A drift here means
    /// the on-disk wire format silently corrupts buffer-element types
    /// across encode/decode  -  a contract-invariant the optimizer cache
    /// and AOT artifact format both rely on.
    #[test]
    fn every_supported_data_type_round_trips_through_the_wire() {
        let cases: Vec<DataType> = vec![
            DataType::U8,
            DataType::U16,
            DataType::U32,
            DataType::U64,
            DataType::I8,
            DataType::I16,
            DataType::I32,
            DataType::I64,
            DataType::F16,
            DataType::BF16,
            DataType::F32,
            DataType::F64,
            DataType::Bool,
            DataType::Bytes,
            DataType::Tensor,
            DataType::Vec2U32,
            DataType::Vec4U32,
            DataType::F8E4M3,
            DataType::F8E5M2,
            DataType::I4,
            DataType::FP4,
            DataType::NF4,
            DataType::Array { element_size: 16 },
            DataType::Handle(vyre_spec::data_type::TypeId(0xDEAD_BEEF)),
            DataType::Vec {
                element: Box::new(DataType::F32),
                count: 4,
            },
            DataType::TensorShaped {
                element: Box::new(DataType::F32),
                shape: smallvec![32, 32],
            },
            DataType::SparseCsr {
                element: Box::new(DataType::F32),
            },
            DataType::SparseCoo {
                element: Box::new(DataType::F32),
            },
            DataType::SparseBsr {
                element: Box::new(DataType::F32),
                block_rows: 8,
                block_cols: 8,
            },
            DataType::DeviceMesh {
                axes: smallvec![4, 8, 16],
            },
            DataType::Quantized {
                storage: Box::new(DataType::I4),
                scale: vyre_spec::QuantizationScale::PerGroup { group_size: 128 },
                zero_point: vyre_spec::QuantizationZeroPoint::Absent,
            },
            DataType::Quantized {
                storage: Box::new(DataType::I8),
                scale: vyre_spec::QuantizationScale::PerChannel { axis: 1 },
                zero_point: vyre_spec::QuantizationZeroPoint::PerChannel { axis: 1 },
            },
            // Extension ids must have the high bit set per
            // reject_reserved_extension_id (low half is reserved for core IR).
            DataType::Opaque(vyre_spec::extension::ExtensionDataTypeId(0x8000_0001)),
        ];

        for ty in &cases {
            let mut encoded = Vec::new();
            put_data_type(&mut encoded, ty)
                .unwrap_or_else(|e| panic!("encode {ty:?} failed: {e:?}"));
            let mut reader = Reader {
                bytes: &encoded,
                pos: 0,
                depth: 0,
            };
            let decoded = reader
                .data_type()
                .unwrap_or_else(|e| panic!("decode {ty:?} failed: {e}"));
            assert_eq!(
                &decoded, ty,
                "round-trip diverged for {ty:?}: re-decoded as {decoded:?}"
            );
            assert_eq!(
                reader.pos,
                encoded.len(),
                "encoder produced trailing bytes for {ty:?}"
            );
        }
    }

    /// A tensor whose rank is exactly `MAX_TENSOR_RANK` must still round-trip:
    /// the I10 bound is inclusive, so it must not reject the largest valid rank.
    #[test]
    fn tensor_rank_at_limit_round_trips() {
        use crate::serial::wire::MAX_TENSOR_RANK;
        let shape: smallvec::SmallVec<[u32; 4]> = (0..MAX_TENSOR_RANK as u32).collect();
        let ty = DataType::TensorShaped {
            element: Box::new(DataType::F32),
            shape,
        };
        let mut encoded = Vec::new();
        put_data_type(&mut encoded, &ty).expect("encode max-rank tensor");
        let mut reader = Reader {
            bytes: &encoded,
            pos: 0,
            depth: 0,
        };
        let decoded = reader.data_type().expect("max-rank tensor must decode");
        assert_eq!(decoded, ty, "rank == MAX_TENSOR_RANK must round-trip");
    }

    /// A tensor declaring a rank above `MAX_TENSOR_RANK` must be rejected at the
    /// I10 `bounded_len` gate (O(1), naming the field) — never by attempting to
    /// read the dimensions. Crafted by encoding a rank-0 tensor (whose final 4
    /// bytes are the rank u32) and overwriting the rank with the limit + 1.
    #[test]
    fn tensor_rank_exceeding_limit_is_rejected() {
        use crate::serial::wire::MAX_TENSOR_RANK;
        let mut encoded = Vec::new();
        put_data_type(
            &mut encoded,
            &DataType::TensorShaped {
                element: Box::new(DataType::F32),
                shape: smallvec![],
            },
        )
        .expect("encode rank-0 tensor");
        let cut = encoded.len() - 4;
        encoded.truncate(cut);
        encoded.extend_from_slice(&(MAX_TENSOR_RANK as u32 + 1).to_le_bytes());

        let mut reader = Reader {
            bytes: &encoded,
            pos: 0,
            depth: 0,
        };
        let err = reader
            .data_type()
            .expect_err("over-limit tensor rank must be rejected, not read dim-by-dim");
        assert!(
            err.contains("tensor rank") && err.contains("exceeds"),
            "rejection must name the tensor rank limit and 'exceeds': {err}"
        );
    }

    /// The device-mesh twin: an axis count above `MAX_MESH_AXES` is rejected at
    /// the same I10 gate.
    #[test]
    fn device_mesh_axes_exceeding_limit_is_rejected() {
        use crate::serial::wire::MAX_MESH_AXES;
        let mut encoded = Vec::new();
        put_data_type(&mut encoded, &DataType::DeviceMesh { axes: smallvec![] })
            .expect("encode 0-axis device mesh");
        let cut = encoded.len() - 4;
        encoded.truncate(cut);
        encoded.extend_from_slice(&(MAX_MESH_AXES as u32 + 1).to_le_bytes());

        let mut reader = Reader {
            bytes: &encoded,
            pos: 0,
            depth: 0,
        };
        let err = reader
            .data_type()
            .expect_err("over-limit device-mesh axis count must be rejected");
        assert!(
            err.contains("device-mesh axes count") && err.contains("exceeds"),
            "rejection must name the mesh axes limit and 'exceeds': {err}"
        );
    }

    /// Encoder/decoder symmetry: the encoder must refuse to emit a tensor whose
    /// rank exceeds `MAX_TENSOR_RANK` (which the decoder would reject), failing
    /// loudly instead of producing an undecodable blob.
    #[test]
    fn encoding_tensor_rank_above_limit_is_rejected() {
        use crate::serial::wire::MAX_TENSOR_RANK;
        // 0..=MAX_TENSOR_RANK is MAX_TENSOR_RANK + 1 elements.
        let shape: smallvec::SmallVec<[u32; 4]> = (0..=MAX_TENSOR_RANK as u32).collect();
        let ty = DataType::TensorShaped {
            element: Box::new(DataType::F32),
            shape,
        };
        let mut out = Vec::new();
        let err = put_data_type(&mut out, &ty)
            .expect_err("encoding an over-limit tensor rank must fail, not emit a blob");
        let msg = format!("{err:?}");
        assert!(
            msg.contains("tensor shape rank") && msg.contains("exceeds"),
            "encoder rejection must name the rank and 'exceeds': {msg}"
        );
    }

    /// The device-mesh twin of the encoder symmetry check.
    #[test]
    fn encoding_device_mesh_axes_above_limit_is_rejected() {
        use crate::serial::wire::MAX_MESH_AXES;
        let axes: smallvec::SmallVec<[u32; 3]> = (0..=MAX_MESH_AXES as u32).collect();
        let ty = DataType::DeviceMesh { axes };
        let mut out = Vec::new();
        let err = put_data_type(&mut out, &ty)
            .expect_err("encoding an over-limit device-mesh axis count must fail");
        let msg = format!("{err:?}");
        assert!(
            msg.contains("device-mesh axes count") && msg.contains("exceeds"),
            "encoder rejection must name the axes count and 'exceeds': {msg}"
        );
    }
}
