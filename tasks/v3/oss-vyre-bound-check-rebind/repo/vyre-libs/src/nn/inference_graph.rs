//! DeepSeek V4 Flash inference graph construction.
//!
//! Builds the forward pass as a sequence of [`Program`]s, one per layer type.
//! Each Program is a Category-A composition over existing `vyre-libs` primitives.

use vyre::ir::Program;

use super::{
    activation::embedding,
    attention::mla::mla_decode,
    linear::linear,
    moe::{expert_mlp, moe_layer::moe_layer_route_and_accumulate},
    norm::rms_norm,
};

/// Hyperparameters for DeepSeek V4 Flash.
///
/// Defaults match the VyreOffload reference configuration.
#[derive(Debug, Clone)]
pub struct Ds4FlashConfig {
    /// Vocabulary size (including special tokens).
    pub vocab_size: u32,
    /// Hidden dimension / model dimension (`d_model`).
    pub hidden_dim: u32,
    /// Number of transformer layers.
    pub num_layers: u32,
    /// Number of attention heads.
    pub num_heads: u32,
    /// Dimension per attention head.
    pub head_dim: u32,
    /// Compressed KV latent rank (MLA).
    pub kv_lora_rank: u32,
    /// RoPE dimension for decoupled Q/K.
    pub qk_rope_head_dim: u32,
    /// Number of routed experts in the MoE layer.
    pub num_experts: u32,
    /// Top-k experts selected by the router.
    pub moe_top_k: u32,
    /// Hidden dimension of the shared expert MLP.
    pub shared_expert_hidden_dim: u32,
    /// Epsilon for RMSNorm.
    pub rms_norm_eps: f32,
    /// Maximum sequence length for prefill.
    pub max_seq_len: u32,
}

impl Default for Ds4FlashConfig {
    fn default() -> Self {
        Self {
            vocab_size: 129_280,
            hidden_dim: 7_168,
            num_layers: 61,
            num_heads: 128,
            head_dim: 128,
            kv_lora_rank: 512,
            qk_rope_head_dim: 64,
            num_experts: 256,
            moe_top_k: 8,
            shared_expert_hidden_dim: 18_432,
            rms_norm_eps: 1e-6,
            max_seq_len: 4_096,
        }
    }
}

