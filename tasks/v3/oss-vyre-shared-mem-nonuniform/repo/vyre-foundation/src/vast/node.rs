//! VAST node and file-table row types.

use super::layout::read_u32_at;

/// One node row = 10 `u32` words (`kind` + 9 fields), 40 bytes.
pub const NODE_STRIDE_U32: usize = 10;
/// Sentinel parent / child / sibling index meaning “none”.
pub const SENTINEL: u32 = u32::MAX;

/// One node row in the packed node table (host view).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VastNode {
    /// Language-local kind tag (low 16 bits are meaningful for tools).
    pub kind: u32,
    /// Parent node index, or [`SENTINEL`] for the synthetic root.
    pub parent_idx: u32,
    /// First child index, or [`SENTINEL`].
    pub first_child: u32,
    /// Next sibling index, or [`SENTINEL`].
    pub next_sibling: u32,
    /// File table index for source mapping.
    pub src_file: u32,
    /// Byte offset into that file.
    pub src_byte_off: u32,
    /// Span length in bytes.
    pub src_byte_len: u32,
    /// Offset into `attr_blob`.
    pub attr_off: u32,
    /// Length in `attr_blob`.
    pub attr_len: u32,
    /// Reserved for alignment / forward-compatible fields.
    pub reserved: u32,
}

/// One file metadata row in the packed file table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VastFile {
    /// Offset into the string blob for this file path.
    pub path_off: u32,
    /// Byte length of this file path in the string blob.
    pub path_len: u32,
    /// Source file byte length used to validate node spans.
    pub size: u32,
}

impl VastFile {
    /// Read one file row from the file-table byte region.
    #[must_use]
    pub fn read_row_bytes(file_bytes: &[u8], file_index: u32) -> Option<Self> {
        let base = (file_index as usize).checked_mul(3)?;
        Some(Self {
            path_off: read_u32_at(file_bytes, base)?,
            path_len: read_u32_at(file_bytes, base + 1)?,
            size: read_u32_at(file_bytes, base + 2)?,
        })
    }
}

impl VastNode {
    /// Read one node from the node-table byte region (`HEADER_LEN..`).
    #[must_use]
    pub fn read_row_bytes(node_bytes: &[u8], node_index: u32) -> Option<Self> {
        let base = (node_index as usize).checked_mul(NODE_STRIDE_U32)?;
        Some(Self {
            kind: read_u32_at(node_bytes, base)?,
            parent_idx: read_u32_at(node_bytes, base + 1)?,
            first_child: read_u32_at(node_bytes, base + 2)?,
            next_sibling: read_u32_at(node_bytes, base + 3)?,
            src_file: read_u32_at(node_bytes, base + 4)?,
            src_byte_off: read_u32_at(node_bytes, base + 5)?,
            src_byte_len: read_u32_at(node_bytes, base + 6)?,
            attr_off: read_u32_at(node_bytes, base + 7)?,
            attr_len: read_u32_at(node_bytes, base + 8)?,
            reserved: read_u32_at(node_bytes, base + 9)?,
        })
    }

    /// Encode a node row to exactly [`NODE_STRIDE_U32`] little-endian words.
    #[must_use]
    pub fn to_bytes(self) -> [u8; NODE_STRIDE_U32 * 4] {
        let mut out = [0u8; NODE_STRIDE_U32 * 4];
        let w = [
            self.kind,
            self.parent_idx,
            self.first_child,
            self.next_sibling,
            self.src_file,
            self.src_byte_off,
            self.src_byte_len,
            self.attr_off,
            self.attr_len,
            self.reserved,
        ];
        for (i, word) in w.iter().enumerate() {
            out[i * 4..(i + 1) * 4].copy_from_slice(&word.to_le_bytes());
        }
        out
    }
}
