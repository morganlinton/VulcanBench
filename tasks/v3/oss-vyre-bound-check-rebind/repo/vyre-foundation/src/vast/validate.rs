//! VAST structural validation.

use super::error::VastError;
use super::header::{VastHeader, HEADER_LEN};
use super::layout::{layout_prefix_len, span_in_bounds, table_byte_len};
use super::node::{VastFile, VastNode, NODE_STRIDE_U32, SENTINEL};

/// Validate magic, version, exact byte length, tree edge indices, and blob spans.
///
/// # Errors
///
/// Returns [`VastError`] when any structural invariant is violated.
pub fn validate_vast(bytes: &[u8]) -> Result<VastHeader, VastError> {
    let hdr = VastHeader::decode(bytes)?;
    let expected = hdr.total_byte_len().ok_or(VastError::LengthMismatch {
        expected: usize::MAX,
        got: bytes.len(),
    })?;
    if expected != bytes.len() {
        return Err(VastError::LengthMismatch {
            expected,
            got: bytes.len(),
        });
    }
    let node_bytes_len = table_byte_len(hdr.node_count, NODE_STRIDE_U32 * 4, bytes.len())?;
    let node_region_end =
        HEADER_LEN
            .checked_add(node_bytes_len)
            .ok_or(VastError::LengthMismatch {
                expected: usize::MAX,
                got: bytes.len(),
            })?;
    let node_region = bytes
        .get(HEADER_LEN..node_region_end)
        .ok_or(VastError::TooShort {
            need: node_region_end,
            got: bytes.len(),
        })?;
    let file_bytes_len = table_byte_len(hdr.file_count, 12, bytes.len())?;
    let file_region_start = node_region_end;
    let file_region_end =
        file_region_start
            .checked_add(file_bytes_len)
            .ok_or(VastError::LengthMismatch {
                expected: usize::MAX,
                got: bytes.len(),
            })?;
    let file_region = bytes
        .get(file_region_start..file_region_end)
        .ok_or(VastError::TooShort {
            need: file_region_end,
            got: bytes.len(),
        })?;
    validate_tree_edges(node_region, hdr.node_count)?;
    validate_blob_spans(node_region, hdr, file_region)?;
    Ok(hdr)
}

fn validate_blob_spans(
    node_bytes: &[u8],
    hdr: VastHeader,
    file_bytes: &[u8],
) -> Result<(), VastError> {
    let layout_prefix = layout_prefix_len(hdr, HEADER_LEN + node_bytes.len() + file_bytes.len())?;
    for file_idx in 0u32..hdr.file_count {
        let file = VastFile::read_row_bytes(file_bytes, file_idx).ok_or(VastError::TooShort {
            need: layout_prefix,
            got: HEADER_LEN + node_bytes.len() + file_bytes.len(),
        })?;
        if !span_in_bounds(file.path_off, file.path_len, hdr.string_blob_len) {
            return Err(VastError::BadFilePath {
                file: file_idx,
                off: file.path_off,
                len: file.path_len,
                string_blob_len: hdr.string_blob_len,
            });
        }
    }

    let node_need = HEADER_LEN
        .checked_add(table_byte_len(
            hdr.node_count,
            NODE_STRIDE_U32 * 4,
            HEADER_LEN + node_bytes.len(),
        )?)
        .ok_or(VastError::LengthMismatch {
            expected: usize::MAX,
            got: HEADER_LEN + node_bytes.len(),
        })?;
    for node_idx in 0u32..hdr.node_count {
        let node = VastNode::read_row_bytes(node_bytes, node_idx).ok_or(VastError::TooShort {
            need: node_need,
            got: HEADER_LEN + node_bytes.len(),
        })?;

        if !span_in_bounds(node.attr_off, node.attr_len, hdr.attr_blob_len) {
            return Err(VastError::BadAttrSpan {
                node: node_idx,
                off: node.attr_off,
                len: node.attr_len,
                attr_blob_len: hdr.attr_blob_len,
            });
        }

        let has_source_span = node.src_byte_off != 0 || node.src_byte_len != 0;
        if hdr.file_count == 0 {
            if has_source_span || node.src_file != 0 {
                return Err(VastError::BadSourceFile {
                    node: node_idx,
                    file: node.src_file,
                    file_count: hdr.file_count,
                });
            }
            continue;
        }
        let effective_src_file = if node.src_file < hdr.file_count {
            node.src_file
        } else if is_c_internal_previous_sibling_field(node_idx, node.src_file, hdr) {
            0
        } else {
            return Err(VastError::BadSourceFile {
                node: node_idx,
                file: node.src_file,
                file_count: hdr.file_count,
            });
        };
        let file = VastFile::read_row_bytes(file_bytes, effective_src_file).ok_or(
            VastError::TooShort {
                need: layout_prefix,
                got: HEADER_LEN + node_bytes.len() + file_bytes.len(),
            },
        )?;
        if !span_in_bounds(node.src_byte_off, node.src_byte_len, file.size) {
            return Err(VastError::BadSourceSpan {
                node: node_idx,
                file: effective_src_file,
                off: node.src_byte_off,
                len: node.src_byte_len,
                file_size: file.size,
            });
        }
    }
    Ok(())
}

fn is_c_internal_previous_sibling_field(node_idx: u32, field: u32, hdr: VastHeader) -> bool {
    hdr.file_count == 1 && (field == SENTINEL || field < node_idx)
}

fn validate_tree_edges(node_bytes: &[u8], node_count: u32) -> Result<(), VastError> {
    if node_count == 0 {
        return Ok(());
    }
    let nc = node_count as usize;
    let node_need = HEADER_LEN
        .checked_add(table_byte_len(
            node_count,
            NODE_STRIDE_U32 * 4,
            HEADER_LEN + node_bytes.len(),
        )?)
        .ok_or(VastError::LengthMismatch {
            expected: usize::MAX,
            got: HEADER_LEN + node_bytes.len(),
        })?;
    for i in 0u32..node_count {
        let n = VastNode::read_row_bytes(node_bytes, i).ok_or(VastError::TooShort {
            need: node_need,
            got: HEADER_LEN + node_bytes.len(),
        })?;
        let check = |to: u32| -> Result<(), VastError> {
            if to == SENTINEL {
                return Ok(());
            }
            if (to as usize) >= nc {
                return Err(VastError::BadEdge { from: i, to });
            }
            Ok(())
        };
        check(n.first_child)?;
        check(n.next_sibling)?;
        if n.parent_idx != SENTINEL {
            check(n.parent_idx)?;
        }
    }
    Ok(())
}