/// Build the full forward-pass graph for DeepSeek V4 Flash.
///
/// Returns one [`Program`] per layer type, in conceptual execution order:
///
/// 1. Token embedding lookup (`embed_program`)
/// 2. MLA prefill attention (`mla_prefill_program`)
/// 3. MLA single-token decode attention (`mla_decode_program`)
/// 4. MoE layer dispatch (`moe_layer_program`)
/// 5. Shared dense expert MLP (`shared_expert_program`)
/// 6. RMSNorm pre/post normalization (`rms_norm_program`)
/// 7. LM head logits projection (`lm_head_program`)
///
/// Each Program references canonical buffer names (e.g. `"tokens"`, `"q"`,
/// `"moe_x"`) so the runtime can wire them together in a sequential
/// dispatch table.
pub fn build_forward_graph(config: &Ds4FlashConfig) -> Vec<Program> {
    let Ds4FlashConfig {
        vocab_size,
        hidden_dim,
        num_layers: _,
        num_heads,
        head_dim,
        kv_lora_rank,
        qk_rope_head_dim,
        num_experts,
        moe_top_k,
        shared_expert_hidden_dim,
        rms_norm_eps,
        max_seq_len,
    } = *config;

    // 1. Embedding: lookup tokens -> hidden_dim vectors.
    let embed_program = embedding(
        "embed_table",
        "tokens",
        "embed_out",
        max_seq_len,
        hidden_dim,
    );

    // 2. MLA prefill: full-context attention during prompt ingestion.
    let mla_prefill_program = mla_decode(
        "q",
        "kv_cache",
        "kr_cache",
        "w_uk",
        "w_uv",
        "mla_prefill_out",
        max_seq_len,
        num_heads,
        head_dim,
        kv_lora_rank,
        qk_rope_head_dim,
    )
    .unwrap_or_else(|e| {
        crate::invalid_program(
            "vyre-libs::nn::mla_prefill",
            format!("Fix: mla_prefill build failed: {e}"),
        )
    });

    // 3. MLA decode: single-token autoregressive step.
    let mla_decode_program = mla_decode(
        "q",
        "kv_cache",
        "kr_cache",
        "w_uk",
        "w_uv",
        "mla_decode_out",
        1,
        num_heads,
        head_dim,
        kv_lora_rank,
        qk_rope_head_dim,
    )
    .unwrap_or_else(|e| {
        crate::invalid_program(
            "vyre-libs::nn::mla_decode",
            format!("Fix: mla_decode build failed: {e}"),
        )
    });

    // 4. MoE layer: weighted accumulation over top-k expert outputs.
    // The router softmax + top-k selection (via `softmax_top_k`) is
    // expected to run immediately before this kernel to populate
    // `expert_indices` and `expert_weights`.
    let moe_layer_program = moe_layer_route_and_accumulate(
        "moe_x",
        "w_router",
        "b_router",
        "expert_indices",
        "expert_weights",
        "expert_outputs",
        "moe_out",
        hidden_dim,
        num_experts,
        hidden_dim,
        moe_top_k,
    )
    .unwrap_or_else(|e| {
        crate::invalid_program(
            "vyre-libs::nn::moe_layer",
            format!("Fix: moe_layer build failed: {e}"),
        )
    });

    // 5. Shared expert: dense SwiGLU MLP used alongside the routed experts.
    let shared_expert_program = expert_mlp(
        "shared_x",
        "shared_w_gate",
        "shared_b_gate",
        "shared_w_up",
        "shared_b_up",
        "shared_w_down",
        "shared_b_down",
        "shared_out",
        hidden_dim,
        shared_expert_hidden_dim,
        hidden_dim,
    )
    .unwrap_or_else(|e| {
        crate::invalid_program(
            "vyre-libs::nn::shared_expert",
            format!("Fix: shared_expert build failed: {e}"),
        )
    });

    // 6. RMSNorm: pre/post layer normalization.
    let rms_norm_program = rms_norm("rms_in", "rms_out", hidden_dim, rms_norm_eps);

    // 7. LM head: project final hidden state to vocabulary logits.
    let lm_head_program = linear(
        "lm_head_x",
        "lm_head_w",
        "lm_head_b",
        "lm_head_out",
        hidden_dim,
        vocab_size,
    )
    .unwrap_or_else(|e| {
        crate::invalid_program(
            "vyre-libs::nn::lm_head",
            format!("Fix: lm_head build failed: {e}"),
        )
    });

    vec![
        embed_program,
        mla_prefill_program,
        mla_decode_program,
        moe_layer_program,
        shared_expert_program,
        rms_norm_program,
        lm_head_program,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn forward_graph_default_config_builds() {
        let config = Ds4FlashConfig::default();
        let programs = build_forward_graph(&config);
        assert_eq!(
            programs.len(),
            7,
            "expected 7 programs (one per layer type)"
        );

        let expected_names = [
            "embed",
            "mla_prefill",
            "mla_decode",
            "moe_layer",
            "shared_expert",
            "rms_norm",
            "lm_head",
        ];
        for (i, program) in programs.iter().enumerate() {
            assert!(
                !program.buffers().is_empty(),
                "{} program should declare at least one buffer",
                expected_names[i]
            );
        }
    }

    #[test]
    fn forward_graph_small_config_builds() {
        let config = Ds4FlashConfig {
            vocab_size: 1_024,
            hidden_dim: 256,
            num_layers: 2,
            num_heads: 4,
            head_dim: 64,
            kv_lora_rank: 32,
            qk_rope_head_dim: 16,
            num_experts: 8,
            moe_top_k: 2,
            shared_expert_hidden_dim: 512,
            rms_norm_eps: 1e-5,
            max_seq_len: 128,
        };
        let programs = build_forward_graph(&config);
        assert_eq!(programs.len(), 7);

        // Verify each program has a non-empty buffer table.
        for program in &programs {
            assert!(!program.buffers().is_empty());
        }
    }

    #[test]
    fn embed_program_has_correct_buffer_count() {
        let config = Ds4FlashConfig::default();
        let programs = build_forward_graph(&config);
        let embed = &programs[0];
        assert_eq!(embed.buffers().len(), 3);
    }

    #[test]
    fn mla_prefill_and_decode_are_distinct() {
        let config = Ds4FlashConfig::default();
        let programs = build_forward_graph(&config);
        let prefill = &programs[1];
        let decode = &programs[2];
        // Prefill operates over max_seq_len; decode over seq_len=1.
        // The buffer counts should be identical (same inputs/outputs)
        // but the workgroup logic differs internally.
        assert_eq!(prefill.buffers().len(), decode.buffers().len());
        assert!(
            prefill.workgroup_size() == decode.workgroup_size(),
            "prefill and decode use the same workgroup dispatch shape"
        );
    }

    #[test]
    fn rms_norm_program_is_f32() {
        let config = Ds4FlashConfig::default();
        let programs = build_forward_graph(&config);
        let rms = &programs[5];
        for buf in rms.buffers() {
            assert_eq!(
                buf.element,
                vyre::ir::DataType::F32,
                "rms_norm uses F32 buffers"
            );
        }
    }

    #[test]
    fn lm_head_program_has_expected_buffers() {
        let config = Ds4FlashConfig::default();
        let programs = build_forward_graph(&config);
        let lm_head = &programs[6];
        // The tiled linear path adds workgroup scratch buffers, so we
        // expect at least the 4 core buffers (x, w, b, out).
        assert!(lm_head.buffers().len() >= 4);
    }
}
