use super::*;

#[test]
fn dot_zero_lanes_traps() {
    assert!(i4x8_dot_i32("lhs", "rhs", "out", 0).stats().trap());
}

#[test]
fn scaled_dot_zero_lanes_traps() {
    assert!(
        i4x8_dot_f32_scaled("lhs", "rhs", "lhs_scale", "rhs_scale", "out", 0)
            .stats()
            .trap()
    );
}

#[test]
fn matvec_zero_shape_traps() {
    assert!(
        i4x8_matvec_f32_scaled("weights", "x", "scales", "out", 0, 8)
            .stats()
            .trap()
    );
    assert!(
        i4x8_matvec_f32_scaled("weights", "x", "scales", "out", 4, 0)
            .stats()
            .trap()
    );
}

#[test]
fn batched_matvec_zero_shape_traps() {
    assert!(
        i4x8_batched_matvec_f32_scaled("weights", "x", "scales", "out", 0, 4, 8)
            .stats()
            .trap()
    );
    assert!(
        i4x8_batched_matvec_f32_scaled("weights", "x", "scales", "out", 2, 0, 8)
            .stats()
            .trap()
    );
    assert!(
        i4x8_batched_matvec_f32_scaled("weights", "x", "scales", "out", 2, 4, 0)
            .stats()
            .trap()
    );
}

#[test]
fn batched_matmul_zero_shape_traps() {
    assert!(i4x8_batched_matmul_f32_scaled(
        "weights",
        "activations",
        "row_scales",
        "batch_scales",
        "out",
        0,
        4,
        8
    )
    .stats()
    .trap());
    assert!(i4x8_batched_matmul_f32_scaled(
        "weights",
        "activations",
        "row_scales",
        "batch_scales",
        "out",
        2,
        0,
        8
    )
    .stats()
    .trap());
    assert!(i4x8_batched_matmul_f32_scaled(
        "weights",
        "activations",
        "row_scales",
        "batch_scales",
        "out",
        2,
        4,
        0
    )
    .stats()
    .trap());
}

#[test]
fn batched_matmul_top1_zero_shape_traps() {
    assert!(i4x8_batched_matmul_top1_f32_scaled(
        "weights",
        "activations",
        "row_scales",
        "batch_scales",
        "out",
        0,
        4,
        8
    )
    .stats()
    .trap());
    assert!(i4x8_batched_matmul_top1_f32_scaled(
        "weights",
        "activations",
        "row_scales",
        "batch_scales",
        "out",
        2,
        0,
        8
    )
    .stats()
    .trap());
    assert!(i4x8_batched_matmul_top1_f32_scaled(
        "weights",
        "activations",
        "row_scales",
        "batch_scales",
        "out",
        2,
        4,
        0
    )
    .stats()
    .trap());
}

