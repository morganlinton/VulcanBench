//! GGML K-Quants dequantization primitives.
//!
//! Supports Q2_K, Q4_K, Q6_K block formats used by llama.cpp/GGUF.
//! These are block-wise quantization formats with per-block (or per-super-block)
//! scales and zero-points.
//!
//! Category A composition.

use vyre::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

use crate::region::wrap_anonymous;

// ---------------------------------------------------------------------------
// Q4_K: "type-1" 4-bit quantization
// Super-blocks: 8 blocks per super-block
// Block: 32 weights per block
// Scales/mins: quantized with 6 bits each
// Total: 4.5 bits per weight
// ---------------------------------------------------------------------------

/// Q4_K super-block layout constants.
/** Q4_K super-block size: 8 blocks × 32 weights = 256 weights. */
pub const Q4_K_SUPER_BLOCK_SIZE: u32 = 256;
/** Q4_K block size in weights. */
pub const Q4_K_BLOCK_SIZE: u32 = 32;
/** Q4_K blocks per super-block. */
pub const Q4_K_BLOCKS_PER_SUPER: u32 = 8;

/// Dequantize Q4_K weights.
///
/// Buffer layout (per super-block):
///   - bytes 0..1:   scale_min_low (u16)  -  low 6 bits of 8 scales + 8 mins
///   - bytes 2..3:   scale_min_high (u16)  -  high bits
///   - bytes 4..11:  8 scales (6-bit each, packed)
///   - bytes 12..19: 8 mins (6-bit each, packed)
///   - bytes 20..147: 256 nibbles (128 bytes) = 8 blocks * 32 weights * 4 bits
///
/// For simplicity, this kernel assumes pre-unpacked scales/mins buffers
/// produced by the loader. The `scales` and `mins` buffers are F32
/// with one element per block.
///
/// `packed` contains the 4-bit nibbles, 2 per byte, stored as U32 words
/// for aligned access.
pub fn q4_k_unpack(
    packed: &str,
    scales: &str,
    mins: &str,
    output: &str,
    n: u32,
) -> Result<Program, String> {
    if n == 0 {
        return Err("Fix: q4_k_unpack n=0 is invalid".to_string());
    }
    let n_blocks = n.div_ceil(Q4_K_BLOCK_SIZE);

    let i = Expr::var("i");
    let body = vec![
        Node::let_bind("i", Expr::InvocationId { axis: 0 }),
        Node::if_then(
            Expr::lt(i.clone(), Expr::u32(n)),
            vec![
                // block_idx = i / 32
                Node::let_bind(
                    "block_idx",
                    Expr::div(i.clone(), Expr::u32(Q4_K_BLOCK_SIZE)),
                ),
                // within_block = i % 32
                Node::let_bind(
                    "within_block",
                    Expr::rem(i.clone(), Expr::u32(Q4_K_BLOCK_SIZE)),
                ),
                // nibble_idx = within_block (each nibble is one weight)
                // byte_idx = within_block / 2
                // shift = (within_block % 2) * 4
                Node::let_bind(
                    "byte_idx",
                    Expr::div(Expr::var("within_block"), Expr::u32(2)),
                ),
                Node::let_bind(
                    "shift",
                    Expr::mul(
                        Expr::rem(Expr::var("within_block"), Expr::u32(2)),
                        Expr::u32(4),
                    ),
                ),
                // packed_word = packed[block_idx * 16 + byte_idx / 4]
                // Actually: each block has 32 nibbles = 16 bytes = 4 u32 words
                Node::let_bind(
                    "word_idx",
                    Expr::add(
                        Expr::mul(Expr::var("block_idx"), Expr::u32(4)),
                        Expr::div(Expr::var("byte_idx"), Expr::u32(4)),
                    ),
                ),
                Node::let_bind(
                    "word_shift",
                    Expr::mul(Expr::rem(Expr::var("byte_idx"), Expr::u32(4)), Expr::u32(8)),
                ),
                Node::let_bind("packed_word", Expr::load(packed, Expr::var("word_idx"))),
                // Extract the byte containing our nibble
                Node::let_bind(
                    "byte_val",
                    Expr::bitand(
                        Expr::shr(Expr::var("packed_word"), Expr::var("word_shift")),
                        Expr::u32(0xFF),
                    ),
                ),
                // Extract the nibble
                Node::let_bind(
                    "nibble",
                    Expr::bitand(
                        Expr::shr(Expr::var("byte_val"), Expr::var("shift")),
                        Expr::u32(0xF),
                    ),
                ),
                // dequant = (nibble * scale + min) where scale/min are per-block
                Node::let_bind("scale", Expr::load(scales, Expr::var("block_idx"))),
                Node::let_bind("min", Expr::load(mins, Expr::var("block_idx"))),
                Node::Store {
                    buffer: output.into(),
                    index: i,
                    value: Expr::add(
                        Expr::mul(
                            Expr::cast(DataType::F32, Expr::var("nibble")),
                            Expr::var("scale"),
                        ),
                        Expr::var("min"),
                    ),
                },
            ],
        ),
    ];

    let packed_count = n_blocks * 4; // 4 u32 words per block

    Ok(Program::wrapped(
        vec![
            BufferDecl::storage(packed, 0, BufferAccess::ReadOnly, DataType::U32)
                .with_count(packed_count),
            BufferDecl::storage(scales, 1, BufferAccess::ReadOnly, DataType::F32)
                .with_count(n_blocks),
            BufferDecl::storage(mins, 2, BufferAccess::ReadOnly, DataType::F32)
                .with_count(n_blocks),
            BufferDecl::output(output, 3, DataType::F32).with_count(n),
        ],
        [256, 1, 1],
        vec![wrap_anonymous("vyre-libs::quant::q4_k_unpack", body)],
    ))
}

