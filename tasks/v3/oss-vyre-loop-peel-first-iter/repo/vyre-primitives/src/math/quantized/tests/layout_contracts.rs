use super::*;

#[test]
fn dot_program_layout_matches_packed_shape() {
    let program = i4x8_dot_i32("lhs", "rhs", "out", 65);
    assert_eq!(program.workgroup_size, [1, 1, 1]);
    assert_eq!(program.buffers[0].name(), "lhs");
    assert_eq!(program.buffers[0].count(), 9);
    assert_eq!(program.buffers[1].name(), "rhs");
    assert_eq!(program.buffers[1].count(), 9);
    assert_eq!(program.buffers[2].name(), "out");
    assert_eq!(program.buffers[2].count(), 1);
}

#[test]
fn scaled_dot_program_layout_matches_fused_packed_shape() {
    let program = i4x8_dot_f32_scaled("lhs", "rhs", "lhs_scale", "rhs_scale", "out", 65);
    assert_eq!(program.workgroup_size, [1, 1, 1]);
    assert_eq!(program.buffers[0].name(), "lhs");
    assert_eq!(program.buffers[0].count(), 9);
    assert_eq!(program.buffers[1].name(), "rhs");
    assert_eq!(program.buffers[1].count(), 9);
    assert_eq!(program.buffers[2].name(), "lhs_scale");
    assert_eq!(program.buffers[2].count(), 1);
    assert_eq!(program.buffers[3].name(), "rhs_scale");
    assert_eq!(program.buffers[3].count(), 1);
    assert_eq!(program.buffers[4].name(), "out");
    assert_eq!(program.buffers[4].count(), 1);
}

#[test]
fn matvec_program_layout_matches_row_major_packed_shape() {
    let program = i4x8_matvec_f32_scaled("weights", "x", "scales", "out", 3, 65);
    assert_eq!(program.workgroup_size, [64, 1, 1]);
    assert_eq!(program.buffers[0].name(), "weights");
    assert_eq!(program.buffers[0].count(), 27);
    assert_eq!(program.buffers[1].name(), "x");
    assert_eq!(program.buffers[1].count(), 65);
    assert_eq!(program.buffers[2].name(), "scales");
    assert_eq!(program.buffers[2].count(), 3);
    assert_eq!(program.buffers[3].name(), "out");
    assert_eq!(program.buffers[3].count(), 3);
}

#[test]
fn batched_matvec_program_layout_matches_reused_weights_shape() {
    let program = i4x8_batched_matvec_f32_scaled("weights", "x", "scales", "out", 4, 3, 65);
    assert_eq!(program.workgroup_size, [64, 1, 1]);
    assert_eq!(program.buffers[0].name(), "weights");
    assert_eq!(program.buffers[0].count(), 27);
    assert_eq!(program.buffers[1].name(), "x");
    assert_eq!(program.buffers[1].count(), 260);
    assert_eq!(program.buffers[2].name(), "scales");
    assert_eq!(program.buffers[2].count(), 3);
    assert_eq!(program.buffers[3].name(), "out");
    assert_eq!(program.buffers[3].count(), 12);
}

#[test]
fn batched_matmul_program_layout_matches_packed_activation_shape() {
    let program = i4x8_batched_matmul_f32_scaled(
        "weights",
        "activations",
        "row_scales",
        "batch_scales",
        "out",
        4,
        3,
        65,
    );
    assert_eq!(program.workgroup_size, [64, 1, 1]);
    assert_eq!(program.buffers[0].name(), "weights");
    assert_eq!(program.buffers[0].count(), 27);
    assert_eq!(program.buffers[1].name(), "activations");
    assert_eq!(program.buffers[1].count(), 36);
    assert_eq!(program.buffers[2].name(), "row_scales");
    assert_eq!(program.buffers[2].count(), 3);
    assert_eq!(program.buffers[3].name(), "batch_scales");
    assert_eq!(program.buffers[3].count(), 4);
    assert_eq!(program.buffers[4].name(), "out");
    assert_eq!(program.buffers[4].count(), 12);
}

#[test]
fn batched_matmul_top1_program_layout_matches_packed_activation_shape() {
    let program = i4x8_batched_matmul_top1_f32_scaled(
        "weights",
        "activations",
        "row_scales",
        "batch_scales",
        "out",
        4,
        3,
        65,
    );

    assert_eq!(program.workgroup_size, [64, 1, 1]);
    assert_eq!(program.buffers[0].name(), "weights");
    assert_eq!(program.buffers[0].count(), 27);
    assert_eq!(program.buffers[1].name(), "activations");
    assert_eq!(program.buffers[1].count(), 36);
    assert_eq!(program.buffers[2].name(), "row_scales");
    assert_eq!(program.buffers[2].count(), 3);
    assert_eq!(program.buffers[3].name(), "batch_scales");
    assert_eq!(program.buffers[3].count(), 4);
    assert_eq!(program.buffers[4].name(), "out");
    assert_eq!(program.buffers[4].count(), 8);
}

