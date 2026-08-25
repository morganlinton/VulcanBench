//! ReLU: `y = max(0, x)`.
//!
//! Category A composition  -  one primitive per invocation. Element-wise
//! so the optimizer can trivially fuse into any upstream operation.

use vyre::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

use crate::region::wrap_anonymous;

/// Shared unsigned ReLU expression used by the standalone activation builder.
#[must_use]
pub(crate) fn relu_u32_expr(x: Expr) -> Expr {
    Expr::max(Expr::u32(0), x)
}

/// Shared floating-point ReLU expression used by fused activation builders.
#[must_use]
pub(crate) fn relu_f32_expr(x: Expr) -> Expr {
    Expr::max(Expr::f32(0.0), x)
}

/// Build a Program that applies ReLU element-wise from `input` into
/// `output`. `n` is the element count of both buffers. u32 semantics:
/// values are unsigned so "max(0, x)" is the identity; this module
/// provides the structural Category-A shape and a future i32/f32
/// overload replaces the primitive.
#[must_use]
pub fn relu(input: &str, output: &str, n: u32) -> Program {
    let input_decl = BufferDecl::storage(input, 0, BufferAccess::ReadOnly, DataType::U32);
    let input_decl = if n == 0 {
        input_decl
    } else {
        input_decl.with_count(n)
    };
    let output_decl = BufferDecl::output(output, 1, DataType::U32)
        .with_count(n.max(1))
        .with_output_byte_range(0..(n as usize).saturating_mul(4));
    let i = Expr::var("i");
    let body = vec![
        Node::let_bind("i", Expr::InvocationId { axis: 0 }),
        Node::if_then(
            Expr::lt(i.clone(), Expr::buf_len(input)),
            vec![Node::Store {
                buffer: output.into(),
                index: i.clone(),
                // max(0, x): the u32 identity. Swapping DataType to
                // I32 and replacing with Expr::max(Expr::i32(0), x)
                // handles the signed case.
                value: relu_u32_expr(Expr::load(input, i)),
            }],
        ),
    ];
    Program::wrapped(
        vec![input_decl, output_decl],
        [64, 1, 1],
        vec![wrap_anonymous("vyre-libs::nn::relu", body)],
    )
}

inventory::submit! {
    crate::harness::OpEntry {
        id: "vyre-libs::nn::relu",
        build: || relu("input", "output", 4),
        test_inputs: Some(|| vec![vec![
            vyre_primitives::wire::pack_u32_slice(&[0u32, 5, 10, 0]),
        ]]),
        expected_output: Some(|| vec![vec![
            // Only ReadWrite buffer: output = max(0, input) = identity for u32
            vyre_primitives::wire::pack_u32_slice(&[0u32, 5, 10, 0]),
        ]]),
        category: Some("nn"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::byte_pack::u32_bytes;
    use vyre_reference::value::Value;

    #[test]
    fn relu_empty_tensor_produces_no_panic() {
        let program = relu("input", "output", 0);
        let outputs =
            vyre_reference::reference_eval(&program, &[Value::from(vec![]), Value::from(vec![])])
                .expect("Fix: relu n=0 must not panic");
        assert!(outputs[0].to_bytes().is_empty());
    }

    #[test]
    fn relu_single_element_identity() {
        let input = [42u32];
        let program = relu("input", "output", 1);
        let outputs = vyre_reference::reference_eval(
            &program,
            &[Value::from(u32_bytes(&input)), Value::from(vec![0u8; 4])],
        )
        .expect("Fix: relu single element must execute");
        let out: Vec<u32> = vyre_primitives::wire::decode_u32_le_bytes_all(&outputs[0].to_bytes());
        assert_eq!(out, vec![42]);
    }

    #[test]
    fn relu_all_zeros_identity() {
        let input = [0u32, 0, 0, 0];
        let program = relu("input", "output", 4);
        let outputs = vyre_reference::reference_eval(
            &program,
            &[Value::from(u32_bytes(&input)), Value::from(vec![0u8; 16])],
        )
        .expect("Fix: relu all-zeros must execute");
        let out: Vec<u32> = vyre_primitives::wire::decode_u32_le_bytes_all(&outputs[0].to_bytes());
        assert_eq!(out, vec![0, 0, 0, 0]);
    }

    #[test]
    fn relu_all_max_u32_identity() {
        let input = [u32::MAX; 4];
        let program = relu("input", "output", 4);
        let outputs = vyre_reference::reference_eval(
            &program,
            &[Value::from(u32_bytes(&input)), Value::from(vec![0u8; 16])],
        )
        .expect("Fix: relu all-max-u32 must execute");
        let out: Vec<u32> = vyre_primitives::wire::decode_u32_le_bytes_all(&outputs[0].to_bytes());
        assert_eq!(out, vec![u32::MAX; 4]);
    }
}