// ---------------------------------------------------------------------------
// Q2_K: "type-1" 2-bit quantization
// Super-blocks: 16 blocks per super-block
// Block: 16 weights per block
// Scales/mins: quantized with 4 bits each
// Total: 2.5625 bits per weight
// ---------------------------------------------------------------------------

/** Q2_K super-block size: 16 blocks × 16 weights = 256 weights. */
pub const Q2_K_SUPER_BLOCK_SIZE: u32 = 256;
/** Q2_K block size in weights. */
pub const Q2_K_BLOCK_SIZE: u32 = 16;
/** Q2_K blocks per super-block. */
pub const Q2_K_BLOCKS_PER_SUPER: u32 = 16;

/// Dequantize Q2_K weights.
///
/// `packed` contains 2-bit values, 4 per byte, stored as U32 words.
/// `scales` and `mins` are per-block F32 values.
pub fn q2_k_unpack(
    packed: &str,
    scales: &str,
    mins: &str,
    output: &str,
    n: u32,
) -> Result<Program, String> {
    if n == 0 {
        return Err("Fix: q2_k_unpack n=0 is invalid".to_string());
    }
    let n_blocks = n.div_ceil(Q2_K_BLOCK_SIZE);

    let i = Expr::var("i");
    let body = vec![
        Node::let_bind("i", Expr::InvocationId { axis: 0 }),
        Node::if_then(
            Expr::lt(i.clone(), Expr::u32(n)),
            vec![
                // block_idx = i / 16
                Node::let_bind(
                    "block_idx",
                    Expr::div(i.clone(), Expr::u32(Q2_K_BLOCK_SIZE)),
                ),
                // within_block = i % 16
                Node::let_bind(
                    "within_block",
                    Expr::rem(i.clone(), Expr::u32(Q2_K_BLOCK_SIZE)),
                ),
                // byte_idx = within_block / 4
                // shift = (within_block % 4) * 2
                Node::let_bind(
                    "byte_idx",
                    Expr::div(Expr::var("within_block"), Expr::u32(4)),
                ),
                Node::let_bind(
                    "shift",
                    Expr::mul(
                        Expr::rem(Expr::var("within_block"), Expr::u32(4)),
                        Expr::u32(2),
                    ),
                ),
                // Each block has 16 weights = 4 bytes = 1 u32 word
                Node::let_bind("word", Expr::load(packed, Expr::var("block_idx"))),
                // Extract byte, then 2-bit value
                Node::let_bind(
                    "byte_val",
                    Expr::bitand(
                        Expr::shr(
                            Expr::var("word"),
                            Expr::mul(Expr::var("byte_idx"), Expr::u32(8)),
                        ),
                        Expr::u32(0xFF),
                    ),
                ),
                Node::let_bind(
                    "q2",
                    Expr::bitand(
                        Expr::shr(Expr::var("byte_val"), Expr::var("shift")),
                        Expr::u32(0x3),
                    ),
                ),
                // dequant = q2 * scale + min
                Node::let_bind("scale", Expr::load(scales, Expr::var("block_idx"))),
                Node::let_bind("min", Expr::load(mins, Expr::var("block_idx"))),
                Node::Store {
                    buffer: output.into(),
                    index: i,
                    value: Expr::add(
                        Expr::mul(
                            Expr::cast(DataType::F32, Expr::var("q2")),
                            Expr::var("scale"),
                        ),
                        Expr::var("min"),
                    ),
                },
            ],
        ),
    ];

    Ok(Program::wrapped(
        vec![
            BufferDecl::storage(packed, 0, BufferAccess::ReadOnly, DataType::U32)
                .with_count(n_blocks),
            BufferDecl::storage(scales, 1, BufferAccess::ReadOnly, DataType::F32)
                .with_count(n_blocks),
            BufferDecl::storage(mins, 2, BufferAccess::ReadOnly, DataType::F32)
                .with_count(n_blocks),
            BufferDecl::output(output, 3, DataType::F32).with_count(n),
        ],
        [256, 1, 1],
        vec![wrap_anonymous("vyre-libs::quant::q2_k_unpack", body)],
    ))
}

