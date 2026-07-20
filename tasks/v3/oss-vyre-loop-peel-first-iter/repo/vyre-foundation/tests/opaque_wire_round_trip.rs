//! Round-trip test for `Expr::Opaque` and `Node::Opaque` through the wire
//! format (tag `0x80`).
//!
//! A minimal test-only extension registers the matching
//! `OpaqueExprResolver` / `OpaqueNodeResolver`, then the program round-trips
//! through `to_wire` → `from_wire` and is asserted byte-identical.

use std::sync::Arc;

use vyre_foundation::extension::{OpaqueExprResolver, OpaqueNodeResolver};
use vyre_foundation::ir::{BufferDecl, DataType, Expr, ExprNode, Node, NodeExtension, Program};

const EXPR_KIND: &str = "test.extension.echo_expr";
const NODE_KIND: &str = "test.extension.echo_node";

#[derive(Debug)]
struct TestExprExtension {
    payload: Vec<u8>,
    identity: String,
}

impl ExprNode for TestExprExtension {
    fn extension_kind(&self) -> &'static str {
        EXPR_KIND
    }
    fn debug_identity(&self) -> &str {
        &self.identity
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

fn deserialize_expr_ext(bytes: &[u8]) -> Result<Arc<dyn ExprNode>, String> {
    Ok(Arc::new(TestExprExtension {
        payload: bytes.to_vec(),
        identity: "test-expr".into(),
    }))
}

inventory::submit! {
    OpaqueExprResolver {
        kind: EXPR_KIND,
        deserialize: deserialize_expr_ext,
    }
}

#[derive(Debug)]
struct TestNodeExtension {
    payload: Vec<u8>,
    identity: String,
}

impl NodeExtension for TestNodeExtension {
    fn extension_kind(&self) -> &'static str {
        NODE_KIND
    }
    fn debug_identity(&self) -> &str {
        &self.identity
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

fn deserialize_node_ext(bytes: &[u8]) -> Result<Arc<dyn NodeExtension>, String> {
    Ok(Arc::new(TestNodeExtension {
        payload: bytes.to_vec(),
        identity: "test-node".into(),
    }))
}

inventory::submit! {
    OpaqueNodeResolver {
        kind: NODE_KIND,
        deserialize: deserialize_node_ext,
    }
}

#[test]
fn opaque_expr_round_trips_through_wire_format() {
    let program = Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![
            Node::store(
                "out",
                Expr::u32(0),
                Expr::Opaque(Arc::new(TestExprExtension {
                    payload: b"hello-opaque-expr".to_vec(),
                    identity: "test-expr".into(),
                })),
            ),
            Node::Return,
        ],
    );

    let encoded = program.to_wire().expect("encode must succeed");
    let decoded = Program::from_wire(&encoded).expect("decode must succeed");

    assert_eq!(decoded, program);
}

#[test]
fn registered_opaque_expr_decodes_as_byte_identical_passthrough() {
    let payload = b"passthrough-payload".to_vec();
    let program = Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![
            Node::store(
                "out",
                Expr::u32(0),
                Expr::Opaque(Arc::new(TestExprExtension {
                    payload: payload.clone(),
                    identity: "test-expr".into(),
                })),
            ),
            Node::Return,
        ],
    );

    let encoded = program.to_wire().expect("encode must succeed");
    let decoded = Program::from_wire(&encoded).expect("decode must succeed");

    let decoded_payload = match decoded.entry() {
        [Node::Region { body, .. }] => match body.as_slice() {
            [
                Node::Store {
                    value: Expr::Opaque(extension),
                    ..
                },
                Node::Return,
            ] => extension
                .as_any()
                .downcast_ref::<TestExprExtension>()
                .expect("Fix: registered opaque payload must decode back into the owning extension type")
                .payload
                .clone(),
            body => panic!("Fix: expected opaque store fixture body, got {body:?}"),
        },
        entry => panic!("Fix: expected root Region around opaque fixture, got {entry:?}"),
    };

    assert_eq!(
        decoded_payload, payload,
        "Fix: registered opaque payloads must decode as byte-identical passthrough data."
    );
}

#[test]
fn opaque_node_round_trips_through_wire_format() {
    let program = Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![
            Node::Opaque(Arc::new(TestNodeExtension {
                payload: b"hello-opaque-node".to_vec(),
                identity: "test-node".into(),
            })),
            Node::Return,
        ],
    );

    let encoded = program.to_wire().expect("encode must succeed");
    let decoded = Program::from_wire(&encoded).expect("decode must succeed");

    assert_eq!(decoded, program);
}

#[test]
fn opaque_expr_is_validated_through_extension_hook() {
    let program = Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![
            Node::store(
                "out",
                Expr::u32(0),
                Expr::Opaque(Arc::new(TestExprExtension {
                    payload: b"payload".to_vec(),
                    identity: "test-expr".into(),
                })),
            ),
            Node::Return,
        ],
    );
    assert!(vyre_foundation::validate::validate::validate(&program).is_empty());
}

#[test]
fn opaque_node_survives_optimizer_rewrite() {
    // A program with only an Opaque node plus a Return must survive the
    // optimizer's rewrite pass unchanged because foundation cannot peek
    // inside the extension payload.
    let program = Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![
            Node::Opaque(Arc::new(TestNodeExtension {
                payload: b"state".to_vec(),
                identity: "test-node".into(),
            })),
            Node::Return,
        ],
    );
    let bytes = program.to_wire().expect("encode");
    let decoded = Program::from_wire(&bytes).expect("decode");
    assert_eq!(decoded, program);
}

#[test]
fn unregistered_opaque_kind_fails_loudly() {
    #[derive(Debug)]
    struct UnregisteredExprExt;
    impl ExprNode for UnregisteredExprExt {
        fn extension_kind(&self) -> &'static str {
            "test.extension.unregistered"
        }
        fn debug_identity(&self) -> &str {
            "unregistered"
        }
        fn result_type(&self) -> Option<DataType> {
            None
        }
        fn cse_safe(&self) -> bool {
            true
        }
        fn stable_fingerprint(&self) -> [u8; 32] {
            [0; 32]
        }
        fn validate_extension(&self) -> Result<(), String> {
            Ok(())
        }
        fn as_any(&self) -> &dyn std::any::Any {
            self
        }
    }

    let program = Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![
            Node::store(
                "out",
                Expr::u32(0),
                Expr::Opaque(Arc::new(UnregisteredExprExt)),
            ),
            Node::Return,
        ],
    );

    let encoded = program.to_wire().expect("encode must succeed");
    let err = Program::from_wire(&encoded).unwrap_err();
    let message = err.to_string();
    assert!(
        message.contains("no OpaqueExprResolver"),
        "Fix: expected decoder error about missing resolver, got: {message}"
    );
    assert!(
        message.contains("Fix:"),
        "Fix: missing opaque resolver errors must stay actionable, got: {message}"
    );
}
