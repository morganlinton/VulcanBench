//! Cooperative-launch dispatch contracts.
//!
//! Verifies that the public `DispatchConfig::cooperative` flag routes
//! `vyre-driver-cuda` through `cuLaunchCooperativeKernel` and that:
//!   - On hardware that supports cooperative launch, output is byte-identical
//!     to the same Program dispatched via the regular `cuLaunchKernel` path.
//!   - On hardware that does NOT support cooperative launch (or when the
//!     device's `cooperative_launch` capability is false), the backend
//!     returns `BackendError::UnsupportedFeature` instead of silently falling
//!     back. Hardware-fail mode is the explicit, structured signal the runtime
//!     needs to make the kernel-split-fallback decision in
//!     `vyre_driver::grid_sync::dispatch_with_grid_sync_split`.
//!
//! These tests require a CUDA device. Backend acquisition failure is a test
//! failure on the GPU-required Vyre test hosts.

mod common;
use common::{bytes_u32, u32_bytes};
use vyre_driver::{grid_sync, BackendError, DispatchConfig};
use vyre_driver_cuda::occupancy::cooperative_thread_residency_block_limit;
use vyre_driver_cuda::{cuda_factory, CudaBackend};
use vyre_foundation::ir::{BufferDecl, DataType, Expr, Node, Program};
use vyre_foundation::memory_model::MemoryOrdering;

fn add_one_program() -> Program {
    Program::wrapped(
        vec![
            BufferDecl::read("input", 0, DataType::U32).with_count(8),
            BufferDecl::output("out", 1, DataType::U32).with_count(8),
        ],
        [128, 1, 1],
        vec![Node::store(
            "out",
            Expr::gid_x(),
            Expr::add(Expr::load("input", Expr::gid_x()), Expr::u32(1)),
        )],
    )
}

#[test]
fn cooperative_dispatch_matches_regular_dispatch_on_supported_hardware() {
    let backend = CudaBackend::acquire()
        .expect("Fix: CUDA backend acquisition must succeed on the GPU-required test host.");
    if !backend.hardware_supports_grid_sync() {
        // Hardware doesn't expose cooperative launch; the request-rejection
        // contract is covered by `cooperative_dispatch_rejected_on_unsupported_hardware`.
        return;
    }

    let program = add_one_program();
    let inputs = [u32_bytes(&[0, 1, 2, 3, 9, 10, 99, u32::MAX - 1])];

    let regular_outputs = backend
        .dispatch(&program, &inputs, &DispatchConfig::default())
        .expect("regular cuLaunchKernel dispatch must succeed for the trivial add-one program");

    let mut cooperative_config = DispatchConfig::default();
    cooperative_config.cooperative = true;
    let cooperative_outputs = backend
        .dispatch(&program, &inputs, &cooperative_config)
        .expect(
            "cuLaunchCooperativeKernel dispatch must succeed when the device reports \
             cooperative_launch support; a failure here means cooperative launch is \
             refused even though hardware_supports_grid_sync() returned true",
        );

    assert_eq!(
        regular_outputs.len(),
        cooperative_outputs.len(),
        "cooperative dispatch must produce the same output buffer count as regular dispatch"
    );
    assert_eq!(
        bytes_u32(&regular_outputs[0]),
        bytes_u32(&cooperative_outputs[0]),
        "cooperative + regular dispatch must produce byte-identical output for the \
         same Program; any divergence means the cooperative-launch path is not parity-clean"
    );
    assert_eq!(
        bytes_u32(&cooperative_outputs[0]),
        vec![1, 2, 3, 4, 10, 11, 100, u32::MAX],
        "cooperative-launch output must be byte-exact for u32 add-one"
    );
}

#[test]
fn cooperative_dispatch_rejected_on_unsupported_hardware() {
    let backend = CudaBackend::acquire()
        .expect("Fix: CUDA backend acquisition must succeed on the GPU-required test host.");
    if backend.hardware_supports_grid_sync() {
        // Hardware DOES support cooperative launch; the rejection-contract test
        // does not apply on this device.
        return;
    }

    let program = add_one_program();
    let inputs = [u32_bytes(&[0; 8])];
    let mut cooperative_config = DispatchConfig::default();
    cooperative_config.cooperative = true;

    match backend.dispatch(&program, &inputs, &cooperative_config) {
        Ok(_) => panic!(
            "cooperative dispatch must NOT silently succeed on hardware that doesn't \
             support cooperative launch; expected BackendError::UnsupportedFeature so \
             the runtime can drive the kernel-split-fallback decision explicitly"
        ),
        Err(BackendError::UnsupportedFeature { name, backend: _ }) => {
            assert!(
                name.contains("cooperative"),
                "rejection error name must mention cooperative launch so the diagnostic is searchable; got: {name}"
            );
        }
        Err(other) => panic!(
            "cooperative dispatch on unsupported hardware must return UnsupportedFeature, \
             not {other:?}"
        ),
    }
}