// ---------------------------------------------------------------------------
// Fused dequant + matmul for Q4_K and Q2_K
// These avoid materializing the full dequantized buffer.
// ---------------------------------------------------------------------------

/// Fused Q4_K dequant + linear: `out = x @ dequant(w_q4k) + b`
///
/// `w_packed` is Q4_K packed nibbles (U32 words).
/// `w_scales` and `w_mins` are per-block F32.
/// `x` is F32 input, `b` is F32 bias, `out` is F32 output.
pub fn q4_k_linear(
    x: &str,
    w_packed: &str,
    w_scales: &str,
    w_mins: &str,
    b: &str,
    out: &str,
    in_dim: u32,
    out_dim: u32,
) -> Result<Program, String> {
    if in_dim == 0 || out_dim == 0 {
        return Err("Fix: q4_k_linear all dims must be > 0".to_string());
    }
    let n_blocks = in_dim.div_ceil(Q4_K_BLOCK_SIZE);

    let i = Expr::var("i");
    let body = vec![
        Node::let_bind("i", Expr::InvocationId { axis: 0 }),
        Node::if_then(
            Expr::lt(i.clone(), Expr::u32(out_dim)),
            vec![
                Node::let_bind("acc", Expr::load(b, i.clone())),
                Node::loop_for(
                    "k",
                    Expr::u32(0),
                    Expr::u32(in_dim),
                    vec![
                        // linear_idx = k * out_dim + i
                        Node::let_bind(
                            "linear_idx",
                            Expr::add(Expr::mul(Expr::var("k"), Expr::u32(out_dim)), i.clone()),
                        ),
                        // block_idx = linear_idx / 32
                        Node::let_bind(
                            "block_idx",
                            Expr::div(Expr::var("linear_idx"), Expr::u32(Q4_K_BLOCK_SIZE)),
                        ),
                        // within_block = linear_idx % 32
                        Node::let_bind(
                            "within_block",
                            Expr::rem(Expr::var("linear_idx"), Expr::u32(Q4_K_BLOCK_SIZE)),
                        ),
                        // byte_idx = within_block / 2
                        Node::let_bind(
                            "byte_idx",
                            Expr::div(Expr::var("within_block"), Expr::u32(2)),
                        ),
                        // shift = (within_block % 2) * 4
                        Node::let_bind(
                            "shift",
                            Expr::mul(
                                Expr::rem(Expr::var("within_block"), Expr::u32(2)),
                                Expr::u32(4),
                            ),
                        ),
                        // word_idx = block_idx * 4 + byte_idx / 4
                        Node::let_bind(
                            "word_idx",
                            Expr::add(
                                Expr::mul(Expr::var("block_idx"), Expr::u32(4)),
                                Expr::div(Expr::var("byte_idx"), Expr::u32(4)),
                            ),
                        ),
                        Node::let_bind(
                            "word_shift",
                            Expr::mul(Expr::rem(Expr::var("byte_idx"), Expr::u32(4)), Expr::u32(8)),
                        ),
                        Node::let_bind("packed_word", Expr::load(w_packed, Expr::var("word_idx"))),
                        Node::let_bind(
                            "byte_val",
                            Expr::bitand(
                                Expr::shr(Expr::var("packed_word"), Expr::var("word_shift")),
                                Expr::u32(0xFF),
                            ),
                        ),
                        Node::let_bind(
                            "nibble",
                            Expr::bitand(
                                Expr::shr(Expr::var("byte_val"), Expr::var("shift")),
                                Expr::u32(0xF),
                            ),
                        ),
                        // scale/min for this block
                        Node::let_bind("scale", Expr::load(w_scales, Expr::var("block_idx"))),
                        Node::let_bind("min", Expr::load(w_mins, Expr::var("block_idx"))),
                        // weight = nibble * scale + min
                        Node::let_bind(
                            "weight",
                            Expr::add(
                                Expr::mul(
                                    Expr::cast(DataType::F32, Expr::var("nibble")),
                                    Expr::var("scale"),
                                ),
                                Expr::var("min"),
                            ),
                        ),
                        // acc += x[k] * weight
                        Node::assign(
                            "acc",
                            Expr::add(
                                Expr::var("acc"),
                                Expr::mul(Expr::load(x, Expr::var("k")), Expr::var("weight")),
                            ),
                        ),
                    ],
                ),
                Node::Store {
                    buffer: out.into(),
                    index: i,
                    value: Expr::var("acc"),
                },
            ],
        ),
    ];

    Ok(Program::wrapped(
        vec![
            BufferDecl::storage(x, 0, BufferAccess::ReadOnly, DataType::F32).with_count(in_dim),
            BufferDecl::storage(w_packed, 1, BufferAccess::ReadOnly, DataType::U32)
                .with_count(n_blocks * 4),
            BufferDecl::storage(w_scales, 2, BufferAccess::ReadOnly, DataType::F32)
                .with_count(n_blocks),
            BufferDecl::storage(w_mins, 3, BufferAccess::ReadOnly, DataType::F32)
                .with_count(n_blocks),
            BufferDecl::storage(b, 4, BufferAccess::ReadOnly, DataType::F32).with_count(out_dim),
            BufferDecl::output(out, 5, DataType::F32).with_count(out_dim),
        ],
        [64, 1, 1],
        vec![wrap_anonymous("vyre-libs::quant::q4_k_linear", body)],
    ))
}

