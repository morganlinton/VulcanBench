mod payload;
use crate::ir_inner::model::program::{MemoryHints, MemoryKind};
use crate::ir_inner::model::types::{BufferAccess, DataType};
use crate::serial::wire::decode::reject_reserved_extension_id;
use crate::serial::wire::framing::{
    FLAG_COMPRESSED, FLAG_OPAQUE_ENDIAN_FIXED, FLAG_SEALED, MAGIC, WIRE_FORMAT_VERSION,
};
use crate::serial::wire::tags::access_from_tag::access_from_tag;
use crate::serial::wire::{
    BufferDecl, Program, Reader, MAX_BUFFERS, MAX_NODES, MAX_OPAQUE_PAYLOAD_LEN, MAX_PROGRAM_BYTES,
};
use payload::{
    data_type_from_tag, memory_kind_from_tag, read_dense_quantization_scale,
    read_dense_quantization_zero_point, read_hints,
};
use std::ops::Range;
use std::sync::Arc;

const HEADER_LEN: usize = 4 + 2 + 2 + 32;
const METADATA_OP_ID: &str = "vyre.program.metadata";

/// Deserialize a complete `VIR0` wire-format program from raw bytes.
///
/// # Decode-time invariants
///
/// The input must be a well-formed VIR0 blob:
/// 1. **Magic & version** – consumed and validated by `Reader::expect_magic`.
///    Wrong magic, truncated header, or mismatched `WIRE_FORMAT_VERSION` are
///    rejected immediately.
/// 2. **Entry-point tag** – a leading `u8`: `0` means no entry op, `1` means a
///    `Reader::string` follows; any other tag is rejected as unknown.
/// 3. **Buffer table** – `Reader::bounded_len` against `MAX_BUFFERS` gives the
///    count, followed by that many `BufferDecl` records (name, binding, access
///    tag, element type, count, output flag).
/// 4. **Work-group size** – three little-endian `u32`s.
/// 5. **Entry body** – decoded via `Reader::nodes`, bounded by `MAX_NODES`
///    and recursively guarded by `MAX_DECODE_DEPTH` (L.1.35).
/// 6. **No trailing bytes** – the cursor must land exactly at `bytes.len()`;
///    anything extra is rejected to prevent concatenation attacks.
///
/// # Bounds checks & rejection criteria (I10)
///
/// Every length-prefix is checked against a compile-time limit before
/// allocation. A blob that advertises more buffers than `MAX_BUFFERS`,
/// more nodes than `MAX_NODES`, or deeper nesting than
/// `MAX_DECODE_DEPTH` is rejected **before** the corresponding `Vec` is
/// allocated.
///
/// # Return semantics
///
/// * `Ok(Program)` – the blob passed all structural and semantic checks.
/// * `Err(String)` – an actionable diagnostic whose message always starts
///   with `Fix:` describing what the caller must change (re-serialize,
///   flatten nesting, reject untrusted input, etc.).
///
/// # Pre-condition
///
/// `bytes` must be the **complete** output of `Program::to_wire`; partial
/// slices or concatenated blobs are rejected.
///
/// # Errors
///
/// Returns an actionable diagnostic when the blob exceeds the framing cap,
/// has a malformed envelope, fails the integrity digest, or contains a
/// structurally invalid program payload.
/// Decodes a valid Vyre VIR0 Program from a byte slice.
#[inline]
#[must_use]
pub fn from_wire(bytes: &[u8]) -> Result<Program, String> {
    if bytes.len() > MAX_PROGRAM_BYTES {
        return Err(format!(
            "Fix: wire blob is {} bytes, exceeding the {}-byte IR framing cap. Reject this untrusted input or split the program before serialization.",
            bytes.len(),
            MAX_PROGRAM_BYTES,
        ));
    }
    let mut header_reader = Reader {
        bytes,
        pos: 0,
        depth: 0,
    };
    header_reader.expect_magic()?;
    if bytes.len() < MAGIC.len() || &bytes[..MAGIC.len()] != MAGIC {
        let found = bytes.get(..bytes.len().min(MAGIC.len())).unwrap_or(bytes);
        return Err(format!(
            "MagicMismatch: found {found:?}. Fix: serialize Program with Program::to_wire() using the VYRE wire format."
        ));
    }
    if bytes.len() < HEADER_LEN {
        return Err(format!(
            "TruncatedPayload: header requires {HEADER_LEN} bytes, got {}. Fix: provide the complete Program bytes.",
            bytes.len()
        ));
    }
    let version = u16::from_le_bytes([bytes[4], bytes[5]]);
    if version != WIRE_FORMAT_VERSION {
        return Err(format!(
            "UnknownSchemaVersion: found {version}, supported {WIRE_FORMAT_VERSION}. Fix: upgrade the consumer or re-serialize with this Vyre version."
        ));
    }
    let flags = u16::from_le_bytes([bytes[6], bytes[7]]);
    let reserved = flags & !(FLAG_COMPRESSED | FLAG_SEALED | FLAG_OPAQUE_ENDIAN_FIXED);
    if reserved != 0 || (flags & (FLAG_COMPRESSED | FLAG_SEALED)) != 0 {
        return Err(format!(
            "InvalidDiscriminant: field flags has value {flags}. Fix: decode only uncompressed, unsigned schema-v1 Program bytes or enable the matching feature."
        ));
    }
    if (flags & FLAG_OPAQUE_ENDIAN_FIXED) == 0 {
        return Err(
            "InvalidDiscriminant: wire header is missing OPAQUE_ENDIAN_FIXED. Fix: reserialize with a producer that writes opaque payload numerics using little-endian bytes."
                .to_string(),
        );
    }
    let mut expected = [0_u8; 32];
    expected.copy_from_slice(&bytes[8..40]);
    let body = &bytes[HEADER_LEN..];
    let actual = blake3::hash(body);
    if actual.as_bytes() != &expected {
        return Err(format!(
            "IntegrityMismatch: expected {}, actual {}. Fix: reject the truncated or tampered Program bytes and re-fetch the payload.",
            hex32(&expected),
            actual.to_hex()
        ));
    }

    let mut reader = Reader {
        bytes: body,
        pos: 0,
        depth: 0,
    };
    let (entry_op_id, workgroup_size, mut metadata) = read_nodes(&mut reader)?;
    read_memory_regions(&mut reader, &mut metadata)?;
    let output_set = read_output_set(&mut reader, &metadata)?;
    if reader.pos != reader.bytes.len() {
        return Err(
            "TruncatedPayload: trailing bytes after OutputSet. Fix: provide exactly one canonical VYRE Program blob."
                .to_string(),
        );
    }

    let non_composable_with_self = metadata.non_composable_with_self;
    let mut buffers = Vec::new();
    reserve_decoded_vec_capacity(
        &mut buffers,
        metadata.buffers.len(),
        "decoded Program buffer table",
    )?;
    for buffer in metadata.buffers {
        buffers.push(BufferDecl {
            name: Arc::from(buffer.name),
            binding: buffer.binding,
            access: buffer.access,
            kind: buffer.kind,
            element: buffer.element,
            count: buffer.count,
            is_output: buffer.is_output,
            pipeline_live_out: buffer.pipeline_live_out,
            output_byte_range: buffer.output_byte_range,
            hints: buffer.hints,
            bytes_extraction: buffer.bytes_extraction,
            // Wire v1 carries no linear_type tag, so decoded buffers default
            // to Unrestricted. New wire versions must map their own tag before
            // constructing BufferDecl.
            linear_type: crate::ir_inner::model::program::LinearType::default(),
            // Wire v1 carries no shape predicates; decoded historical blobs
            // have no static shape refinement.
            shape_predicate: None,
        });
    }
    let program = Program::new_raw(buffers, workgroup_size, metadata.entry)
        .with_optional_entry_op_id(entry_op_id)
        .with_non_composable_with_self(non_composable_with_self);
    program
        .output_buffer_index
        .set(Arc::new(output_set.into_vec()))
        .unwrap_or_else(|_| {
            unreachable!("decoded Program output buffer index is initialized exactly once")
        });
    Ok(program)
}

