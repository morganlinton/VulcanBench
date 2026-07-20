//! OutputSet persistence type for VIR0 wire format.
//!
//! An `OutputSet` records which buffer indices are writable outputs
//! (`ReadWrite` or `WriteOnly`) so that decoders can reconstruct the exact
//! output set without re-scanning the buffer table.

use crate::ir_inner::model::program::BufferDecl;
use crate::ir_inner::model::types::BufferAccess;

/// Ordered list of output (writable) buffer indices.
///
/// Encoded as a LEB128 count followed by that many LEB128 `u32` indices.
/// The indices are in strict declaration order and name only buffers
/// whose access mode is [`BufferAccess::ReadWrite`] or [`BufferAccess::WriteOnly`].
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct OutputSet(Vec<u32>);

impl OutputSet {
    /// Build an `OutputSet` by scanning `buffers` for writable output entries.
    #[must_use]
    pub fn from_buffers(buffers: &[BufferDecl]) -> Self {
        let mut indices = Vec::with_capacity(buffers.len());
        for (index, buffer) in buffers.iter().enumerate() {
            if is_output_access(&buffer.access()) {
                if let Ok(index) = u32::try_from(index) {
                    indices.push(index);
                } else {
                    return Self(indices);
                }
            }
        }
        Self(indices)
    }

    /// Encode this output set into `dst`.
    ///
    /// # Errors
    ///
    /// Returns an actionable diagnostic if the output-set count cannot fit the
    /// encoded length representation.
    pub fn encode_into(&self, dst: &mut Vec<u8>) -> Result<(), String> {
        let count = u64::try_from(self.0.len()).map_err(|err| {
            format!("Fix: output-set count cannot fit u64 ({err}); split the Program.")
        })?;
        dst.reserve(leb_u64_len(count) + self.0.len().saturating_mul(5));
        put_leb_u64(dst, count);
        for &index in &self.0 {
            put_leb_u32(dst, index);
        }
        Ok(())
    }

    /// Encode the canonical output set for `buffers` without materialising an
    /// intermediate `Vec<u32>`.
    ///
    /// # Errors
    ///
    /// Returns an actionable diagnostic if the output-set count or any buffer
    /// index cannot fit the wire representation.
    pub fn encode_from_buffers_into(
        buffers: &[BufferDecl],
        dst: &mut Vec<u8>,
    ) -> Result<(), String> {
        let count = buffers
            .iter()
            .filter(|buffer| is_output_access(&buffer.access()))
            .count();
        let count_u64 = u64::try_from(count).map_err(|err| {
            format!("Fix: output-set count cannot fit u64 ({err}); split the Program.")
        })?;
        dst.reserve(leb_u64_len(count_u64) + count.saturating_mul(5));
        put_leb_u64(dst, count_u64);
        for (index, buffer) in buffers.iter().enumerate() {
            if !is_output_access(&buffer.access()) {
                continue;
            }
            let index = u32::try_from(index).map_err(|err| {
                format!("Fix: output-set index cannot fit u32 ({err}); split the Program.")
            })?;
            put_leb_u32(dst, index);
        }
        Ok(())
    }

    /// Build from an already-validated vec of indices.
    #[must_use]
    pub fn from_vec(indices: Vec<u32>) -> Self {
        Self(indices)
    }

    /// Decode and validate an output set from the canonical wire stream.
    pub(crate) fn decode_from(
        reader: &mut crate::serial::wire::Reader<'_>,
        metadata: &crate::serial::wire::decode::from_wire::DecodedMetadata,
    ) -> Result<Self, String> {
        use crate::serial::wire::decode::from_wire::LebReader;

        let count = reader.leb_len(crate::serial::wire::MAX_BUFFERS, "output set count")?;
        let mut indices = Vec::with_capacity(count);
        for _ in 0..count {
            let index = reader.leb_u32()?;
            let usize_index = usize::try_from(index).map_err(|err| {
                format!(
                    "InvalidDiscriminant: output-set index {index} cannot fit usize ({err}). Fix: reserialize with Program::to_wire()."
                )
            })?;
            let Some(buffer) = metadata.buffers.get(usize_index) else {
                return Err(format!(
                    "InvalidDiscriminant: output-set index {index} is out of bounds for {} buffers. Fix: reject tampered Program bytes.",
                    metadata.buffers.len()
                ));
            };
            if !is_output_access(&buffer.access) {
                return Err(format!(
                    "InvalidDiscriminant: output-set index {index} names non-writable buffer `{}`. Fix: reserialize with Program::to_wire().",
                    buffer.name
                ));
            }
            if indices.last().is_some_and(|previous| *previous >= index) {
                return Err(format!(
                    "InvalidDiscriminant: output-set index {index} is not in strict declaration order. Fix: reserialize with Program::to_wire()."
                ));
            }
            indices.push(index);
        }
        if !matches_canonical_output_indices(&indices, metadata)? {
            let expected = canonical_output_indices(metadata)?;
            return Err(format!(
                "InvalidDiscriminant: output-set {indices:?} does not match writable buffer indices {expected:?}. Fix: reject tampered Program bytes."
            ));
        }
        Ok(Self(indices))
    }

