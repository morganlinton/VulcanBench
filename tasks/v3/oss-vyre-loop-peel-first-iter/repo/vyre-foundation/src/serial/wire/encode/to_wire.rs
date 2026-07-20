//! Program encoder for the stable `VYRE` wire format.

use super::put_node;
use crate::ir_inner::model::program::{BufferDecl, CacheLocality, MemoryKind};
use crate::ir_inner::model::types::{BufferAccess, DataType};
use crate::perf::PerfScope;
use crate::serial::wire::encode::WireEncodeErr;
use crate::serial::wire::framing::{
    put_string, put_u32, put_u8, FLAG_OPAQUE_ENDIAN_FIXED, MAGIC, WIRE_FORMAT_VERSION,
};
use crate::serial::wire::tags::access_tag::access_tag;
use crate::serial::wire::Program;

const METADATA_OP_ID: &str = "vyre.program.metadata";

struct RegionPayloadScratch {
    shape: Vec<u8>,
    hints: Vec<u8>,
}

/// Serialize a complete [`Program`] into the `VYRE` wire envelope.
///
/// # Role
///
/// This is the entry-point encoder. It produces the exact byte
/// sequence that [`Program::from_wire`] expects: magic, version,
/// entry-op id, buffer table, work-group size, and entry body.
///
/// # Invariants
///
/// * The output is a fresh `Vec<u8>`; the caller owns it.
/// * Capacity is pre-allocated heuristically to avoid reallocations
///   on typical programs, but the vector grows naturally if the
///   estimate is low.
///
/// # Pre-conditions
///
/// The program must use only enum variants that have stable wire
/// tags. A well-formed program should always encode successfully;
/// encoding failure signals either an unsupported variant
/// (audit L.1.27 / I4) or a field that exceeds wire-format bounds
/// (audit I10).
///
/// # Return semantics
///
/// * `Ok(Vec<u8>)` – a complete VIR0 blob starting with [`MAGIC`]
///   and [`WIRE_FORMAT_VERSION`].
/// * `Err(String)` – an actionable diagnostic starting with `Fix:`.
///
/// # Failure modes
///
/// * **Buffer count overflow** – more than `u32::MAX` buffers.
/// * **String overflow** – buffer names or the entry op id longer
///   than [`crate::serial::wire::MAX_STRING_LEN`] are rejected.
/// * **Unmapped variant** – `access_tag`, `put_data_type`, or nested
///   `put_expr` / `put_node` calls fail when an enum variant has no
///   wire tag.
///
/// # Versioning
///
/// The version bytes are emitted immediately after the magic
/// (audit L.1.47). Any breaking schema change must bump
/// [`WIRE_FORMAT_VERSION`] so older decoders reject the payload
/// with a clear version-mismatch message instead of arbitrary
/// downstream parse errors.
///
/// # Errors
///
/// Returns an actionable wire-format diagnostic when the program contains an
/// unmapped variant, oversized section, or non-round-trippable shape.
#[inline]
#[must_use]
pub fn to_wire(program: &Program) -> Result<Vec<u8>, String> {
    let mut out = Vec::with_capacity(estimated_wire_capacity(program, program.buffers()));
    to_wire_with_buffer_order_into(program, program.buffers(), &mut out).map_err(String::from)?;
    Ok(out)
}

/// Serialize a complete [`Program`] into the `VYRE` wire envelope,
/// appending to an existing buffer.
///
/// # Role
///
/// Same semantics as [`to_wire`], but appends to `dst` instead of
/// returning a fresh `Vec<u8>`. The caller may `dst.clear()` and
/// reuse the same buffer across many calls to avoid O(N) heap
/// allocations when encoding batched programs.
///
/// # Invariants
///
/// * Bytes are appended to `dst`; existing content is preserved.
/// * Capacity is reserved heuristically to avoid reallocations.
///
/// # Pre-conditions
///
/// Same as [`to_wire`].
///
/// # Return semantics
///
/// * `Ok(())` – the complete VIR0 blob was appended to `dst`.
/// * `Err(String)` – an actionable diagnostic starting with `Fix:`.
///
/// # Errors
///
/// Returns an actionable wire-format diagnostic when the program contains an
/// unmapped variant, oversized section, or non-round-trippable shape.
#[inline]
pub fn to_wire_into(program: &Program, dst: &mut Vec<u8>) -> Result<(), String> {
    to_wire_with_buffer_order_into(program, program.buffers(), dst).map_err(String::from)
}