#[derive(Default)]
pub(crate) struct DecodedMetadata {
    pub(crate) entry: Vec<crate::ir::Node>,
    pub(crate) buffers: Vec<DecodedBuffer>,
    pub(crate) non_composable_with_self: bool,
}

pub(crate) struct DecodedBuffer {
    pub(crate) name: String,
    pub(crate) binding: u32,
    pub(crate) access: BufferAccess,
    pub(crate) kind: MemoryKind,
    pub(crate) element: DataType,
    pub(crate) count: u32,
    pub(crate) is_output: bool,
    pub(crate) pipeline_live_out: bool,
    pub(crate) output_byte_range: Option<Range<usize>>,
    pub(crate) hints: MemoryHints,
    pub(crate) bytes_extraction: bool,
}

fn read_output_set(
    reader: &mut Reader<'_>,
    metadata: &DecodedMetadata,
) -> Result<crate::serial::output_set::OutputSet, String> {
    crate::serial::output_set::OutputSet::decode_from(reader, metadata)
}

fn read_nodes(
    reader: &mut Reader<'_>,
) -> Result<(Option<String>, [u32; 3], DecodedMetadata), String> {
    let count = reader.leb_len(MAX_NODES, "node count")?;
    if count == 0 {
        return Err("TruncatedPayload: missing metadata node. Fix: serialize Program with Program::to_wire().".to_string());
    }
    let (op_id, payload, operands) = read_node_record(reader, 0)?;
    if op_id != METADATA_OP_ID {
        return Err(format!(
            "UnknownOp: op_id `{op_id}`. Fix: the first VYRE node must be `{METADATA_OP_ID}` metadata from Program::to_wire()."
        ));
    }
    if !operands.is_empty() {
        return Err("InvalidDiscriminant: metadata node has operands. Fix: reserialize with Program::to_wire().".to_string());
    }
    let (entry_op_id, workgroup_size, mut metadata) = read_metadata(payload)?;
    reserve_decoded_vec_capacity(&mut metadata.entry, count - 1, "decoded entry nodes")?;
    for node_index in 1..count {
        let (op_id, payload, operands) = read_node_record(reader, node_index)?;
        if !operands.is_empty() {
            return Err(format!(
                "InvalidDiscriminant: node {node_index} carries operand ids but legacy payload nodes are self-contained. Fix: reserialize with this Vyre version."
            ));
        }
        let mut payload_reader = Reader {
            bytes: payload,
            pos: 0,
            depth: 0,
        };
        let node = payload_reader.node()?;
        if payload_reader.pos != payload_reader.bytes.len() {
            return Err(format!(
                "TruncatedPayload: node {node_index} payload has trailing bytes. Fix: reject this non-canonical Program blob."
            ));
        }
        let actual = crate::ir_inner::model::node::node_op_id(&node);
        if actual != op_id {
            return Err(format!(
                "InvalidDiscriminant: node {node_index} op_id `{op_id}` does not match payload `{actual}`. Fix: reject tampered wire bytes."
            ));
        }
        metadata.entry.push(node);
    }
    Ok((entry_op_id, workgroup_size, metadata))
}

