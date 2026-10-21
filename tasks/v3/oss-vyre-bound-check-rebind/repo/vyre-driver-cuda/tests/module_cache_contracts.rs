//! CUDA module-cache performance contracts.

mod common;
use common::{bytes_u32, resident_dispatch_source, u32_bytes};
use vyre_driver::DispatchConfig;
use vyre_driver_cuda::CudaBackend;
use vyre_foundation::ir::{BufferDecl, DataType, Expr, Node, Program};

fn egraph_kernel_plan_source() -> String {
    [
        include_str!("../src/egraph_kernel_plan.rs"),
        include_str!("../src/egraph_kernel_plan/backend_structural.rs"),
        include_str!("../src/egraph_kernel_plan/backend_rewrite.rs"),
    ]
    .join("\n")
}

#[test]
fn repeated_dispatch_reuses_loaded_cuda_module() {
    let backend =
        CudaBackend::acquire().expect("Fix: CUDA backend acquire failed on a GPU-required host.");
    let program = Program::wrapped(
        vec![
            BufferDecl::read("input", 0, DataType::U32).with_count(2),
            BufferDecl::output("out", 1, DataType::U32).with_count(2),
        ],
        [64, 1, 1],
        vec![Node::store(
            "out",
            Expr::gid_x(),
            Expr::add(Expr::load("input", Expr::gid_x()), Expr::u32(7)),
        )],
    );

    assert_eq!(
        backend
            .cached_module_count()
            .expect("Fix: CUDA module cache lock failed."),
        0
    );
    backend
        .dispatch(&program, &[u32_bytes(&[1, 2])], &DispatchConfig::default())
        .expect("Fix: first CUDA dispatch should load one module.");
    assert_eq!(
        backend
            .cached_module_count()
            .expect("Fix: CUDA module cache lock failed."),
        1
    );
    backend
        .dispatch(&program, &[u32_bytes(&[3, 4])], &DispatchConfig::default())
        .expect("Fix: second CUDA dispatch should reuse cached module.");
    assert_eq!(
        backend
            .cached_module_count()
            .expect("Fix: CUDA module cache lock failed."),
        1,
        "Fix: repeated CUDA dispatches of the same program must not load duplicate modules."
    );
}

#[test]
fn compile_native_preloads_module_and_dispatches_without_relowering() {
    let backend =
        CudaBackend::acquire().expect("Fix: CUDA backend acquire failed on a GPU-required host.");
    let program = Program::wrapped(
        vec![
            BufferDecl::read("input", 0, DataType::U32).with_count(2),
            BufferDecl::output("out", 1, DataType::U32).with_count(2),
        ],
        [64, 1, 1],
        vec![Node::store(
            "out",
            Expr::gid_x(),
            Expr::mul(Expr::load("input", Expr::gid_x()), Expr::u32(11)),
        )],
    );

    let pipeline = backend
        .compile_native(&program, &DispatchConfig::default())
        .expect("Fix: CUDA compile_native must lower PTX and preload the CUDA module.");
    assert!(
        pipeline.id().starts_with("cuda:"),
        "Fix: compiled CUDA pipeline id must expose the backend and stable PTX cache key."
    );
    assert_eq!(
        backend
            .cached_module_count()
            .expect("Fix: CUDA module cache lock failed."),
        1,
        "Fix: compile_native must preload exactly one CUDA module for this program."
    );

    let outputs = pipeline
        .dispatch(&[u32_bytes(&[3, 4])], &DispatchConfig::default())
        .expect("Fix: compiled CUDA pipeline must dispatch through the cached PTX.");
    assert_eq!(bytes_u32(&outputs[0]), vec![33, 44]);
    assert_eq!(
        backend
            .cached_module_count()
            .expect("Fix: CUDA module cache lock failed."),
        1,
        "Fix: compiled CUDA pipeline dispatch must reuse the preloaded module."
    );
}

