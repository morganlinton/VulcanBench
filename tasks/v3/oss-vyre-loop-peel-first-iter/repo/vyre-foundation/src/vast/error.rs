//! Structured VAST validation errors.

/// Structured validation failure (never panics on random bytes).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum VastError {
    /// Buffer shorter than required for the operation.
    #[error("VAST buffer too short: need {need} bytes, got {got}")]
    TooShort {
        /// Minimum bytes required.
        need: usize,
        /// Bytes available.
        got: usize,
    },
    /// First four bytes are not `VAST`.
    #[error("bad VAST magic: {0:?}")]
    BadMagic([u8; 4]),
    /// Unsupported [`crate::vast::VastHeader::version`].
    #[error("unsupported VAST version: {0}")]
    UnsupportedVersion(u16),
    /// Total length does not match header-derived layout.
    #[error("VAST length mismatch: expected {expected} bytes, got {got}")]
    LengthMismatch {
        /// Expected total byte length.
        expected: usize,
        /// Actual buffer length.
        got: usize,
    },
    /// `first_child` / `next_sibling` / `parent_idx` edge is out of range.
    #[error("bad VAST edge from node {from} to {to}")]
    BadEdge {
        /// Source node index.
        from: u32,
        /// Target index that was invalid.
        to: u32,
    },
    /// File metadata path points outside the string blob.
    #[error(
        "bad VAST file path for file {file}: off {off} len {len} exceeds string blob {string_blob_len}"
    )]
    BadFilePath {
        /// File table index.
        file: u32,
        /// Byte offset into the string blob.
        off: u32,
        /// Byte length from `off`.
        len: u32,
        /// Total string blob length.
        string_blob_len: u32,
    },
    /// Node source file index is out of range.
    #[error("bad VAST source file: node {node} references file {file} of {file_count}")]
    BadSourceFile {
        /// Node index.
        node: u32,
        /// Referenced file index.
        file: u32,
        /// Number of file metadata rows in the VAST.
        file_count: u32,
    },
    /// Node source span points outside its source file size.
    #[error(
        "bad VAST source span: node {node} file {file} off {off} len {len} exceeds file size {file_size}"
    )]
    BadSourceSpan {
        /// Node index.
        node: u32,
        /// Referenced file index.
        file: u32,
        /// Byte offset into the source file.
        off: u32,
        /// Byte length from `off`.
        len: u32,
        /// Source file byte length.
        file_size: u32,
    },
    /// Node attribute span points outside `attr_blob`.
    #[error("bad VAST attr span: node {node} off {off} len {len} exceeds attr blob {attr_blob_len}")]
    BadAttrSpan {
        /// Node index.
        node: u32,
        /// Byte offset into `attr_blob`.
        off: u32,
        /// Byte length from `off`.
        len: u32,
        /// Total attribute blob length.
        attr_blob_len: u32,
    },
    /// Host walk stack exceeded `max_stack` (pathological graph).
    #[error("VAST walk stack overflow: exceeded cap {cap}")]
    StackOverflow {
        /// Cap that was hit.
        cap: usize,
    },
    /// Node table byte length does not match `node_count`.
    #[error("VAST node table size mismatch: expected {expected} bytes, got {got}")]
    NodeTableSize {
        /// Expected bytes (`node_count * 40`).
        expected: usize,
        /// Actual `node_bytes.len()`.
        got: usize,
    },
}