// VYRE_IR_HOTSPOTS CRIT (from_wire.rs:220): the previous signature
// copied the payload slice into a Vec<u8> for every node record.
// Returning a sub-slice view bound to the reader's input lifetime
// removes one allocation per node  -  N-node wire decodes now skip
// N heap allocations on the payload path entirely.
fn read_node_record<'a>(
    reader: &mut Reader<'a>,
    node_index: usize,
) -> Result<(String, &'a [u8], Vec<u32>), String> {
    let op_id = reader.leb_string()?;
    // Bound node payload length to MAX_OPAQUE_PAYLOAD_LEN so an oversized
    // length field produces an actionable "exceeds limit" error at the gate,
    // not a confusing "truncated" error from the body read that follows.
    // The encoder already enforces this limit on write (put_node.rs:211), so
    // any blob claiming more is either corrupt or crafted.
    let payload_len = reader.leb_len(MAX_OPAQUE_PAYLOAD_LEN, "node payload length")?;
    let payload = reader.take(payload_len)?;
    let operand_count = reader.leb_len(MAX_NODES, "operand count")?;
    let mut operands = Vec::new();
    reserve_decoded_vec_capacity(&mut operands, operand_count, "decoded node operands")?;
    for _ in 0..operand_count {
        let operand = reader.leb_u32()?;
        if usize::try_from(operand).map_or(true, |operand| operand >= node_index) {
            return Err(format!(
                "TruncatedPayload: operand id {operand} is not a prior node for node {node_index}. Fix: topologically order operands and reject forward/self references."
            ));
        }
        operands.push(operand);
    }
    Ok((op_id, payload, operands))
}

