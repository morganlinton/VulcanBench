//! Scalar broadcast  -  copy a single-element `src` to every slot of `dst`.
//!
//! Category A composition. The minimal broadcast case; a full
//! shape-broadcasting version (NumPy semantics) belongs in a future
//! `broadcast_shaped` function that takes source + target shapes.

use vyre::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

use crate::region::wrap_anonymous;

/// Broadcast a scalar into every element of `dst`. `n` is the target
/// element count  -  `dst` receives `n × sizeof(U32)` bytes.
#[must_use]
pub fn broadcast(src: &str, dst: &str, n: u32) -> Program {
    if n == 0 {
        return crate::builder::invalid_output_program(
            "vyre-libs::math::broadcast",
            dst,
            DataType::U32,
            "Fix: broadcast requires n > 0.".to_string(),
        );
    }
    let output = BufferDecl::output(dst, 1, DataType::U32)
        .with_count(n)
        .with_output_byte_range(0..(n as usize).saturating_mul(4));
    let body = vec![
        Node::let_bind("idx", Expr::InvocationId { axis: 0 }),
        Node::if_then(
            Expr::lt(Expr::var("idx"), Expr::u32(n)),
            vec![Node::Store {
                buffer: dst.into(),
                index: Expr::var("idx"),
                value: Expr::load(src, Expr::u32(0)),
            }],
        ),
    ];
    Program::wrapped(
        vec![
            BufferDecl::storage(src, 0, BufferAccess::ReadOnly, DataType::U32).with_count(1),
            output,
        ],
        [64, 1, 1],
        vec![wrap_anonymous("vyre-libs::math::broadcast", body)],
    )
}

inventory::submit! {
    crate::harness::OpEntry {
        id: "vyre-libs::math::broadcast",
        build: || broadcast("src", "dst", 4),
        test_inputs: Some(|| vec![vec![
            42u32.to_le_bytes().to_vec(),                       // src: scalar 42
        ]]),
        expected_output: Some(|| vec![vec![
            // Only ReadWrite buffer: dst filled with 42
            vyre_primitives::wire::pack_u32_slice(&[42u32, 42, 42, 42]),
        ]]),
        category: Some("math"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::byte_pack::{bytes_to_u32 as decode_u32_words, u32_bytes};
    use vyre_reference::value::Value;

    #[test]
    fn broadcast_single_element() {
        let program = broadcast("src", "dst", 1);
        let outputs = vyre_reference::reference_eval(
            &program,
            &[Value::from(u32_bytes(&[99u32])), Value::from(vec![0u8; 4])],
        )
        .expect("Fix: broadcast n=1 must execute");
        let actual = decode_u32_words(&outputs[0].to_bytes());
        assert_eq!(actual, vec![99u32]);
    }

    #[test]
    fn broadcast_zero_elements_should_trap_or_be_consistent() {
        let program = broadcast("src", "dst", 0);
        let error = vyre_reference::reference_eval(
            &program,
            &[Value::from(u32_bytes(&[99u32])), Value::from(vec![0u8; 0])],
        )
        .expect_err("broadcast n=0 must trap instead of succeeding");
        let msg = error.to_string();
        assert!(
            msg.contains("trap") || msg.contains("Fix:"),
            "broadcast n=0 error must be actionable: {msg}"
        );
    }
}