#[test]
fn cooperative_dispatch_rejects_non_resident_grid_before_driver_launch() {
    let backend = CudaBackend::acquire()
        .expect("Fix: CUDA backend acquisition must succeed on the GPU-required test host.");
    if !backend.hardware_supports_grid_sync() {
        return;
    }

    let program = add_one_program();
    let inputs = [u32_bytes(&[0; 8])];
    let workgroup = program.workgroup_size();
    let threads_per_block = workgroup[0]
        .checked_mul(workgroup[1])
        .and_then(|xy| xy.checked_mul(workgroup[2]))
        .expect("test workgroup product must fit u32");
    let resident_blocks =
        cooperative_thread_residency_block_limit(&backend.caps, threads_per_block);
    assert!(
        resident_blocks > 0,
        "Fix: cooperative launch contract test requires a positive resident-block limit on supported hardware."
    );

    let mut cooperative_config = DispatchConfig::default();
    cooperative_config.cooperative = true;
    cooperative_config.grid_override = Some([
        u32::try_from(resident_blocks + 1)
            .expect("test resident-block limit must fit in a 1D CUDA grid"),
        1,
        1,
    ]);

    let cache_before = backend.pipeline_cache_snapshot();
    match backend.dispatch(&program, &inputs, &cooperative_config) {
        Ok(_) => panic!(
            "oversized cooperative grid must be rejected before cuLaunchCooperativeKernel; \
             silently launching here would make grid-sync correctness depend on opaque driver failure"
        ),
        Err(BackendError::CooperativeResidencyExceeded {
            grid_blocks,
            resident_limit,
            detail,
        }) => {
            assert_eq!(
                grid_blocks,
                resident_blocks + 1,
                "residency error must report the requested grid block count (resident_blocks + 1)"
            );
            assert_eq!(
                resident_limit, resident_blocks,
                "residency error must report the device's thread-residency block limit"
            );
            assert!(
                detail.contains("split") || detail.contains("Diagnostic"),
                "residency error detail must point at the split remedy / diagnostic; got: {detail}"
            );
        }
        Err(other) => panic!(
            "oversized cooperative grid must return CooperativeResidencyExceeded with an actionable residency fix, not {other:?}"
        ),
    }
    let cache_after = backend.pipeline_cache_snapshot();
    assert_eq!(
        cache_after.hits, cache_before.hits,
        "Fix: oversized cooperative grids must be rejected before CUDA module-cache lookup; a cache hit here means the hot path still did avoidable launch prep."
    );
    assert_eq!(
        cache_after.misses, cache_before.misses,
        "Fix: oversized cooperative grids must be rejected before CUDA module load/JIT; a cache miss here means invalid cooperative dispatch still paid compile-path cost."
    );
}

#[test]
fn cooperative_compiled_pipeline_does_not_use_regular_cuda_graph_replay() {
    let compiled_dispatch_source = include_str!("../src/pipeline/compiled_dispatch.rs");
    let graph_source = include_str!("../src/backend/cuda_graph.rs");

    assert!(
        compiled_dispatch_source.contains("|| self.prepared.cooperative")
            && compiled_dispatch_source.contains("&& !self.prepared.cooperative"),
        "Fix: cooperative CUDA compiled pipelines must bypass regular CUDA graph replay until cooperative graph capture explicitly records the cooperative launch ABI."
    );
    assert!(
        graph_source.contains("super::launch::launch_cuda_function(")
            && graph_source.contains(
                "false,\n                self.ptx_target_sm(),\n                \"cuLaunchKernel (capture)\","
            )
            && graph_source.contains(
                "false,\n                self.ptx_target_sm(),\n                \"cuLaunchKernel (resident input capture)\","
            )
            && !graph_source.contains(concat!("cuLaunchCooperativeKernel", "(")),
        "Fix: this contract assumes CUDA graph capture still records regular non-cooperative launches through launch_cuda_function(..., cooperative=false); update the replay gate only when cooperative graph capture is implemented explicitly."
    );
}

#[test]
fn cooperative_cuda_graph_recording_is_rejected_explicitly() {
    let backend = CudaBackend::acquire()
        .expect("Fix: CUDA backend acquisition must succeed on the GPU-required test host.");
    let program = add_one_program();
    let input = u32_bytes(&[0, 1, 2, 3, 4, 5, 6, 7]);
    let inputs = [input.as_slice()];
    let mut cooperative_config = DispatchConfig::default();
    cooperative_config.cooperative = true;

    match backend.record_cuda_graph_borrowed(&program, &inputs, &cooperative_config) {
        Ok(_) => panic!(
            "cooperative CUDA graph recording must not silently capture cuLaunchKernel; expected explicit UnsupportedFeature until cooperative graph capture records cuLaunchCooperativeKernel."
        ),
        Err(BackendError::UnsupportedFeature { name, backend: _ }) => {
            assert!(
                name.contains("cooperative") && name.contains("cuLaunchCooperativeKernel"),
                "Fix: cooperative graph rejection must name the missing cooperative launch ABI; got: {name}"
            );
        }
        Err(other) => panic!(
            "cooperative CUDA graph recording must return UnsupportedFeature, not {other:?}"
        ),
    }
}

