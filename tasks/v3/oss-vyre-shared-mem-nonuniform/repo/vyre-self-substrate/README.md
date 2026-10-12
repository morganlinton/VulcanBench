# vyre-self-substrate

The recursion-thesis layer of vyre: using vyre's own LEGO-substrate
primitives (`vyre-primitives`) on vyre's own scheduler / dataflow / cost-
model problems.

Audit cleanup A10 (2026-04-30) extracted this from
`vyre-driver/src/self_substrate/` into a dedicated crate so the
substrate-self-uses live at a layer that depends only on
`vyre-foundation` + `vyre-primitives`: eliminating the layering
muddle where backend-specific dispatch code and substrate self-uses
shared one home in `vyre-driver`.

## Dep direction

```
vyre-foundation
       ↑
vyre-primitives
       ↑
vyre-self-substrate   ← THIS CRATE
       ↑
vyre-driver / vyre-runtime / vyre-libs / vyre-driver-{cuda,wgpu}
```

## What lives here

55 modules implementing vyre-self-uses of substrate primitives, including:

- Megakernel scheduler (`matroid_megakernel_scheduler`,
  `spectral_schedule`, `level_wave_pass`, `tensor_train_chain_fusion`).
- Dataflow analyses (`dataflow_fixpoint`, `reaching_definitions`,
  `live_variables`, `dominator_frontier`).
- Cost models (`cost_model`, `differentiable_autotune`).
- Categorical checks (`categorical_check`, `functorial_pass_composition`).
- Topological signatures (`persistent_homology_loop_signature`).
- Bitset summaries, alias registry, do-calculus impact analysis, etc.

See `src/lib.rs` for the full mod tree.
