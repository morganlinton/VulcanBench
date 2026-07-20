//! Generated `reference_eval` coverage for FMA and Select expression nodes.

use vyre::ir::{BufferDecl, DataType, Expr, Node, Program};
use vyre_reference::reference_eval;

fn eval_f32_expr(expr: Expr) -> f32 {
    let program = Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::F32).with_count(1)],
        [1, 1, 1],
        vec![Node::store("out", Expr::u32(0), expr)],
    );
    let outputs = reference_eval(&program, &[])
        .expect("Fix: generated F32 expression program must execute in the reference oracle.");
    let bytes = outputs[0].to_bytes();
    f32::from_le_bytes(
        bytes
            .get(..4)
            .expect("Fix: F32 reference output must contain one word.")
            .try_into()
            .expect("Fix: F32 reference output slice width must be four bytes."),
    )
}

fn eval_u32_expr(expr: Expr) -> u32 {
    let program = Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![Node::store("out", Expr::u32(0), expr)],
    );
    let outputs = reference_eval(&program, &[])
        .expect("Fix: generated U32 expression program must execute in the reference oracle.");
    let bytes = outputs[0].to_bytes();
    u32::from_le_bytes(
        bytes
            .get(..4)
            .expect("Fix: U32 reference output must contain one word.")
            .try_into()
            .expect("Fix: U32 reference output slice width must be four bytes."),
    )
}

#[test]
fn generated_reference_eval_fma_matches_fused_mul_add_for_4096_cases() {
    let mut checked = 0usize;
    for seed in 0u32..4096 {
        let a = generated_f32(seed ^ 0xA341_316C);
        let b = generated_f32(seed.rotate_left(7) ^ 0xC801_3EA4);
        let c = generated_f32(seed.wrapping_mul(97) ^ 0x9E37_79B9);
        let actual = eval_f32_expr(Expr::fma(Expr::f32(a), Expr::f32(b), Expr::f32(c)));
        let expected = a.mul_add(b, c);
        assert_eq!(
            actual.to_bits(),
            expected.to_bits(),
            "Fix: reference_eval FMA must use fused f32::mul_add semantics for seed {seed}: {a} * {b} + {c}."
        );
        checked += 1;
    }
    assert_eq!(checked, 4096);
}

#[test]
fn generated_reference_eval_select_routes_u32_arms_for_8192_cases() {
    let mut checked = 0usize;
    for seed in 0u32..8192 {
        let cond = (seed.wrapping_mul(0x45D9_F3B) ^ seed.rotate_left(13)) & 1 != 0;
        let true_value = seed
            .wrapping_mul(1_664_525)
            .wrapping_add(1_013_904_223)
            .rotate_left(seed & 31);
        let false_value = seed
            .wrapping_mul(22_695_477)
            .wrapping_add(1)
            .rotate_right((seed >> 3) & 31);
        let actual = eval_u32_expr(Expr::select(
            Expr::bool(cond),
            Expr::u32(true_value),
            Expr::u32(false_value),
        ));
        assert_eq!(
            actual,
            if cond { true_value } else { false_value },
            "Fix: reference_eval Select must route exactly one U32 arm for seed {seed}."
        );
        checked += 1;
    }
    assert_eq!(checked, 8192);
}

#[test]
fn reference_eval_select_can_feed_fma_without_type_erasure() {
    let actual = eval_f32_expr(Expr::fma(
        Expr::select(Expr::bool(true), Expr::f32(2.5), Expr::f32(99.0)),
        Expr::f32(4.0),
        Expr::select(Expr::bool(false), Expr::f32(100.0), Expr::f32(-1.25)),
    ));

    assert_eq!(
        actual.to_bits(),
        8.75f32.to_bits(),
        "Fix: reference_eval must preserve F32 type through Select into FMA."
    );
}

fn generated_f32(seed: u32) -> f32 {
    let magnitude = ((seed & 0x1FF) as f32) / 8.0;
    let signed = if (seed & 0x200) == 0 {
        magnitude
    } else {
        -magnitude
    };
    signed + (((seed >> 10) & 0xF) as f32) * 0.125
}
