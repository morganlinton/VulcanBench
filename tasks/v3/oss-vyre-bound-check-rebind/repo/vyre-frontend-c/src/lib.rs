#![forbid(unsafe_code)]
//! `vyre-frontend-c`  -  GPU-first C compilation driver built on `vyre` and `vyre-libs`.
//!
//! **Implemented:** bounded include/macro/conditional TU preparation → lex → digraph rewrite →
//! `opt_conditional_mask` → macro-token snapshot → `bracket_match` (paren + brace) → function shapes
//! → call sites → ABI layout → `ast_shunting_yard` → typed VAST → ProgramGraph → semantic
//! ProgramGraph + semantic scope → CFG / goto → `opt_lower_elf`; artifacts are embedded in
//! **Linux ET_REL** `.o` files (`object` crate) plus a `VYRECOB2` v3 payload in a `.vyrecob2.*`
//! section. **Link mode** (`vyrec` without `-c`) is rejected in this CUDA-first release path;
//! object emission is the supported contract.
//!
//! The CLI entry point is the `vyrec` binary in the repo workspace (`tools/vyrec`).

/// Thin façade over the vyre-frontend-c compilation pipeline used by the `vyrec` binary.
pub mod api;
/// ELF-on-Linux ET_REL writer: emits `.o` artifacts that hold the embedded VYRECOB2 payload.
pub mod elf_linux;
mod gpu_backend;
mod hash;
/// Consumer-owned adapter for the GPU-resident C translation-unit workspace.
pub mod megakernel_workspace;
/// VYRECOB2 v3 object container: section table, header, and the readers/writers used by
/// `vyrec` and the link step.
pub mod object_format;
/// Bounded TU preparation → lex → digraph → AST → CFG → ELF lowering pipeline used by `vyrec`.
pub mod pipeline;
/// Host orchestration for the GPU-resident translation-unit preprocessor: file I/O,
/// include-path lookup, cache keys, and dependency invalidation only.
pub mod tu_host;

/// VSA-fingerprint a Program through the shared driver substrate.
#[must_use]
pub fn program_fingerprint(program: &vyre::ir::Program) -> Vec<u32> {
    vyre_driver::program_vsa_fingerprint(program)
}

/// Compute the natural-gradient autotune step for vyre-frontend-c's compile
/// hyperparameter loop. The same Fisher-preconditioned step the backend
/// autotuner uses, exposed for compile-time parameter search (loop
/// unroll factor, vectorization width, register tile sizes).
///
/// P-CC-2: vyre-frontend-c gradient direction is the natural gradient.
#[must_use]
#[cfg(any(test, feature = "legacy-infallible"))]
pub fn natural_gradient_step(
    m_inv_sqrt: &[f64],
    grad: &[f64],
    n: u32,
    learning_rate: f64,
) -> Vec<f64> {
    try_natural_gradient_step(m_inv_sqrt, grad, n, learning_rate).unwrap_or_else(|source| {
        panic!(
            "vyre-frontend-c natural-gradient step failed: {source}. Fix: validate autotune dimensions and shard the autotune surface."
        )
    })
}

/// Compute the natural-gradient autotune step for vyre-frontend-c's compile
/// hyperparameter loop with fallible host staging.
///
/// # Errors
///
/// Returns [`vyre_driver::BackendError`] when the natural-gradient vector
/// cannot be staged or the dimension does not fit host indexing.
pub fn try_natural_gradient_step(
    m_inv_sqrt: &[f64],
    grad: &[f64],
    n: u32,
    learning_rate: f64,
) -> Result<Vec<f64>, vyre_driver::BackendError> {
    vyre_runtime::megakernel::MegakernelLaunchPolicy::try_natural_gradient_autotune_step(
        m_inv_sqrt,
        grad,
        n,
        learning_rate,
    )
}

/// Compose a backward-pass functor for the C-compilation IR via the
/// dagger-functor self-consumer. Given a forward-pass functor mapping
/// (the vyre-frontend-c lowering chain), the dagger returns the pseudo-inverse
/// mapping suitable for backward propagation through the compile DAG.
///
/// P-CC-1: backward-pass synthesis through dagger functor. Reuses
/// the substrate's functorial_pass_composition::apply_pass_functor as
/// the dagger composition primitive  -  when invoked with a column
/// permutation that's a left-inverse of the forward mapping it
/// produces the daggered transform.
#[must_use]
pub fn backward_pass_functor(
    forward_view: &[u32],
    inverse_column_mapping: &[u32],
    target_n_cols: u32,
) -> Vec<u32> {
    vyre_self_substrate::functorial_pass_composition::apply_pass_functor(
        forward_view,
        inverse_column_mapping,
        target_n_cols,
    )
}