/// Two-segment program with a cross-block dependency that ONLY a correct
/// whole-grid barrier satisfies: in segment 0 every thread writes its own
/// `scratch` slot; after the grid barrier every thread in every block reads
/// block 0's `scratch[0]`. Without a real grid-wide barrier a block other than
/// 0 could observe the pre-barrier `scratch[0]` (its zero init), so a broken
/// barrier is visible in the output.
fn cross_block_grid_sync_program(n: u32) -> Program {
    Program::wrapped(
        vec![
            BufferDecl::read("input", 0, DataType::U32).with_count(n),
            BufferDecl::read_write("scratch", 1, DataType::U32).with_count(n),
            BufferDecl::output("out", 2, DataType::U32).with_count(n),
        ],
        [256, 1, 1],
        vec![
            // segment 0: scratch[gid] = input[gid] + 1
            Node::store(
                "scratch",
                Expr::gid_x(),
                Expr::add(Expr::load("input", Expr::gid_x()), Expr::u32(1)),
            ),
            // whole-grid barrier: block 0's scratch[0] write must reach all blocks
            Node::barrier_with_ordering(MemoryOrdering::GridSync),
            // segment 1: out[gid] = scratch[0] + input[gid]  (cross-block read of block 0)
            Node::store(
                "out",
                Expr::gid_x(),
                Expr::add(
                    Expr::load("scratch", Expr::u32(0)),
                    Expr::load("input", Expr::gid_x()),
                ),
            ),
        ],
    )
}

/// The native cooperative grid-sync launch (in-kernel monotonic-counter barrier
/// + per-launch counter zeroing) must produce output byte-identical to the
/// proven host-orchestrated split on a residency-fitting cross-block program.
/// This is the end-to-end oracle for the barrier emission, the module-scope
/// counter, the cooperative launch ABI, and the per-launch counter reset: a
/// divergence means a block observed pre-barrier cross-block state.
#[test]
fn native_cooperative_grid_sync_matches_host_split_cross_block() {
    let backend = cuda_factory()
        .expect("Fix: CUDA backend acquisition must succeed on the GPU-required test host.");
    let backend = backend.as_ref();
    if !backend.supports_grid_sync() {
        eprintln!(
            "skip: backend does not lower native grid sync (supports_grid_sync()==false); \
             host-split parity is covered elsewhere"
        );
        return;
    }

    // 8 blocks of 256 threads — comfortably inside cooperative residency.
    let n: u32 = 2048;
    let program = cross_block_grid_sync_program(n);
    let input: Vec<u32> = (0..n).collect();
    let scratch_init = vec![0u32; n as usize];
    let input_bytes = u32_bytes(&input);
    let scratch_bytes = u32_bytes(&scratch_init);
    let inputs: [&[u8]; 2] = [input_bytes.as_slice(), scratch_bytes.as_slice()];

    // Native: supports_grid_sync()==true routes the whole (unsplit) grid-sync
    // program through cuLaunchCooperativeKernel with the in-kernel barrier.
    let native = backend
        .dispatch_borrowed(&program, &inputs, &DispatchConfig::default())
        .expect("native cooperative grid-sync dispatch must succeed for a residency-fitting grid");

    // Baseline: the host-orchestrated split, which always splits at the barrier.
    let host = grid_sync::dispatch_with_grid_sync_split(
        backend,
        &program,
        &inputs,
        &DispatchConfig::default(),
    )
    .expect("host-split grid-sync dispatch must succeed");

    assert_eq!(
        native.len(),
        host.len(),
        "native and host-split must return the same output buffer count"
    );
    for (idx, (native_buf, host_buf)) in native.iter().zip(host.iter()).enumerate() {
        assert_eq!(
            bytes_u32(native_buf),
            bytes_u32(host_buf),
            "native cooperative grid-sync output buffer {idx} must be byte-identical to the host \
             split; a divergence means the in-kernel grid barrier let a block read pre-barrier \
             cross-block state"
        );
    }

    // Absolute values: scratch[0] = input[0] + 1 = 1, so out[gid] = 1 + input[gid] = 1 + gid.
    let out = bytes_u32(native.last().expect("program has an `out` output buffer"));
    let expected: Vec<u32> = (0..n).map(|gid| 1 + gid).collect();
    assert_eq!(
        out, expected,
        "out[gid] must equal scratch[0] + input[gid] = 1 + gid for every thread across all blocks; \
         a wrong value at gid>=256 (block != 0) is the signature of a missing grid barrier"
    );
}

#[test]
fn cooperative_default_is_false_so_existing_callers_unchanged() {
    // The DispatchConfig::default() field must be `cooperative: false` so every
    // existing call site (which constructs DispatchConfig::default() and never
    // sets cooperative) continues to use cuLaunchKernel exactly as before.
    // This test guards the additive-only contract of the field addition.
    let config = DispatchConfig::default();
    assert!(
        !config.cooperative,
        "DispatchConfig::default().cooperative must be false; flipping it would silently \
         opt every existing dispatch into cooperative launch and change behaviour on every \
         consumer of the API"
    );
}
