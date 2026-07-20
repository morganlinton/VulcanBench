//! Dispatch entry point that routes through the `DialectRegistry`.
//!
//! B-B4 routes CPU reference calls through the dialect registry. The
//! execution tree keeps the direct fast path for built-ins while this
//! module provides the extensible dispatch contract that cross-backend
//! comparison, demos, and third-party dialect crates consume.
//!
//! The registry-driven path gives one concrete benefit: external
//! dialect crates can
//! `inventory::submit!` an `OpDef` (`vyre_driver::OpDef`) with an
//! executable `cpu_ref` function, and the reference interpreter
//! immediately knows how to run it  -  no patch to vyre-reference
//! required.
//!
//! The BackendRegistration exported below makes the capability
//! layer aware of the reference backend so Programs that reference
//! unsupported dialects surface a clean `Unsupported` error
//! instead of panicking at dispatch time.

use crate::execution::call::invoke_cpu_ref;
use vyre::{cpu_op::is_cpu_reference_sentinel, Error, OpDef};

/// Run a single op against its registered CPU reference.
///
/// Category C IO ops and foundation's structured CPU reference sentinel are
/// rejected before invocation so the reference backend reports unsupported
/// capability instead of executing a non-executable structured oracle marker.
///
/// # Errors
///
/// Returns `Error::Interp` when:
///
/// * The op id is not registered with any dialect.
/// * The registered op is a Category C IO op, which has no portable CPU path.
/// * The registered op still points at foundation's structured CPU reference sentinel.
pub fn dispatch_op(op_id: &str, input: &[u8], output: &mut Vec<u8>) -> Result<(), Error> {
    let lookup = vyre::dialect_lookup().ok_or_else(|| {
        Error::interp(format!(
            "reference interpreter: no DialectLookup is installed. Fix: initialize vyre-driver before dispatching `{op_id}`."
        ))
    })?;
    let interned = lookup.intern_op(op_id);
    let op_def = lookup.lookup(interned).ok_or_else(|| {
        Error::interp(format!(
            "reference interpreter: op `{op_id}` is not registered. Fix: link the dialect crate that provides `{op_id}`."
        ))
    })?;

    reject_unsupported_cpu_dispatch(op_id, op_def)?;

    invoke_cpu_ref(op_id, op_def.lowerings.cpu_ref, input, output)
}

fn reject_unsupported_cpu_dispatch(op_id: &str, op_def: &OpDef) -> Result<(), Error> {
    if op_def.dialect == "io" {
        return Err(Error::interp(format!(
            "unsupported capability for `{op_id}` on reference/CPU backend: Category C IO ops are registered for composition but require a backend lowering for zero-copy NVMe/GDS execution. Fix: select or register a backend that advertises the `io` dialect capability, or reject the program during capability negotiation before reference dispatch."
        )));
    }

    if is_cpu_reference_sentinel(op_def.lowerings.cpu_ref) {
        return Err(Error::interp(format!(
            "unsupported CPU reference dispatch for `{op_id}`: the op is registered with foundation's structured intrinsic reference sentinel, not an executable flat-ABI CPU implementation. Fix: implement a typed reference adapter for `{op_id}` or route the program to a backend that declares a native lowering for this capability."
        )));
    }

    Ok(())
}