    /// The ordered indices.
    #[must_use]
    pub fn indices(&self) -> &[u32] {
        &self.0
    }

    /// Whether the set is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Number of output buffers.
    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Consume the set and return the inner vec.
    #[must_use]
    pub fn into_vec(self) -> Vec<u32> {
        self.0
    }
}

fn matches_canonical_output_indices(
    indices: &[u32],
    metadata: &crate::serial::wire::decode::from_wire::DecodedMetadata,
) -> Result<bool, String> {
    let mut actual = indices.iter().copied();
    for (index, buffer) in metadata.buffers.iter().enumerate() {
        if !is_output_access(&buffer.access) {
            continue;
        }
        let expected = u32::try_from(index).map_err(|err| {
            format!(
                "InvalidDiscriminant: canonical output-set index cannot fit u32 ({err}). Fix: split the Program."
            )
        })?;
        if actual.next() != Some(expected) {
            return Ok(false);
        }
    }
    Ok(actual.next().is_none())
}

fn canonical_output_indices(
    metadata: &crate::serial::wire::decode::from_wire::DecodedMetadata,
) -> Result<Vec<u32>, String> {
    let mut expected = Vec::with_capacity(metadata.buffers.len());
    for (index, buffer) in metadata.buffers.iter().enumerate() {
        if !is_output_access(&buffer.access) {
            continue;
        }
        expected.push(u32::try_from(index).map_err(|err| {
            format!(
                "InvalidDiscriminant: canonical output-set index cannot fit u32 ({err}). Fix: split the Program."
            )
        })?);
    }
    Ok(expected)
}

#[inline]
fn is_output_access(access: &BufferAccess) -> bool {
    matches!(access, BufferAccess::ReadWrite | BufferAccess::WriteOnly)
}

impl AsRef<[u32]> for OutputSet {
    fn as_ref(&self) -> &[u32] {
        &self.0
    }
}

fn put_leb_u32(out: &mut Vec<u8>, value: u32) {
    put_leb_u64(out, u64::from(value));
}

fn leb_u64_len(mut value: u64) -> usize {
    let mut len = 1;
    while value >= 0x80 {
        value >>= 7;
        len += 1;
    }
    len
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
mod tests {
    use super::*;
    use crate::ir::{BufferDecl, DataType};

    #[test]
    fn from_buffers_picks_writable_outputs() {
        let buffers = vec![
            BufferDecl::read("a", 0, DataType::U32),
            BufferDecl::read_write("b", 1, DataType::U32),
            BufferDecl::read("c", 2, DataType::U32),
            BufferDecl::read_write("d", 3, DataType::U32),
            BufferDecl::storage("e", 4, BufferAccess::WriteOnly, DataType::U32),
        ];
        let set = OutputSet::from_buffers(&buffers);
        assert_eq!(set.indices(), &[1, 3, 4]);
    }

    #[test]
    fn roundtrip_encode_decode() {
        let buffers = vec![
            BufferDecl::read("a", 0, DataType::U32),
            BufferDecl::read_write("b", 1, DataType::U32),
            BufferDecl::read_write("c", 2, DataType::U32),
            BufferDecl::storage("d", 3, BufferAccess::WriteOnly, DataType::U32),
        ];
        let original = OutputSet::from_buffers(&buffers);
        let mut encoded = Vec::new();
        original.encode_into(&mut encoded).unwrap();

        // Build a minimal DecodedMetadata for decoding
        let metadata = crate::serial::wire::decode::from_wire::DecodedMetadata {
            buffers: buffers
                .iter()
                .map(|b| crate::serial::wire::decode::from_wire::DecodedBuffer {
                    name: b.name().to_string(),
                    binding: b.binding(),
                    access: b.access(),
                    kind: b.kind(),
                    element: b.element(),
                    count: b.count(),
                    is_output: b.is_output(),
                    pipeline_live_out: b.is_pipeline_live_out(),
                    output_byte_range: b.output_byte_range(),
                    hints: b.hints(),
                    bytes_extraction: false,
                })
                .collect(),
            ..Default::default()
        };

        let mut reader = crate::serial::wire::Reader {
            bytes: &encoded,
            pos: 0,
            depth: 0,
        };
        let decoded = OutputSet::decode_from(&mut reader, &metadata).unwrap();
        assert_eq!(original, decoded);
    }

    #[test]
    fn direct_buffer_encode_matches_materialized_output_set() {
        let buffers = vec![
            BufferDecl::read("a", 0, DataType::U32),
            BufferDecl::read_write("b", 1, DataType::U32),
            BufferDecl::read("c", 2, DataType::U32),
            BufferDecl::read_write("d", 3, DataType::U32),
            BufferDecl::storage("e", 4, BufferAccess::WriteOnly, DataType::U32),
        ];

        let mut materialized = Vec::new();
        OutputSet::from_buffers(&buffers)
            .encode_into(&mut materialized)
            .unwrap();

        let mut direct = Vec::new();
        OutputSet::encode_from_buffers_into(&buffers, &mut direct).unwrap();

        assert_eq!(direct, materialized);
    }
}
