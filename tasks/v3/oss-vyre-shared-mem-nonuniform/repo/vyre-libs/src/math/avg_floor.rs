use vyre::ir::{Expr, Program};

const OP_ID: &str = "vyre-libs::math::avg_floor";

/// Computes average floor.
#[must_use]
pub fn avg_floor(a: &str, b: &str, out: &str, size: u32) -> Program {
    super::elementwise::u32_elementwise_binary(OP_ID, a, b, out, size, |lx, rx| {
        Expr::add(
            Expr::bitand(lx.clone(), rx.clone()),
            Expr::shr(Expr::bitxor(lx, rx), Expr::u32(1)),
        )
    })
}

inventory::submit! {
    crate::harness::OpEntry {
        id: OP_ID,
        build: || avg_floor("a", "b", "out", 4),
        test_inputs: Some(|| {
            let a = [10u32, u32::MAX, 7, 100];
            let b = [20u32, u32::MAX, 12, 0];
            let to_bytes = vyre_primitives::wire::pack_u32_slice;
            vec![vec![to_bytes(&a), to_bytes(&b)]]
        }),
        expected_output: Some(|| {
            // HD-style floor((a+b)/2) that never overflows:
            //   (a & b) + ((a ^ b) >> 1). For the fixture
            //   (10,20)->15, (MAX,MAX)->MAX, (7,12)->9, (100,0)->50.
            let a = [10u32, u32::MAX, 7, 100];
            let b = [20u32, u32::MAX, 12, 0];
            let expected: Vec<u32> = a
                .iter()
                .zip(b.iter())
                .map(|(&x, &y)| (x & y).wrapping_add((x ^ y) >> 1))
                .collect();
            let bytes = vyre_primitives::wire::pack_u32_slice(&expected);
            vec![vec![bytes]]
        }),
        category: Some("math"),
    }
}