/// Fused Q2_K dequant + linear: `out = x @ dequant(w_q2k) + b`
pub fn q2_k_linear(
    x: &str,
    w_packed: &str,
    w_scales: &str,
    w_mins: &str,
    b: &str,
    out: &str,
    in_dim: u32,
    out_dim: u32,
) -> Result<Program, String> {
    if in_dim == 0 || out_dim == 0 {
        return Err("Fix: q2_k_linear all dims must be > 0".to_string());
    }
    let n_blocks = in_dim
        .checked_mul(out_dim)
        .ok_or("overflow")?
        .div_ceil(Q2_K_BLOCK_SIZE);

    let i = Expr::var("i");
    let body = vec![
        Node::let_bind("i", Expr::InvocationId { axis: 0 }),
        Node::if_then(
            Expr::lt(i.clone(), Expr::u32(out_dim)),
            vec![
                Node::let_bind("acc", Expr::load(b, i.clone())),
                Node::loop_for(
                    "k",
                    Expr::u32(0),
                    Expr::u32(in_dim),
                    vec![
                        Node::let_bind(
                            "linear_idx",
                            Expr::add(Expr::mul(Expr::var("k"), Expr::u32(out_dim)), i.clone()),
                        ),
                        Node::let_bind(
                            "block_idx",
                            Expr::div(Expr::var("linear_idx"), Expr::u32(Q2_K_BLOCK_SIZE)),
                        ),
                        Node::let_bind(
                            "within_block",
                            Expr::rem(Expr::var("linear_idx"), Expr::u32(Q2_K_BLOCK_SIZE)),
                        ),
                        Node::let_bind(
                            "byte_idx",
                            Expr::div(Expr::var("within_block"), Expr::u32(4)),
                        ),
                        Node::let_bind(
                            "shift",
                            Expr::mul(
                                Expr::rem(Expr::var("within_block"), Expr::u32(4)),
                                Expr::u32(2),
                            ),
                        ),
                        Node::let_bind("word", Expr::load(w_packed, Expr::var("block_idx"))),
                        Node::let_bind(
                            "byte_val",
                            Expr::bitand(
                                Expr::shr(
                                    Expr::var("word"),
                                    Expr::mul(Expr::var("byte_idx"), Expr::u32(8)),
                                ),
                                Expr::u32(0xFF),
                            ),
                        ),
                        Node::let_bind(
                            "q2",
                            Expr::bitand(
                                Expr::shr(Expr::var("byte_val"), Expr::var("shift")),
                                Expr::u32(0x3),
                            ),
                        ),
                        Node::let_bind("scale", Expr::load(w_scales, Expr::var("block_idx"))),
                        Node::let_bind("min", Expr::load(w_mins, Expr::var("block_idx"))),
                        Node::let_bind(
                            "weight",
                            Expr::add(
                                Expr::mul(
                                    Expr::cast(DataType::F32, Expr::var("q2")),
                                    Expr::var("scale"),
                                ),
                                Expr::var("min"),
                            ),
                        ),
                        Node::assign(
                            "acc",
                            Expr::add(
                                Expr::var("acc"),
                                Expr::mul(Expr::load(x, Expr::var("k")), Expr::var("weight")),
                            ),
                        ),
                    ],
                ),
                Node::Store {
                    buffer: out.into(),
                    index: i,
                    value: Expr::var("acc"),
                },
            ],
        ),
    ];

    Ok(Program::wrapped(
        vec![
            BufferDecl::storage(x, 0, BufferAccess::ReadOnly, DataType::F32).with_count(in_dim),
            BufferDecl::storage(w_packed, 1, BufferAccess::ReadOnly, DataType::U32)
                .with_count(n_blocks),
            BufferDecl::storage(w_scales, 2, BufferAccess::ReadOnly, DataType::F32)
                .with_count(n_blocks),
            BufferDecl::storage(w_mins, 3, BufferAccess::ReadOnly, DataType::F32)
                .with_count(n_blocks),
            BufferDecl::storage(b, 4, BufferAccess::ReadOnly, DataType::F32).with_count(out_dim),
            BufferDecl::output(out, 5, DataType::F32).with_count(out_dim),
        ],
        [64, 1, 1],
        vec![wrap_anonymous("vyre-libs::quant::q2_k_linear", body)],
    ))
}

