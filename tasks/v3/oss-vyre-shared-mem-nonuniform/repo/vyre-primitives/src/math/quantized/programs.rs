//! IR program builders for packed INT4 quantized primitives.

use std::sync::Arc;

use super::program_helpers::{
    i4_dot_accumulation_body, i4_matvec_scaled_body, signed_i4_nibble_expr,
    signed_i4_nibble_f32_expr,
};

use super::{
    i4_packed_words, I4_BATCHED_MATMUL_F32_SCALED_OP_ID, I4_BATCHED_MATMUL_TOP1_F32_SCALED_OP_ID,
    I4_BATCHED_MATVEC_F32_SCALED_OP_ID, I4_DOT_F32_SCALED_OP_ID, I4_DOT_I32_OP_ID,
    I4_LANES_PER_WORD, I4_MATVEC_F32_SCALED_OP_ID, UNPACK_I4_OP_ID,
};

use vyre_foundation::ir::model::expr::Ident;

use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

/// Build a Program that unpacks packed signed INT4 lanes into i32 lanes.
pub fn unpack_i4x8(packed_words: &str, out_lanes: &str, lane_count: u32) -> Program {
    if lane_count == 0 {
        return crate::invalid_output_program(
            UNPACK_I4_OP_ID,
            out_lanes,
            DataType::I32,
            "Fix: unpack_i4x8 requires lane_count > 0.".to_string(),
        );
    }

    let t = Expr::InvocationId { axis: 0 };
    let word_count = i4_packed_words(lane_count);
    let body = vec![
        Node::let_bind(
            "i4_word_index",
            Expr::div(t.clone(), Expr::u32(I4_LANES_PER_WORD)),
        ),
        Node::let_bind(
            "i4_lane_in_word",
            Expr::rem(t.clone(), Expr::u32(I4_LANES_PER_WORD)),
        ),
        Node::let_bind(
            "i4_shift",
            Expr::mul(Expr::var("i4_lane_in_word"), Expr::u32(4)),
        ),
        Node::let_bind(
            "i4_nibble",
            Expr::bitand(
                Expr::shr(
                    Expr::load(packed_words, Expr::var("i4_word_index")),
                    Expr::var("i4_shift"),
                ),
                Expr::u32(0xF),
            ),
        ),
        Node::let_bind("i4_signed", signed_i4_nibble_expr(Expr::var("i4_nibble"))),
        Node::store(out_lanes, t.clone(), Expr::var("i4_signed")),
    ];

    Program::wrapped(
        vec![
            BufferDecl::storage(packed_words, 0, BufferAccess::ReadOnly, DataType::U32)
                .with_count(word_count),
            BufferDecl::storage(out_lanes, 1, BufferAccess::ReadWrite, DataType::I32)
                .with_count(lane_count),
        ],
        [256, 1, 1],
        vec![Node::Region {
            generator: Ident::from(UNPACK_I4_OP_ID),
            source_region: None,
            body: Arc::new(vec![Node::if_then(
                Expr::lt(t, Expr::u32(lane_count)),
                body,
            )]),
        }],
    )
}

/// Build a Program that computes a packed signed INT4 dot product.
///
/// `lhs_packed` and `rhs_packed` store eight signed 4-bit lanes per u32 word.
/// Lane products are accumulated directly into `out[0]` as i32, avoiding the
/// extra memory traffic of unpacking either vector into temporary lane buffers.
#[must_use]
/// Build a Program that computes a packed signed INT4 dot product.
pub fn i4x8_dot_i32(lhs_packed: &str, rhs_packed: &str, out: &str, lane_count: u32) -> Program {
    if lane_count == 0 {
        return crate::invalid_output_program(
            I4_DOT_I32_OP_ID,
            out,
            DataType::I32,
            "Fix: i4x8_dot_i32 requires lane_count > 0.".to_string(),
        );
    }

    let word_count = i4_packed_words(lane_count);
    let body = i4_dot_accumulation_body(
        lhs_packed,
        rhs_packed,
        lane_count,
        Expr::i32(0),
        signed_i4_nibble_expr,
        Node::store(out, Expr::u32(0), Expr::var("i4_dot_acc")),
    );

    Program::wrapped(
        vec![
            BufferDecl::storage(lhs_packed, 0, BufferAccess::ReadOnly, DataType::U32)
                .with_count(word_count),
            BufferDecl::storage(rhs_packed, 1, BufferAccess::ReadOnly, DataType::U32)
                .with_count(word_count),
            BufferDecl::output(out, 2, DataType::I32).with_count(1),
        ],
        [1, 1, 1],
        vec![Node::Region {
            generator: Ident::from(I4_DOT_I32_OP_ID),
            source_region: None,
            body: Arc::new(body),
        }],
    )
}

