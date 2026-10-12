//! Tier 2.5 mathematical primitives.
//!
//! Each module exposes one reusable GPU composition with a stable op id.
//! Callers import the narrow module they need so region-chain audits can see
//! which primitive owns the shared work.

/// 1D separable convolution (domain-neutral: blur, signal processing, audio).
pub mod conv1d;
/// Shared dot-product partial accumulator.
pub mod dot_partial;
/// Value-set analysis interval arithmetic.
pub mod interval;
/// Classical RK4 next-state combiner for ODE integration. Same Program
/// serves user-dialect neural-ODE / physics-flow callers AND vyre-self
/// substrate (#9 homotopy_continuation path-tracking).
pub mod ode_step;
/// Subgroup prefix-sum scan used by compaction, histograms, and reductions.
pub mod prefix_scan;
pub(crate) mod u32_binary_map;

/// Differential-privacy accountant  -  Gaussian-mechanism RDP step with
/// host-side `(ε, δ)` conversion. Same Program serves user DP-SGD
/// trainers AND vyre's own profiler-telemetry hardening.
pub mod dp_accountant;

/// Fractional-calculus kernel  -  Grünwald-Letnikov weight generator
/// that feeds the existing `conv1d` primitive. No new GPU dispatch;
/// the lego rule is satisfied by composition.
pub mod fractional;

/// Submodular greedy step  -  argmax-of-marginals primitive driving
/// (1 - 1/e)-approximation greedy maximization. Same Program serves
/// user active-learning / coreset / sensor-placement dialects AND
/// vyre-self compile-cache eviction as submodular coverage.
pub mod submodular_greedy;

/// Conformal prediction  -  finite-sample distribution-free uncertainty
/// intervals. Same Program serves user calibrated-NN dialects AND
/// vyre-self dispatch cost-model intervals (#28).
pub mod conformal;

/// Sinkhorn-Knopp scaling step for entropic optimal transport.
/// Composes with `semiring_gemm` for the matvec halves of the
/// iteration. User: OT/Wasserstein loss, distribution alignment;
/// vyre-self: dispatch-graph clustering via Sinkhorn-OT distance.
pub mod sinkhorn;

/// Full iterative Sinkhorn balance primitive.
pub mod sinkhorn_iterate;

/// Differentiable algorithm primitives  -  softmax + temperature-scaled
/// argmax. Same Programs serve user attention/structured-prediction
/// dialects AND vyre-self differentiable autotuner (#27).
pub mod differentiable;

/// Quantized packing primitives for INT4 / packed low-bit tensors.
pub mod quantized;

/// Score-based generative one-step denoise combiner. User: diffusion /
/// flow-matching / SDE simulation.
pub mod score_denoise;

/// KFAC block-diagonal inverse for natural gradient.
pub mod kfac_block_inverse;

/// Newton-Schulz inverse-square-root step (Shampoo / KFAC core kernel).
/// Matrix preconditioner without SVD. User: Shampoo / KFAC / Sophia
/// optimizers, general matrix-function family.
pub mod preconditioner;

/// Natural-gradient block-apply  -  multiply gradient by precomputed
/// `M^{-1/2}` block. Composes with #16 preconditioner pipeline.
pub mod natural_gradient;

/// Iterative hard thresholding for sparse signal recovery (#48).
/// User: compressed-sensing decoders, NN pruning, dictionary learning.
/// Self: vyre's sparse-buffer compaction (when output is mostly zero).
pub mod sparse_recovery;

/// DP-SGD per-sample gradient clip (#42). User: DP-SGD trainers,
/// gradient-norm-clipped optimizers.
pub mod dp_clip;

/// Mori-Zwanzig Markovian projection step  -  closed-form coarse-
/// graining of dynamical systems (#58). User: scientific ML
/// emulators. Self: vyre's coarse view of own dispatch graph.
pub mod mori_zwanzig;

/// Information-geometry primitives  -  Bhattacharyya / Fisher-Rao /
/// Amari α-connection (#57). User: distribution-aware loss design,
/// MoE routing.
pub mod info_geometry;

/// Fast Multipole Method primitives  -  P2M / M2L / L2P (#51). User:
/// n-body simulations, kernel methods at scale, Poisson solvers.
/// Self: hierarchical compression of all-pairs dispatch dependency
/// analysis (#19 polyhedral fusion).
pub mod fmm;