#[test]
fn repeated_dispatch_reuses_transient_device_allocations() {
    let backend =
        CudaBackend::acquire().expect("Fix: CUDA backend acquire failed on a GPU-required host.");
    let program = Program::wrapped(
        vec![
            BufferDecl::read("input", 0, DataType::U32).with_count(1024),
            BufferDecl::output("out", 1, DataType::U32).with_count(1024),
        ],
        [256, 1, 1],
        vec![Node::store(
            "out",
            Expr::gid_x(),
            Expr::add(Expr::load("input", Expr::gid_x()), Expr::u32(1)),
        )],
    );
    let input = u32_bytes(&(0..1024).collect::<Vec<_>>());

    assert_eq!(
        backend
            .cached_transient_allocation_bytes()
            .expect("Fix: CUDA transient allocation pool lock failed."),
        0
    );
    backend
        .dispatch(
            &program,
            std::slice::from_ref(&input),
            &DispatchConfig::default(),
        )
        .expect("Fix: first CUDA dispatch should complete and return transient allocations.");
    let after_first = backend
        .cached_transient_allocation_bytes()
        .expect("Fix: CUDA transient allocation pool lock failed.");
    assert!(
        after_first > 0,
        "Fix: CUDA dispatch must retain transient device allocations for reuse instead of freeing every sample."
    );

    backend
        .dispatch(&program, &[input], &DispatchConfig::default())
        .expect("Fix: second CUDA dispatch should reuse transient allocations.");
    let after_second = backend
        .cached_transient_allocation_bytes()
        .expect("Fix: CUDA transient allocation pool lock failed.");
    assert_eq!(
        after_second, after_first,
        "Fix: repeated same-shape CUDA dispatches must reuse the transient allocation pool without unbounded growth."
    );

    backend
        .cleanup()
        .expect("Fix: CUDA cleanup must clear transient allocation pool.");
    assert_eq!(
        backend
            .cached_transient_allocation_bytes()
            .expect("Fix: CUDA transient allocation pool lock failed."),
        0
    );
}

#[test]
fn repeated_dispatch_reuses_cuda_launch_resources() {
    let backend =
        CudaBackend::acquire().expect("Fix: CUDA backend acquire failed on a GPU-required host.");
    let program = Program::wrapped(
        vec![
            BufferDecl::read("input", 0, DataType::U32).with_count(256),
            BufferDecl::output("out", 1, DataType::U32).with_count(256),
        ],
        [256, 1, 1],
        vec![Node::store(
            "out",
            Expr::gid_x(),
            Expr::add(Expr::load("input", Expr::gid_x()), Expr::u32(9)),
        )],
    );
    let input = u32_bytes(&(0..256).collect::<Vec<_>>());

    assert_eq!(
        backend
            .cached_launch_resource_counts()
            .expect("Fix: CUDA launch-resource pool lock failed."),
        (0, 0)
    );
    backend
        .dispatch(
            &program,
            std::slice::from_ref(&input),
            &DispatchConfig::default(),
        )
        .expect("Fix: first CUDA dispatch should complete and return launch resources.");
    let after_first = backend
        .cached_launch_resource_counts()
        .expect("Fix: CUDA launch-resource pool lock failed.");
    assert_eq!(
        after_first,
        (1, 1),
        "Fix: CUDA dispatch must retain the stream and completion event for reuse."
    );

    backend
        .dispatch(&program, &[input], &DispatchConfig::default())
        .expect("Fix: second CUDA dispatch should reuse launch resources.");
    assert_eq!(
        backend
            .cached_launch_resource_counts()
            .expect("Fix: CUDA launch-resource pool lock failed."),
        after_first,
        "Fix: repeated same-shape CUDA dispatches must reuse launch resources without growth."
    );

    backend
        .cleanup()
        .expect("Fix: CUDA cleanup must clear cached launch resources.");
    assert_eq!(
        backend
            .cached_launch_resource_counts()
            .expect("Fix: CUDA launch-resource pool lock failed."),
        (0, 0)
    );
}

#[test]
fn module_cache_eviction_scores_entries_without_collect_then_relookup() {
    let source = include_str!("../src/backend/module_cache.rs");

    assert!(
        source.contains("for entry in self.sources.iter()")
            && source.contains("for entry in self.modules.iter()")
            && source.contains("gains.push(entry.access_count.load(Ordering::Relaxed));"),
        "Fix: CUDA module/PTX cache eviction must score keys and gains in one pass."
    );
    assert!(
        !source.contains(concat!("iter().map(|entry| *entry.key()).collect", "();"))
            && !source.contains(concat!(".get(key)\n                    .map(|module|", " module.access_count"))
            && !source.contains(concat!(".get(key)\n                    .map(|source|", " source.access_count")),
        "Fix: CUDA module/PTX cache eviction must not collect keys and then relookup every entry for access scores."
    );
    assert!(
        !source.contains(concat!("cached_source_bytes", "\n                .load(Ordering::Acquire)\n                .saturating_add"))
            && !source.contains("dropped_bytes.saturating_add"),
        "Fix: CUDA PTX source-cache byte accounting must be exact around memory caps, not saturating."
    );
}