/// Build a Program that computes a packed signed INT4 dot product and applies
/// symmetric dequantization scales without materializing either lane vector.
///
/// `out[0] = dot(lhs_i4, rhs_i4) as f32 * lhs_scale[0] * rhs_scale[0]`.
#[must_use]
/// Build a Program that computes a packed signed INT4 dot product with f32 scales.
pub fn i4x8_dot_f32_scaled(
    lhs_packed: &str,
    rhs_packed: &str,
    lhs_scale: &str,
    rhs_scale: &str,
    out: &str,
    lane_count: u32,
) -> Program {
    if lane_count == 0 {
        return crate::invalid_output_program(
            I4_DOT_F32_SCALED_OP_ID,
            out,
            DataType::F32,
            "Fix: i4x8_dot_f32_scaled requires lane_count > 0.".to_string(),
        );
    }

    let word_count = i4_packed_words(lane_count);
    let scaled_dot = Expr::mul(
        Expr::mul(Expr::var("i4_dot_acc"), Expr::load(lhs_scale, Expr::u32(0))),
        Expr::load(rhs_scale, Expr::u32(0)),
    );
    let body = i4_dot_accumulation_body(
        lhs_packed,
        rhs_packed,
        lane_count,
        Expr::f32(0.0),
        signed_i4_nibble_f32_expr,
        Node::store(out, Expr::u32(0), scaled_dot),
    );

    Program::wrapped(
        vec![
            BufferDecl::storage(lhs_packed, 0, BufferAccess::ReadOnly, DataType::U32)
                .with_count(word_count),
            BufferDecl::storage(rhs_packed, 1, BufferAccess::ReadOnly, DataType::U32)
                .with_count(word_count),
            BufferDecl::storage(lhs_scale, 2, BufferAccess::ReadOnly, DataType::F32).with_count(1),
            BufferDecl::storage(rhs_scale, 3, BufferAccess::ReadOnly, DataType::F32).with_count(1),
            BufferDecl::output(out, 4, DataType::F32).with_count(1),
        ],
        [1, 1, 1],
        vec![Node::Region {
            generator: Ident::from(I4_DOT_F32_SCALED_OP_ID),
            source_region: None,
            body: Arc::new(body),
        }],
    )
}

/// Build a Program that computes `out[row] = scale[row] * dot(i4_row[row], x)`.
///
/// Weights are packed row-major with `i4_packed_words(cols)` u32 words per row.
/// The kernel fuses unpack, dequant scale, and matvec accumulation so neither a
/// dequantized weight matrix nor a temporary lane buffer is materialized.
#[must_use]
/// Build a Program that computes `out[row] = scale[row] * dot(i4_row[row], x)`.
pub fn i4x8_matvec_f32_scaled(
    weights_packed: &str,
    x: &str,
    row_scales: &str,
    out: &str,
    rows: u32,
    cols: u32,
) -> Program {
    if rows == 0 || cols == 0 {
        return crate::invalid_output_program(
            I4_MATVEC_F32_SCALED_OP_ID,
            out,
            DataType::F32,
            "Fix: i4x8_matvec_f32_scaled requires rows > 0 and cols > 0.".to_string(),
        );
    }

    let row = Expr::InvocationId { axis: 0 };
    let words_per_row = i4_packed_words(cols);
    let body = i4_matvec_scaled_body(
        weights_packed,
        x,
        row_scales,
        out,
        cols,
        words_per_row,
        row.clone(),
        Expr::u32(0),
        row.clone(),
    );

    Program::wrapped(
        vec![
            BufferDecl::storage(weights_packed, 0, BufferAccess::ReadOnly, DataType::U32)
                .with_count(rows * words_per_row),
            BufferDecl::storage(x, 1, BufferAccess::ReadOnly, DataType::F32).with_count(cols),
            BufferDecl::storage(row_scales, 2, BufferAccess::ReadOnly, DataType::F32)
                .with_count(rows),
            BufferDecl::output(out, 3, DataType::F32).with_count(rows),
        ],
        [64, 1, 1],
        vec![Node::Region {
            generator: Ident::from(I4_MATVEC_F32_SCALED_OP_ID),
            source_region: None,
            body: Arc::new(vec![Node::if_then(Expr::lt(row, Expr::u32(rows)), body)]),
        }],
    )
}

