//! Top-K selection: indices of the K largest elements.
//!
//! Category-A composition. Sequential implementation for the reference
//! oracle; parallel bitonic top-k lands in Tier 2.

use super::topk_selection::{
    copy_top_k_indices, init_top_k_slots, insert_top_k_candidate, BEST_IDXS, BEST_VALS,
};
use crate::region::wrap_anonymous;
use vyre::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

/// Build a Program that finds the indices of the `k` largest elements in `input`.
/// `input`: `n`, `output_indices`: `k`.
///
/// Uses a sequential insertion-sort-into-slots algorithm: maintains `k` best
/// (value, index) pairs in descending order, updating on every new element.
#[must_use]
pub fn top_k(input: &str, output_indices: &str, n: u32, k: u32) -> Program {
    if k == 0 {
        return crate::builder::invalid_output_program(
            "vyre-libs::nn::top_k",
            output_indices,
            DataType::U32,
            "Fix: top_k requires k > 0 so the selection scratch has at least one slot.".to_string(),
        );
    }
    let mut body = init_top_k_slots(k);

    // For each input element i:
    //   val = input[i]
    //   Scan j=0..k: if val > best_vals[j], shift j..k-1 down and insert at j
    body.push(Node::loop_for(
        "i",
        Expr::u32(0),
        Expr::u32(n),
        vec![
            Node::let_bind("val", Expr::load(input, Expr::var("i"))),
            Node::let_bind("idx", Expr::var("i")),
            Node::Block(insert_top_k_candidate(
                k,
                Expr::var("val"),
                Expr::var("idx"),
            )),
        ],
    ));

    body.extend(copy_top_k_indices(output_indices, k));

    Program::wrapped(
        vec![
            BufferDecl::storage(input, 0, BufferAccess::ReadOnly, DataType::F32).with_count(n),
            BufferDecl::output(output_indices, 1, DataType::U32).with_count(k),
            // Internal scratch buffers
            BufferDecl::read_write(BEST_VALS, 2, DataType::F32).with_count(k),
            BufferDecl::read_write(BEST_IDXS, 3, DataType::U32).with_count(k),
        ],
        [1, 1, 1],
        vec![wrap_anonymous("vyre-libs::nn::top_k", body)],
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::byte_pack::f32_bytes;
    use vyre_reference::value::Value;

    fn u32_from_bytes(bytes: &[u8]) -> Vec<u32> {
        vyre_primitives::wire::decode_u32_le_bytes_all(bytes)
    }

    #[test]
    fn top_k_descending_input() {
        let scores: Vec<f32> = (1..=8).map(|i| i as f32).collect();
        let program = top_k("input", "output", 8, 2);
        let outputs = vyre_reference::reference_eval(
            &program,
            &[
                Value::from(f32_bytes(&scores)),
                Value::from(vec![0u8; 2 * 4]),
                Value::from(vec![0u8; 2 * 4]),
            ],
        )
        .unwrap();
        let indices = u32_from_bytes(&outputs[0].to_bytes());
        assert_eq!(indices[0], 7); // max = 8.0 at index 7
        assert_eq!(indices[1], 6); // second = 7.0 at index 6
    }

    #[test]
    fn top_k_ascending_input() {
        let scores: Vec<f32> = (1..=8).rev().map(|i| i as f32).collect();
        let program = top_k("input", "output", 8, 2);
        let outputs = vyre_reference::reference_eval(
            &program,
            &[
                Value::from(f32_bytes(&scores)),
                Value::from(vec![0u8; 2 * 4]),
                Value::from(vec![0u8; 2 * 4]),
            ],
        )
        .unwrap();
        let indices = u32_from_bytes(&outputs[0].to_bytes());
        assert_eq!(indices[0], 0); // max = 8.0 at index 0
        assert_eq!(indices[1], 1); // second = 7.0 at index 1
    }

    #[test]
    fn top_k_with_duplicates() {
        let scores = vec![3.0, 1.0, 4.0, 1.0, 5.0, 9.0, 2.0, 6.0];
        let program = top_k("input", "output", 8, 3);
        let outputs = vyre_reference::reference_eval(
            &program,
            &[
                Value::from(f32_bytes(&scores)),
                Value::from(vec![0u8; 3 * 4]),
                Value::from(vec![0u8; 3 * 4]),
            ],
        )
        .unwrap();
        let indices = u32_from_bytes(&outputs[0].to_bytes());
        // 9.0(5), 6.0(7), 5.0(4), 4.0(2), 3.0(0), 2.0(6), 1.0(1), 1.0(3)
        assert_eq!(indices[0], 5);
        assert_eq!(indices[1], 7);
        assert_eq!(indices[2], 4);
    }
}

inventory::submit! {
    crate::harness::OpEntry {
        id: "vyre-libs::nn::top_k",
        build: || top_k("input", "output", 8, 2),
        test_inputs: Some(|| {
            let scores: [f32; 8] = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0];
            let input_bytes = vyre_primitives::wire::pack_f32_slice(&scores);
            vec![vec![
                input_bytes,
                vec![0u8; 4 * 2],
                vec![0u8; 4 * 2],
            ]]
        }),
        expected_output: Some(|| {
            // Top-2 of ascending [1..8] are indices 7 and 6
            let best_vals = vyre_primitives::wire::pack_f32_slice(&[8.0f32, 7.0f32]);
            let best_idxs = vyre_primitives::wire::pack_u32_slice(&[7u32, 6u32]);
            vec![vec![
                best_idxs.clone(),
                best_vals,
                best_idxs,
            ]]
        }),
        category: Some("nn"),
    }
}