/// Capabilities advertised by the reference backend.
///
/// The reference interpreter supports every dialect whose ops
/// declare a non-trivial `cpu_ref`. The registration below is
/// discovered via `inventory::iter`; callers ask the registry
/// "does this program fit the reference backend" and the answer
/// folds into the capability-negotiation layer (B-B5).
pub const REFERENCE_BACKEND_NAME: &str = "reference";

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use vyre::{
        install_dialect_lookup, intern_string, DialectLookup, InternedOpId, LoweringTable, OpDef,
    };

    struct TestLookup;

    fn echo_ref(input: &[u8], output: &mut Vec<u8>) {
        output.extend_from_slice(input);
    }

    fn panic_ref(_: &[u8], output: &mut Vec<u8>) {
        output.extend_from_slice(&[0xDE, 0xAD]);
        panic!("malformed primitive input");
    }

    static IO_DEF: std::sync::OnceLock<OpDef> = std::sync::OnceLock::new();
    static ECHO_DEF: std::sync::OnceLock<OpDef> = std::sync::OnceLock::new();
    static FALLBACK_DEF: std::sync::OnceLock<OpDef> = std::sync::OnceLock::new();
    static PANIC_DEF: std::sync::OnceLock<OpDef> = std::sync::OnceLock::new();

    impl vyre_foundation::dialect_lookup::private::Sealed for TestLookup {}

    impl DialectLookup for TestLookup {
        fn provider_id(&self) -> &'static str {
            "vyre-reference::dialect_dispatch::TestLookup"
        }

        fn intern_op(&self, name: &str) -> InternedOpId {
            intern_string(name)
        }

        fn lookup(&self, id: InternedOpId) -> Option<&'static OpDef> {
            if id == intern_string("io.dma_from_nvme") {
                return Some(IO_DEF.get_or_init(|| OpDef {
                    id: "io.dma_from_nvme",
                    dialect: "io",
                    lowerings: LoweringTable::new(echo_ref),
                    ..OpDef::default()
                }));
            }
            if id == intern_string("test.echo") {
                return Some(ECHO_DEF.get_or_init(|| OpDef {
                    id: "test.echo",
                    dialect: "test",
                    lowerings: LoweringTable::new(echo_ref),
                    ..OpDef::default()
                }));
            }
            if id == intern_string("test.structured_fallback") {
                return Some(FALLBACK_DEF.get_or_init(|| OpDef {
                    id: "test.structured_fallback",
                    dialect: "test",
                    lowerings: LoweringTable::empty(),
                    ..OpDef::default()
                }));
            }
            if id == intern_string("test.panics") {
                return Some(PANIC_DEF.get_or_init(|| OpDef {
                    id: "test.panics",
                    dialect: "test",
                    lowerings: LoweringTable::new(panic_ref),
                    ..OpDef::default()
                }));
            }
            None
        }
    }

    fn install_test_lookup() {
        install_dialect_lookup(Arc::new(TestLookup))
            .expect("Fix: test dialect lookup install should succeed or be idempotent");
    }

    #[test]
    fn unknown_op_surfaces_clean_error() {
        install_test_lookup();
        let mut out = Vec::new();
        let err = dispatch_op("nonexistent.op", &[], &mut out).expect_err("must fail");
        let msg = format!("{err}");
        assert!(
            msg.contains("nonexistent.op"),
            "error message must name the op: {msg}"
        );
        assert!(
            msg.contains("Fix:"),
            "error must carry actionable Fix: hint"
        );
    }

    #[test]
    fn registered_executable_cpu_ref_dispatches() {
        install_test_lookup();
        let mut out = Vec::new();
        dispatch_op("test.echo", &[9, 8, 7], &mut out)
            .expect("Fix: echo has executable cpu_ref; restore this invariant before continuing.");
        assert_eq!(out, [9, 8, 7]);
    }

    #[test]
    fn panicking_cpu_ref_returns_structured_error_without_output_drift() {
        install_test_lookup();
        let mut out = vec![0xAA];
        let err = dispatch_op("test.panics", &[1, 2, 3], &mut out)
            .expect_err("panicking cpu_ref must become a structured interpreter error");
        let msg = format!("{err}");
        assert!(
            msg.contains("test.panics") && msg.contains("panicked") && msg.contains("Fix:"),
            "panic error must name the op and stay actionable: {msg}"
        );
        assert_eq!(
            out,
            vec![0xAA],
            "failed CPU refs must not leak partial output bytes"
        );
    }

    #[test]
    fn io_cat_c_op_refuses_reference_cpu_dispatch() {
        install_test_lookup();
        let mut out = vec![0xAA];
        let err = dispatch_op("io.dma_from_nvme", &[], &mut out)
            .expect_err("Category C io must not execute on reference/CPU");
        let msg = format!("{err}");
        assert!(
            msg.contains("io.dma_from_nvme"),
            "error message must name the op: {msg}"
        );
        assert!(
            msg.contains("unsupported capability"),
            "error must classify this as unsupported capability: {msg}"
        );
        assert!(
            msg.contains("Category C IO"),
            "error must identify Category C IO: {msg}"
        );
        assert!(
            msg.contains("Fix:"),
            "error must carry actionable Fix: hint: {msg}"
        );
        assert_eq!(
            out,
            vec![0xAA],
            "dispatcher must reject before invoking the CPU sentinel"
        );
    }

    #[test]
    fn structured_cpu_reference_sentinel_refuses_reference_dispatch() {
        install_test_lookup();
        let mut out = vec![0xAA];
        let err = dispatch_op("test.structured_fallback", &[1, 2], &mut out)
            .expect_err("structured reference sentinel must not look executable");
        let msg = format!("{err}");
        assert!(
            msg.contains("test.structured_fallback"),
            "error message must name the op: {msg}"
        );
        assert!(
            msg.contains("structured intrinsic reference sentinel"),
            "error must name foundation's reference sentinel: {msg}"
        );
        assert!(
            msg.contains("Fix:"),
            "error must carry actionable Fix: hint: {msg}"
        );
        assert_eq!(
            out,
            vec![0xAA],
            "dispatcher must reject before invoking structured_intrinsic_cpu"
        );
    }

    #[test]
    fn reference_backend_name_is_stable() {
        install_test_lookup();
        assert_eq!(REFERENCE_BACKEND_NAME, "reference");
    }
}
