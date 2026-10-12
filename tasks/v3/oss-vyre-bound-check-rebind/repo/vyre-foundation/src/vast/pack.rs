//! Small VAST construction helpers for tests and graph-walk contracts.

use super::header::{VAST_MAGIC, VAST_VERSION};
use super::node::{VastNode, SENTINEL};

/// Build a minimal valid VAST buffer: one root + optional linear first-child chain.
#[must_use]
pub fn pack_spine_vast(node_kinds: &[u32]) -> Vec<u8> {
    let n = u32::try_from(node_kinds.len()).unwrap_or(u32::MAX);
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&VAST_MAGIC);
    bytes.extend_from_slice(&VAST_VERSION.to_le_bytes());
    bytes.extend_from_slice(&0u16.to_le_bytes()); // source_lang
    bytes.extend_from_slice(&n.to_le_bytes());
    bytes.extend_from_slice(&0u32.to_le_bytes()); // file_count
    bytes.extend_from_slice(&0u32.to_le_bytes()); // string_blob_len
    bytes.extend_from_slice(&0u32.to_le_bytes()); // attr_blob_len
    for i in 0..n {
        let fc = if i + 1 < n { i + 1 } else { SENTINEL };
        let parent = if i == 0 { SENTINEL } else { i - 1 };
        let row = VastNode {
            kind: node_kinds[i as usize],
            parent_idx: parent,
            first_child: fc,
            next_sibling: SENTINEL,
            src_file: 0,
            src_byte_off: 0,
            src_byte_len: 0,
            attr_off: 0,
            attr_len: 0,
            reserved: 0,
        };
        bytes.extend_from_slice(&row.to_bytes());
    }
    bytes
}
