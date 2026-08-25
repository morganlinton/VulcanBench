// Stable binary IR wire format for serialized IR programs.

use crate::ir::{BufferDecl, DataType, Expr, Node, Program};

/// The `decode` module.
pub mod decode;
/// The `encode` module.
pub mod encode;
/// The `framing` module.
pub mod framing;
/// The `tags` module.
pub mod tags;

/// Maximum buffers accepted from one IR wire-format program.
///
/// I10 requires bounded allocation before validating semantics. This limit
/// rejects hostile wire blobs before allocating the buffer table.
pub const MAX_BUFFERS: usize = 16_384;

/// Maximum statement nodes accepted from any single wire-format node list.
///
/// I10 requires node vectors to be bounded before allocation; nested lists are
/// each checked against this budget as they are decoded.
pub const MAX_NODES: usize = 1_000_000;

/// Maximum call arguments accepted from one wire-format call expression.
///
/// I10 requires expression argument vectors to be bounded before allocation.
pub const MAX_ARGS: usize = 4_096;

/// Maximum tensor rank (dimension count) accepted from a wire-format
/// `DataType::TensorShaped` shape.
///
/// I10 requires the shape vector to be bounded before the decoder reads each
/// dimension. Real tensors are single-digit rank (the inline `SmallVec`
/// capacity is 4); this ceiling is generous enough never to reject a real
/// program yet bounds the shape allocation to a small fixed size instead of the
/// transitive `MAX_PROGRAM_BYTES / 4` worst case. Makes the "rank-limited shape"
/// contract on `DataType::TensorShaped` actually enforced.
pub const MAX_TENSOR_RANK: usize = 4_096;

/// Maximum device-mesh axis count accepted from a wire-format
/// `DataType::DeviceMesh`.
///
/// I10 requires the axis vector to be bounded before the decoder reads each
/// axis. Real meshes have a handful of axes (data/model/pipeline parallelism);
/// this ceiling never rejects a real program yet bounds the allocation.
pub const MAX_MESH_AXES: usize = 4_096;

/// Maximum UTF-8 string length accepted from the IR wire format.
///
/// I10 bounds allocation for names and operation identifiers carried by
/// attacker-controlled wire bytes.
pub const MAX_STRING_LEN: usize = 1 << 20;

/// Maximum opaque payload length accepted from the IR wire format.
///
/// I10 bounds allocation for extension-defined `Expr::Opaque` and
/// `Node::Opaque` payloads carried by attacker-controlled wire bytes.
/// Must match the encoder limit in `put_node.rs` and `put_expr.rs`.
pub const MAX_OPAQUE_PAYLOAD_LEN: usize = MAX_ARGS * 1024;

/// Maximum recursive decode depth for the IR wire format.
///
/// The limit is applied to the **shared** recursion counter in `Reader`
/// that `Reader::node` and `Reader::expr` both increment on entry and
/// decrement on exit. A hostile blob cannot evade the cap by alternating
/// statement and expression nesting  -  every nested decode call, whether it
/// descends into a `Node::If`/`Loop`/`Block` body or into a nested
/// [`Expr`] argument tree, counts against the same budget. Depth ≥
/// `MAX_DECODE_DEPTH` is rejected with a `Fix:`-prefixed error before any
/// stack frame is pushed, preventing stack-overflow `DoS` from a blob that
/// nests `Block(Block(... Block(...) ...))` a million times deep.
///
/// Covers audit L.1.35 (HIGH).
pub const MAX_DECODE_DEPTH: u32 = 64;

/// Hard ceiling on the size of a single wire-encoded Program in bytes.
///
/// The framing layer rejects larger blobs before any decode allocation so
/// attacker-controlled input cannot force unbounded memory growth.
pub const MAX_PROGRAM_BYTES: usize = 64 * 1024 * 1024;

pub(crate) struct Reader<'a> {
    pub bytes: &'a [u8],
    pub pos: usize,
    /// Current recursion depth on the decode call stack. Incremented by
    /// every `node()` and `expr()` call and compared against
    /// [`MAX_DECODE_DEPTH`] before any nested decode proceeds.
    pub depth: u32,
}

impl Program {
    /// Serialize this IR program into the stable `VIR0` IR wire format.
    ///
    /// # Errors
    ///
    /// Returns [`crate::error::Error::WireFormatValidation`] when a count
    /// cannot be represented in the versioned wire format or when a public
    /// enum variant has no registered stable wire tag. The `message` field
    /// carries the actionable diagnostic prose including a `Fix:` hint.
    #[inline]
    #[must_use]
    pub fn to_wire(&self) -> Result<Vec<u8>, crate::error::Error> {
        encode::to_wire(self).map_err(wire_err)
    }

    /// Serialize this IR program into the stable `VIR0` IR wire format,
    /// appending to an existing buffer.
    ///
    /// # Errors
    ///
    /// Returns [`crate::error::Error::WireFormatValidation`] when a count
    /// cannot be represented in the versioned wire format or when a public
    /// enum variant has no registered stable wire tag. The `message` field
    /// carries the actionable diagnostic prose including a `Fix:` hint.
    #[inline]
    pub fn to_wire_into(&self, dst: &mut Vec<u8>) -> Result<(), crate::error::Error> {
        encode::to_wire_into(self, dst).map_err(wire_err)
    }

