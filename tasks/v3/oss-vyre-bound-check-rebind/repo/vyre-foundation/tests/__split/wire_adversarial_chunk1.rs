// Adversarial tests for VIR0 wire-format serialization / deserialization.
//
// These tests exercise truncation, enum-tag validation, mutation survival,
// max-size stress, opaque-payload cap symmetry, and text-format resilience
// that are either uncovered or only lightly covered by the existing suite.

use std::sync::Arc;

use vyre::ir::{BufferDecl, DataType, Expr, Node, Program};
use vyre_foundation::extension::{OpaqueExprResolver, OpaqueNodeResolver};
use vyre_foundation::ir::{ExprNode, NodeExtension};
use vyre_foundation::serial::wire::MAX_OPAQUE_PAYLOAD_LEN;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn minimal_program() -> Program {
    Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![Node::Return],
    )
}

fn recompute_checksum(bytes: &mut [u8]) {
    let body = &bytes[40..];
    let digest = blake3::hash(body);
    bytes[8..40].copy_from_slice(digest.as_bytes());
}

fn corrupt_byte_and_rechecksum(bytes: &mut [u8], offset: usize, new_value: u8) {
    bytes[offset] = new_value;
    recompute_checksum(bytes);
}

/// Find the first occurrence of `pattern` in the wire body (after the 40-byte header).
fn find_in_body(bytes: &[u8], pattern: &[u8]) -> Option<usize> {
    bytes[40..]
        .windows(pattern.len())
        .position(|w| w == pattern)
        .map(|p| p + 40)
}

/// Assert that decoding either returns a structured error or (in the pathological
/// case) an `Ok` that is observably different from the original program.  Panics
/// are always treated as failures.
fn assert_decode_fails(result: std::thread::Result<Result<Program, vyre::error::Error>>, hint: &str) {
    match result {
        Ok(Ok(_)) => panic!("wire decoder accepted malformed input: {hint}"),
        Ok(Err(e)) => {
            let msg = e.to_string();
            assert!(
                msg.contains("Fix:")
                    || msg.contains("TruncatedPayload")
                    || msg.contains("InvalidDiscriminant")
                    || msg.contains("IntegrityMismatch")
                    || msg.contains("MagicMismatch")
                    || msg.contains("UnknownSchemaVersion"),
                "error must be actionable for: {hint}\ngot: {msg}"
            );
        }
        Err(_) => panic!("wire decoder panicked on: {hint}"),
    }
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

// ---------------------------------------------------------------------------
// Test-only opaque resolvers
// ---------------------------------------------------------------------------

const TEST_KIND: &str = "test.adversarial.echo";

#[derive(Debug)]
struct TestExprExt {
    payload: Vec<u8>,
}

impl ExprNode for TestExprExt {
    fn extension_kind(&self) -> &'static str {
        TEST_KIND
    }
    fn debug_identity(&self) -> &str {
        "test-expr"
    }
    fn result_type(&self) -> Option<DataType> {
        Some(DataType::U32)
    }
    fn cse_safe(&self) -> bool {
        true
    }
    fn stable_fingerprint(&self) -> [u8; 32] {
        *blake3::hash(&self.payload).as_bytes()
    }
    fn validate_extension(&self) -> Result<(), String> {
        Ok(())
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn wire_payload(&self) -> Vec<u8> {
        self.payload.clone()
    }
}

fn deserialize_expr(bytes: &[u8]) -> Result<Arc<dyn ExprNode>, String> {
    Ok(Arc::new(TestExprExt {
        payload: bytes.to_vec(),
    }))
}

inventory::submit! {
    OpaqueExprResolver {
        kind: TEST_KIND,
        deserialize: deserialize_expr,
    }
}

#[derive(Debug)]
struct TestNodeExt {
    payload: Vec<u8>,
}

impl NodeExtension for TestNodeExt {
    fn extension_kind(&self) -> &'static str {
        TEST_KIND
    }
    fn debug_identity(&self) -> &str {
        "test-node"
    }
    fn stable_fingerprint(&self) -> [u8; 32] {
        *blake3::hash(&self.payload).as_bytes()
    }
    fn validate_extension(&self) -> Result<(), String> {
        Ok(())
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn wire_payload(&self) -> Vec<u8> {
        self.payload.clone()
    }
}

fn deserialize_node(bytes: &[u8]) -> Result<Arc<dyn NodeExtension>, String> {
    if bytes.starts_with(&[0xDE, 0xAD]) {
        return Err(
            "Fix: test resolver rejects payloads starting with 0xDE 0xAD as malformed".into(),
        );
    }
    Ok(Arc::new(TestNodeExt {
        payload: bytes.to_vec(),
    }))
}

inventory::submit! {
    OpaqueNodeResolver {
        kind: TEST_KIND,
        deserialize: deserialize_node,
    }
}

// ---------------------------------------------------------------------------
// 1. Truncated / malformed wire payloads
// ---------------------------------------------------------------------------

#[test]
fn from_wire_empty_slice() {
    let result = std::panic::catch_unwind(|| Program::from_wire(&[]));
    assert_decode_fails(result, "empty slice");
}

#[test]
fn from_wire_truncated_body() {
    let bytes = minimal_program().to_wire().unwrap();
    let truncated = &bytes[..bytes.len().saturating_sub(4)];
    let result = std::panic::catch_unwind(|| Program::from_wire(truncated));
    assert_decode_fails(result, "truncated body");
}

#[test]
fn from_wire_invalid_magic() {
    let mut bytes = minimal_program().to_wire().unwrap();
    bytes[0] = b'X';
    bytes[1] = b'X';
    bytes[2] = b'X';
    bytes[3] = b'X';
    let result = std::panic::catch_unwind(|| Program::from_wire(&bytes));
    assert_decode_fails(result, "invalid magic");
}

#[test]
fn from_wire_unsupported_version() {
    let mut bytes = minimal_program().to_wire().unwrap();
    bytes[4] = 0xFF;
    bytes[5] = 0x7F;
    let result = std::panic::catch_unwind(|| Program::from_wire(&bytes));
    assert_decode_fails(result, "unsupported version");
}

#[test]
fn from_wire_invalid_data_type_tag_in_cast() {
    let program = Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![Node::store(
            "out",
            Expr::u32(0),
            Expr::cast(DataType::F32, Expr::u32(1)),
        )],
    );
    let mut bytes = program.to_wire().unwrap();
    let pattern = [0x0D, 0x0B]; // Cast tag, F32 tag
    let offset = find_in_body(&bytes, &pattern).expect("Cast+F32 pattern must exist in body");
    corrupt_byte_and_rechecksum(&mut bytes, offset + 1, 0x99);
    let result = std::panic::catch_unwind(|| Program::from_wire(&bytes));
    assert_decode_fails(result, "invalid DataType tag");
}

