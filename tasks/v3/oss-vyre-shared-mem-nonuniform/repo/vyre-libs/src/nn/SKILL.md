# vyre-libs::nn SKILL

Neural-network primitives  -  activation, linear layers, normalization,
attention. Every op is a Cat-A composition over `vyre-ops` primitives
and lower-level `vyre-libs::math` functions.

## Coverage targets

- Activations: `relu`. Future: `gelu`, `silu`, `tanh`, `sigmoid`.
- Linear: `linear` (feature-depends on `math-linalg`).
- Normalization: `layer_norm`. Future: `rms_norm`, `batch_norm`,
  `group_norm`.
- Attention: `softmax`, `attention`. Future: `flash_attention_v2`
  (post-0.6 LLM template crate R-3).

## Witness sources

- `relu`: trivial  -  identity for non-negative u32.
- `layer_norm`: PyTorch's `torch.nn.LayerNorm` reference with
  `eps=1e-5`, plus a corpus of edge cases (constant input, zero
  variance, large variance).
- `softmax`: exact probabilities summing to 1 ± 1e-6 (tolerance for
  `f32` rounding).
- `attention`: reference pulled from `scaled_dot_product_attention`
  in PyTorch.

## Benchmark targets (criterion)

- `softmax` on 4096 F32 elements: ≤ 500 µs sequential, ≤ 20 µs with
  workgroup-shared variant once `DataType::Shared` lands.
- `layer_norm` on 4096 F32 elements: ≤ 500 µs sequential.
- `attention` at seq_len=128, head_dim=64: ≤ 5 ms sequential; the
  FlashAttention-v2 variant (R-3, post-0.6) targets ≤ 200 µs on a
  3090.

## Backend parity contract

- F32 ops must be bit-identical across backends on inputs whose
  reduction tree is associativity-safe. For non-associative float
  reductions, document an explicit tolerance ≤ `f32::EPSILON * n`.

## Shape contract

- `softmax(input, output, n)`: both 1-D F32 length `n`.
- `layer_norm(input, output, n, eps)`: both 1-D F32 length `n`.
- `attention(q, k, v, out, s, d)`: all four 2-D F32 shape `[s, d]`.
- All builders route through `check_tensors` for collision, dtype,
  and overflow  -  no op-specific shape logic lives outside the builder.