    /// Serialize this IR program into bytes.
    ///
    /// This compatibility wrapper preserves the pre-`to_wire` API name.
    ///
    /// On an encoding error, an empty vector is returned after logging the
    /// failure. Use [`Program::to_wire`] when the caller needs to handle the
    /// error explicitly.
    #[must_use]
    #[inline]
    pub fn to_bytes(&self) -> Vec<u8> {
        match self.to_wire() {
            Ok(bytes) => bytes,
            Err(error) => {
                tracing::error!(
                    error = %error,
                    "Program::to_bytes: wire encoding failed; returning empty bytes. \
                     Fix: call Program::to_wire and handle the validation error explicitly."
                );
                Vec::new()
            }
        }
    }

    /// Deserialize an IR program from the stable `VYRE` IR wire format.
    ///
    /// # Errors
    ///
    /// Returns [`crate::error::Error::VersionMismatch`] when the
    /// payload advertises a schema version this runtime does not
    /// understand. Returns [`crate::error::Error::WireFormatValidation`]
    /// for any other decode failure  -  truncated bytes, unknown enum
    /// tag, integrity digest mismatch, or malformed structural
    /// section.
    #[inline]
    #[must_use]
    pub fn from_wire(bytes: &[u8]) -> Result<Self, crate::error::Error> {
        if bytes.len() > MAX_PROGRAM_BYTES {
            return Err(wire_err(format!(
                "Fix: wire blob is {} bytes, exceeding the {}-byte IR framing cap. Reject this input or split the Program before serialization.",
                bytes.len(),
                MAX_PROGRAM_BYTES
            )));
        }
        // The version field is validated before the string-based
        // decoder so that an out-of-range version surfaces as the
        // typed `VersionMismatch` variant instead of being absorbed
        // into the generic `WireFormatValidation` bucket. Tooling
        // that hangs off the diagnostic code `E-WIRE-VERSION` relies
        // on this distinction.
        if bytes.len() >= framing::MAGIC.len() + 2
            && &bytes[..framing::MAGIC.len()] == framing::MAGIC
        {
            let version = u16::from_le_bytes([bytes[4], bytes[5]]);
            if version != framing::WIRE_FORMAT_VERSION {
                return Err(crate::error::Error::VersionMismatch {
                    expected: u32::from(framing::WIRE_FORMAT_VERSION),
                    found: u32::from(version),
                });
            }
        }
        decode::from_wire(bytes).map_err(wire_err)
    }

    /// Deserialize an IR program from bytes.
    ///
    /// This compatibility wrapper preserves the pre-`from_wire` API name.
    ///
    /// # Errors
    ///
    /// Returns the same actionable decode errors as [`Program::from_wire`].
    #[inline]
    #[must_use]
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, crate::error::Error> {
        Self::from_wire(bytes)
    }

    /// Stable content hash of this Program, used as a cache identity.
    ///
    /// Computed as BLAKE3 of the canonical wire-format encoding. This is the
    /// exact-match identity for persistent-cache consumers that need a
    /// deterministic key per Program without re-implementing canonicalization.
    /// On canonical wire-encoding failure, the value is a domain-separated
    /// error digest rather than an all-zero sentinel, so malformed programs do
    /// not collapse into the same cache identity.
    #[must_use]
    pub fn content_hash(&self) -> [u8; 32] {
        self.fingerprint()
    }
}

/// Wrap an internal wire-format error string in the typed [`crate::error::Error`]
/// so every public boundary of this module returns a structured variant
/// callers can match on.
fn wire_err(message: String) -> crate::error::Error {
    crate::error::Error::WireFormatValidation { message }
}

/// Append stable VIR0 wire bytes for a [`DataType`] (tag + any payload) into
/// `buf`. Used by disk-cache fingerprinting where `Debug` output would be
/// the wrong contract.
///
/// # Errors
///
/// Returns a wire-format diagnostic when `value` contains a datatype variant
/// without a stable tag or a payload that cannot fit the VIR0 encoding.
pub fn append_data_type_fingerprint(buf: &mut Vec<u8>, value: &DataType) -> Result<(), String> {
    tags::data_type_tag::put_data_type(buf, value).map_err(String::from)
}

