//! VAST layout overflow and malformed-size contracts.

use vyre_foundation::vast::{
    validate_vast, walk_postorder_indices, walk_preorder_indices, VAST_MAGIC, VAST_VERSION,
};

fn header(node_count: u32, file_count: u32, string_blob_len: u32, attr_blob_len: u32) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&VAST_MAGIC);
    bytes.extend_from_slice(&VAST_VERSION.to_le_bytes());
    bytes.extend_from_slice(&0u16.to_le_bytes());
    bytes.extend_from_slice(&node_count.to_le_bytes());
    bytes.extend_from_slice(&file_count.to_le_bytes());
    bytes.extend_from_slice(&string_blob_len.to_le_bytes());
    bytes.extend_from_slice(&attr_blob_len.to_le_bytes());
    bytes
}

#[test]
fn validate_vast_rejects_huge_header_without_panicking() {
    let bytes = header(u32::MAX, u32::MAX, u32::MAX, u32::MAX);
    let error = validate_vast(&bytes).expect_err("huge VAST layout must be rejected");
    let msg = format!("{error:?}");
    assert!(
        msg.contains("LengthMismatch"),
        "huge layout must fail as a structured length error, got {msg}"
    );
}

#[test]
fn vast_walks_reject_huge_node_count_without_panicking() {
    let preorder =
        walk_preorder_indices(&[], u32::MAX, 64).expect_err("empty node table must fail");
    let postorder =
        walk_postorder_indices(&[], u32::MAX, 64).expect_err("empty node table must fail");

    assert!(
        format!("{preorder:?}").contains("NodeTableSize")
            || format!("{preorder:?}").contains("LengthMismatch"),
        "preorder must report a structured layout error, got {preorder:?}"
    );
    assert!(
        format!("{postorder:?}").contains("NodeTableSize")
            || format!("{postorder:?}").contains("LengthMismatch"),
        "postorder must report a structured layout error, got {postorder:?}"
    );
}