fn read_metadata(payload: &[u8]) -> Result<(Option<String>, [u32; 3], DecodedMetadata), String> {
    let mut reader = Reader {
        bytes: payload,
        pos: 0,
        depth: 0,
    };
    if reader.take(9)? != b"VYRE-META" {
        return Err("InvalidDiscriminant: metadata payload marker is invalid. Fix: serialize Program with Program::to_wire().".to_string());
    }
    let entry_op_id = match reader.u8()? {
        0 => None,
        1 => Some(reader.string()?),
        value => {
            return Err(format!(
                "InvalidDiscriminant: field entry_op_id tag has value {value}. Fix: reserialize with Program::to_wire()."
            ));
        }
    };
    let workgroup_size = [reader.u32()?, reader.u32()?, reader.u32()?];
    super::invariants::validate_workgroup_size(workgroup_size)?;
    let non_composable_with_self = match reader.u8()? {
        0 => false,
        1 => true,
        value => {
            return Err(format!(
                "InvalidDiscriminant: field non_composable_with_self has value {value}. Fix: reserialize with Program::to_wire()."
            ));
        }
    };
    let buffer_count = reader.leb_len(MAX_BUFFERS, "metadata buffer count")?;
    let mut buffers = Vec::new();
    reserve_decoded_vec_capacity(&mut buffers, buffer_count, "decoded metadata buffers")?;
    for _ in 0..buffer_count {
        let name = reader.string()?;
        let binding = reader.u32()?;
        let count = reader.u32()?;
        let is_output = reader.u8()? != 0;
        let pipeline_live_out = reader.u8()? != 0;
        if count == 0 && is_output {
            return Err(format!(
                "InvalidDiscriminant: output buffer `{name}` has count 0. Fix: output buffers need a concrete positive element count before serialization."
            ));
        }
        if count == 0 && pipeline_live_out {
            return Err(format!(
                "InvalidDiscriminant: live-out buffer `{name}` has count 0. Fix: externally-visible buffers need a concrete positive element count before serialization."
            ));
        }
        let output_byte_range = match reader.u8()? {
            0 => None,
            1 => {
                let start = usize::try_from(reader.leb_u64()?).map_err(|err| {
                    format!("TruncatedPayload: output range start cannot fit usize ({err}). Fix: reject this payload on this target.")
                })?;
                let end = usize::try_from(reader.leb_u64()?).map_err(|err| {
                    format!("TruncatedPayload: output range end cannot fit usize ({err}). Fix: reject this payload on this target.")
                })?;
                super::invariants::validate_output_range_order(start, end)?;
                Some(start..end)
            }
            value => {
                return Err(format!(
                    "InvalidDiscriminant: field output range tag has value {value}. Fix: reserialize with Program::to_wire()."
                ));
            }
        };
        let hints = read_hints(&mut reader)?;
        buffers.push(DecodedBuffer {
            name,
            binding,
            access: BufferAccess::ReadOnly,
            kind: MemoryKind::Readonly,
            element: DataType::U32,
            count,
            is_output,
            pipeline_live_out,
            output_byte_range,
            hints,
            bytes_extraction: false,
        });
    }
    if reader.pos != reader.bytes.len() {
        return Err("TruncatedPayload: metadata payload has trailing bytes. Fix: reject non-canonical Program bytes.".to_string());
    }
    Ok((
        entry_op_id,
        workgroup_size,
        DecodedMetadata {
            entry: Vec::new(),
            buffers,
            non_composable_with_self,
        },
    ))
}