/// Append stable VIR0 wire bytes for a `Node` statement list (count + each
/// node). Matches the statement encoding used in full program wire (`to_wire`)
/// (without the file envelope, metadata, or buffer table).
///
/// # Errors
///
/// Returns a wire-format diagnostic when the node list or any nested payload
/// cannot be represented in VIR0.
pub fn append_node_list_fingerprint(buf: &mut Vec<u8>, nodes: &[Node]) -> Result<(), String> {
    encode::put_nodes(buf, nodes).map_err(String::from)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{BufferAccess, BufferDecl, DataType, Node, Program};

    #[test]
    #[inline]
    pub(crate) fn to_bytes_returns_empty_on_wire_error() {
        let long_name = "x".repeat(MAX_STRING_LEN + 1);
        let program = Program::wrapped(
            vec![BufferDecl::storage(
                &long_name,
                0,
                BufferAccess::ReadOnly,
                DataType::U32,
            )],
            [1, 1, 1],
            vec![],
        );
        assert!(program.to_wire().is_err());
        assert!(program.to_bytes().is_empty());
    }

    /// EDGE-001 regression: `MAX_DECODE_DEPTH` covers **both** Node and Expr
    /// recursion through the same counter. A blob that nests statement
    /// bodies past the depth limit must be rejected at decode time,
    /// preventing stack-overflow DoS on untrusted input.
    ///
    /// The test runs on a dedicated thread with an 8 MiB stack because
    /// the encode/decode walk down a `MAX_DECODE_DEPTH + 1`-deep Block
    /// tree uses ~3–4× the native frames the default 2 MiB test stack
    /// allocates. Without the explicit stack, the test itself
    /// stack-overflows before the decode guard ever fires  -  masking
    /// the real assertion.
    #[test]
    pub(crate) fn decode_depth_cap_rejects_deeply_nested_blocks() {
        std::thread::Builder::new()
            .stack_size(8 * 1024 * 1024)
            .spawn(run_decode_depth_cap)
            .expect("Fix: spawn test worker")
            .join()
            .expect("Fix: decode-depth-cap worker panicked");
    }

    fn run_decode_depth_cap() {
        // Build the nested program iteratively so the test thread's
        // stack only owns the tree, not a recursion chain the depth
        // of the tree.
        let mut inner = Node::Block(vec![]);
        for _ in 0..MAX_DECODE_DEPTH {
            inner = Node::Block(vec![inner]);
        }
        let program = Program::wrapped(
            vec![BufferDecl::read_write("out", 0, DataType::U32)],
            [1, 1, 1],
            vec![inner],
        );
        let bytes = program
            .to_wire()
            .expect("Fix: building a (MAX_DEPTH+1)-nested program must still encode");
        let decoded = Program::from_wire(&bytes);
        assert!(
            decoded.is_err(),
            "decoding a program deeper than MAX_DECODE_DEPTH must fail; got Ok"
        );
        let err = decoded.unwrap_err().to_string();
        assert!(
            err.contains("Fix:"),
            "depth-exceed error must carry a `Fix:` hint, got: {err}"
        );
    }
}

/// OPAQUE-001 regression: encoder and decoder must agree on the
/// maximum opaque payload length. A payload at MAX_OPAQUE_PAYLOAD_LEN
/// must encode; a payload one byte larger must fail at encode time.
#[test]
pub(crate) fn opaque_payload_limit_is_symmetric() {
    use crate::ir::{Expr, ExprNode};
    use std::any::Any;

    #[derive(Debug)]
    struct BigOpaque(Vec<u8>);
    impl ExprNode for BigOpaque {
        fn extension_kind(&self) -> &'static str {
            "test.big"
        }
        fn debug_identity(&self) -> &str {
            "test.big"
        }
        fn result_type(&self) -> Option<DataType> {
            Some(DataType::U32)
        }
        fn cse_safe(&self) -> bool {
            false
        }
        fn stable_fingerprint(&self) -> [u8; 32] {
            [0; 32]
        }
        fn validate_extension(&self) -> Result<(), String> {
            Ok(())
        }
        fn as_any(&self) -> &dyn Any {
            self
        }
        fn wire_payload(&self) -> Vec<u8> {
            self.0.clone()
        }
    }

    // At the limit: must encode successfully.
    let expr_ok = Expr::opaque(BigOpaque(vec![0u8; MAX_OPAQUE_PAYLOAD_LEN]));
    let program_ok = Program::wrapped(
        vec![BufferDecl::read_write("out", 0, DataType::U32)],
        [1, 1, 1],
        vec![Node::let_bind("_", expr_ok)],
    );
    assert!(
        program_ok.to_wire().is_ok(),
        "at-limit opaque payload ({MAX_OPAQUE_PAYLOAD_LEN} bytes) must encode"
    );

    // One byte over: must fail at encode time.
    let expr_over = Expr::opaque(BigOpaque(vec![0u8; MAX_OPAQUE_PAYLOAD_LEN + 1]));
    let program_over = Program::wrapped(
        vec![BufferDecl::read_write("out", 0, DataType::U32)],
        [1, 1, 1],
        vec![Node::let_bind("_", expr_over)],
    );
    let err = program_over
        .to_wire()
        .expect_err("opaque payload exceeding MAX_OPAQUE_PAYLOAD_LEN must fail at encode");
    let msg = err.to_string();
    assert!(
        msg.contains("MAX_OPAQUE_PAYLOAD_LEN") || msg.contains(&MAX_OPAQUE_PAYLOAD_LEN.to_string()),
        "error should mention the limit, got: {msg}"
    );
}