#[test]
fn cuda_module_keying_reuses_ptx_source_digest_instead_of_rehashing_full_ptx() {
    let module_cache = include_str!("../src/backend/module_cache.rs");
    let capabilities = include_str!("../src/backend/capabilities.rs");
    let dispatch = include_str!("../src/backend/dispatch.rs");
    let host_dispatch = include_str!("../src/backend/host_dispatch.rs");
    let resident_dispatch = resident_dispatch_source();
    let cuda_graph = include_str!("../src/backend/cuda_graph.rs");
    let egraph = egraph_kernel_plan_source();
    let pipeline = include_str!("../src/pipeline.rs");

    assert!(
        module_cache.contains("pub(crate) fn key_for_ptx_source_key")
            && module_cache.contains("module_cache_key_from_domain_digest(")
            && module_cache.contains("CUDA_MODULE_FROM_PTX_SOURCE_KEY_DOMAIN")
            && module_cache.contains("ptx_source_key.as_bytes()")
            && module_cache.contains("domain_separated_exact_input_key("),
        "Fix: CUDA module-cache keys must derive from the already-computed PTX source cache digest through the shared domain-separated identity contract."
    );
    assert!(
        capabilities.contains("ptx_for_program_cached_with_key")
            && capabilities.contains("Ok((ptx, key))"),
        "Fix: CUDA lowering must return the PTX source cache key with the cached source."
    );
    assert!(
        dispatch.contains("module_cache_key_for_ptx_source_key")
            && host_dispatch.contains("ptx_for_program_cached_with_key")
            && resident_dispatch.contains("ptx_for_program_cached_with_key")
            && cuda_graph.contains("ptx_for_program_cached_with_key"),
        "Fix: CUDA host, resident, and graph paths must thread the source digest into module-cache keying."
    );
    assert!(
        !host_dispatch.contains("module_cache_key(&ptx_src)")
            && !resident_dispatch.contains("module_cache_key(&ptx_src)")
            && !cuda_graph.contains("module_cache_key(&ptx_src)")
            && !dispatch.contains("fn module_cache_key(&self, ptx_src")
            && !pipeline.contains("hasher.update(ptx_src.as_bytes())"),
        "Fix: CUDA warm paths must not rehash full PTX text after PTX source-cache lookup."
    );
    assert!(
        pipeline.contains("ptx_source_key: PtxSourceCacheKey")
            && pipeline.contains("cuda_compiled_pipeline_identity_key")
            && pipeline.contains("ptx_source_key.as_bytes()")
            && pipeline.contains("domain_separated_exact_input_key(")
            && !pipeline.contains("try_normalized_program_cache_digest")
            && !pipeline.contains("program_vsa_fingerprint_words")
            && !pipeline.contains("update_dispatch_policy_cache_hash"),
        "Fix: compiled CUDA pipeline IDs must reuse the PTX source digest through the shared identity contract instead of repeating normalized Program/VSA/config hashing."
    );
    assert!(
        module_cache.contains("pub(crate) fn key_for_raw_ptx_artifact")
            && dispatch.contains("module_cache_key_for_raw_ptx_artifact")
            && egraph.contains("warm_egraph_structural_equivalence_kernel_with_key")
            && egraph.contains("warm_egraph_canonical_rewrite_kernel_with_key")
            && egraph.contains("warm_egraph_signature_refresh_kernel_with_key")
            && !host_dispatch.contains("module_cache_key_for_raw_ptx_artifact")
            && !resident_dispatch.contains("module_cache_key_for_raw_ptx_artifact")
            && !cuda_graph.contains("module_cache_key_for_raw_ptx_artifact"),
        "Fix: raw PTX artifact keying is allowed only for standalone e-graph kernels that do not originate from Program PTX source-cache lowering."
    );
    assert_eq!(
        egraph
            .matches("module_cache_key_for_raw_ptx_artifact(&kernel.source)")
            .count(),
        3,
        "Fix: each standalone e-graph CUDA kernel should compute its raw-artifact module key once in its warm-and-key helper, not again in the run path."
    );
}
