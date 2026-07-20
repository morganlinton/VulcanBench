//! Multi-head Latent Attention (MLA).
//!
//! DeepSeek V4 Flash uses MLA with compressed KV cache. The key insight:
//! instead of caching full K and V tensors per head, MLA compresses them
//! into a low-rank latent vector c_t, then projects back at attention time.
//!
//! Formulation (simplified for single-token decode):
//!   c_t = W_DK @ h_t                    (compress, dim = kv_lora_rank)
//!   k_t = W_UK @ c_t + W_KR @ h_t       (decompress K, with decoupled RoPE)
//!   v_t = W_UV @ c_t                    (decompress V)
//!   o_t = softmax(q_t @ K^T / sqrt(d)) @ V
//!
//! The KV cache stores only c_t (and optionally the RoPE-decoupled key
//! component). For long context this reduces cache size by ~93%.
//!
//! Category A composition.

use vyre::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program, UnOp};

use crate::region::wrap_anonymous;
use vyre_primitives::nn::attention_stability::{
    bounded_exp_arg, bounded_score, flush_tiny, positive_denominator,
};

/// MLA single-token decode with compressed KV cache.
///
/// This computes one step of autoregressive decode: given the current
/// token's query and the full compressed KV cache, produce the attention
/// output for this token.
///
/// Shapes:
///   `q: [num_heads, head_dim]`  -  query vectors for current token
///   `kv_cache: [seq_len, kv_lora_rank]`  -  compressed KV for all prior tokens
///   `kr_cache: [seq_len, qk_rope_head_dim]`  -  decoupled RoPE keys for prior tokens
///   `w_uk: [kv_lora_rank, num_heads * head_dim]`  -  K up-projection
///   `w_uv: [kv_lora_rank, num_heads * head_dim]`  -  V up-projection
///   `out: [num_heads, head_dim]`  -  attention output
///
/// # Errors
/// Returns `Err` when any dimension is zero.
#[allow(clippy::too_many_arguments)]
pub fn mla_decode(
    q: &str,
    kv_cache: &str,
    kr_cache: &str,
    w_uk: &str,
    w_uv: &str,
    out: &str,
    seq_len: u32,
    num_heads: u32,
    head_dim: u32,
    kv_lora_rank: u32,
    qk_rope_head_dim: u32,
) -> Result<Program, String> {
    if seq_len == 0 || num_heads == 0 || head_dim == 0 || kv_lora_rank == 0 || qk_rope_head_dim == 0
    {
        return Err("Fix: mla_decode all dims must be > 0".to_string());
    }

    const WORKGROUP_LANES: u32 = 64;
    const TILE_SIZE: u32 = 64;

    let head_stride = head_dim;
    let uv_stride = num_heads.checked_mul(head_dim).ok_or("overflow")?;

    let q_scratch_count = WORKGROUP_LANES.checked_mul(head_dim).ok_or("overflow")?;
    let score_scratch_count = WORKGROUP_LANES.checked_mul(TILE_SIZE).ok_or("overflow")?;
    let o_acc_count = WORKGROUP_LANES.checked_mul(head_dim).ok_or("overflow")?;

    let scale = 1.0f32 / (head_dim as f32).sqrt();
    let scale_expr = Expr::f32(scale);
    let num_tiles = seq_len.div_ceil(TILE_SIZE);

    // Scratch index helpers: each lane gets its own sub-slice.
    let q_idx = |local: Expr, d: Expr| Expr::add(Expr::mul(local.clone(), Expr::u32(head_dim)), d);
    let score_idx =
        |local: Expr, j: Expr| Expr::add(Expr::mul(local.clone(), Expr::u32(TILE_SIZE)), j);
    let o_idx = |local: Expr, d: Expr| Expr::add(Expr::mul(local.clone(), Expr::u32(head_dim)), d);

    // ---- Load the query vector for this head into workgroup scratch ----
    let load_q = vec![Node::loop_for(
        "load_d",
        Expr::u32(0),
        Expr::u32(head_dim),
        vec![Node::store(
            "q_scratch",
            q_idx(Expr::var("local"), Expr::var("load_d")),
            Expr::load(
                q,
                Expr::add(
                    Expr::mul(Expr::var("head"), Expr::u32(head_dim)),
                    Expr::var("load_d"),
                ),
            ),
        )],
    )];

    // ---- Zero the per-head output accumulator ----
    let zero_o_acc = vec![Node::loop_for(
        "zero_d",
        Expr::u32(0),
        Expr::u32(head_dim),
        vec![Node::store(
            "o_acc",
            o_idx(Expr::var("local"), Expr::var("zero_d")),
            Expr::f32(0.0),
        )],
    )];

    // ---- Compute all scores for the current tile ----
    let compute_tile_scores = vec![Node::loop_for(
        "tile_j",
        Expr::u32(0),
        Expr::var("tile_len"),
        vec![
            // decompress k_t on the fly and accumulate dot product
            Node::let_bind("dot_val", Expr::f32(0.0)),
            Node::loop_for(
                "dim",
                Expr::u32(0),
                Expr::u32(head_dim),
                vec![
                    Node::let_bind("k_val", Expr::f32(0.0)),
                    Node::loop_for(
                        "r",
                        Expr::u32(0),
                        Expr::u32(kv_lora_rank),
                        vec![Node::assign(
                            "k_val",
                            Expr::add(
                                Expr::var("k_val"),
                                Expr::mul(
                                    Expr::load(
                                        w_uk,
                                        Expr::add(
                                            Expr::mul(Expr::var("r"), Expr::u32(uv_stride)),
                                            Expr::add(
                                                Expr::mul(
                                                    Expr::var("head"),
                                                    Expr::u32(head_stride),
                                                ),
                                                Expr::var("dim"),
                                            ),
                                        ),
                                    ),
                                    Expr::load(
                                        kv_cache,
                                        Expr::add(
                                            Expr::mul(
                                                Expr::add(
                                                    Expr::var("tile_start"),
                                                    Expr::var("tile_j"),
                                                ),
                                                Expr::u32(kv_lora_rank),
                                            ),
                                            Expr::var("r"),
                                        ),
                                    ),
                                ),
                            ),
                        )],
                    ),
                    Node::if_then(
                        Expr::lt(Expr::var("dim"), Expr::u32(qk_rope_head_dim)),
                        vec![Node::assign(
                            "k_val",
                            Expr::add(
                                Expr::var("k_val"),
                                Expr::load(
                                    kr_cache,
                                    Expr::add(
                                        Expr::mul(
                                            Expr::add(Expr::var("tile_start"), Expr::var("tile_j")),
                                            Expr::u32(qk_rope_head_dim),
                                        ),
                                        Expr::var("dim"),
                                    ),
                                ),
                            ),
                        )],
                    ),
                    Node::assign(
                        "dot_val",
                        Expr::add(
                            Expr::var("dot_val"),
                            Expr::mul(
                                Expr::load(
                                    "q_scratch",
                                    q_idx(Expr::var("local"), Expr::var("dim")),
                                ),
                                Expr::var("k_val"),
                            ),
                        ),
                    ),
                ],
            ),
            Node::let_bind(
                "raw_score",
                Expr::mul(Expr::var("dot_val"), scale_expr.clone()),
            ),
            Node::let_bind("score", bounded_score(Expr::var("raw_score"))),
            Node::store(
                "score_tile",
                score_idx(Expr::var("local"), Expr::var("tile_j")),
                Expr::var("score"),
            ),
        ],
    )];

    // ---- Find max score inside the tile ----
    let find_tile_max = vec![
        Node::let_bind("tile_max", Expr::f32(f32::MIN)),
        Node::loop_for(
            "max_j",
            Expr::u32(0),
            Expr::var("tile_len"),
            vec![Node::assign(
                "tile_max",
                Expr::select(
                    Expr::is_nan(Expr::var("tile_max")),
                    Expr::var("tile_max"),
                    Expr::select(
                        Expr::gt(
                            Expr::load(
                                "score_tile",
                                score_idx(Expr::var("local"), Expr::var("max_j")),
                            ),
                            Expr::var("tile_max"),
                        ),
                        Expr::load(
                            "score_tile",
                            score_idx(Expr::var("local"), Expr::var("max_j")),
                        ),
                        Expr::var("tile_max"),
                    ),
                ),
            )],
        ),
    ];

    // ---- m_new = max(m, tile_max) ----
    let compute_m_new = vec![Node::let_bind(
        "m_new",
        Expr::select(
            Expr::gt(Expr::var("tile_max"), Expr::var("m")),
            Expr::var("tile_max"),
            Expr::var("m"),
        ),
    )];

    // ---- rescale = exp(m - m_new) ----
    let compute_rescale = vec![Node::let_bind(
        "rescale",
        Expr::UnOp {
            op: UnOp::Exp,
            operand: Box::new(bounded_exp_arg(Expr::sub(
                Expr::var("m"),
                Expr::var("m_new"),
            ))),
        },
    )];

    // ---- tile_sum = sum_j exp(score[j] - m_new) ----
    let compute_tile_sum = vec![
        Node::let_bind("tile_sum", Expr::f32(0.0)),
        Node::loop_for(
            "sum_j",
            Expr::u32(0),
            Expr::var("tile_len"),
            vec![Node::assign(
                "tile_sum",
                Expr::add(
                    Expr::var("tile_sum"),
                    Expr::UnOp {
                        op: UnOp::Exp,
                        operand: Box::new(bounded_exp_arg(Expr::sub(
                            Expr::load(
                                "score_tile",
                                score_idx(Expr::var("local"), Expr::var("sum_j")),
                            ),
                            Expr::var("m_new"),
                        ))),
                    },
                ),
            )],
        ),
    ];

    // ---- l = rescale * l + tile_sum ----
    let update_l = vec![Node::assign(
        "l",
        Expr::add(
            Expr::mul(Expr::var("rescale"), Expr::var("l")),
            Expr::var("tile_sum"),
        ),
    )];

    // ---- o_acc[d] = rescale * o_acc[d] + sum_j weight_j * v_t_j[d] ----
    let update_o_acc = vec![
        // rescale existing accumulator
        Node::loop_for(
            "rescale_d",
            Expr::u32(0),
            Expr::u32(head_dim),
            vec![Node::store(
                "o_acc",
                o_idx(Expr::var("local"), Expr::var("rescale_d")),
                Expr::mul(
                    Expr::var("rescale"),
                    Expr::load("o_acc", o_idx(Expr::var("local"), Expr::var("rescale_d"))),
                ),
            )],
        ),
        // iterate tokens in tile
        Node::loop_for(
            "v_j",
            Expr::u32(0),
            Expr::var("tile_len"),
            vec![
                Node::let_bind(
                    "weight",
                    Expr::UnOp {
                        op: UnOp::Exp,
                        operand: Box::new(bounded_exp_arg(Expr::sub(
                            Expr::load(
                                "score_tile",
                                score_idx(Expr::var("local"), Expr::var("v_j")),
                            ),
                            Expr::var("m_new"),
                        ))),
                    },
                ),
                // for each dimension, decompress v and accumulate
                Node::loop_for(
                    "v_dim",
                    Expr::u32(0),
                    Expr::u32(head_dim),
                    vec![
                        Node::let_bind("v_val", Expr::f32(0.0)),
                        Node::loop_for(
                            "r",
                            Expr::u32(0),
                            Expr::u32(kv_lora_rank),
                            vec![Node::assign(
                                "v_val",
                                Expr::add(
                                    Expr::var("v_val"),
                                    Expr::mul(
                                        Expr::load(
                                            w_uv,
                                            Expr::add(
                                                Expr::mul(Expr::var("r"), Expr::u32(uv_stride)),
                                                Expr::add(
                                                    Expr::mul(
                                                        Expr::var("head"),
                                                        Expr::u32(head_stride),
                                                    ),
                                                    Expr::var("v_dim"),
                                                ),
                                            ),
                                        ),
                                        Expr::load(
                                            kv_cache,
                                            Expr::add(
                                                Expr::mul(
                                                    Expr::add(
                                                        Expr::var("tile_start"),
                                                        Expr::var("v_j"),
                                                    ),
                                                    Expr::u32(kv_lora_rank),
                                                ),
                                                Expr::var("r"),
                                            ),
                                        ),
                                    ),
                                ),
                            )],
                        ),
                        Node::store(
                            "o_acc",
                            o_idx(Expr::var("local"), Expr::var("v_dim")),
                            Expr::add(
                                Expr::load("o_acc", o_idx(Expr::var("local"), Expr::var("v_dim"))),
                                Expr::mul(Expr::var("weight"), Expr::var("v_val")),
                            ),
                        ),
                    ],
                ),
            ],
        ),
    ];

    // ---- m = m_new ----
    let update_m = vec![Node::assign("m", Expr::var("m_new"))];

    // Assemble the per-tile body
    let mut tile_body = vec![
        Node::let_bind(
            "tile_start",
            Expr::mul(Expr::var("tile_idx"), Expr::u32(TILE_SIZE)),
        ),
        Node::let_bind(
            "tile_end",
            Expr::min(
                Expr::add(Expr::var("tile_start"), Expr::u32(TILE_SIZE)),
                Expr::u32(seq_len),
            ),
        ),
        Node::let_bind(
            "tile_len",
            Expr::sub(Expr::var("tile_end"), Expr::var("tile_start")),
        ),
    ];
    tile_body.extend(compute_tile_scores);
    tile_body.extend(find_tile_max);
    tile_body.extend(compute_m_new);
    tile_body.extend(compute_rescale);
    tile_body.extend(compute_tile_sum);
    tile_body.extend(update_l);
    tile_body.extend(update_o_acc);
    tile_body.extend(update_m);

    // Assemble the per-head body
    let mut per_head = Vec::new();
    per_head.extend(load_q);
    per_head.push(Node::let_bind("m", Expr::f32(f32::MIN)));
    per_head.push(Node::let_bind("l", Expr::f32(0.0)));
    per_head.extend(zero_o_acc);
    per_head.push(Node::loop_for(
        "tile_idx",
        Expr::u32(0),
        Expr::u32(num_tiles),
        tile_body,
    ));
    // Finalise: out[head, d] = o_acc[d] / max(l, MIN_POSITIVE)
    per_head.push(Node::let_bind(
        "denom",
        positive_denominator(Expr::var("l")),
    ));
    per_head.push(Node::loop_for(
        "final_d",
        Expr::u32(0),
        Expr::u32(head_dim),
        vec![Node::store(
            out,
            Expr::add(
                Expr::mul(Expr::var("head"), Expr::u32(head_dim)),
                Expr::var("final_d"),
            ),
            flush_tiny(Expr::div(
                Expr::load("o_acc", o_idx(Expr::var("local"), Expr::var("final_d"))),
                Expr::var("denom"),
            )),
        )],
    ));

    let mut body = vec![
        Node::let_bind("head", Expr::InvocationId { axis: 0 }),
        Node::let_bind("local", Expr::LocalId { axis: 0 }),
    ];
    body.push(Node::if_then(
        Expr::lt(Expr::var("head"), Expr::u32(num_heads)),
        per_head,
    ));

    Ok(Program::wrapped(
        vec![
            BufferDecl::storage(q, 0, BufferAccess::ReadOnly, DataType::F32)
                .with_count(num_heads * head_dim),
            BufferDecl::storage(kv_cache, 1, BufferAccess::ReadOnly, DataType::F32)
                .with_count(seq_len * kv_lora_rank),
            BufferDecl::storage(kr_cache, 2, BufferAccess::ReadOnly, DataType::F32)
                .with_count(seq_len * qk_rope_head_dim),
            BufferDecl::storage(w_uk, 3, BufferAccess::ReadOnly, DataType::F32)
                .with_count(kv_lora_rank * uv_stride),
            BufferDecl::storage(w_uv, 4, BufferAccess::ReadOnly, DataType::F32)
                .with_count(kv_lora_rank * uv_stride),
            BufferDecl::workgroup("q_scratch", q_scratch_count, DataType::F32),
            BufferDecl::workgroup("score_tile", score_scratch_count, DataType::F32),
            BufferDecl::workgroup("o_acc", o_acc_count, DataType::F32),
            BufferDecl::output(out, 5, DataType::F32).with_count(num_heads * head_dim),
        ],
        [WORKGROUP_LANES, 1, 1],
        vec![wrap_anonymous("vyre-libs::nn::mla_decode", body)],
    ))
}

