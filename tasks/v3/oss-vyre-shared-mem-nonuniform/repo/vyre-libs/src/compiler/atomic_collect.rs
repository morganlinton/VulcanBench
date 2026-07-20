use crate::region::wrap_anonymous;
use vyre::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

/// Build a one-input u32 atomic collection pass.
///
/// Each invocation reads `input[t]`, checks `predicate(item, t)`, claims a
/// monotonic counter slot with `atomic_add(counter[0], claim_stride)`, and
/// writes one u32 record into `output`. The output index and value are supplied
/// by callers so the same kernel skeleton covers both dense registries
/// (`output[claim] = t`) and sparse in-place side tables (`output[t] = claim`).
#[allow(clippy::too_many_arguments)]
pub(crate) fn atomic_collect_u32<P, I, V>(
    op_id: &'static str,
    input: &str,
    output: &str,
    counter: &str,
    count: Expr,
    claim_stride: u32,
    overflow_trap: Option<&'static str>,
    predicate: P,
    output_index: I,
    output_value: V,
) -> Program
where
    P: Fn(Expr, Expr) -> Expr,
    I: Fn(Expr, Expr) -> Expr,
    V: Fn(Expr, Expr) -> Expr,
{
    let t = Expr::InvocationId { axis: 0 };
    let item = Expr::var("item");
    let claim = Expr::var("claim");
    let mut claim_body = vec![Node::let_bind(
        "claim",
        Expr::atomic_add(counter, Expr::u32(0), Expr::u32(claim_stride)),
    )];
    if let Some(message) = overflow_trap {
        claim_body.push(Node::if_then(
            Expr::ge(claim.clone(), count.clone()),
            vec![Node::trap(claim.clone(), message)],
        ));
    }
    claim_body.push(Node::store(
        output,
        output_index(t.clone(), claim.clone()),
        output_value(t.clone(), claim),
    ));

    let loop_body = vec![
        Node::let_bind("item", Expr::load(input, t.clone())),
        Node::if_then(predicate(item, t.clone()), claim_body),
    ];
    let count_value = match &count {
        Expr::LitU32(n) => *n,
        _ => 1,
    };
    Program::wrapped(
        vec![
            BufferDecl::storage(input, 0, BufferAccess::ReadOnly, DataType::U32)
                .with_count(count_value),
            BufferDecl::storage(output, 1, BufferAccess::ReadWrite, DataType::U32)
                .with_count(count_value),
            BufferDecl::storage(counter, 2, BufferAccess::ReadWrite, DataType::U32).with_count(1),
        ],
        [256, 1, 1],
        vec![wrap_anonymous(
            op_id,
            vec![Node::if_then(Expr::lt(t, count), loop_body)],
        )],
    )
    .with_entry_op_id(op_id)
    .with_non_composable_with_self(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use vyre_reference::value::Value;

    fn unpack_u32(bytes: &[u8]) -> Vec<u32> {
        bytes
            .chunks_exact(4)
            .map(|chunk| u32::from_le_bytes(chunk.try_into().expect("Fix: byte slices must be 4-byte aligned for u32 chunks; reject odd-length buffers - u32 chunk")))
            .collect()
    }

    #[test]
    fn atomic_collect_dense_registry_uses_claimed_slots() {
        let input = [1u32, 12, 3, 14, 15, 0];
        let program = atomic_collect_u32(
            "vyre-libs::test::atomic_collect_dense",
            "input",
            "out",
            "count",
            Expr::u32(input.len() as u32),
            1,
            Some("dense-registry-overflow"),
            |item, _t| Expr::ge(item, Expr::u32(10)),
            |_t, claim| claim,
            |t, _claim| t,
        );
        let outputs = vyre_reference::reference_eval(
            &program,
            &[
                Value::from(vyre_primitives::wire::pack_u32_slice(&input)),
                Value::from(vec![0u8; input.len() * 4]),
                Value::from(vec![0u8; 4]),
            ],
        )
        .expect("Fix: dense atomic collection must execute.");
        assert_eq!(unpack_u32(&outputs[0].to_bytes())[..3], [1, 3, 4]);
        assert_eq!(unpack_u32(&outputs[1].to_bytes()), [3]);
    }

    #[test]
    fn atomic_collect_sparse_side_table_uses_claim_as_value() {
        let input = [3u32, 20, 7, 30];
        let program = atomic_collect_u32(
            "vyre-libs::test::atomic_collect_sparse",
            "regs",
            "spills",
            "frame",
            Expr::u32(input.len() as u32),
            8,
            None,
            |item, _t| Expr::ge(item, Expr::u32(16)),
            |t, _claim| t,
            |_t, claim| claim,
        );
        let outputs = vyre_reference::reference_eval(
            &program,
            &[
                Value::from(vyre_primitives::wire::pack_u32_slice(&input)),
                Value::from(vec![0u8; input.len() * 4]),
                Value::from(vec![0u8; 4]),
            ],
        )
        .expect("Fix: sparse atomic collection must execute.");
        assert_eq!(unpack_u32(&outputs[0].to_bytes()), [0, 0, 0, 8]);
        assert_eq!(unpack_u32(&outputs[1].to_bytes()), [16]);
    }
}