fn read_memory_regions(
    reader: &mut Reader<'_>,
    metadata: &mut DecodedMetadata,
) -> Result<(), String> {
    let count = reader.leb_len(MAX_BUFFERS, "memory-region count")?;
    if count != metadata.buffers.len() {
        return Err(format!(
            "InvalidDiscriminant: memory-region count {count} does not match metadata buffer count {}. Fix: reject tampered Program bytes.",
            metadata.buffers.len()
        ));
    }
    for index in 0..count {
        let id = reader.leb_u32()?;
        if usize::try_from(id).ok() != Some(index) {
            return Err(format!(
                "InvalidDiscriminant: memory-region id {id} is out of canonical order at index {index}. Fix: reserialize with Program::to_wire()."
            ));
        }
        let kind = memory_kind_from_tag(reader.u8()?)?;
        let access = access_from_tag(reader.u8()?)?;
        let element_tag = reader.u8()?;
        let shape_tag = reader.u8()?;
        if shape_tag != 0 {
            return Err(format!(
                "InvalidDiscriminant: field shape_tag has value {shape_tag}. Fix: this decoder supports Dense regions only in schema {WIRE_FORMAT_VERSION}."
            ));
        }
        // VYRE_IR_HOTSPOTS CRIT (from_wire.rs:355): `.to_vec()` on
        // every buffer's shape payload cost one heap alloc per
        // buffer. Using the raw sub-slice from the parent reader
        // keeps decoding zero-copy.
        let shape_len = reader.leb_len(64, "shape payload length")?;
        let shape_payload = reader.take(shape_len)?;
        let mut shape_reader = Reader {
            bytes: shape_payload,
            pos: 0,
            depth: 0,
        };
        let count_value = u32::try_from(shape_reader.leb_u64()?).map_err(|err| {
            format!("TruncatedPayload: dense shape count cannot fit u32 ({err}). Fix: split the memory region.")
        })?;
        let element = if element_tag == 0x08 {
            let element_size = usize::try_from(shape_reader.leb_u64()?).map_err(|err| {
                format!("TruncatedPayload: array element size cannot fit usize ({err}). Fix: reject this payload on this target.")
            })?;
            DataType::Array { element_size }
        } else if element_tag == 0x13 {
            let id_value = u32::try_from(shape_reader.leb_u64()?).map_err(|err| {
                format!("TruncatedPayload: handle DataType id cannot fit u32 ({err}). Fix: reject this payload.")
            })?;
            DataType::Handle(vyre_spec::data_type::TypeId(id_value))
        } else if element_tag == 0x80 {
            let id_value = u32::try_from(shape_reader.leb_u64()?).map_err(|err| {
                format!("TruncatedPayload: opaque DataType id cannot fit u32 ({err}). Fix: reject this payload.")
            })?;
            let id_value = reject_reserved_extension_id(id_value, "DataType")?;
            DataType::Opaque(vyre_spec::extension::ExtensionDataTypeId(id_value))
        } else if element_tag == 0x1F {
            let storage_tag = u8::try_from(shape_reader.leb_u64()?).map_err(|err| {
                format!("TruncatedPayload: quantized storage DataType tag cannot fit u8 ({err}). Fix: reject this payload.")
            })?;
            let storage = data_type_from_tag(storage_tag)?;
            if !storage.is_quantized_storage() {
                return Err(format!(
                    "InvalidDiscriminant: quantized storage tag {storage_tag} decodes to `{storage}`, which is not a valid quantized storage type. Fix: reserialize with I4/I8/I16/U8/U16/F8/FP4/NF4 storage."
                ));
            }
            let scale = read_dense_quantization_scale(&mut shape_reader)?;
            let zero_point = read_dense_quantization_zero_point(&mut shape_reader)?;
            DataType::Quantized {
                storage: Box::new(storage),
                scale,
                zero_point,
            }
        } else {
            data_type_from_tag(element_tag)?
        };
        if shape_reader.pos != shape_reader.bytes.len() {
            return Err("TruncatedPayload: shape payload has trailing bytes. Fix: reject non-canonical Program bytes.".to_string());
        }
        // VYRE_IR_HOTSPOTS CRIT (from_wire.rs:387): same zero-copy
        // sub-slice pattern as the shape payload above.
        let hints_len = reader.leb_len(64, "hints payload length")?;
        let hints_payload = reader.take(hints_len)?;
        let mut hints_reader = Reader {
            bytes: hints_payload,
            pos: 0,
            depth: 0,
        };
        let hints = read_hints(&mut hints_reader)?;
        if hints_reader.pos != hints_reader.bytes.len() {
            return Err("TruncatedPayload: hints payload has trailing bytes. Fix: reject non-canonical Program bytes.".to_string());
        }
        let metadata_buffer = &metadata.buffers[index];
        if metadata_buffer.count != count_value {
            return Err(format!(
                "InvalidDiscriminant: memory-region count {count_value} does not match metadata count {} for buffer `{}`. Fix: reserialize with Program::to_wire().",
                metadata_buffer.count, metadata_buffer.name
            ));
        }
        if count_value == 0 && access == BufferAccess::Workgroup {
            return Err(format!(
                "InvalidDiscriminant: workgroup buffer `{}` has count 0. Fix: workgroup memory requires a concrete positive element count.",
                metadata_buffer.name
            ));
        }
        if count_value == 0 && metadata_buffer.is_output {
            return Err(format!(
                "InvalidDiscriminant: output buffer `{}` has count 0. Fix: output buffers need a concrete positive element count before serialization.",
                metadata_buffer.name
            ));
        }
        if count_value == 0 && metadata_buffer.pipeline_live_out {
            return Err(format!(
                "InvalidDiscriminant: live-out buffer `{}` has count 0. Fix: externally-visible buffers need a concrete positive element count before serialization.",
                metadata_buffer.name
            ));
        }
        super::invariants::validate_output_range_fits(metadata_buffer, &element, count_value)?;
        let buffer = &mut metadata.buffers[index];
        buffer.kind = kind;
        buffer.access = access;
        buffer.element = element;
        buffer.count = count_value;
        buffer.hints = hints;
    }
    Ok(())
}

