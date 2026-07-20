//! Failure-oriented tests for VAST (packed AST) wire-format invariants.
//!
//! Every malformed input must produce a structured [`VastError`] rather than
//! panicking or returning a fake success path.

use vyre_foundation::vast::{
    pack_spine_vast, validate_vast, walk_postorder_indices, walk_preorder_indices, VastError,
    VastFile, VastHeader, VastNode, HEADER_LEN, NODE_STRIDE_U32, SENTINEL, VAST_MAGIC,
    VAST_VERSION,
};

fn pack_vast_with_file_and_blobs(
    node: VastNode,
    file: VastFile,
    string_blob: &[u8],
    attr_blob: &[u8],
) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&VAST_MAGIC);
    bytes.extend_from_slice(&VAST_VERSION.to_le_bytes());
    bytes.extend_from_slice(&0u16.to_le_bytes());
    bytes.extend_from_slice(&1u32.to_le_bytes());
    bytes.extend_from_slice(&1u32.to_le_bytes());
    bytes.extend_from_slice(&(string_blob.len() as u32).to_le_bytes());
    bytes.extend_from_slice(&(attr_blob.len() as u32).to_le_bytes());
    bytes.extend_from_slice(&node.to_bytes());
    bytes.extend_from_slice(&file.path_off.to_le_bytes());
    bytes.extend_from_slice(&file.path_len.to_le_bytes());
    bytes.extend_from_slice(&file.size.to_le_bytes());
    bytes.extend_from_slice(string_blob);
    bytes.extend_from_slice(attr_blob);
    bytes
}

fn valid_node_with_spans() -> VastNode {
    VastNode {
        kind: 1,
        parent_idx: SENTINEL,
        first_child: SENTINEL,
        next_sibling: SENTINEL,
        src_file: 0,
        src_byte_off: 2,
        src_byte_len: 4,
        attr_off: 1,
        attr_len: 3,
        reserved: 0,
    }
}

fn valid_file() -> VastFile {
    VastFile {
        path_off: 0,
        path_len: 6,
        size: 16,
    }
}

#[test]
fn too_short_buffer_is_rejected() {
    let bytes = vec![0x56, 0x41, 0x53, 0x54]; // "VAST" but no header
    let err = VastHeader::decode(&bytes).unwrap_err();
    assert!(
        matches!(
            err,
            VastError::TooShort {
                need: HEADER_LEN,
                got: 4
            }
        ),
        "short buffer must be rejected, got {err:?}"
    );
}

#[test]
fn bad_magic_is_rejected() {
    let bytes = vec![b'X'; HEADER_LEN];
    let err = VastHeader::decode(&bytes).unwrap_err();
    assert!(
        matches!(err, VastError::BadMagic([0x58, 0x58, 0x58, 0x58])),
        "bad magic must be rejected, got {err:?}"
    );
}

#[test]
fn unsupported_version_is_rejected() {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&VAST_MAGIC);
    bytes.extend_from_slice(&9999u16.to_le_bytes());
    bytes.extend_from_slice(&0u16.to_le_bytes()); // source_lang
    bytes.extend_from_slice(&0u32.to_le_bytes()); // node_count
    bytes.extend_from_slice(&0u32.to_le_bytes()); // file_count
    bytes.extend_from_slice(&0u32.to_le_bytes()); // string_blob_len
    bytes.extend_from_slice(&0u32.to_le_bytes()); // attr_blob_len
    let err = VastHeader::decode(&bytes).unwrap_err();
    assert!(
        matches!(err, VastError::UnsupportedVersion(9999)),
        "unsupported version must be rejected, got {err:?}"
    );
}

#[test]
fn length_mismatch_is_rejected() {
    let mut bytes = pack_spine_vast(&[1]);
    bytes.pop(); // truncate by one byte
    let err = validate_vast(&bytes).unwrap_err();
    assert!(
        matches!(err, VastError::LengthMismatch { expected, got } if expected == bytes.len() + 1 && got == bytes.len()),
        "length mismatch must be rejected, got {err:?}"
    );
}