/// Build a Program that computes a batch of row-scaled packed INT4 matvecs.
///
/// `x_batches` is laid out batch-major as `[batch][col]`; `out` is batch-major
/// as `[batch][row]`. The packed weights and row scales are reused across every
/// batch element, avoiding repeated kernel submissions for small inference
/// batches.
#[must_use]
/// Build a Program that computes a batch of row-scaled packed INT4 matvecs.
pub fn i4x8_batched_matvec_f32_scaled(
    weights_packed: &str,
    x_batches: &str,
    row_scales: &str,
    out: &str,
    batch: u32,
    rows: u32,
    cols: u32,
) -> Program {
    if batch == 0 || rows == 0 || cols == 0 {
        return crate::invalid_output_program(
            I4_BATCHED_MATVEC_F32_SCALED_OP_ID,
            out,
            DataType::F32,
            "Fix: i4x8_batched_matvec_f32_scaled requires batch > 0, rows > 0, and cols > 0."
                .to_string(),
        );
    }

    let item = Expr::InvocationId { axis: 0 };
    let words_per_row = i4_packed_words(cols);
    let total_outputs = batch * rows;
    let row = Expr::rem(item.clone(), Expr::u32(rows));
    let batch_index = Expr::div(item.clone(), Expr::u32(rows));
    let x_base = Expr::mul(batch_index, Expr::u32(cols));
    let body = i4_matvec_scaled_body(
        weights_packed,
        x_batches,
        row_scales,
        out,
        cols,
        words_per_row,
        row,
        x_base,
        item.clone(),
    );

    Program::wrapped(
        vec![
            BufferDecl::storage(weights_packed, 0, BufferAccess::ReadOnly, DataType::U32)
                .with_count(rows * words_per_row),
            BufferDecl::storage(x_batches, 1, BufferAccess::ReadOnly, DataType::F32)
                .with_count(batch * cols),
            BufferDecl::storage(row_scales, 2, BufferAccess::ReadOnly, DataType::F32)
                .with_count(rows),
            BufferDecl::output(out, 3, DataType::F32).with_count(total_outputs),
        ],
        [64, 1, 1],
        vec![Node::Region {
            generator: Ident::from(I4_BATCHED_MATVEC_F32_SCALED_OP_ID),
            source_region: None,
            body: Arc::new(vec![Node::if_then(
                Expr::lt(item, Expr::u32(total_outputs)),
                body,
            )]),
        }],
    )
}