#[test]
fn from_wire_invalid_bin_op_tag() {
    let program = Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![Node::store(
            "out",
            Expr::u32(0),
            Expr::add(Expr::u32(1), Expr::u32(2)),
        )],
    );
    let mut bytes = program.to_wire().unwrap();
    let pattern = [0x09, 0x01]; // BinOp tag, Add tag
    let offset = find_in_body(&bytes, &pattern).expect("BinOp+Add pattern must exist");
    corrupt_byte_and_rechecksum(&mut bytes, offset + 1, 0x99);
    let result = std::panic::catch_unwind(|| Program::from_wire(&bytes));
    assert_decode_fails(result, "invalid BinOp tag");
}

#[test]
fn from_wire_invalid_un_op_tag() {
    let program = Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![Node::store(
            "out",
            Expr::u32(0),
            Expr::negate(Expr::u32(1)),
        )],
    );
    let mut bytes = program.to_wire().unwrap();
    let pattern = [0x0A, 0x01]; // UnOp tag, Negate tag
    let offset = find_in_body(&bytes, &pattern).expect("UnOp+Negate pattern must exist");
    corrupt_byte_and_rechecksum(&mut bytes, offset + 1, 0x99);
    let result = std::panic::catch_unwind(|| Program::from_wire(&bytes));
    assert_decode_fails(result, "invalid UnOp tag");
}