#[test]
fn bad_edge_out_of_range_child_is_rejected() {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&VAST_MAGIC);
    bytes.extend_from_slice(&VAST_VERSION.to_le_bytes());
    bytes.extend_from_slice(&0u16.to_le_bytes());
    bytes.extend_from_slice(&1u32.to_le_bytes()); // node_count = 1
    bytes.extend_from_slice(&0u32.to_le_bytes());
    bytes.extend_from_slice(&0u32.to_le_bytes());
    bytes.extend_from_slice(&0u32.to_le_bytes());
    // Node 0 with first_child = 5 (out of range)
    let row = [
        0u32,     // kind
        SENTINEL, // parent
        5u32,     // first_child (invalid)
        SENTINEL, // next_sibling
        0, 0, 0, 0, 0, 0,
    ];
    for word in row {
        bytes.extend_from_slice(&word.to_le_bytes());
    }
    let err = validate_vast(&bytes).unwrap_err();
    assert!(
        matches!(err, VastError::BadEdge { from: 0, to: 5 }),
        "bad edge must be rejected, got {err:?}"
    );
}

#[test]
fn bad_edge_out_of_range_sibling_is_rejected() {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&VAST_MAGIC);
    bytes.extend_from_slice(&VAST_VERSION.to_le_bytes());
    bytes.extend_from_slice(&0u16.to_le_bytes());
    bytes.extend_from_slice(&1u32.to_le_bytes());
    bytes.extend_from_slice(&0u32.to_le_bytes());
    bytes.extend_from_slice(&0u32.to_le_bytes());
    bytes.extend_from_slice(&0u32.to_le_bytes());
    let row = [
        0u32, SENTINEL, SENTINEL, 99u32, // next_sibling (invalid)
        0, 0, 0, 0, 0, 0,
    ];
    for word in row {
        bytes.extend_from_slice(&word.to_le_bytes());
    }
    let err = validate_vast(&bytes).unwrap_err();
    assert!(
        matches!(err, VastError::BadEdge { from: 0, to: 99 }),
        "bad sibling edge must be rejected, got {err:?}"
    );
}

#[test]
fn bad_edge_out_of_range_parent_is_rejected() {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&VAST_MAGIC);
    bytes.extend_from_slice(&VAST_VERSION.to_le_bytes());
    bytes.extend_from_slice(&0u16.to_le_bytes());
    bytes.extend_from_slice(&1u32.to_le_bytes());
    bytes.extend_from_slice(&0u32.to_le_bytes());
    bytes.extend_from_slice(&0u32.to_le_bytes());
    bytes.extend_from_slice(&0u32.to_le_bytes());
    let row = [
        0u32, 7u32, // parent_idx (invalid, not SENTINEL)
        SENTINEL, SENTINEL, 0, 0, 0, 0, 0, 0,
    ];
    for word in row {
        bytes.extend_from_slice(&word.to_le_bytes());
    }
    let err = validate_vast(&bytes).unwrap_err();
    assert!(
        matches!(err, VastError::BadEdge { from: 0, to: 7 }),
        "bad parent edge must be rejected, got {err:?}"
    );
}

#[test]
fn file_path_outside_string_blob_is_rejected() {
    let bytes = pack_vast_with_file_and_blobs(
        valid_node_with_spans(),
        VastFile {
            path_off: 4,
            path_len: 8,
            size: 16,
        },
        b"main.c",
        b"attrs",
    );
    let err = validate_vast(&bytes).unwrap_err();
    assert!(
        matches!(
            err,
            VastError::BadFilePath {
                file: 0,
                off: 4,
                len: 8,
                string_blob_len: 6
            }
        ),
        "bad file path span must be rejected, got {err:?}"
    );
}