/// Serialize `program` while reading its buffer table from `buffers`.
///
/// This is for canonical cache keys that need declaration-order normalization
/// without constructing a second [`Program`] that duplicates the entry body.
///
/// # Errors
///
/// Returns [`WireEncodeErr`] when `program` or the supplied buffer order cannot
/// be represented by the stable VIR0 wire format.
#[inline]
pub fn to_wire_with_buffer_order_into(
    program: &Program,
    buffers: &[BufferDecl],
    dst: &mut Vec<u8>,
) -> Result<(), WireEncodeErr> {
    let perf_scope = PerfScope::start("vyre-foundation", "foundation.wire.encode");
    reject_non_roundtrippable_shapes(program, buffers)?;
    let mut body = Vec::with_capacity(estimated_body_capacity(program, buffers));
    put_nodes_section(&mut body, program, buffers)?;
    put_memory_regions(&mut body, buffers)?;
    crate::serial::output_set::OutputSet::encode_from_buffers_into(buffers, &mut body)
        .map_err(WireEncodeErr::from)?;

    let digest = blake3::hash(&body);
    dst.reserve(MAGIC.len() + 2 + 2 + 32 + body.len());
    dst.extend_from_slice(MAGIC);
    dst.extend_from_slice(&WIRE_FORMAT_VERSION.to_le_bytes());
    dst.extend_from_slice(&FLAG_OPAQUE_ENDIAN_FIXED.to_le_bytes());
    dst.extend_from_slice(digest.as_bytes());
    dst.extend_from_slice(&body);
    let _ = perf_scope.finish();
    Ok(())
}

#[inline]
fn estimated_wire_capacity(program: &Program, buffers: &[BufferDecl]) -> usize {
    MAGIC
        .len()
        .saturating_add(2)
        .saturating_add(2)
        .saturating_add(32)
        .saturating_add(estimated_body_capacity(program, buffers))
}

#[inline]
fn estimated_body_capacity(program: &Program, buffers: &[BufferDecl]) -> usize {
    let buffer_name_bytes = buffers
        .iter()
        .map(|buffer| buffer.name().len())
        .sum::<usize>();
    program
        .entry()
        .len()
        .saturating_mul(48)
        .saturating_add(buffers.len().saturating_mul(40))
        .saturating_add(buffer_name_bytes)
        .saturating_add(buffers.len().saturating_mul(2))
        .saturating_add(256)
}