#[cfg(test)]

mod tests {
    use super::*;
    use crate::test_support::byte_pack::decode_f32;
    use crate::test_support::byte_pack::f32_bytes;
    use crate::test_support::byte_pack::u32_bytes;
    use vyre_reference::value::Value;

    #[test]
    fn q4_k_unpack_simple() {
        // 32 weights, 1 block
        // scales = [1.0], mins = [0.0]
        // packed: 16 bytes = 4 u32 words
        // nibbles: 0,1,2,3,...,15 (first 16 weights), then repeat
        let scales = vec![1.0f32];
        let mins = vec![0.0f32];
        let packed = vec![0x7654_3210u32, 0xFEDC_BA98, 0x0, 0x0];
        let program = q4_k_unpack("packed", "scales", "mins", "out", 16).unwrap();
        let outputs = vyre_reference::reference_eval(
            &program,
            &[
                Value::from(u32_bytes(&packed)),
                Value::from(f32_bytes(&scales)),
                Value::from(f32_bytes(&mins)),
                Value::from(vec![0u8; 64]),
            ],
        )
        .expect("Fix: q4_k_unpack must execute");
        let out = decode_f32(&outputs[0].to_bytes());
        assert_eq!(out[0], 0.0);
        assert_eq!(out[1], 1.0);
        assert_eq!(out[2], 2.0);
        assert_eq!(out[15], 15.0);
    }