#[test]
fn from_wire_invalid_atomic_op_tag() {
    let program = Program::wrapped(
        vec![BufferDecl::read_write("out", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![Node::store(
            "out",
            Expr::u32(0),
            Expr::atomic_add("out", Expr::u32(0), Expr::u32(1)),
        )],
    );
    let mut bytes = program.to_wire().unwrap();
    let pattern = [0x0E, 0x01]; // Atomic tag, Add tag
    let offset = find_in_body(&bytes, &pattern).expect("Atomic+Add pattern must exist");
    corrupt_byte_and_rechecksum(&mut bytes, offset + 1, 0x99);
    let result = std::panic::catch_unwind(|| Program::from_wire(&bytes));
    assert_decode_fails(result, "invalid AtomicOp tag");
}

#[test]
fn from_wire_node_count_exceeds_available_bytes() {
    let mut bytes = minimal_program().to_wire().unwrap();
    // Craft a body that claims 1000 nodes but only provides 10 trailing bytes.
    let mut new_body = Vec::new();
    put_leb_u64(&mut new_body, 1000);
    new_body.extend_from_slice(&[0xFF; 10]);
    bytes.truncate(40);
    bytes.extend_from_slice(&new_body);
    recompute_checksum(&mut bytes);
    let result = std::panic::catch_unwind(|| Program::from_wire(&bytes));
    assert_decode_fails(result, "node count exceeds available bytes");
}

// ---------------------------------------------------------------------------
// 2. Round-trip identity under mutation
// ---------------------------------------------------------------------------

#[test]
fn roundtrip_single_byte_body_mutation_fails() {
    // Mutate a single byte in the body WITHOUT updating the BLAKE3 checksum.
    // The decoder must reject the tampered payload via IntegrityMismatch.
    let bytes = minimal_program().to_wire().unwrap();
    let body_start = 40;
    let body_len = bytes.len() - body_start;
    for offset in [0, body_len / 2, body_len - 1] {
        let mut mutated = bytes.clone();
        let abs_offset = body_start + offset;
        mutated[abs_offset] = mutated[abs_offset].wrapping_add(1);
        // Do NOT recompute checksum  -  we are testing checksum integrity.
        let result = std::panic::catch_unwind(|| Program::from_wire(&mutated));
        assert_decode_fails(result, "single byte mutation without checksum update");
    }
}

#[test]
fn roundtrip_mutated_buffer_count_field() {
    let program = Program::wrapped(
        vec![
            BufferDecl::output("a", 0, DataType::U32).with_count(1),
            BufferDecl::output("b", 1, DataType::U32).with_count(1),
        ],
        [1, 1, 1],
        vec![Node::Return],
    );
    let mut bytes = program.to_wire().unwrap();
    let body = &bytes[40..];
    let meta_offset = body
        .windows(9)
        .position(|w| w == b"VYRE-META")
        .expect("VYRE-META must exist in body");
    // After VYRE-META: entry_op_id tag (1) + workgroup_size (12) + non_composable (1)
    let buffer_count_offset = 40 + meta_offset + 9 + 1 + 12 + 1;
    assert_eq!(
        bytes[buffer_count_offset], 0x02,
        "expected buffer count LEB == 2"
    );
    corrupt_byte_and_rechecksum(&mut bytes, buffer_count_offset, 100);
    let result = std::panic::catch_unwind(|| Program::from_wire(&bytes));
    assert_decode_fails(result, "mutated buffer count");
}

// ---------------------------------------------------------------------------
// 3. Max-size stress
// ---------------------------------------------------------------------------

#[test]
fn max_nodes_stress_roundtrip_preserves_fingerprint() {
    let nodes: Vec<Node> = (0..100_000).map(|_| Node::Return).collect();
    let program = Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        nodes,
    );
    let wire = program.to_wire().expect("max-node program must encode");
    let decoded = Program::from_wire(&wire).expect("max-node program must decode");
    assert_eq!(
        decoded.fingerprint(),
        program.fingerprint(),
        "fingerprint must be stable across max-node round-trip"
    );
}

#[test]
fn max_buffers_stress_roundtrip_preserves_fingerprint() {
    let buffers: Vec<BufferDecl> = (0..16_384u32)
        .map(|i| BufferDecl::output("b", i, DataType::U32).with_count(1))
        .collect();
    let program = Program::wrapped(buffers, [1, 1, 1], vec![Node::Return]);
    let wire = program.to_wire().expect("max-buffer program must encode");
    let decoded = Program::from_wire(&wire).expect("max-buffer program must decode");
    assert_eq!(
        decoded.fingerprint(),
        program.fingerprint(),
        "fingerprint must be stable across max-buffer round-trip"
    );
}

// ---------------------------------------------------------------------------
// 4. Opaque extension payloads
// ---------------------------------------------------------------------------

#[test]
fn opaque_node_empty_payload_roundtrips() {
    let program = Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![Node::Opaque(Arc::new(TestNodeExt { payload: vec![] }))],
    );
    let wire = program.to_wire().unwrap();
    let decoded = Program::from_wire(&wire).unwrap();
    assert_eq!(decoded, program);
}

#[test]
fn opaque_expr_empty_payload_roundtrips() {
    let program = Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![Node::store(
            "out",
            Expr::u32(0),
            Expr::Opaque(Arc::new(TestExprExt { payload: vec![] })),
        )],
    );
    let wire = program.to_wire().unwrap();
    let decoded = Program::from_wire(&wire).unwrap();
    assert_eq!(decoded, program);
}

#[test]
fn opaque_payload_encoder_decoder_cap_symmetry() {
    // Encoder and decoder must enforce the same opaque payload cap. An
    // oversized payload must fail before wire materialization so invalid
    // extension data cannot be persisted and rejected only later.
    let oversized = vec![0u8; MAX_OPAQUE_PAYLOAD_LEN + 1];
    let program = Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![Node::Opaque(Arc::new(TestNodeExt {
            payload: oversized,
        }))],
    );
    let error = program
        .to_wire()
        .expect_err("oversized opaque payload must fail at encode");
    assert!(
        error.to_string().contains("opaque node payload")
            && error.to_string().contains(&MAX_OPAQUE_PAYLOAD_LEN.to_string()),
        "oversized opaque payload encode error must name the capped field and limit, got {error}"
    );
}