fn reject_non_roundtrippable_shapes(
    program: &Program,
    buffers: &[BufferDecl],
) -> Result<(), WireEncodeErr> {
    for (axis, size) in program.workgroup_size().into_iter().enumerate() {
        if size == 0 {
            return Err(WireEncodeErr::fmt_usize(
                "Fix: workgroup_size[",
                axis,
                "] is 0. Encode only programs whose workgroup dimensions are >= 1.",
            ));
        }
    }

    for buffer in buffers {
        if buffer.count() == 0 && buffer.access() == BufferAccess::Workgroup {
            let mut buf = arrayvec::ArrayString::<256>::new();
            buf.push_str("Fix: workgroup buffer `");
            buf.push_str(buffer.name());
            buf.push_str("` has count 0. Encode only positive-length shared-memory buffers.");
            return Err(WireEncodeErr::Dynamic(Box::new(buf)));
        }
        // Output buffers may legitimately carry count 0 to signal a
        // runtime-determined size (the dispatch layer rebinds them
        // with a concrete byte length once it knows the host's
        // capacity). The wire format records `count = 0` and the
        // `output_byte_range` check below validates start/end
        // ordering without needing a fixed full-size. The earlier
        // strict rejection failed every Program fingerprinted with
        // a zero-length scratch output (run_arbitrary, conformance,
        // dispatch_determinism  -  see tests).
        if buffer.count() == 0 && buffer.is_pipeline_live_out() {
            let mut buf = arrayvec::ArrayString::<256>::new();
            buf.push_str("Fix: live-out buffer `");
            buf.push_str(buffer.name());
            buf.push_str("` has count 0. Encode only positive-length externally-visible buffers.");
            return Err(WireEncodeErr::Dynamic(Box::new(buf)));
        }
        if let Some(range) = buffer.output_byte_range() {
            let count = u64::from(buffer.count());
            let full_size = if count == 0 {
                // runtime-sized: we can't validate against full_size here,
                // but we can still check start <= end.
                u64::MAX
            } else {
                let elem_size = buffer.element().size_bytes().ok_or_else(|| {
                    let mut buf = arrayvec::ArrayString::<256>::new();
                    buf.push_str("Fix: static output buffer `");
                    buf.push_str(buffer.name());
                    buf.push_str("` uses a runtime-sized element type. Lower it to a fixed-width GPU storage type before wire encoding.");
                    WireEncodeErr::Dynamic(Box::new(buf))
                })? as u64;
                count.checked_mul(elem_size).ok_or_else(|| {
                    let mut buf = arrayvec::ArrayString::<256>::new();
                    buf.push_str("Fix: output buffer `");
                    buf.push_str(buffer.name());
                    buf.push_str("` byte size overflows u64 during wire encoding. Split the buffer before serialization.");
                    WireEncodeErr::Dynamic(Box::new(buf))
                })?
            };
            let start = range.start as u64;
            let end = range.end as u64;
            if start > end {
                let mut buf = arrayvec::ArrayString::<256>::new();
                let mut tmp = itoa::Buffer::new();
                buf.push_str("Fix: buffer `");
                buf.push_str(buffer.name());
                buf.push_str("` output byte range has start (");
                buf.push_str(tmp.format(range.start));
                buf.push_str(") > end (");
                buf.push_str(tmp.format(range.end));
                buf.push_str("). Encode only valid ranges.");
                return Err(WireEncodeErr::Dynamic(Box::new(buf)));
            }
            if end > full_size && full_size != u64::MAX {
                let mut buf = arrayvec::ArrayString::<256>::new();
                let mut tmp = itoa::Buffer::new();
                buf.push_str("Fix: buffer `");
                buf.push_str(buffer.name());
                buf.push_str("` output byte range end (");
                buf.push_str(tmp.format(range.end));
                buf.push_str(") exceeds full buffer size (");
                buf.push_str(tmp.format(full_size));
                buf.push_str("). Encode only ranges that fit within the declared buffer size.");
                return Err(WireEncodeErr::Dynamic(Box::new(buf)));
            }
        }
    }

    Ok(())
}

fn put_nodes_section(
    out: &mut Vec<u8>,
    program: &Program,
    buffers: &[BufferDecl],
) -> Result<(), WireEncodeErr> {
    let mut payload = Vec::with_capacity(256);
    put_nodes_section_with_payload(out, program, buffers, &mut payload)
}

fn put_nodes_section_with_payload(
    out: &mut Vec<u8>,
    program: &Program,
    buffers: &[BufferDecl],
    payload: &mut Vec<u8>,
) -> Result<(), WireEncodeErr> {
    put_leb_u64(
        out,
        u64::try_from(program.entry().len() + 1).map_err(|_| {
            WireEncodeErr::static_msg(
                "Fix: node count cannot fit u64; split the Program before serialization.",
            )
        })?,
    );
    payload.clear();
    put_metadata_payload(payload, program, buffers)?;
    put_node_record(out, METADATA_OP_ID, payload, &[])?;
    for node in program.entry() {
        payload.clear();
        put_node(payload, node)?;
        put_node_record(
            out,
            crate::ir_inner::model::node::node_op_id(node),
            payload,
            &[],
        )?;
    }
    Ok(())
}