/// Build a Program that computes a batch of packed-activation INT4 matmuls.
///
/// Both `weights_packed` and `activation_batches_packed` use signed INT4 lanes
/// packed eight per u32 word. Weights are row-major `[row][col]`; activations
/// are batch-major `[batch][col]`; output is `[batch][row]`.
#[must_use]
/// Build a Program that computes a batch of packed-activation INT4 matmuls.
pub fn i4x8_batched_matmul_f32_scaled(
    weights_packed: &str,
    activation_batches_packed: &str,
    row_scales: &str,
    batch_scales: &str,
    out: &str,
    batch: u32,
    rows: u32,
    cols: u32,
) -> Program {
    if batch == 0 || rows == 0 || cols == 0 {
        return crate::invalid_output_program(
            I4_BATCHED_MATMUL_F32_SCALED_OP_ID,
            out,
            DataType::F32,
            "Fix: i4x8_batched_matmul_f32_scaled requires batch > 0, rows > 0, and cols > 0."
                .to_string(),
        );
    }

    let item = Expr::InvocationId { axis: 0 };
    let words_per_row = i4_packed_words(cols);
    let total_outputs = batch * rows;
    let row = Expr::rem(item.clone(), Expr::u32(rows));
    let batch_index = Expr::div(item.clone(), Expr::u32(rows));
    let body = vec![
        Node::let_bind("i4_matmul_row", row),
        Node::let_bind("i4_matmul_batch", batch_index),
        Node::let_bind("i4_matmul_out_index", item.clone()),
        Node::let_bind("i4_matmul_acc", Expr::f32(0.0)),
        Node::loop_for(
            "i4_matmul_col",
            Expr::u32(0),
            Expr::u32(cols),
            vec![
                Node::let_bind(
                    "i4_matmul_weight_word",
                    Expr::add(
                        Expr::mul(Expr::var("i4_matmul_row"), Expr::u32(words_per_row)),
                        Expr::div(Expr::var("i4_matmul_col"), Expr::u32(I4_LANES_PER_WORD)),
                    ),
                ),
                Node::let_bind(
                    "i4_matmul_activation_word",
                    Expr::add(
                        Expr::mul(Expr::var("i4_matmul_batch"), Expr::u32(words_per_row)),
                        Expr::div(Expr::var("i4_matmul_col"), Expr::u32(I4_LANES_PER_WORD)),
                    ),
                ),
                Node::let_bind(
                    "i4_matmul_shift",
                    Expr::mul(
                        Expr::rem(Expr::var("i4_matmul_col"), Expr::u32(I4_LANES_PER_WORD)),
                        Expr::u32(4),
                    ),
                ),
                Node::let_bind(
                    "i4_matmul_weight_nibble",
                    Expr::bitand(
                        Expr::shr(
                            Expr::load(weights_packed, Expr::var("i4_matmul_weight_word")),
                            Expr::var("i4_matmul_shift"),
                        ),
                        Expr::u32(0xF),
                    ),
                ),
                Node::let_bind(
                    "i4_matmul_activation_nibble",
                    Expr::bitand(
                        Expr::shr(
                            Expr::load(
                                activation_batches_packed,
                                Expr::var("i4_matmul_activation_word"),
                            ),
                            Expr::var("i4_matmul_shift"),
                        ),
                        Expr::u32(0xF),
                    ),
                ),
                Node::let_bind(
                    "i4_matmul_weight",
                    signed_i4_nibble_f32_expr(Expr::var("i4_matmul_weight_nibble")),
                ),
                Node::let_bind(
                    "i4_matmul_activation",
                    signed_i4_nibble_f32_expr(Expr::var("i4_matmul_activation_nibble")),
                ),
                Node::assign(
                    "i4_matmul_acc",
                    Expr::add(
                        Expr::var("i4_matmul_acc"),
                        Expr::mul(
                            Expr::var("i4_matmul_weight"),
                            Expr::var("i4_matmul_activation"),
                        ),
                    ),
                ),
            ],
        ),
        Node::store(
            out,
            Expr::var("i4_matmul_out_index"),
            Expr::mul(
                Expr::mul(
                    Expr::var("i4_matmul_acc"),
                    Expr::load(row_scales, Expr::var("i4_matmul_row")),
                ),
                Expr::load(batch_scales, Expr::var("i4_matmul_batch")),
            ),
        ),
    ];

    Program::wrapped(
        vec![
            BufferDecl::storage(weights_packed, 0, BufferAccess::ReadOnly, DataType::U32)
                .with_count(rows * words_per_row),
            BufferDecl::storage(
                activation_batches_packed,
                1,
                BufferAccess::ReadOnly,
                DataType::U32,
            )
            .with_count(batch * words_per_row),
            BufferDecl::storage(row_scales, 2, BufferAccess::ReadOnly, DataType::F32)
                .with_count(rows),
            BufferDecl::storage(batch_scales, 3, BufferAccess::ReadOnly, DataType::F32)
                .with_count(batch),
            BufferDecl::output(out, 4, DataType::F32).with_count(total_outputs),
        ],
        [64, 1, 1],
        vec![Node::Region {
            generator: Ident::from(I4_BATCHED_MATMUL_F32_SCALED_OP_ID),
            source_region: None,
            body: Arc::new(vec![Node::if_then(
                Expr::lt(item, Expr::u32(total_outputs)),
                body,
            )]),
        }],
    )
}

/// Build a Program that computes the top-1 row for each packed INT4 activation.
///
/// This fuses packed INT4 dot products, dequantization, and argmax routing so
/// routing/search workloads emit one score/index pair per batch item instead
/// of materializing the full `[batch][row]` logits matrix.
#[must_use]
/// Build a Program that emits the top-1 row score and index per packed INT4 activation.

