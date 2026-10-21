//! External contracts for decode-scan fusion error handling.

#![cfg(feature = "decode")]

use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Program};
use vyre_libs::decode::streaming::{fuse_decode_scan, DecodeScanFuseError};

fn program_with_handoff(handoff: &str, access: BufferAccess) -> Program {
    Program::wrapped(
        vec![BufferDecl::storage(handoff, 0, access, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![],
    )
}

#[test]
fn zero_handoff_capacity_returns_error_not_panic() {
    let decoder = program_with_handoff("decoded", BufferAccess::ReadWrite);
    let scanner = program_with_handoff("decoded", BufferAccess::ReadOnly);
    let error = fuse_decode_scan(decoder, scanner, "decoded", 0)
        .expect_err("zero handoff capacity must be a structured error");

    assert!(matches!(error, DecodeScanFuseError::ZeroHandoff { .. }));
    assert!(
        error.to_string().contains("Fix:"),
        "error must stay actionable: {error}"
    );
}