fn put_node_record(
    out: &mut Vec<u8>,
    op_id: &str,
    payload: &[u8],
    operands: &[u32],
) -> Result<(), WireEncodeErr> {
    put_leb_str(out, op_id)?;
    put_leb_u64(
        out,
        u64::try_from(payload.len()).map_err(|_| {
            WireEncodeErr::static_msg("Fix: node payload length cannot fit u64; split the Program.")
        })?,
    );
    out.extend_from_slice(payload);
    put_leb_u64(
        out,
        u64::try_from(operands.len()).map_err(|_| {
            WireEncodeErr::static_msg("Fix: node operand count cannot fit u64; split the Program.")
        })?,
    );
    for operand in operands {
        put_leb_u32(out, *operand);
    }
    Ok(())
}

fn put_metadata_payload(
    out: &mut Vec<u8>,
    program: &Program,
    buffers: &[BufferDecl],
) -> Result<(), WireEncodeErr> {
    out.extend_from_slice(b"VYRE-META");
    match program.entry_op_id() {
        Some(op_id) => {
            put_u8(out, 1);
            put_string(out, op_id)?;
        }
        None => put_u8(out, 0),
    }
    for size in program.workgroup_size() {
        put_u32(out, size);
    }
    put_u8(out, u8::from(program.is_non_composable_with_self()));
    put_leb_u64(
        out,
        u64::try_from(buffers.len()).map_err(|_| {
            WireEncodeErr::static_msg(
                "Fix: buffer metadata count cannot fit u64; split the Program.",
            )
        })?,
    );
    for buffer in buffers {
        put_string(out, buffer.name())?;
        put_u32(out, buffer.binding());
        put_u32(out, buffer.count());
        put_u8(out, u8::from(buffer.is_output()));
        put_u8(out, u8::from(buffer.is_pipeline_live_out()));
        match buffer.output_byte_range() {
            Some(range) => {
                put_u8(out, 1);
                put_leb_u64(
                    out,
                    u64::try_from(range.start).map_err(|_| {
                        WireEncodeErr::static_msg(
                            "Fix: output range start cannot fit u64; split the output buffer.",
                        )
                    })?,
                );
                put_leb_u64(
                    out,
                    u64::try_from(range.end).map_err(|_| {
                        WireEncodeErr::static_msg(
                            "Fix: output range end cannot fit u64; split the output buffer.",
                        )
                    })?,
                );
            }
            None => put_u8(out, 0),
        }
        put_hints_payload(out, buffer.hints());
    }
    Ok(())
}

fn put_memory_regions(out: &mut Vec<u8>, buffers: &[BufferDecl]) -> Result<(), WireEncodeErr> {
    let mut scratch = RegionPayloadScratch {
        shape: Vec::with_capacity(16),
        hints: Vec::with_capacity(16),
    };
    put_memory_regions_with_scratch(out, buffers, &mut scratch.shape, &mut scratch.hints)
}