/// Algebraic Multigrid V-cycle Jacobi smoother step (#50). User:
/// Poisson / Laplace / diffusion solvers. Self: dispatch-graph
/// hierarchy levels match V-cycle levels.
pub mod multigrid;

/// Algebraic Multigrid V-cycle (#P-PRIM-3). User:
/// Poisson / Laplace / diffusion solvers. Self: dispatch-graph
/// hierarchy levels match V-cycle levels.
pub mod amg_v_cycle;

/// Sheaf Laplacian eigenvalue (#P-PRIM-9). User:
/// spectral clustering, heterophilic GNN. Self: spectral gap of
/// dispatch-graph sheaf Laplacian.
pub mod sheaf_laplacian_eigenvalue;

/// Full Edmonds augmenting-path matroid intersection (#P-PRIM-10).
/// User: combinatorial scheduling, bipartite matching. Self:
/// megakernel scheduler fusion-grouping.
pub mod matroid_intersection_full;

/// Tensor-train decomposition via SVD-truncation per mode (#P-PRIM-12).
/// User: NN compression, long-context attention. Self: compress
/// the dispatch-graph cost tensor via TT decomposition.
pub mod tensor_train_decompose;

/// Tensor-train one-step contraction (#6). User: NN compression,
/// long-context attention, scientific-tensor compression. Self:
/// vyre's chain-shaped Region tree as a TT  -  optimal contraction
/// order = optimal fusion order.
pub mod tensor_train;

/// Randomized SVD random-projection step (#3). User: low-rank
/// attention, NN compression, PCA at scale. Self: dispatch dependency
/// matrix compression for #19 polyhedral fusion at workspace scale.
pub mod randomized_svd;

/// Sum-of-squares (Positivstellensatz) Gram-matrix construction (#14).
/// User: formal verification (Lyapunov), polynomial optimization,
/// SOS-based buffer-safety certificates.
pub mod sos_certificate;

/// Quantum singular-value transform (classical) block-encoding +
/// Chebyshev apply (#34). User: matrix-function family without
/// eigendecomposition. Self: Wasserstein-over-dispatch fusion.
pub mod qsvt;

/// Pairwise tensor-network contraction (#35). User: PEPS / MPS quantum
/// chemistry, compressed NN weights. Self: Region tree contraction.
pub mod tensor_network;

/// RMT-based Marchenko-Pastur edge clip (#17). User: implicit
/// regularization, training-dynamics-aware optimizers. Self: spectrum
/// projection for #23 dispatch-graph spectral schedule.
pub mod spectral_shape;

/// p-adic Hensel-lift step (#54, research scaffold). Stable
/// arithmetic for ill-conditioned problems.
pub mod padic;

/// Multi-limb big-integer ripple-carry addition primitive (#P-PRIM-BIGINT).
/// Foundational building block for RSA / ECDSA / X25519 / lattice crypto.
/// Emits `(sum_partial, carry_partial)` per-limb for a downstream
/// parallel-prefix carry-fix wave. Same Program serves user crypto-dialect
/// callers AND vyre-self bigint-cost-model arithmetic.
pub mod bigint_add_carry;

/// Generic-semiring matrix multiply  -  spine of the LEGO substrate.
/// Same Program serves user dialects (security reachability, dataflow,
/// CKY parsing, Viterbi, GF(2)) AND vyre-self consumers (#19 fusion-graph
/// analysis, #22 megakernel scheduler critical path, #26 region-graph
/// dataflow fixpoint, #39 Scallop-join provenance semiring).
pub mod semiring_gemm;

/// Sparse-kernel selector evidence for library comparisons.
pub mod sparse_selector;

/// Bellman-Ford shortest path primitive over an edge list. Composes
/// `persistent_fixpoint`. Self-consumer: tensor-network contraction order.
pub mod bellman_shortest_path;

mod scallop_persistent;

/// Scallop-style probabilistic Datalog join (#39). Emits a lineage
/// semiring join inside a GPU-resident fixpoint kernel. User dialect:
/// probabilistic Datalog.
/// Self-consumer: rule-provenance tracking
/// (`vyre-libs::self_substrate::scallop_provenance`).
pub mod scallop_join;
pub mod scallop_join_wide;
#[cfg(test)]
mod scallop_join_wide_tests;
/// Prefix-scan backed stream compaction over live-lane flags.
pub mod stream_compact;
/// SCC-local matrix fixpoint primitive for recursive graph components.
pub mod tensor_scc;