/// MLA KV cache compression: `c_t = W_DK @ h_t`.
///
/// Computes the compressed latent vector for the current token
/// to be appended to the KV cache.
///
/// Shapes:
///   `h: [hidden_dim]`  -  current token hidden state
///   `w_dk: [hidden_dim, kv_lora_rank]`  -  down-projection weights
///   `c_out: [kv_lora_rank]`  -  compressed latent output

pub fn mla_compress_kv(
    h: &str,
    w_dk: &str,
    c_out: &str,
    hidden_dim: u32,
    kv_lora_rank: u32,
) -> Result<Program, String> {
    if hidden_dim == 0 || kv_lora_rank == 0 {
        return Err("Fix: mla_compress_kv all dims must be > 0".to_string());
    }

    let i = Expr::var("i");
    let body = vec![
        Node::let_bind("i", Expr::InvocationId { axis: 0 }),
        Node::if_then(
            Expr::lt(i.clone(), Expr::u32(kv_lora_rank)),
            vec![
                Node::let_bind("acc", Expr::f32(0.0)),
                Node::loop_for(
                    "j",
                    Expr::u32(0),
                    Expr::u32(hidden_dim),
                    vec![Node::assign(
                        "acc",
                        Expr::add(
                            Expr::var("acc"),
                            Expr::mul(
                                Expr::load(h, Expr::var("j")),
                                Expr::load(
                                    w_dk,
                                    Expr::add(
                                        Expr::mul(Expr::var("j"), Expr::u32(kv_lora_rank)),
                                        i.clone(),
                                    ),
                                ),
                            ),
                        ),
                    )],
                ),
                Node::Store {
                    buffer: c_out.into(),
                    index: i,
                    value: Expr::var("acc"),
                },
            ],
        ),
    ];

    Ok(Program::wrapped(
        vec![
            BufferDecl::storage(h, 0, BufferAccess::ReadOnly, DataType::F32).with_count(hidden_dim),
            BufferDecl::storage(w_dk, 1, BufferAccess::ReadOnly, DataType::F32)
                .with_count(hidden_dim * kv_lora_rank),
            BufferDecl::output(c_out, 2, DataType::F32).with_count(kv_lora_rank),
        ],
        [64, 1, 1],
        vec![wrap_anonymous("vyre-libs::nn::mla_compress_kv", body)],
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::byte_pack::decode_f32;
    use crate::test_support::byte_pack::f32_bytes;
    use vyre_reference::value::Value;

    #[test]
    fn mla_compress_kv_identity() {
        let h = vec![2.0f32, 3.0];
        let w_dk = vec![1.0f32, 0.0, 0.0, 1.0];
        let program = mla_compress_kv("h", "w_dk", "c", 2, 2).unwrap();
        let outputs = vyre_reference::reference_eval(
            &program,
            &[
                Value::from(f32_bytes(&h)),
                Value::from(f32_bytes(&w_dk)),
                Value::from(vec![0u8; 8]),
            ],
        )
        .expect("Fix: mla_compress_kv must execute");
        let c = decode_f32(&outputs[0].to_bytes());
        assert_eq!(c, vec![2.0, 3.0]);
    }

    #[test]
    fn mla_decode_simple() {
        let q = vec![1.0f32, 0.0];
        let kv_cache = vec![1.0f32, 0.0];
        let kr_cache = vec![0.0f32, 0.0];
        let w_uk = vec![1.0f32, 0.0, 0.0, 1.0];
        let w_uv = vec![1.0f32, 0.0, 0.0, 1.0];

        let program = mla_decode(
            "q", "kv_cache", "kr_cache", "w_uk", "w_uv", "out", 1, 1, 2, 2, 2,
        )
        .unwrap();

        let outputs = vyre_reference::reference_eval(
            &program,
            &[
                Value::from(f32_bytes(&q)),
                Value::from(f32_bytes(&kv_cache)),
                Value::from(f32_bytes(&kr_cache)),
                Value::from(f32_bytes(&w_uk)),
                Value::from(f32_bytes(&w_uv)),
                Value::from(vec![0u8; 8]),
            ],
        )
        .expect("Fix: mla_decode must execute");

        let out = decode_f32(&outputs[0].to_bytes());
        assert!(
            (out[0] - 1.0).abs() < 1e-4,
            "mla_decode out[0] = {}",
            out[0]
        );
        assert!((out[1]).abs() < 1e-4, "mla_decode out[1] = {}", out[1]);
    }

    #[test]
    fn mla_decode_two_tokens() {
        // seq_len=2, num_heads=1, head_dim=2
        // q = [1.0, 0.0]
        // kv_cache = [[1,0], [0,1]]
        // w_uk = identity, w_uv = identity, kr_cache = zeros
        // k_0 = [1,0], k_1 = [0,1]
        // score_0 = dot([1,0],[1,0])/sqrt(2) = 1/sqrt(2)
        // score_1 = dot([1,0],[0,1])/sqrt(2) = 0
        // softmax: w0 ≈ 0.67, w1 ≈ 0.33
        // v_0 = [1,0], v_1 = [0,1]
        // out = [0.67, 0.33]
        let q = vec![1.0f32, 0.0];
        let kv_cache = vec![1.0f32, 0.0, 0.0, 1.0];
        let kr_cache = vec![0.0f32; 4];
        let w_uk = vec![1.0f32, 0.0, 0.0, 1.0];
        let w_uv = vec![1.0f32, 0.0, 0.0, 1.0];

        let program = mla_decode(
            "q", "kv_cache", "kr_cache", "w_uk", "w_uv", "out", 2, 1, 2, 2, 2,
        )
        .unwrap();

        let outputs = vyre_reference::reference_eval(
            &program,
            &[
                Value::from(f32_bytes(&q)),
                Value::from(f32_bytes(&kv_cache)),
                Value::from(f32_bytes(&kr_cache)),
                Value::from(f32_bytes(&w_uk)),
                Value::from(f32_bytes(&w_uv)),
                Value::from(vec![0u8; 8]),
            ],
        )
        .expect("Fix: mla_decode two tokens must execute");

        let out = decode_f32(&outputs[0].to_bytes());
        assert!(
            out[0] > 0.6 && out[0] < 0.7,
            "mla_decode out[0] = {}",
            out[0]
        );
        assert!(
            out[1] > 0.3 && out[1] < 0.4,
            "mla_decode out[1] = {}",
            out[1]
        );
    }

    #[test]
    fn mla_decode_zero_dim_errors() {
        for (batch, seq, kv_heads, head_dim, latent) in [
            (0, 1, 2, 2, 2),
            (1, 0, 2, 2, 2),
            (1, 1, 0, 2, 2),
            (1, 1, 2, 0, 2),
            (1, 1, 2, 2, 0),
        ] {
            let err = mla_decode(
                "q", "kv", "kr", "w_uk", "w_uv", "out", batch, seq, kv_heads, head_dim, latent,
            )
            .expect_err("zero dim must error");
            assert!(
                err.contains("mla_decode") && err.contains("> 0"),
                "mla_decode zero-dim ({batch},{seq},{kv_heads},{head_dim},{latent}): {err}"
            );
        }
    }
}