fn put_memory_regions_with_scratch(
    out: &mut Vec<u8>,
    buffers: &[BufferDecl],
    shape: &mut Vec<u8>,
    hints: &mut Vec<u8>,
) -> Result<(), WireEncodeErr> {
    put_leb_u64(
        out,
        u64::try_from(buffers.len()).map_err(|_| {
            WireEncodeErr::static_msg("Fix: memory-region count cannot fit u64; split the Program.")
        })?,
    );
    for (index, buffer) in buffers.iter().enumerate() {
        put_leb_u32(
            out,
            u32::try_from(index).map_err(|_| {
                WireEncodeErr::fmt_usize(
                    "Fix: memory-region id ",
                    index,
                    " cannot fit u32; split the Program.",
                )
            })?,
        );
        put_u8(out, memory_kind_tag(buffer.kind()));
        put_u8(
            out,
            access_tag(&buffer.access()).map_err(WireEncodeErr::from)?,
        );
        put_u8(out, data_type_tag(&buffer.element())?);
        put_u8(out, 0);
        shape.clear();
        put_leb_u64(shape, u64::from(buffer.count()));
        if let DataType::Array { element_size } = buffer.element() {
            put_leb_u64(
                shape,
                u64::try_from(element_size).map_err(|_| {
                    WireEncodeErr::static_msg(
                        "Fix: array element size cannot fit u64; cap the element size.",
                    )
                })?,
            );
        }
        if let DataType::Handle(id) = buffer.element() {
            put_leb_u64(shape, u64::from(id.as_u32()));
        }
        if let DataType::Opaque(id) = buffer.element() {
            // Opaque payload = u32 extension id (LEB-encoded as u64 to match
            // the surrounding wire convention; decoder caps at u32::MAX).
            put_leb_u64(shape, u64::from(id.as_u32()));
        }
        if let DataType::Quantized {
            storage,
            scale,
            zero_point,
        } = buffer.element()
        {
            if !storage.is_quantized_storage() {
                return Err(WireEncodeErr::static_msg(
                    "Fix: quantized memory-region storage must be I4/I8/I16/U8/U16/F8E4M3/F8E5M2/FP4/NF4.",
                ));
            }
            // `buffer.element()` returns DataType by value, so the
            // `Quantized { storage, scale, zero_point }` pattern binds
            // by value: storage is Box<DataType>, scale and zero_point
            // are owned. The helpers want references - &*storage drops
            // the Box to get &DataType.
            put_leb_u64(shape, u64::from(data_type_tag(&storage)?));
            put_dense_quantization_scale(shape, &scale)?;
            put_dense_quantization_zero_point(shape, &zero_point)?;
        }
        put_leb_u64(
            out,
            u64::try_from(shape.len()).map_err(|_| {
                WireEncodeErr::static_msg(
                    "Fix: shape payload length cannot fit u64; split the Program.",
                )
            })?,
        );
        out.extend_from_slice(shape);
        hints.clear();
        put_hints_payload(hints, buffer.hints());
        put_leb_u64(
            out,
            u64::try_from(hints.len()).map_err(|_| {
                WireEncodeErr::static_msg(
                    "Fix: hints payload length cannot fit u64; split the Program.",
                )
            })?,
        );
        out.extend_from_slice(hints);
    }
    Ok(())
}

fn put_hints_payload(out: &mut Vec<u8>, hints: crate::ir::MemoryHints) {
    match hints.coalesce_axis {
        Some(axis) => {
            put_u8(out, 1);
            put_u8(out, axis);
        }
        None => put_u8(out, 0),
    }
    put_u32(out, hints.preferred_alignment);
    put_u8(
        out,
        match hints.cache_locality {
            CacheLocality::Streaming => 0,
            CacheLocality::Temporal => 1,
            CacheLocality::Random => 2,
        },
    );
}

fn memory_kind_tag(kind: MemoryKind) -> u8 {
    match kind {
        MemoryKind::Global => 0,
        MemoryKind::Shared => 1,
        MemoryKind::Uniform => 2,
        MemoryKind::Local => 3,
        MemoryKind::Readonly => 4,
        MemoryKind::Push => 5,
        MemoryKind::Persistent => 6,
    }
}

