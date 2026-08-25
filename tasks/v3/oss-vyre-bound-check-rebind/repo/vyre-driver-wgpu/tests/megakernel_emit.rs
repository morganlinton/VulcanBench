//! End-to-end lifecycle test for the persistent megakernel on wgpu.
//!
//! Lowers the vyre-runtime megakernel Program to Naga IR through
//! `vyre_emit_naga::program::emit_module`, wraps it in a
//! compiled pipeline, and dispatches a single cycle with the SHUTDOWN
//! flag pre-set so the persistent loop exits on its first iteration.
//!
//! This test closes Phase 1 Gate E: it proves the "persistent kernel
//! on wgpu" path actually runs. The two Naga emission gates (atomic element
//! types + CompareExchange result projection) are already fixed
//! upstream; this test is the load-bearing proof that fact holds end
//! to end, not just in isolation.
//!
//! When this test fails, read the error:
//!
//! - `InvalidAtomic(InvalidPointer(...))` → a regression in the
//!   atomic-target scan pass in `naga_emit/mod.rs::emit_module` (the
//!   pre-pass that detects which buffers need `atomic<u32>`).
//! - `CompareExchange requires expected value` → a regression in
//!   `naga_emit/expr.rs::Expr::Atomic`.
//! - `unknown future Node variant reached wgpu Naga lowering` → a
//!   regression in `naga_emit/node.rs`  -  the megakernel started using
//!   a Node variant the emitter does not cover.
//!
//! Running: `cargo test -p vyre-driver-wgpu --test megakernel_emit`.

use vyre_driver::{DispatchConfig, VyreBackend};
use vyre_driver_wgpu::WgpuBackend;
use vyre_runtime::megakernel::{self, Megakernel};

/// Megakernel's control word index for the SHUTDOWN flag.
const SHUTDOWN_WORD_INDEX: usize = 0;