#[test]
fn source_file_outside_file_table_is_rejected() {
    let mut node = valid_node_with_spans();
    node.src_file = 2;
    let bytes = pack_vast_with_file_and_blobs(node, valid_file(), b"main.c", b"attrs");
    let err = validate_vast(&bytes).unwrap_err();
    assert!(
        matches!(
            err,
            VastError::BadSourceFile {
                node: 0,
                file: 2,
                file_count: 1
            }
        ),
        "bad source file index must be rejected, got {err:?}"
    );
}

#[test]
fn source_span_outside_file_size_is_rejected() {
    let mut node = valid_node_with_spans();
    node.src_byte_off = 14;
    node.src_byte_len = 4;
    let bytes = pack_vast_with_file_and_blobs(node, valid_file(), b"main.c", b"attrs");
    let err = validate_vast(&bytes).unwrap_err();
    assert!(
        matches!(
            err,
            VastError::BadSourceSpan {
                node: 0,
                file: 0,
                off: 14,
                len: 4,
                file_size: 16
            }
        ),
        "bad source span must be rejected, got {err:?}"
    );
}

#[test]
fn attr_span_outside_attr_blob_is_rejected() {
    let mut node = valid_node_with_spans();
    node.attr_off = 4;
    node.attr_len = 2;
    let bytes = pack_vast_with_file_and_blobs(node, valid_file(), b"main.c", b"attrs");
    let err = validate_vast(&bytes).unwrap_err();
    assert!(
        matches!(
            err,
            VastError::BadAttrSpan {
                node: 0,
                off: 4,
                len: 2,
                attr_blob_len: 5
            }
        ),
        "bad attr span must be rejected, got {err:?}"
    );
}

#[test]
fn valid_file_source_and_attr_spans_are_accepted() {
    let bytes =
        pack_vast_with_file_and_blobs(valid_node_with_spans(), valid_file(), b"main.c", b"attrs");
    let hdr = validate_vast(&bytes).expect("valid VAST spans must be accepted");
    assert_eq!(hdr.file_count, 1);
    assert_eq!(hdr.string_blob_len, 6);
    assert_eq!(hdr.attr_blob_len, 5);
}

#[test]
fn spine_pre_and_post_order_are_stable() {
    let bytes = pack_spine_vast(&[1, 2, 3]);
    let header = validate_vast(&bytes).expect("valid spine VAST must validate");
    assert_eq!(header.node_count, 3);

    let node_len = 3usize * NODE_STRIDE_U32 * 4;
    let node_bytes = &bytes[HEADER_LEN..HEADER_LEN + node_len];
    assert_eq!(
        walk_preorder_indices(node_bytes, 3, 64).unwrap(),
        vec![0, 1, 2]
    );
    assert_eq!(
        walk_postorder_indices(node_bytes, 3, 64).unwrap(),
        vec![2, 1, 0]
    );
}

#[test]
fn stack_overflow_on_deep_preorder_walk_is_rejected() {
    // Node 0 has 5 children (1..5) via next_sibling; pushing them all exceeds max_stack=4.
    let n = 6u32;
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&VAST_MAGIC);
    bytes.extend_from_slice(&VAST_VERSION.to_le_bytes());
    bytes.extend_from_slice(&0u16.to_le_bytes());
    bytes.extend_from_slice(&n.to_le_bytes());
    bytes.extend_from_slice(&0u32.to_le_bytes());
    bytes.extend_from_slice(&0u32.to_le_bytes());
    bytes.extend_from_slice(&0u32.to_le_bytes());
    // Node 0: first_child = 1
    let row0 = [0u32, SENTINEL, 1u32, SENTINEL, 0, 0, 0, 0, 0, 0];
    for word in row0 {
        bytes.extend_from_slice(&word.to_le_bytes());
    }
    // Nodes 1..5: linked via next_sibling, no children
    for i in 1..=5 {
        let ns = if i < 5 { i + 1 } else { SENTINEL };
        let row = [i, SENTINEL, SENTINEL, ns, 0, 0, 0, 0, 0, 0];
        for word in row {
            bytes.extend_from_slice(&word.to_le_bytes());
        }
    }
    let node_len = (n as usize) * NODE_STRIDE_U32 * 4;
    let node_bytes = &bytes[HEADER_LEN..HEADER_LEN + node_len];
    let err = walk_preorder_indices(node_bytes, n, 4).unwrap_err();
    assert!(
        matches!(err, VastError::StackOverflow { cap: 4 }),
        "walk with branching factor > max_stack must overflow, got {err:?}"
    );
}

