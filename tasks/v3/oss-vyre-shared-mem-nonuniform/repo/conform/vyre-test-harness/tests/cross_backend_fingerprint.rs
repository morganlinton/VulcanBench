//! P-CONFORM-2: Cross-backend conformance via VSA fingerprint.
//!
//! Every registered backend and vyre-aot
//! expose the same VSA fingerprint helper. A given Program must
//! produce the same fingerprint regardless of which crate's helper
//! is consulted  -  that's the cross-backend identity contract.

use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

fn sample_program() -> Program {
    let buffers = vec![
        BufferDecl::storage("a", 0, BufferAccess::ReadOnly, DataType::U32),
        BufferDecl::storage("b", 1, BufferAccess::ReadWrite, DataType::U32),
    ];
    let entry = vec![Node::store(
        "b",
        Expr::InvocationId { axis: 0 },
        Expr::load("a", Expr::InvocationId { axis: 0 }),
    )];
    Program::wrapped(buffers, [64, 1, 1], entry)
}

#[test]
fn all_backends_agree_on_program_fingerprint() {
    let p = sample_program();
    let driver_fp = vyre_driver::program_vsa_fingerprint(&p);
    let substrate_fp = vyre_self_substrate::vsa_fingerprint::vsa_fingerprint(&p);

    assert_eq!(driver_fp, substrate_fp, "driver diverged from substrate");
    assert!(!driver_fp.is_empty(), "fingerprint must be non-empty");
}

#[test]
fn fingerprint_is_deterministic() {
    let p1 = sample_program();
    let p2 = sample_program();
    let fp1 = vyre_self_substrate::vsa_fingerprint::vsa_fingerprint(&p1);
    let fp2 = vyre_self_substrate::vsa_fingerprint::vsa_fingerprint(&p2);
    assert_eq!(fp1, fp2);
}
