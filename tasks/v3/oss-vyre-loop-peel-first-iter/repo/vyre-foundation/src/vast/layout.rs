//! Shared VAST byte-layout helpers.

use super::error::VastError;
use super::header::{VastHeader, HEADER_LEN};
use super::node::NODE_STRIDE_U32;

pub(crate) fn table_byte_len(rows: u32, row_bytes: usize, got: usize) -> Result<usize, VastError> {
    (rows as usize)
        .checked_mul(row_bytes)
        .ok_or(VastError::LengthMismatch {
            expected: usize::MAX,
            got,
        })
}

pub(crate) fn layout_prefix_len(hdr: VastHeader, got: usize) -> Result<usize, VastError> {
    let node_bytes = table_byte_len(hdr.node_count, NODE_STRIDE_U32 * 4, got)?;
    let file_bytes = table_byte_len(hdr.file_count, 12, got)?;
    HEADER_LEN
        .checked_add(node_bytes)
        .and_then(|len| len.checked_add(file_bytes))
        .ok_or(VastError::LengthMismatch {
            expected: usize::MAX,
            got,
        })
}

pub(crate) fn read_u32_at(chunk: &[u8], word_off: usize) -> Option<u32> {
    let b = word_off.checked_mul(4)?;
    chunk
        .get(b..b + 4)
        .map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
}

pub(crate) fn span_in_bounds(off: u32, len: u32, limit: u32) -> bool {
    off.checked_add(len).is_some_and(|end| end <= limit)
}