fn reserve_decoded_vec_capacity<T>(
    vec: &mut Vec<T>,
    capacity: usize,
    field: &'static str,
) -> Result<(), String> {
    if vec.capacity() >= capacity {
        return Ok(());
    }
    vec.try_reserve_exact(capacity - vec.capacity())
        .map_err(|error| {
            format!(
                "TruncatedPayload: failed to reserve {field} for {capacity} entries: {error}. Fix: reject this untrusted wire payload or split the Program before serialization."
            )
        })
}

fn hex32(bytes: &[u8; 32]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(64);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0F) as usize] as char);
    }
    out
}

pub(crate) trait LebReader {
    fn leb_u64(&mut self) -> Result<u64, String>;
    fn leb_u32(&mut self) -> Result<u32, String>;
    fn leb_len(&mut self, max: usize, label: &str) -> Result<usize, String>;
    fn leb_string(&mut self) -> Result<String, String>;
}

impl LebReader for Reader<'_> {
    fn leb_u64(&mut self) -> Result<u64, String> {
        let mut result = 0_u64;
        for shift in (0..70).step_by(7) {
            let byte = self.u8()?;
            if shift == 63 && byte > 1 {
                return Err("TruncatedPayload: LEB128 integer exceeds u64. Fix: reject malformed wire bytes.".to_string());
            }
            result |= u64::from(byte & 0x7F) << shift;
            if byte & 0x80 == 0 {
                return Ok(result);
            }
        }
        Err(
            "TruncatedPayload: unterminated LEB128 integer. Fix: reject malformed wire bytes."
                .to_string(),
        )
    }

    fn leb_u32(&mut self) -> Result<u32, String> {
        let value = self.leb_u64()?;
        u32::try_from(value).map_err(|err| {
            format!("TruncatedPayload: LEB128 value {value} cannot fit u32 ({err}). Fix: reject malformed wire bytes.")
        })
    }

    fn leb_len(&mut self, max: usize, label: &str) -> Result<usize, String> {
        let value = usize::try_from(self.leb_u64()?).map_err(|err| {
            format!("TruncatedPayload: {label} cannot fit usize ({err}). Fix: reject malformed wire bytes on this target.")
        })?;
        if value > max {
            return Err(format!(
                "TruncatedPayload: {label} {value} exceeds limit {max}. Fix: split the Program or reject this untrusted payload."
            ));
        }
        Ok(value)
    }

    fn leb_string(&mut self) -> Result<String, String> {
        // VYRE_NAGA_LOWER MEDIUM (from_wire.rs:567-571): previous
        // code did `bytes.to_vec()` then `String::from_utf8(vec)`.
        // `std::str::from_utf8` validates the borrowed slice, then
        // we own-copy once via `to_owned()`. Same cost as before for
        // the happy path (one copy) but no intermediate Vec.
        let len = self.leb_len(crate::serial::wire::MAX_STRING_LEN, "string length")?;
        let bytes = self.take(len)?;
        std::str::from_utf8(bytes).map(str::to_owned).map_err(|err| {
            format!("TruncatedPayload: invalid UTF-8 in string ({err}). Fix: serialize valid UTF-8 operation identifiers.")
        })
    }
}

#[cfg(test)]
mod tests;