fn data_type_tag(value: &DataType) -> Result<u8, WireEncodeErr> {
    Ok(match value {
        DataType::U32 => 0x01,
        DataType::I32 => 0x02,
        DataType::U64 => 0x03,
        DataType::Vec2U32 => 0x04,
        DataType::Vec4U32 => 0x05,
        DataType::Bool => 0x06,
        DataType::Bytes => 0x07,
        DataType::Array { .. } => 0x08,
        DataType::F16 => 0x09,
        DataType::BF16 => 0x0A,
        DataType::F32 => 0x0B,
        DataType::F64 => 0x0C,
        DataType::Tensor => 0x0D,
        DataType::U8 => 0x0E,
        DataType::U16 => 0x0F,
        DataType::I8 => 0x10,
        DataType::I16 => 0x11,
        DataType::I64 => 0x12,
        DataType::Handle(_) => 0x13,
        DataType::Vec { .. } => 0x14,
        DataType::TensorShaped { .. } => 0x15,
        // Quantised scalar families are valid buffer elements (no
        // additional payload  -  same shape as the U8/I32/F32 family).
        DataType::F8E4M3 => 0x19,
        DataType::F8E5M2 => 0x1A,
        DataType::I4 => 0x1B,
        DataType::FP4 => 0x1C,
        DataType::NF4 => 0x1D,
        DataType::Quantized { .. } => 0x1F,
        DataType::Opaque(_) => 0x80,
        _ => {
            return Err(WireEncodeErr::static_msg(
                "Fix: unknown DataType variant cannot be serialized into VYRE wire format. \
                 Sparse/Vec/TensorShaped/DeviceMesh types are not valid buffer elements in the \
                 dense memory-region encoder; lower to a supported scalar/array/handle/opaque \
                 first.",
            ));
        }
    })
}

fn put_dense_quantization_scale(
    out: &mut Vec<u8>,
    scale: &vyre_spec::QuantizationScale,
) -> Result<(), WireEncodeErr> {
    match scale {
        vyre_spec::QuantizationScale::PerTensor => {
            put_leb_u64(out, 0);
            put_leb_u64(out, 0);
        }
        vyre_spec::QuantizationScale::PerChannel { axis } => {
            put_leb_u64(out, 1);
            put_leb_u64(out, u64::from(*axis));
        }
        vyre_spec::QuantizationScale::PerGroup { group_size } => {
            if *group_size == 0 {
                return Err(WireEncodeErr::static_msg(
                    "Fix: quantized PerGroup scale requires group_size > 0.",
                ));
            }
            put_leb_u64(out, 2);
            put_leb_u64(out, u64::from(*group_size));
        }
    }
    Ok(())
}

fn put_dense_quantization_zero_point(
    out: &mut Vec<u8>,
    zero_point: &vyre_spec::QuantizationZeroPoint,
) -> Result<(), WireEncodeErr> {
    match zero_point {
        vyre_spec::QuantizationZeroPoint::Absent => {
            put_leb_u64(out, 0);
            put_leb_u64(out, 0);
        }
        vyre_spec::QuantizationZeroPoint::PerTensor => {
            put_leb_u64(out, 1);
            put_leb_u64(out, 0);
        }
        vyre_spec::QuantizationZeroPoint::PerChannel { axis } => {
            put_leb_u64(out, 2);
            put_leb_u64(out, u64::from(*axis));
        }
        vyre_spec::QuantizationZeroPoint::PerGroup { group_size } => {
            if *group_size == 0 {
                return Err(WireEncodeErr::static_msg(
                    "Fix: quantized PerGroup zero-point requires group_size > 0.",
                ));
            }
            put_leb_u64(out, 3);
            put_leb_u64(out, u64::from(*group_size));
        }
    }
    Ok(())
}

fn put_leb_str(out: &mut Vec<u8>, value: &str) -> Result<(), WireEncodeErr> {
    put_leb_u64(
        out,
        u64::try_from(value.len()).map_err(|_| {
            WireEncodeErr::static_msg("Fix: string length cannot fit u64; shorten the identifier.")
        })?,
    );
    out.extend_from_slice(value.as_bytes());
    Ok(())
}

fn put_leb_u32(out: &mut Vec<u8>, value: u32) {
    put_leb_u64(out, u64::from(value));
}

fn put_leb_u64(out: &mut Vec<u8>, mut value: u64) {
    loop {
        let mut byte = (value & 0x7F) as u8;
        value >>= 7;
        if value != 0 {
            byte |= 0x80;
        }
        out.push(byte);
        if value == 0 {
            break;
        }
    }
}

#[cfg(test)]
mod tests;