pub fn i4x8_batched_matmul_top1_f32_scaled(
    weights_packed: &str,
    activation_batches_packed: &str,
    row_scales: &str,
    batch_scales: &str,
    out: &str,
    batch: u32,
    rows: u32,
    cols: u32,
) -> Program {
    if batch == 0 || rows == 0 || cols == 0 {
        return crate::invalid_output_program(
            I4_BATCHED_MATMUL_TOP1_F32_SCALED_OP_ID,
            out,
            DataType::F32,
            "Fix: i4x8_batched_matmul_top1_f32_scaled requires batch > 0, rows > 0, and cols > 0."
                .to_string(),
        );
    }

    let batch_index = Expr::InvocationId { axis: 0 };
    let words_per_row = i4_packed_words(cols);
    let body = vec![
        Node::let_bind("i4_top1_batch", batch_index.clone()),
        Node::let_bind("i4_top1_best_score", Expr::f32(f32::MIN)),
        Node::let_bind("i4_top1_best_index", Expr::u32(0)),
        Node::loop_for(
            "i4_top1_row",
            Expr::u32(0),
            Expr::u32(rows),
            vec![
                Node::let_bind("i4_top1_acc", Expr::f32(0.0)),
                Node::loop_for(
                    "i4_top1_col",
                    Expr::u32(0),
                    Expr::u32(cols),
                    vec![
                        Node::let_bind(
                            "i4_top1_weight_word",
                            Expr::add(
                                Expr::mul(Expr::var("i4_top1_row"), Expr::u32(words_per_row)),
                                Expr::div(Expr::var("i4_top1_col"), Expr::u32(I4_LANES_PER_WORD)),
                            ),
                        ),
                        Node::let_bind(
                            "i4_top1_activation_word",
                            Expr::add(
                                Expr::mul(Expr::var("i4_top1_batch"), Expr::u32(words_per_row)),
                                Expr::div(Expr::var("i4_top1_col"), Expr::u32(I4_LANES_PER_WORD)),
                            ),
                        ),
                        Node::let_bind(
                            "i4_top1_shift",
                            Expr::mul(
                                Expr::rem(Expr::var("i4_top1_col"), Expr::u32(I4_LANES_PER_WORD)),
                                Expr::u32(4),
                            ),
                        ),
                        Node::let_bind(
                            "i4_top1_weight_nibble",
                            Expr::bitand(
                                Expr::shr(
                                    Expr::load(weights_packed, Expr::var("i4_top1_weight_word")),
                                    Expr::var("i4_top1_shift"),
                                ),
                                Expr::u32(0xF),
                            ),
                        ),
                        Node::let_bind(
                            "i4_top1_activation_nibble",
                            Expr::bitand(
                                Expr::shr(
                                    Expr::load(
                                        activation_batches_packed,
                                        Expr::var("i4_top1_activation_word"),
                                    ),
                                    Expr::var("i4_top1_shift"),
                                ),
                                Expr::u32(0xF),
                            ),
                        ),
                        Node::let_bind(
                            "i4_top1_weight",
                            signed_i4_nibble_f32_expr(Expr::var("i4_top1_weight_nibble")),
                        ),
                        Node::let_bind(
                            "i4_top1_activation",
                            signed_i4_nibble_f32_expr(Expr::var("i4_top1_activation_nibble")),
                        ),
                        Node::assign(
                            "i4_top1_acc",
                            Expr::add(
                                Expr::var("i4_top1_acc"),
                                Expr::mul(
                                    Expr::var("i4_top1_weight"),
                                    Expr::var("i4_top1_activation"),
                                ),
                            ),
                        ),
                    ],
                ),
                Node::let_bind(
                    "i4_top1_score",
                    Expr::mul(
                        Expr::mul(
                            Expr::var("i4_top1_acc"),
                            Expr::load(row_scales, Expr::var("i4_top1_row")),
                        ),
                        Expr::load(batch_scales, Expr::var("i4_top1_batch")),
                    ),
                ),
                Node::if_then(
                    Expr::lt(Expr::var("i4_top1_best_score"), Expr::var("i4_top1_score")),
                    vec![
                        Node::assign("i4_top1_best_score", Expr::var("i4_top1_score")),
                        Node::assign("i4_top1_best_index", Expr::var("i4_top1_row")),
                    ],
                ),
            ],
        ),
        Node::store(
            out,
            Expr::var("i4_top1_batch"),
            Expr::var("i4_top1_best_score"),
        ),
        Node::store(
            out,
            Expr::add(Expr::u32(batch), Expr::var("i4_top1_batch")),
            Expr::cast(DataType::F32, Expr::var("i4_top1_best_index")),
        ),
    ];

    Program::wrapped(
        vec![
            BufferDecl::storage(weights_packed, 0, BufferAccess::ReadOnly, DataType::U32)
                .with_count(rows * words_per_row),
            BufferDecl::storage(
                activation_batches_packed,
                1,
                BufferAccess::ReadOnly,
                DataType::U32,
            )
            .with_count(batch * words_per_row),
            BufferDecl::storage(row_scales, 2, BufferAccess::ReadOnly, DataType::F32)
                .with_count(rows),
            BufferDecl::storage(batch_scales, 3, BufferAccess::ReadOnly, DataType::F32)
                .with_count(batch),
            BufferDecl::output(out, 4, DataType::F32).with_count(batch * 2),
        ],
        [64, 1, 1],
        vec![Node::Region {
            generator: Ident::from(I4_BATCHED_MATMUL_TOP1_F32_SCALED_OP_ID),
            source_region: None,
            body: Arc::new(vec![Node::if_then(
                Expr::lt(batch_index, Expr::u32(batch)),
                body,
            )]),
        }],
    )
}