/// Compile + dispatch the megakernel with SHUTDOWN=1 prearmed. The
/// persistent loop's first iteration reads SHUTDOWN, sees non-zero,
/// and returns. The dispatch returns cleanly with no validator errors.
#[test]
fn megakernel_shutdown_on_first_iteration_dispatches_cleanly() {
    let backend = WgpuBackend::acquire().expect(
        "Fix: GPU adapter required for megakernel emit test; missing adapter is a configuration bug, not graceful fallback.",
    );

    // Use a modest slot_count so the first dispatch is small enough to
    // leave plenty of headroom under every platform's GPU timeout. 64
    // slots × 64 workgroup is one workgroup; kernel exits on its first
    // iteration thanks to the prearmed SHUTDOWN.
    let workgroup_size_x: u32 = 64;
    let slot_count: u32 = 64;
    let program = megakernel::build_program_sharded(workgroup_size_x, &[]);

    // Sanity check: the megakernel lowers to valid Naga IR and emits valid WGSL. This
    // is what Gate E cares about  -  both halves must succeed end to
    // end, without the test having to dig into naga_emit internals.
    {
        use vyre_emit_naga::program::emit_module;
        let module = emit_module(
            &program,
            &DispatchConfig::default(),
            [workgroup_size_x, 1, 1],
        )
        .expect("megakernel Program must emit valid Naga");
        let info = naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::all(),
        )
        .validate(&module)
        .expect("Naga validator must accept megakernel module");
        let _ =
            naga::back::wgsl::write_string(&module, &info, naga::back::wgsl::WriterFlags::empty())
                .expect("Naga WGSL writer must serialize the megakernel module");
    }

    // Arm SHUTDOWN=1 in the control buffer so the kernel exits on its
    // first iteration. Without this the persistent loop runs until
    // the platform TDR kicks in (~2 s on Windows), turning this
    // smoke test into a GPU hang.
    let control = Megakernel::encode_control(
        /* shutdown = */ true, /* tenant_count = */ 1, /* observable_slots = */ 0,
    )
    .expect("Fix: control buffer encoding must fit the megakernel smoke test shape");
    assert!(
        control.len() >= (SHUTDOWN_WORD_INDEX + 1) * 4,
        "Fix: encode_control must allocate at least the SHUTDOWN word"
    );
    let off = SHUTDOWN_WORD_INDEX * 4;
    assert_eq!(
        u32::from_le_bytes(control[off..off + 4].try_into().unwrap()),
        1,
        "Fix: SHUTDOWN flag must be set on the first cycle or the kernel will spin."
    );

    // Mutate `control` in place so the compiler has a concrete
    // lifetime to borrow.  Dispatch expects owned Vec<u8> buffers.
    let ring = Megakernel::encode_empty_ring(slot_count)
        .expect("Fix: ring encoding must fit the megakernel smoke test shape");
    let debug_log = Megakernel::encode_empty_debug_log(/* record_capacity = */ 4)
        .expect("Fix: debug-log encoding must fit the megakernel smoke test shape");
    // io_queue shape from vyre-runtime/src/megakernel/builder.rs: 64
    // slots × 8 words each. Must be allocated per-program-buffer or
    // dispatch rejects the input-count mismatch.
    let io_queue = vec![0u8; 64 * 8 * 4];

    let outputs = backend
        .dispatch(
            &program,
            &[control, ring, debug_log, io_queue],
            &DispatchConfig::default(),
        )
        .expect(
            "Fix: megakernel dispatch failed. Check lowering/naga_emit error output  -  \
             if it is an atomic-target or CompareExchange error the Phase 1 gate \
             regressed.",
        );

    // The vyre-driver-wgpu readback path surfaces only the buffers the
    // caller declared as `is_output` (BufferDecl::output). The
    // megakernel declares its three ring buffers via read_write, so the
    // generic dispatch returns 0 output slices  -  the kernel's side
    // effects land in the host-visible GPU memory, not in a returned
    // Vec. A dedicated MegakernelDispatch entry point (tracked as a
    // follow-up) will surface a full readback for observability; this
    // test only asserts the persistent loop compiles, validates, and
    // terminates.
    assert!(
        outputs.len() <= 4,
        "persistent-kernel dispatch must return at most four RW buffers (control, ring, debug_log, io_queue)"
    );
}

/// The megakernel's CompareExchange on STATUS_WORD is the core of the
/// slot-claim protocol. Verify it lowers through the expr.rs
/// AtomicResult + AccessIndex projection path by emitting a
/// build_program_sharded(…, &[]) and asserting the WGSL serialization
/// contains no `atomic_compare_exchange_weak` sentinel. (We emit
/// through AtomicFunction::Exchange { compare: Some } which WGSL
/// spells as `atomicCompareExchangeWeak`  -  the projection is what
/// makes the IR-level Expr::Atomic usable as a scalar.)
#[test]
fn megakernel_wgsl_contains_compute_entry_and_atomic_cas() {
    use vyre_emit_naga::program::emit_module;

    let program = megakernel::build_program_sharded(64, &[]);
    let module = emit_module(&program, &DispatchConfig::default(), [64, 1, 1])
        .expect("megakernel must emit Naga");
    let info = naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::all(),
    )
    .validate(&module)
    .expect("Naga validator must accept module");
    let wgsl =
        naga::back::wgsl::write_string(&module, &info, naga::back::wgsl::WriterFlags::empty())
            .expect("WGSL emit");

    assert!(
        wgsl.contains("@compute") && wgsl.contains("@workgroup_size(64"),
        "WGSL must declare a @compute entry with the requested workgroup size: {wgsl}"
    );
    assert!(
        wgsl.contains("atomicCompareExchangeWeak"),
        "CompareExchange on STATUS_WORD must lower to atomicCompareExchangeWeak: {wgsl}"
    );
    assert!(
        wgsl.contains("atomicAdd"),
        "DONE_COUNT increment must lower to atomicAdd: {wgsl}"
    );
}