#[test]
fn stack_overflow_on_deep_postorder_walk_is_rejected() {
    // Node 0 has 5 children (1..5) via next_sibling; pushing them all exceeds max_stack=4.
    let n = 6u32;
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&VAST_MAGIC);
    bytes.extend_from_slice(&VAST_VERSION.to_le_bytes());
    bytes.extend_from_slice(&0u16.to_le_bytes());
    bytes.extend_from_slice(&n.to_le_bytes());
    bytes.extend_from_slice(&0u32.to_le_bytes());
    bytes.extend_from_slice(&0u32.to_le_bytes());
    bytes.extend_from_slice(&0u32.to_le_bytes());
    let row0 = [0u32, SENTINEL, 1u32, SENTINEL, 0, 0, 0, 0, 0, 0];
    for word in row0 {
        bytes.extend_from_slice(&word.to_le_bytes());
    }
    for i in 1..=5 {
        let ns = if i < 5 { i + 1 } else { SENTINEL };
        let row = [i, SENTINEL, SENTINEL, ns, 0, 0, 0, 0, 0, 0];
        for word in row {
            bytes.extend_from_slice(&word.to_le_bytes());
        }
    }
    let node_len = (n as usize) * NODE_STRIDE_U32 * 4;
    let node_bytes = &bytes[HEADER_LEN..HEADER_LEN + node_len];
    let err = walk_postorder_indices(node_bytes, n, 4).unwrap_err();
    assert!(
        matches!(err, VastError::StackOverflow { cap: 4 }),
        "postorder walk with branching factor > max_stack must overflow, got {err:?}"
    );
}

#[test]
fn node_table_size_mismatch_is_rejected() {
    let node_bytes = vec![0u8; 4]; // too short for any node
    let err = walk_preorder_indices(&node_bytes, 1, 64).unwrap_err();
    assert!(
        matches!(
            err,
            VastError::NodeTableSize {
                expected: 40,
                got: 4
            }
        ),
        "node table size mismatch must be rejected, got {err:?}"
    );
}

#[test]
fn total_byte_len_computes_expected_size() {
    let hdr = VastHeader {
        version: VAST_VERSION,
        source_lang: 0,
        node_count: 3,
        file_count: 0,
        string_blob_len: 0,
        attr_blob_len: 0,
    };
    let expected = HEADER_LEN + 3 * NODE_STRIDE_U32 * 4;
    assert_eq!(
        hdr.total_byte_len(),
        Some(expected),
        "total_byte_len must match header + node table"
    );
}

#[test]
fn zero_nodes_empty_blob_validates() {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&VAST_MAGIC);
    bytes.extend_from_slice(&VAST_VERSION.to_le_bytes());
    bytes.extend_from_slice(&0u16.to_le_bytes());
    bytes.extend_from_slice(&0u32.to_le_bytes());
    bytes.extend_from_slice(&0u32.to_le_bytes());
    bytes.extend_from_slice(&0u32.to_le_bytes());
    bytes.extend_from_slice(&0u32.to_le_bytes());
    let hdr = validate_vast(&bytes).unwrap();
    assert_eq!(hdr.node_count, 0);
}

#[test]
fn vast_error_cloned_equality() {
    let a = VastError::TooShort { need: 10, got: 5 };
    let b = a.clone();
    assert_eq!(a, b, "VastError must be Clone and Eq");
}
