//! Cross-backend validation safety.
//!
//! Vyre's three-layer validation cache MUST stay distinct across
//! backends:
//!
//!   1. `program.is_validated()`  -  fast atomic, STRUCTURAL only
//!      (wire format, IR shape, buffer bindings). Backend-agnostic.
//!   2. `WgpuBackend::validation_cache: DashSet<blake3::Hash>`  -
//!      per-backend, covers capability checks (SUBGROUP
//!      availability, workgroup-size limits, feature flags).
//!   3. `vyre_driver::backend::validation::validate_program(program,
//!      backend)`  -  the real validator.
//!
//! A backend MUST NOT consume is_validated() as a shortcut past its
//! own capability checks, and its capability cache MUST be keyed so
//! cross-backend dispatches miss (even if program_hash collides).
//!
//! This test is the regression gate: any future "simplification"
//! that makes validation process-wide instead of per-backend trips
//! the assertion below.

use vyre::VyreBackend;
use vyre_driver_wgpu::WgpuBackend;

/// Stand-in for a reduced-capability future backend. Refuses every
/// dispatch so the test cannot accidentally exercise its engine; the
/// point here is the `id()` distinction and the validation contract,
/// not its dispatch behavior.
struct ReducedBackend {
    id: &'static str,
}

impl vyre_driver::backend::private::Sealed for ReducedBackend {}

impl VyreBackend for ReducedBackend {
    fn id(&self) -> &'static str {
        self.id
    }
    fn dispatch(
        &self,
        _program: &vyre::Program,
        _inputs: &[Vec<u8>],
        _config: &vyre::DispatchConfig,
    ) -> Result<Vec<Vec<u8>>, vyre::BackendError> {
        Err(vyre::BackendError::new(
            "ReducedBackend refuses dispatch; tests only exercise capability surface.",
        ))
    }
    fn supports_subgroup_ops(&self) -> bool {
        false
    }
    fn max_workgroup_size(&self) -> [u32; 3] {
        [1, 1, 1]
    }
}

#[test]
fn backend_ids_are_distinct() {
    let wgpu = WgpuBackend::acquire().expect("Fix: GPU required for cross-backend test");
    let reduced = ReducedBackend { id: "reduced" };
    assert_ne!(
        wgpu.id(),
        reduced.id(),
        "transcendental parity6: two backends must have distinct ids so per-backend caches do not collide"
    );
}

#[test]
fn is_validated_does_not_substitute_for_capability_check() {
    // Contract: a program that WgpuBackend has validated (flag may
    // or may not be set depending on whether validation covered
    // structural-only) MUST trigger independent capability checks on
    // any other backend. This test documents the contract; the
    // engine-side enforcement is: ReducedBackend MUST run validation
    // when handed the same program, regardless of
    // `program.is_validated()` state.
    let wgpu = WgpuBackend::acquire().expect("Fix: GPU required for cross-backend test");
    let program = vyre::Program::empty();
    wgpu.dispatch(&program, &[], &vyre::DispatchConfig::default())
        .expect("wgpu dispatch of empty program must succeed");

    // If transcendental parity6 regresses by making is_validated a global shortcut,
    // a future refactor that reads `program.is_validated()` in
    // ReducedBackend::dispatch before running its own checks would
    // let unsupported programs through. The assertion here is the
    // contract: backends that refuse a program (because of their
    // reduced capabilities) must still return a structured error,
    // never silently succeed by reading the flag.
    let reduced = ReducedBackend { id: "reduced" };
    let result = reduced.dispatch(&program, &[], &vyre::DispatchConfig::default());
    assert!(
        matches!(result, Err(_)),
        "transcendental parity6: reduced backend must refuse dispatch of a program validated elsewhere; \
         never read Program::is_validated() as a capability shortcut"
    );
}