    #[test]
    fn q2_k_unpack_simple() {
        // 16 weights, 1 block
        // scales = [1.0], mins = [0.0]
        // packed: 1 u32 word containing 16 2-bit values
        // q2 values: 0,1,2,3,0,1,2,3,... (4 bytes)
        let scales = vec![1.0f32];
        let mins = vec![0.0f32];
        let packed = vec![0xE4E4_E4E4u32]; // 11_10_01_00 repeated
        let program = q2_k_unpack("packed", "scales", "mins", "out", 16).unwrap();
        let outputs = vyre_reference::reference_eval(
            &program,
            &[
                Value::from(u32_bytes(&packed)),
                Value::from(f32_bytes(&scales)),
                Value::from(f32_bytes(&mins)),
                Value::from(vec![0u8; 64]),
            ],
        )
        .expect("Fix: q2_k_unpack must execute");
        let out = decode_f32(&outputs[0].to_bytes());
        // Byte pattern: 0xE4 = 11_10_01_00 -> values 0,1,2,3
        assert_eq!(out[0], 0.0);
        assert_eq!(out[1], 1.0);
        assert_eq!(out[2], 2.0);
        assert_eq!(out[3], 3.0);
    }

    #[test]
    fn q4_k_linear_simple() {
        // in_dim=2, out_dim=2
        // weights = [[0,1],[2,3]] in row-major
        // x = [1.0, 0.0], b = [0.0, 0.0]
        // out[0] = 1*0 + 0*2 = 0
        // out[1] = 1*1 + 0*3 = 1
        let x = vec![1.0f32, 0.0];
        let b = vec![0.0f32, 0.0];
        // linear_idx 0: nibble=0, linear_idx 1: nibble=1
        // linear_idx 2: nibble=2, linear_idx 3: nibble=3
        // All in one block (4 < 32)
        // byte 0 = 0x10 (nibble0=0, nibble1=1)
        // byte 1 = 0x32 (nibble2=2, nibble3=3)
        // Little-endian u32: bytes [0x10, 0x32, 0x00, 0x00] = 0x0000_3210
        let packed = vec![0x0000_3210u32, 0, 0, 0];
        let scales = vec![1.0f32];
        let mins = vec![0.0f32];
        let program = q4_k_linear("x", "packed", "scales", "mins", "b", "out", 2, 2).unwrap();
        let outputs = vyre_reference::reference_eval(
            &program,
            &[
                Value::from(f32_bytes(&x)),
                Value::from(u32_bytes(&packed)),
                Value::from(f32_bytes(&scales)),
                Value::from(f32_bytes(&mins)),
                Value::from(f32_bytes(&b)),
                Value::from(vec![0u8; 8]),
            ],
        )
        .expect("Fix: q4_k_linear must execute");
        let out = decode_f32(&outputs[0].to_bytes());
        assert_eq!(out[0], 0.0);
        assert_eq!(out[1], 1.0);
    }
}
