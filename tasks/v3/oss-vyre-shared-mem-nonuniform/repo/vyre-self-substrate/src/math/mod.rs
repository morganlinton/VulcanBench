//! Advanced mathematical substrate passes and optimizer-theory kernels.

pub mod amg_pass_solver;
pub mod bellman_tn_order;
pub mod conv1d_latency_smoothing;
pub mod dataflow_compaction_pipeline;
pub mod differentiable_autotune;
pub mod fmm_polyhedral_compress;
pub mod kfac_autotune_step;
pub mod mori_zwanzig_region_coarsen;
pub mod multigrid_matroid_solver;
pub mod natural_gradient_autotuner;
pub mod numerical_kernel_pipeline;
pub mod persistent_homology_loop_signature;
pub mod qsvt_matrix_function_fusion;
pub mod quantized_dispatch;
pub mod scientific_kernel_pipeline;
pub mod sheaf_heterophilic_dispatch;
pub mod sheaf_spectral_clustering;
pub mod sinkhorn_dispatch_clustering;
pub mod sinkhorn_full_clustering;
pub mod tensor_network_fusion_order;
pub mod tensor_train_chain_fusion;
pub mod tensor_train_compression;
